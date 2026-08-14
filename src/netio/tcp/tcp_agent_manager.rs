use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::netio::tcp::agent_worker::TcpAgentWorker;

const MAX_CONNECTIONS_PER_AGENT: usize = 50;
const DEFAULT_KEEPALIVE_INTERVAL_MS: u64 = 5000;

/// Manages TCP agent connections pooled by obj_hash.
pub struct TcpAgentManager {
    /// obj_hash -> connection queue
    agents: DashMap<i32, VecDeque<Arc<TcpAgentWorker>>>,
    keepalive_interval: Duration,
}

impl TcpAgentManager {
    pub fn new() -> Self {
        Self {
            agents: DashMap::new(),
            keepalive_interval: Duration::from_millis(DEFAULT_KEEPALIVE_INTERVAL_MS),
        }
    }

    /// Add a new agent connection.
    pub fn add(&self, obj_hash: i32, worker: Arc<TcpAgentWorker>) -> usize {
        let mut entry = self.agents.entry(obj_hash).or_insert_with(VecDeque::new);
        if entry.len() < MAX_CONNECTIONS_PER_AGENT {
            entry.push_back(worker);
        }
        entry.len()
    }

    /// Get the next available agent connection for an obj_hash.
    pub fn get(&self, obj_hash: i32) -> Option<Arc<TcpAgentWorker>> {
        if let Some(mut entry) = self.agents.get_mut(&obj_hash) {
            // Round-robin: pop front, push back
            while let Some(worker) = entry.pop_front() {
                if !worker.is_closed() {
                    entry.push_back(worker.clone());
                    return Some(worker);
                }
                // Skip closed workers
            }
        }
        None
    }

    /// Get count of connections for an obj_hash.
    pub fn connection_count(&self, obj_hash: i32) -> usize {
        self.agents.get(&obj_hash).map(|e| e.len()).unwrap_or(0)
    }

    /// Remove all connections for an obj_hash.
    pub fn remove(&self, obj_hash: i32) {
        if let Some((_, workers)) = self.agents.remove(&obj_hash) {
            for w in workers {
                w.close();
            }
        }
    }

    /// Start the background monitor that sends keepalives and cleans up dead connections.
    pub fn start_monitor(self: Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()> {
        let keepalive_interval = self.keepalive_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    _ = interval.tick() => {}
                }

                // Collect obj_hashes to check
                let hashes: Vec<i32> = self.agents.iter().map(|e| *e.key()).collect();

                for hash in hashes {
                    if let Some(mut entry) = self.agents.get_mut(&hash) {
                        // Remove closed connections
                        entry.retain(|w| !w.is_closed());

                        // Send keepalive to expired connections
                        for worker in entry.iter() {
                            let elapsed = worker.last_write_time.lock().await.elapsed();
                            if elapsed >= keepalive_interval {
                                let w = worker.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = w.send_keep_alive().await {
                                        debug!(
                                            "Keepalive failed for agent {:#x}: {}",
                                            w.obj_hash, e
                                        );
                                        w.close();
                                    }
                                });
                            }
                        }

                        // Remove empty entries
                        if entry.is_empty() {
                            drop(entry);
                            self.agents.remove(&hash);
                        }
                    }
                }
            }
            let hashes: Vec<i32> = self.agents.iter().map(|entry| *entry.key()).collect();
            for hash in hashes {
                self.remove(hash);
            }
            info!("TCP agent monitor stopped");
        })
    }

    /// Total number of agent connections.
    pub fn total_connections(&self) -> usize {
        self.agents.iter().map(|e| e.value().len()).sum()
    }
}
