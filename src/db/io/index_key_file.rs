use std::io;

use crate::db::io::mem_hash_block::MemHashBlock;
use crate::db::io::real_key_file::RealKeyFile;
use crate::util::hash::hash_bytes;

/// Composite hash + key file index.
/// Matches Java IndexKeyFile.
///
/// Combines MemHashBlock (hash table → chain head position)
/// with RealKeyFile (collision chain storage).
pub struct IndexKeyFile {
    hash_block: MemHashBlock,
    key_file: RealKeyFile,
}

impl IndexKeyFile {
    /// Open or create an IndexKeyFile.
    /// Creates `{path}.hfile` and `{path}.kfile`.
    /// `hash_size_mb` controls the hash table size in megabytes.
    pub fn open(path: &str, hash_size_mb: usize) -> io::Result<Self> {
        let hash_block = MemHashBlock::open(path, hash_size_mb * 1024 * 1024)?;
        let key_file = RealKeyFile::open(path)?;
        Ok(Self { hash_block, key_file })
    }

    /// Insert a key → data_pos mapping.
    /// Chains to any existing entries with the same hash.
    pub fn put(&self, key: &[u8], data_pos: &[u8]) -> io::Result<()> {
        let key_hash = hash_bytes(key);
        let prev_pos = self.hash_block.get(key_hash);
        let new_pos = self.key_file.append(prev_pos, key, data_pos)?;
        self.hash_block.put(key_hash, new_pos);
        Ok(())
    }

    /// Look up the first matching data_pos for a key.
    /// Walks the collision chain comparing keys.
    pub fn get(&self, key: &[u8]) -> io::Result<Option<Vec<u8>>> {
        let key_hash = hash_bytes(key);
        let mut pos = self.hash_block.get(key_hash);

        while pos > 0 {
            if !self.key_file.is_deleted(pos)? {
                let stored_key = self.key_file.get_key(pos)?;
                if stored_key == key {
                    return Ok(Some(self.key_file.get_data_pos(pos)?));
                }
            }
            pos = self.key_file.get_prev_pos(pos)?;
        }

        Ok(None)
    }

    /// Get ALL matching data_pos entries for a key (for multi-value lookups like gxid).
    pub fn get_all(&self, key: &[u8]) -> io::Result<Vec<Vec<u8>>> {
        let key_hash = hash_bytes(key);
        let mut pos = self.hash_block.get(key_hash);
        let mut results = Vec::new();

        while pos > 0 {
            if !self.key_file.is_deleted(pos)? {
                let stored_key = self.key_file.get_key(pos)?;
                if stored_key == key {
                    results.push(self.key_file.get_data_pos(pos)?);
                }
            }
            pos = self.key_file.get_prev_pos(pos)?;
        }

        Ok(results)
    }

    /// Check if a key exists.
    pub fn has_key(&self, key: &[u8]) -> io::Result<bool> {
        Ok(self.get(key)?.is_some())
    }

    /// Iterate all non-deleted records.
    pub fn read_all<F>(&self, mut handler: F) -> io::Result<()>
    where
        F: FnMut(&[u8], &[u8]),
    {
        let mut pos = self.key_file.get_first_pos();
        let length = self.key_file.get_length()?;

        while pos < length && pos > 0 {
            match self.key_file.get_record(pos) {
                Ok(record) => {
                    if !record.deleted {
                        handler(&record.key, &record.data_pos);
                    }
                    pos = record.next_offset;
                }
                Err(_) => break,
            }
        }

        Ok(())
    }

    /// Delete all entries matching a key.
    pub fn delete(&self, key: &[u8]) -> io::Result<i32> {
        let key_hash = hash_bytes(key);
        let mut pos = self.hash_block.get(key_hash);
        let mut deleted = 0;

        while pos > 0 {
            if !self.key_file.is_deleted(pos)? {
                let stored_key = self.key_file.get_key(pos)?;
                if stored_key == key {
                    self.key_file.set_delete(pos, true)?;
                    deleted += 1;
                }
            }
            pos = self.key_file.get_prev_pos(pos)?;
        }

        Ok(deleted)
    }

