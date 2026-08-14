use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Background cleaner that deletes old YYYYMMDD directories.
pub struct Cleaner;

impl Cleaner {
    /// Start a background task that periodically cleans up old data directories.
    pub fn start(db_dir: String, keep_days: u32, shutdown: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Run immediately on start, then every hour
            Self::cleanup_once(&db_dir, keep_days);

            let mut interval = tokio::time::interval(Duration::from_secs(3600));
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    _ = interval.tick() => {}
                }
                Self::cleanup_once(&db_dir, keep_days);
            }
            info!("DB cleaner stopped");
        })
    }

    fn cleanup_once(db_dir: &str, keep_days: u32) {
        let cutoff = chrono::Local::now() - chrono::Duration::days(keep_days as i64);
        let cutoff_str = cutoff.format("%Y%m%d").to_string();

        let dir = PathBuf::from(db_dir);
        if !dir.exists() {
            return;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Only process YYYYMMDD directories (8 digits)
            if name.len() != 8 || !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            if name < cutoff_str {
                match fs::remove_dir_all(entry.path()) {
                    Ok(_) => info!("Cleaned up old data directory: {}", name),
                    Err(e) => warn!("Failed to clean up directory {}: {}", name, e),
                }
            }
        }
    }
}
