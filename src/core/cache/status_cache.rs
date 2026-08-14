use dashmap::DashMap;

use crate::protocol::pack::StatusPack;

/// Caches the latest status for each object+key combination.
pub struct StatusCache {
    /// Key: (obj_hash, key) -> StatusPack
    table: DashMap<(i32, String), StatusPack>,
}

impl StatusCache {
    pub fn new() -> Self {
        Self {
            table: DashMap::new(),
        }
    }

    pub fn put(&self, pack: StatusPack) {
        let key = (pack.obj_hash, pack.key.clone());
        self.table.insert(key, pack);
    }

    pub fn get(&self, obj_hash: i32, key: &str) -> Option<StatusPack> {
        self.table
            .get(&(obj_hash, key.to_string()))
            .map(|v| v.value().clone())
    }

    pub fn get_by_obj(&self, obj_hash: i32) -> Vec<StatusPack> {
        self.table
            .iter()
            .filter(|e| e.key().0 == obj_hash)
            .map(|e| e.value().clone())
            .collect()
    }

    pub fn size(&self) -> usize {
        self.table.len()
    }
}
