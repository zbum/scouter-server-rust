use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::core::alert_summary::AlertSummary;
use crate::core::cache::CacheManager;
use crate::db::db_manager::DbManager;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{AlertPack, Pack};
use crate::util::date;

/// Processes alert events from agents.
pub struct AlertCore {
    tx: mpsc::Sender<AlertPack>,
}

impl AlertCore {
    pub fn new(
        cache: Arc<CacheManager>,
        db: Arc<DbManager>,
        queue_size: usize,
        token: CancellationToken,
        alert_summary: Arc<AlertSummary>,
        tasks: &TaskTracker,
    ) -> Self {
        let (tx, rx) = mpsc::channel(queue_size);
        Self::start_worker(cache, db, rx, token, alert_summary, tasks);
        Self { tx }
    }

    fn start_worker(
        cache: Arc<CacheManager>,
        db: Arc<DbManager>,
        mut rx: mpsc::Receiver<AlertPack>,
        token: CancellationToken,
        alert_summary: Arc<AlertSummary>,
        tasks: &TaskTracker,
    ) {
        tasks.spawn(async move {
            loop {
                tokio::select! {
                    Some(pack) = rx.recv() => {
                        alert_summary.add(&pack).await;
                        Self::process(&cache, &db, pack);
                    }
                    _ = token.cancelled() => {
                        while let Ok(pack) = rx.try_recv() {
                            Self::process(&cache, &db, pack);
                        }
                        info!("AlertCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(cache: &CacheManager, db: &DbManager, mut pack: AlertPack) {
        // Set time if not set
        if pack.time == 0 {
            pack.time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
        }

        // Cache
        cache.alert.put(pack.clone());

        // Write to DB
        let mut dout = DataOutputX::new();
        if let Err(e) = dout.write_pack(&Pack::Alert(pack.clone())) {
            warn!("Failed to serialize Alert: {}", e);
            return;
        }
        let bytes = dout.to_bytes();
        let date_str = date::yyyymmdd(pack.time);

        match db.get_or_create(&date_str) {
            Ok(container) => {
                if let Err(e) = container.alert.write(pack.time, &bytes) {
                    warn!("Failed to write alert to DB: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to open DB container for {}: {}", date_str, e);
            }
        }

        debug!(
            "Alert processed: level={} title={} obj={:#x}",
            pack.level, pack.title, pack.obj_hash
        );
    }

    pub async fn add(&self, pack: AlertPack) {
        if self.tx.send(pack).await.is_err() {
            warn!("AlertCore queue overflow");
        }
    }
}