    pub fn flush(&self) -> io::Result<()> {
        self.hash_block.flush()?;
        self.key_file.flush()?;
        Ok(())
    }
}

impl Drop for IndexKeyFile {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cleanup(path: &str) {
        let _ = fs::remove_file(format!("{}.hfile", path));
        let _ = fs::remove_file(format!("{}.kfile", path));
    }

    #[test]
    fn test_put_and_get() {
        let path = "/tmp/scouter_test_ikf";
        cleanup(path);

        let ikf = IndexKeyFile::open(path, 1).unwrap();

        ikf.put(b"key1", b"value1").unwrap();
        ikf.put(b"key2", b"value2").unwrap();

        let v1 = ikf.get(b"key1").unwrap();
        assert_eq!(v1, Some(b"value1".to_vec()));

        let v2 = ikf.get(b"key2").unwrap();
        assert_eq!(v2, Some(b"value2".to_vec()));

        let v3 = ikf.get(b"key3").unwrap();
        assert_eq!(v3, None);

        cleanup(path);
    }

    #[test]
    fn test_get_all() {
        let path = "/tmp/scouter_test_ikf_all";
        cleanup(path);

        let ikf = IndexKeyFile::open(path, 1).unwrap();

        // Multiple values for same key
        ikf.put(b"gxid_key", b"pos1").unwrap();
        ikf.put(b"gxid_key", b"pos2").unwrap();
        ikf.put(b"gxid_key", b"pos3").unwrap();

        let all = ikf.get_all(b"gxid_key").unwrap();
        assert_eq!(all.len(), 3);

        cleanup(path);
    }

    #[test]
    fn test_has_key() {
        let path = "/tmp/scouter_test_ikf_has";
        cleanup(path);

        let ikf = IndexKeyFile::open(path, 1).unwrap();
        ikf.put(b"exists", b"val").unwrap();

        assert!(ikf.has_key(b"exists").unwrap());
        assert!(!ikf.has_key(b"nope").unwrap());

        cleanup(path);
    }

    #[test]
    fn test_read_all() {
        let path = "/tmp/scouter_test_ikf_read";
        cleanup(path);

        let ikf = IndexKeyFile::open(path, 1).unwrap();
        ikf.put(b"a", b"1").unwrap();
        ikf.put(b"b", b"2").unwrap();
        ikf.put(b"c", b"3").unwrap();

        let mut entries = Vec::new();
        ikf.read_all(|k, v| {
            entries.push((k.to_vec(), v.to_vec()));
        })
        .unwrap();

        assert_eq!(entries.len(), 3);

        cleanup(path);
    }

    #[test]
    fn test_delete() {
        let path = "/tmp/scouter_test_ikf_del";
        cleanup(path);

        let ikf = IndexKeyFile::open(path, 1).unwrap();
        ikf.put(b"del_me", b"val").unwrap();

        assert!(ikf.has_key(b"del_me").unwrap());

        let deleted = ikf.delete(b"del_me").unwrap();
        assert_eq!(deleted, 1);

        assert!(!ikf.has_key(b"del_me").unwrap());

        cleanup(path);
    }

    #[test]
    fn test_persistence() {
        let path = "/tmp/scouter_test_ikf_persist";
        cleanup(path);

        {
            let ikf = IndexKeyFile::open(path, 1).unwrap();
            ikf.put(b"persist_key", b"persist_val").unwrap();
            ikf.flush().unwrap();
        }

        {
            let ikf = IndexKeyFile::open(path, 1).unwrap();
            let v = ikf.get(b"persist_key").unwrap();
            assert_eq!(v, Some(b"persist_val".to_vec()));
        }

        cleanup(path);
    }
}
