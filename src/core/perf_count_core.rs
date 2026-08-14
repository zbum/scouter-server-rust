use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::core::cache::CacheManager;
use crate::db::counter_store::TIME_TYPE_FIVE_MIN;
use crate::db::db_manager::DbManager;
use crate::protocol::pack::PerfCounterPack;
use crate::protocol::value::Value;
use crate::util::date;

/// Processes performance counter data from agents.
pub struct PerfCountCore {
    tx: mpsc::Sender<PerfCounterPack>,
}

impl PerfCountCore {
    pub fn new(
        cache: Arc<CacheManager>,
        db: Arc<DbManager>,
        queue_size: usize,
        token: CancellationToken,
        tasks: &TaskTracker,
    ) -> Self {
        let (tx, rx) = mpsc::channel(queue_size);
        Self::start_worker(cache, db, rx, token, tasks);
        Self { tx }
    }

    fn start_worker(
        cache: Arc<CacheManager>,
        db: Arc<DbManager>,
        mut rx: mpsc::Receiver<PerfCounterPack>,
        token: CancellationToken,
        tasks: &TaskTracker,
    ) {
        tasks.spawn(async move {
            loop {
                tokio::select! {
                    Some(pack) = rx.recv() => {
                        Self::process(&cache, &db, pack);
                    }
                    _ = token.cancelled() => {
                        while let Ok(pack) = rx.try_recv() {
                            Self::process(&cache, &db, pack);
                        }
                        info!("PerfCountCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(cache: &CacheManager, db: &DbManager, pack: PerfCounterPack) {
        let obj_hash = crate::util::hash::hash(&pack.obj_name);

        // Store each counter value in cache
        for (name, value) in &pack.data.table {
            cache
                .counter
                .put(obj_hash, name, pack.timetype, value.clone());
        }

        // Write to DB (only for 5-minute aggregated counters, matching Java behavior)
        if pack.timetype == TIME_TYPE_FIVE_MIN as i8 {
            let time = if pack.time > 0 {
                pack.time
            } else {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64
            };
            let date_str = date::yyyymmdd(time);
            let hhmm = date::hhmm(time);

            match db.get_or_create(&date_str) {
                Ok(container) => {
                    for (name, value) in &pack.data.table {
                        let f_value = match value {
                            Value::Float(v) => *v as f64,
                            Value::Double(v) => *v,
                            Value::Decimal(v) => *v as f64,
                            _ => continue,
                        };

                        // Counter key = obj_hash(4) + counter_name_hash(4)
                        let name_hash = crate::util::hash::hash(name);
                        let mut key = Vec::with_capacity(8);
                        key.extend_from_slice(&obj_hash.to_be_bytes());
                        key.extend_from_slice(&name_hash.to_be_bytes());

                        if let Err(e) = container.counter.write(&key, hhmm, f_value, pack.timetype)
                        {
                            warn!("Failed to write counter to DB: {}", e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to open DB container for {}: {}", date_str, e);
                }
            }
        }

        debug!(
            "Counter processed: obj={} timetype={} counters={}",
            pack.obj_name,
            pack.timetype,
            pack.data.table.len()
        );
    }

    pub async fn add(&self, pack: PerfCounterPack) {
        if self.tx.send(pack).await.is_err() {
            warn!("PerfCountCore queue overflow");
        }
    }
}
