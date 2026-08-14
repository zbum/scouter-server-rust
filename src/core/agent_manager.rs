use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::config::Config;
use crate::core::cache::CacheManager;
use crate::protocol::pack::ObjectPack;
use crate::util::hash;

const DEFAULT_DEAD_TIME_MS: u64 = 8000;

/// Manages agent/object lifecycle: active -> inactive -> dead
pub struct AgentManager {
    #[allow(dead_code)]
    config: Arc<Config>,
    cache: Arc<CacheManager>,
}

impl AgentManager {
    pub fn new(config: Arc<Config>, cache: Arc<CacheManager>) -> Self {
        Self { config, cache }
    }

    /// Called when an ObjectPack is received from an agent.
    pub fn active(&self, mut pack: ObjectPack) {
        let now = current_millis();

        // Compute obj_hash from name if not set
        if pack.obj_hash == 0 && !pack.obj_name.is_empty() {
            pack.obj_hash = hash::hash(&pack.obj_name);
        }

        pack.wakeup = now as i64;
        pack.alive = true;

        let is_new = !self.cache.object.contains(pack.obj_hash);

        if is_new {
            info!(
                "New agent registered: type={} name={} hash={:#x} addr={}",
                pack.obj_type, pack.obj_name, pack.obj_hash, pack.address
            );
        }

        self.cache.object.put(pack);
    }

    /// Check all agents for timeout and mark as inactive.
    /// Should be called periodically (every 1 second).
    pub fn check_inactive(&self) {
        let now = current_millis() as i64;
        let dead_time = DEFAULT_DEAD_TIME_MS as i64;

        let all_objects = self.cache.object.get_all_objects();
        for obj in all_objects {
            if obj.alive && (now - obj.wakeup) > dead_time {
                // Mark as inactive
                let mut updated = obj.clone();
                updated.alive = false;
                info!(
                    "Agent inactive: type={} name={} hash={:#x} (last seen {}ms ago)",
                    updated.obj_type,
                    updated.obj_name,
                    updated.obj_hash,
                    now - updated.wakeup
                );
                self.cache.object.put(updated);
            }
        }
    }

    /// Start the background daemon that checks for inactive agents.
    pub fn start_monitor(self: Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    _ = interval.tick() => {}
                }
                self.check_inactive();
            }
            info!("Agent monitor stopped");
        })
    }
}

fn current_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
