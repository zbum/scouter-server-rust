use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;

use crate::core::summary_core::SummaryCore;
use crate::protocol::pack::{AlertPack, SummaryPack, SUMMARY_ALERT};
use crate::protocol::value::{MapValue, Value};
use crate::util::hash;

const WINDOW_MS: i64 = 300_000; // 5 minutes

struct AlertWindow {
    start_time: i64,
    counts: HashMap<i32, i32>, // hash(obj_type+title) -> count
}

/// Aggregates alerts in 5-minute windows and emits SummaryPack(stype=ALERT).
pub struct AlertSummary {
    summary_core: Arc<SummaryCore>,
    window: Mutex<AlertWindow>,
}

impl AlertSummary {
    pub fn new(summary_core: Arc<SummaryCore>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            summary_core,
            window: Mutex::new(AlertWindow {
                start_time: now,
                counts: HashMap::new(),
            }),
        }
    }

    pub async fn add(&self, alert: &AlertPack) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let mut window = self.window.lock().await;

        // Check if window expired
        if now - window.start_time >= WINDOW_MS && !window.counts.is_empty() {
            let old_counts = std::mem::take(&mut window.counts);
            window.start_time = now;
            // Flush asynchronously
            self.flush_window(old_counts).await;
        }

        // Add to current window
        let key = format!("{}{}", alert.obj_type, alert.title);
        let key_hash = hash::hash(&key);
        *window.counts.entry(key_hash).or_insert(0) += 1;
    }

    async fn flush_window(&self, counts: HashMap<i32, i32>) {
        let mut table = MapValue::new();
        for (key_hash, count) in counts {
            table.put(key_hash.to_string(), Value::Decimal(count as i64));
        }

        let pack = SummaryPack {
            stype: SUMMARY_ALERT,
            table,
        };

        self.summary_core.add(pack).await;
    }
}
