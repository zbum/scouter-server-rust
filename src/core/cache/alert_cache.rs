use std::collections::VecDeque;
use std::sync::Mutex;

use crate::protocol::pack::AlertPack;

/// Cached alert entry with monotonically increasing index.
#[derive(Debug, Clone)]
pub struct AlertCacheEntry {
    pub pack: AlertPack,
    pub index: u64,
}

/// Circular buffer cache for recent alerts with index-based polling.
/// Follows the same pattern as XLogCache for incremental client polling.
pub struct AlertCache {
    entries: Mutex<VecDeque<AlertCacheEntry>>,
    max_size: usize,
    next_index: Mutex<u64>,
    loop_count: Mutex<u64>,
}

impl AlertCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
            next_index: Mutex::new(0),
            loop_count: Mutex::new(0),
        }
    }

    pub fn put(&self, pack: AlertPack) {
        let mut idx = self.next_index.lock().unwrap();
        let index = *idx;
        *idx += 1;
        drop(idx);

        let entry = AlertCacheEntry { pack, index };

        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= self.max_size {
            entries.pop_front();
            // Increment loop count when buffer wraps
            let mut lc = self.loop_count.lock().unwrap();
            *lc += 1;
        }
        entries.push_back(entry);
    }

    /// Get alerts since the given index. Returns (entries, current_loop, current_index).
    pub fn get_since(&self, since_index: u64) -> (Vec<AlertCacheEntry>, u64, u64) {
        let entries = self.entries.lock().unwrap();
        let loop_count = *self.loop_count.lock().unwrap();
        let next_index = *self.next_index.lock().unwrap();

        let result: Vec<AlertCacheEntry> = entries
            .iter()
            .filter(|e| e.index > since_index)
            .cloned()
            .collect();

        (result, loop_count, next_index)
    }

    pub fn get_recent(&self, count: usize) -> Vec<AlertPack> {
        let entries = self.entries.lock().unwrap();
        entries.iter().rev().take(count).map(|e| e.pack.clone()).collect()
    }

    pub fn current_loop(&self) -> u64 {
        *self.loop_count.lock().unwrap()
    }

    pub fn current_index(&self) -> u64 {
        *self.next_index.lock().unwrap()
    }

    pub fn size(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}
