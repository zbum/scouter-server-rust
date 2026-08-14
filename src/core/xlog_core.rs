use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
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
    pub fn new(cache: Arc<CacheManager>, db: Arc<DbManager>, queue_size: usize, token: CancellationToken) -> Self {
        let (tx, rx) = mpsc::channel(queue_size);
        Self::start_worker(cache, db, rx, token);
        Self { tx }
    }

    fn start_worker(cache: Arc<CacheManager>, db: Arc<DbManager>, mut rx: mpsc::Receiver<XLogPack>, token: CancellationToken) {
        tokio::spawn(async move {
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
        cache.xlog.put(pack.obj_hash, pack.elapsed, has_error, bytes.clone());

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
