use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::db::db_manager::DbManager;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::SummaryPack;
use crate::util::date;

/// Processes summary data and writes to DB.
pub struct SummaryCore {
    tx: mpsc::Sender<SummaryPack>,
}

impl SummaryCore {
    pub fn new(
        db: Arc<DbManager>,
        queue_size: usize,
        token: CancellationToken,
        tasks: &TaskTracker,
    ) -> Self {
        let (tx, rx) = mpsc::channel(queue_size);
        Self::start_worker(db, rx, token, tasks);
        Self { tx }
    }

    fn start_worker(
        db: Arc<DbManager>,
        mut rx: mpsc::Receiver<SummaryPack>,
        token: CancellationToken,
        tasks: &TaskTracker,
    ) {
        tasks.spawn(async move {
            loop {
                tokio::select! {
                    Some(pack) = rx.recv() => {
                        Self::process(&db, pack);
                    }
                    _ = token.cancelled() => {
                        while let Ok(pack) = rx.try_recv() {
                            Self::process(&db, pack);
                        }
                        info!("SummaryCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(db: &DbManager, pack: SummaryPack) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let date_str = date::yyyymmdd(now);
        let hhmm = date::hhmm(now);

        match db.get_or_create(&date_str) {
            Ok(container) => {
                // Each entry in the table is keyed by a hash string
                for (key_str, value) in &pack.table.table {
                    // The key in the map is typically a hash value as string
                    let id_hash: i32 = key_str
                        .parse()
                        .unwrap_or_else(|_| crate::util::hash::hash(key_str));

                    // Serialize the value as the record data
                    let mut dout = DataOutputX::new();
                    if let Err(e) = dout.write_value(value) {
                        warn!("Failed to serialize summary value: {}", e);
                        continue;
                    }
                    let data = dout.to_bytes();

                    if let Err(e) = container.summary.write(pack.stype, hhmm, id_hash, &data) {
                        warn!("Failed to write summary to DB: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to open DB container for {}: {}", date_str, e);
            }
        }

        debug!(
            "Summary processed: stype={} entries={}",
            pack.stype,
            pack.table.table.len()
        );
    }

    pub async fn add(&self, pack: SummaryPack) {
        if self.tx.send(pack).await.is_err() {
            warn!("SummaryCore queue overflow");
        }
    }
}
