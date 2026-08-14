use dashmap::DashMap;

/// Caches text data (SQL, method names, etc.) keyed by (type, hash).
/// This allows clients to resolve hash codes to human-readable text.
pub struct TextCache {
    /// Key: (text_type, hash) -> text
    table: DashMap<(String, i32), String>,
}

impl TextCache {
    pub fn new() -> Self {
        Self {
            table: DashMap::new(),
        }
    }

    pub fn put(&self, text_type: &str, hash: i32, text: &str) {
        self.table.insert((text_type.to_string(), hash), text.to_string());
    }

    pub fn get(&self, text_type: &str, hash: i32) -> Option<String> {
        self.table
            .get(&(text_type.to_string(), hash))
            .map(|v| v.value().clone())
    }

    pub fn contains(&self, text_type: &str, hash: i32) -> bool {
        self.table.contains_key(&(text_type.to_string(), hash))
    }

    pub fn size(&self) -> usize {
        self.table.len()
    }
}
