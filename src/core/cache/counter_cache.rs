use dashmap::DashMap;

use crate::protocol::value::Value;

/// Key for counter cache: (obj_hash, counter_name, time_type)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CounterKey {
    pub obj_hash: i32,
    pub counter_name: String,
    pub time_type: i8,
}

/// Caches the latest counter values for real-time display.
pub struct CounterCache {
    table: DashMap<CounterKey, Value>,
}

impl CounterCache {
    pub fn new() -> Self {
        Self {
            table: DashMap::new(),
        }
    }

    pub fn put(&self, obj_hash: i32, counter_name: &str, time_type: i8, value: Value) {
        let key = CounterKey {
            obj_hash,
            counter_name: counter_name.to_string(),
            time_type,
        };
        self.table.insert(key, value);
    }

    pub fn get(&self, obj_hash: i32, counter_name: &str, time_type: i8) -> Option<Value> {
        let key = CounterKey {
            obj_hash,
            counter_name: counter_name.to_string(),
            time_type,
        };
        self.table.get(&key).map(|v| v.value().clone())
    }

    /// Get all counters for a specific object and time type.
    pub fn get_counters_by_obj(&self, obj_hash: i32, time_type: i8) -> Vec<(String, Value)> {
        self.table
            .iter()
            .filter(|e| e.key().obj_hash == obj_hash && e.key().time_type == time_type)
            .map(|e| (e.key().counter_name.clone(), e.value().clone()))
            .collect()
    }

    /// Get all (obj_hash, value) pairs for a specific counter name and time type.
    pub fn get_all_for_counter(&self, counter_name: &str, time_type: i8) -> Vec<(i32, Value)> {
        self.table
            .iter()
            .filter(|e| e.key().counter_name == counter_name && e.key().time_type == time_type)
            .map(|e| (e.key().obj_hash, e.value().clone()))
            .collect()
    }

    pub fn size(&self) -> usize {
        self.table.len()
    }
}
