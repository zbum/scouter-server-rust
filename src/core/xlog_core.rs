use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::core::cache::CacheManager;
use crate::db::db_manager::DbManager;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{Pack, XLogPack};
use crate::util::date;

/// Processes transaction logs (XLog) from agents.
pub struct XLogCore {
    tx: mpsc::Sender<XLogPack>,
}

impl XLogCore {
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
        mut rx: mpsc::Receiver<XLogPack>,
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
                        info!("XLogCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(cache: &CacheManager, db: &DbManager, pack: XLogPack) {
        // Serialize to bytes for cache storage
        let mut dout = DataOutputX::new();
        if let Err(e) = dout.write_pack(&Pack::XLog(pack.clone())) {
            warn!("Failed to serialize XLog: {}", e);
            return;
        }
        let bytes = dout.to_bytes();

        // Cache for real-time streaming
        let has_error = pack.error != 0;
        cache
            .xlog
            .put(pack.obj_hash, pack.elapsed, has_error, bytes.clone());

        // Write to DB
        let time = if pack.end_time > 0 {
            pack.end_time
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        };
        let date_str = date::yyyymmdd(time);
        match db.get_or_create(&date_str) {
            Ok(container) => {
                if let Err(e) = container.xlog.write(time, pack.txid, pack.gxid, &bytes) {
                    warn!("Failed to write XLog to DB: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to open DB container for {}: {}", date_str, e);
            }
        }

        debug!(
            "XLog processed: obj={:#x} svc={:#x} elapsed={}ms err={}",
            pack.obj_hash, pack.service, pack.elapsed, pack.error
        );
    }

    pub async fn add(&self, pack: XLogPack) {
        if self.tx.send(pack).await.is_err() {
            warn!("XLogCore queue overflow");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::fs;

    #[tokio::test]
    async fn shutdown_drains_queued_xlog_to_db() {
        let test_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_dir = std::env::temp_dir().join(format!("scouter-xlog-drain-{test_id}"));

        let mut config = Config::default();
        config.db_dir = db_dir.to_string_lossy().into_owned();
        let config = Arc::new(config);
        let cache = Arc::new(CacheManager::new(&config));
        let db = Arc::new(DbManager::new(config));
        let shutdown = CancellationToken::new();
        let tasks = TaskTracker::new();
        let core = XLogCore::new(cache, db.clone(), 8, shutdown.clone(), &tasks);

        let end_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let txid = 0x1234_5678_i64;
        core.add(XLogPack {
            end_time,
            txid,
            ..XLogPack::default()
        })
        .await;

        tasks.close();
        shutdown.cancel();
        tasks.wait().await;
        db.flush_all().unwrap();

        let date = date::yyyymmdd(end_time);
        let stored = db
            .get_or_create(&date)
            .unwrap()
            .xlog
            .read_by_txid(txid)
            .unwrap();
        assert!(
            stored.is_some(),
            "queued XLog must be stored before shutdown"
        );

        drop(core);
        drop(db);
        fs::remove_dir_all(db_dir).unwrap();
    }
}
