use dashmap::DashMap;

use crate::protocol::pack::ObjectPack;

/// Caches agent/object information and tracks lifecycle state.
pub struct ObjectCache {
    /// objHash -> ObjectPack
    objects: DashMap<i32, ObjectPack>,
}

impl ObjectCache {
    pub fn new() -> Self {
        Self {
            objects: DashMap::new(),
        }
    }

    pub fn put(&self, pack: ObjectPack) {
        self.objects.insert(pack.obj_hash, pack);
    }

    pub fn get(&self, obj_hash: i32) -> Option<ObjectPack> {
        self.objects.get(&obj_hash).map(|v| v.value().clone())
    }

    pub fn contains(&self, obj_hash: i32) -> bool {
        self.objects.contains_key(&obj_hash)
    }

    pub fn remove(&self, obj_hash: i32) -> Option<ObjectPack> {
        self.objects.remove(&obj_hash).map(|(_, v)| v)
    }

    /// Get all live (alive) objects.
    pub fn get_live_objects(&self) -> Vec<ObjectPack> {
        self.objects
            .iter()
            .filter(|e| e.value().alive)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get all live objects of a specific type.
    pub fn get_live_objects_by_type(&self, obj_type: &str) -> Vec<ObjectPack> {
        self.objects
            .iter()
            .filter(|e| e.value().alive && e.value().obj_type == obj_type)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Get all live obj_hashes.
    pub fn get_live_obj_hashes(&self) -> Vec<i32> {
        self.objects
            .iter()
            .filter(|e| e.value().alive)
            .map(|e| *e.key())
            .collect()
    }

    /// Get all objects (alive and dead).
    pub fn get_all_objects(&self) -> Vec<ObjectPack> {
        self.objects.iter().map(|e| e.value().clone()).collect()
    }

    pub fn size(&self) -> usize {
        self.objects.len()
    }
}
