use std::collections::HashMap;
use std::time::Instant;
use tokio::sync::Mutex;

struct MultiPacketEntry {
    total: i16,
    received: HashMap<i16, Vec<u8>>,
    created_at: Instant,
}

pub struct MultiPacketProcessor {
    entries: Mutex<HashMap<i64, MultiPacketEntry>>,
}

impl MultiPacketProcessor {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Add a fragment. Returns assembled data when all fragments received.
    pub async fn add(
        &self,
        packet_id: i64,
        total: i16,
        num: i16,
        data: Vec<u8>,
    ) -> Option<Vec<u8>> {
        let mut entries = self.entries.lock().await;

        // Cleanup expired entries (>1 second old)
        let now = Instant::now();
        entries.retain(|_, v| now.duration_since(v.created_at).as_millis() < 1000);

        let entry = entries.entry(packet_id).or_insert_with(|| MultiPacketEntry {
            total,
            received: HashMap::new(),
            created_at: Instant::now(),
        });

        entry.received.insert(num, data);

        if entry.received.len() == entry.total as usize {
            let entry = entries.remove(&packet_id).unwrap();
            // Reassemble in order
            let mut result = Vec::new();
            for i in 0..entry.total {
                if let Some(fragment) = entry.received.get(&i) {
                    result.extend_from_slice(fragment);
                }
            }
            Some(result)
        } else {
            None
        }
    }
}
