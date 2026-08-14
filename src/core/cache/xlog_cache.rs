use std::collections::VecDeque;
use std::sync::Mutex;

/// Cached XLog entry for real-time streaming to clients.
#[derive(Debug, Clone)]
pub struct XLogCacheEntry {
    pub obj_hash: i32,
    pub elapsed: i32,
    pub has_error: bool,
    pub data: Vec<u8>,
    pub index: u64,
}

/// Circular buffer cache for recent XLogs.
pub struct XLogCache {
    entries: Mutex<VecDeque<XLogCacheEntry>>,
    max_size: usize,
    next_index: Mutex<u64>,
}

impl XLogCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_size)),
            max_size,
            next_index: Mutex::new(0),
        }
    }

    pub fn put(&self, obj_hash: i32, elapsed: i32, has_error: bool, data: Vec<u8>) -> u64 {
        let mut idx = self.next_index.lock().unwrap();
        let index = *idx;
        *idx += 1;
        drop(idx);

        let entry = XLogCacheEntry {
            obj_hash,
            elapsed,
            has_error,
            data,
            index,
        };

        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= self.max_size {
            entries.pop_front();
        }
        entries.push_back(entry);
        index
    }

    /// Get entries since a given index, optionally filtered by obj_hash.
    pub fn get_since(&self, since_index: u64, obj_hash: Option<i32>) -> Vec<XLogCacheEntry> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .filter(|e| e.index > since_index)
            .filter(|e| obj_hash.map_or(true, |h| e.obj_hash == h))
            .cloned()
            .collect()
    }

    /// Get the latest index.
    pub fn latest_index(&self) -> u64 {
        let idx = self.next_index.lock().unwrap();
        if *idx == 0 { 0 } else { *idx - 1 }
    }

    /// Get the latest N entries.
    pub fn get_latest(&self, count: usize) -> Vec<XLogCacheEntry> {
        let entries = self.entries.lock().unwrap();
        let len = entries.len();
        let skip = if len > count { len - count } else { 0 };
        entries.iter().skip(skip).cloned().collect()
    }

    pub fn size(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}
