use dashmap::DashMap;

/// Simple in-memory Key-Value store for global data sharing via TCP clients.
pub struct KvStore {
    data: DashMap<String, String>,
}

impl KvStore {
    pub fn new() -> Self {
        Self {
            data: DashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| v.value().clone())
    }

    pub fn set(&self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    pub fn delete(&self, key: &str) {
        self.data.remove(key);
    }

    pub fn list(&self) -> Vec<(String, String)> {
        self.data.iter().map(|e| (e.key().clone(), e.value().clone())).collect()
    }
}
