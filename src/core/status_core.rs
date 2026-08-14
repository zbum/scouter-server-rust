use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::core::cache::CacheManager;
use crate::protocol::pack::StatusPack;

/// Processes status data from agents.
pub struct StatusCore {
    tx: mpsc::Sender<StatusPack>,
}

impl StatusCore {
    pub fn new(cache: Arc<CacheManager>, queue_size: usize, token: CancellationToken) -> Self {
        let (tx, rx) = mpsc::channel(queue_size);
        Self::start_worker(cache, rx, token);
        Self { tx }
    }

    fn start_worker(cache: Arc<CacheManager>, mut rx: mpsc::Receiver<StatusPack>, token: CancellationToken) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(pack) = rx.recv() => {
                        Self::process(&cache, pack);
                    }
                    _ = token.cancelled() => {
                        while let Ok(pack) = rx.try_recv() {
                            Self::process(&cache, pack);
                        }
                        info!("StatusCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(cache: &CacheManager, mut pack: StatusPack) {
        if pack.time == 0 {
            pack.time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
        }

        cache.status.put(pack.clone());

        debug!(
            "Status processed: obj={:#x} key={}",
            pack.obj_hash, pack.key
        );
    }

    pub async fn add(&self, pack: StatusPack) {
        if self.tx.send(pack).await.is_err() {
            warn!("StatusCore queue overflow");
        }
    }
}
