use std::fs;
use std::io;
use std::path::Path;

use crate::db::io::index_key_file::IndexKeyFile;

/// Text storage using IndexKeyFile.
/// Matches Java TextTable.
///
/// Key = [int(div_hash), int(text_hash)] (8 bytes)
/// Value = UTF-8 text bytes
pub struct TextStore {
    index: IndexKeyFile,
}

impl TextStore {
    pub fn open(dir: &Path, index_mb: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join("text");
        let index = IndexKeyFile::open(path.to_str().unwrap(), index_mb)?;
        Ok(Self { index })
    }

    /// Store a text entry.
    /// `div` is the text type hash, `hash` is the text content hash.
    pub fn put(&self, div: i32, hash: i32, text: &str) -> io::Result<()> {
        let key = make_key(div, hash);
        // Check if already exists (dedup)
        if self.index.has_key(&key)? {
            return Ok(());
        }
        self.index.put(&key, text.as_bytes())
    }

    /// Retrieve text by div + hash.
    pub fn get(&self, div: i32, hash: i32) -> io::Result<Option<String>> {
        let key = make_key(div, hash);
        match self.index.get(&key)? {
            Some(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).to_string())),
            None => Ok(None),
        }
    }

    pub fn flush(&self) -> io::Result<()> {
        self.index.flush()
    }
}

fn make_key(div: i32, hash: i32) -> [u8; 8] {
    let mut key = [0u8; 8];
    key[0..4].copy_from_slice(&div.to_be_bytes());
    key[4..8].copy_from_slice(&hash.to_be_bytes());
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleanup(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_put_and_get() {
        let dir = "/tmp/scouter_test_text_store";
        cleanup(dir);

        let store = TextStore::open(Path::new(dir), 1).unwrap();

        store.put(1, 100, "SELECT * FROM users").unwrap();
        store.put(2, 200, "/api/v1/users").unwrap();

        let t1 = store.get(1, 100).unwrap();
        assert_eq!(t1, Some("SELECT * FROM users".to_string()));

        let t2 = store.get(2, 200).unwrap();
        assert_eq!(t2, Some("/api/v1/users".to_string()));

        let t3 = store.get(3, 300).unwrap();
        assert_eq!(t3, None);

        // Dedup: putting same key again should not fail
        store.put(1, 100, "SELECT * FROM users").unwrap();

        cleanup(dir);
    }
}
