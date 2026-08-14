use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, info, warn};

use crate::db::db_manager::DbManager;
use crate::protocol::pack::{XLogProfilePack, XLogProfilePack2};
use crate::util::date;

/// Unified profile pack for queueing
#[derive(Debug, Clone)]
pub enum ProfilePack {
    V1(XLogProfilePack),
    V2(XLogProfilePack2),
}

impl ProfilePack {
    pub fn txid(&self) -> i64 {
        match self {
            ProfilePack::V1(p) => p.txid,
            ProfilePack::V2(p) => p.txid,
        }
    }

    pub fn obj_hash(&self) -> i32 {
        match self {
            ProfilePack::V1(p) => p.obj_hash,
            ProfilePack::V2(p) => p.obj_hash,
        }
    }

    pub fn profile_data(&self) -> &[u8] {
        match self {
            ProfilePack::V1(p) => &p.profile,
            ProfilePack::V2(p) => &p.profile,
        }
    }
}

/// Processes profile data (method call traces).
pub struct ProfileCore {
    tx: mpsc::Sender<ProfilePack>,
}

impl ProfileCore {
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
        mut rx: mpsc::Receiver<ProfilePack>,
        token: CancellationToken,
        tasks: &TaskTracker,
    ) {
        tasks.spawn(async move {
            loop {
                tokio::select! {
                    Some(pack) = rx.recv() => {
                        Self::process(&db, &pack);
                    }
                    _ = token.cancelled() => {
                        while let Ok(pack) = rx.try_recv() {
                            Self::process(&db, &pack);
                        }
                        info!("ProfileCore worker drained and stopped");
                        break;
                    }
                }
            }
        });
    }

    fn process(db: &DbManager, pack: &ProfilePack) {
        if pack.profile_data().is_empty() {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let date_str = date::yyyymmdd(now);

        match db.get_or_create(&date_str) {
            Ok(container) => {
                if let Err(e) = container
                    .profile
                    .write_with_length(pack.txid(), pack.profile_data())
                {
                    warn!("Failed to write profile to DB: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to open DB container for {}: {}", date_str, e);
            }
        }

        debug!(
            "Profile processed: txid={:#x} obj={:#x} size={}",
            pack.txid(),
            pack.obj_hash(),
            pack.profile_data().len()
        );
    }

    pub async fn add(&self, pack: ProfilePack) {
        if self.tx.send(pack).await.is_err() {
            warn!("ProfileCore queue overflow");
        }
    }
}
