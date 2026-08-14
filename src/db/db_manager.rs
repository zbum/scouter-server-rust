use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::config::Config;
use crate::db::alert_store::AlertStore;
use crate::db::counter_store::CounterStore;
use crate::db::profile_store::ProfileStore;
use crate::db::summary_store::SummaryStore;
use crate::db::text_store::TextStore;
use crate::db::xlog_store::XLogStore;

/// A daily container holding all stores for a single YYYYMMDD date.
pub struct DailyContainer {
    pub xlog: XLogStore,
    pub text: TextStore,
    pub counter: CounterStore,
    pub alert: AlertStore,
    pub profile: ProfileStore,
    pub summary: SummaryStore,
    pub date: String,
    pub last_access: std::sync::Mutex<Instant>,
}

impl DailyContainer {
    pub fn touch(&self) {
        *self.last_access.lock().unwrap() = Instant::now();
    }

    pub fn idle_duration(&self) -> Duration {
        self.last_access.lock().unwrap().elapsed()
    }

    pub fn flush(&self) -> std::io::Result<()> {
        let mut first_error = None;
        for result in [
            self.xlog.flush(),
            self.text.flush(),
            self.counter.flush(),
            self.alert.flush(),
            self.profile.flush(),
            self.summary.flush(),
        ] {
            if let Err(error) = result {
                first_error.get_or_insert(error);
            }
        }

        first_error.map_or(Ok(()), Err)
    }
}

/// Central DB manager that manages daily containers.
/// Each date gets its own set of stores, lazily created.
pub struct DbManager {
    config: Arc<Config>,
    containers: DashMap<String, Arc<DailyContainer>>,
    db_dir: PathBuf,
}

impl DbManager {
    pub fn new(config: Arc<Config>) -> Self {
        let db_dir = PathBuf::from(&config.db_dir);
        Self {
            config,
            containers: DashMap::new(),
            db_dir,
        }
    }

    pub fn db_dir(&self) -> &Path {
        &self.db_dir
    }

    /// Get or create a daily container for the given date.
    pub fn get_or_create(&self, date: &str) -> std::io::Result<Arc<DailyContainer>> {
        // Fast path: already exists
        if let Some(entry) = self.containers.get(date) {
            entry.value().touch();
            return Ok(entry.value().clone());
        }

        // Create new container
        let day_dir = self.db_dir.join(date);
        std::fs::create_dir_all(&day_dir)?;

        let xlog_dir = day_dir.join("xlog");
        let text_dir = day_dir.join("text");
        let alert_dir = day_dir.join("alert");
        let profile_dir = day_dir.join("profile");
        let summary_dir = day_dir.join("summary");

        let xlog_index_mb = self.config.mgr_xlog_id_index_mb;
        let text_index_mb = self.config.mgr_text_db_daily_index_mb;
        let counter_index_mb = self.config.mgr_counter_index_mb;

        let xlog = XLogStore::open(&xlog_dir, xlog_index_mb)?;
        let text = TextStore::open(&text_dir, text_index_mb)?;
        let counter = CounterStore::open(&day_dir, date, counter_index_mb)?;
        let alert = AlertStore::open(&alert_dir)?;
        let profile = ProfileStore::open(&profile_dir)?;
        let summary = SummaryStore::open(&summary_dir)?;

        let container = Arc::new(DailyContainer {
            xlog,
            text,
            counter,
            alert,
            profile,
            summary,
            date: date.to_string(),
            last_access: std::sync::Mutex::new(Instant::now()),
        });

        self.containers.insert(date.to_string(), container.clone());
        debug!("Opened daily container for date: {}", date);

        Ok(container)
    }

    /// Close idle containers that haven't been accessed for `max_idle`.
    pub fn close_idle(&self, max_idle: Duration) {
        let mut to_remove = Vec::new();
        for entry in self.containers.iter() {
            if entry.value().idle_duration() > max_idle {
                to_remove.push(entry.key().clone());
            }
        }
        for date in to_remove {
            if let Some((_, container)) = self.containers.remove(&date) {
                match container.flush() {
                    Ok(()) => info!("Closed idle daily container: {}", date),
                    Err(error) => {
                        tracing::error!("Failed to flush idle daily container {}: {}", date, error)
                    }
                }
            }
        }
    }

    /// Flush all containers.
    pub fn flush_all(&self) -> std::io::Result<()> {
        let mut first_error = None;
        for entry in self.containers.iter() {
            if let Err(error) = entry.value().flush() {
                tracing::error!("Failed to flush daily container {}: {}", entry.key(), error);
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Start the background monitor that periodically flushes and closes idle containers.
    pub fn start_monitor(self: Arc<Self>, shutdown: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    _ = interval.tick() => {}
                }
                // Flush all dirty data
                if let Err(error) = self.flush_all() {
                    tracing::error!("DB monitor flush failed: {}", error);
                }
                // Close containers idle for more than 30 minutes
                self.close_idle(Duration::from_secs(30 * 60));
            }
            if let Err(error) = self.flush_all() {
                tracing::error!("Final DB monitor flush failed: {}", error);
            }
            info!("DB monitor stopped");
        })
    }
}

impl Drop for DbManager {
    fn drop(&mut self) {
        for entry in self.containers.iter() {
            if let Err(error) = entry.value().flush() {
                tracing::error!("DB flush during drop failed: {}", error);
            }
        }
    }
}
