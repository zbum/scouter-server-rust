use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::core::cache::CacheManager;
use crate::db::db_manager::DbManager;
use crate::protocol::pack::TextPack;
use crate::util::date;

/// Processes text data (SQL, method names, etc.) with deduplication.
pub struct TextCore {
    tx: mpsc::Sender<TextPack>,
    cache: Arc<CacheManager>,
}

impl TextCore {
    pub fn new(cache: Arc<CacheManager>, db: Arc<DbManager>, queue_size: usize, token: CancellationToken) -> Self {
        let cache_clone = cache.clone();
        let (tx, rx) = mpsc::channel(queue_size);
        Self::start_worker(cache_clone, db, rx, token);
        Self { tx, cache }
    }

    fn start_worker(cache: Arc<CacheManager>, db: Arc<DbManager>, mut rx: mpsc::Receiver<TextPack>, token: CancellationToken) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(pack) = rx.recv() => {
                        Self::process(&cache, &db, &pack);
                    }
                    _ = token.cancelled() => {
                        while let Ok(pack) = rx.try_recv() {
                            Self::process(&cache, &db, &pack);
                        }
                        info!("TextCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(cache: &CacheManager, db: &DbManager, pack: &TextPack) {
        // Store in cache (dedup - only store if not present)
        if !cache.text.contains(&pack.xtype, pack.hash) {
            cache.text.put(&pack.xtype, pack.hash, &pack.text);
            debug!(
                "Text cached: type={} hash={:#x} text={}",
                pack.xtype,
                pack.hash,
                if pack.text.len() > 80 { &pack.text[..80] } else { &pack.text }
            );
        }

        // Write to DB
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let date_str = date::yyyymmdd(now);

        // div = text type hash, hash = text content hash
        let div = crate::util::hash::hash(&pack.xtype);
        match db.get_or_create(&date_str) {
            Ok(container) => {
                if let Err(e) = container.text.put(div, pack.hash, &pack.text) {
                    warn!("Failed to write text to DB: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to open DB container for {}: {}", date_str, e);
            }
        }
    }

    /// Add text to cache immediately (before queueing) for real-time lookups,
    /// then queue for persistence.
    pub async fn add(&self, pack: TextPack) {
        // Cache immediately like Java TextCore does
        self.cache.text.put(&pack.xtype, pack.hash, &pack.text);

        if self.tx.send(pack).await.is_err() {
            warn!("TextCore queue overflow");
        }
    }
}
