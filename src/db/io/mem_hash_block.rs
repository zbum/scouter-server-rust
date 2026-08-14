use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;

use crate::protocol::data_output::to_bytes5;

const COUNT_POS: usize = 4;
const MEM_HEAD_RESERVED: usize = 1024;
const KEY_LENGTH: usize = 5;

/// In-memory hash block backed by an .hfile.
/// Matches Java MemHashBlock.
///
/// File format:
///   [0xCA, 0xFE]  (2 bytes magic at offset 0)
///   [padding]      (up to offset 4)
///   [count: 4B]    (at offset 4)
///   [padding]      (up to offset 1024)
///   [slots...]     (5 bytes each, long5 encoded)
pub struct MemHashBlock {
    inner: Mutex<MemHashBlockInner>,
    path: String,
}

struct MemHashBlockInner {
    buf: Vec<u8>,
    count: i32,
    capacity: usize,
    dirty: bool,
}

impl MemHashBlock {
    pub fn open(path: &str, mem_size: usize) -> io::Result<Self> {
        let file_path = format!("{}.hfile", path);
        let p = Path::new(&file_path);
        let is_new = !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) < MEM_HEAD_RESERVED as u64;

        let (buf, count, actual_mem_size) = if is_new {
            let mut buf = vec![0u8; MEM_HEAD_RESERVED + mem_size];
            buf[0] = 0xCA;
            buf[1] = 0xFE;
            (buf, 0, mem_size)
        } else {
            let buf = fs::read(&file_path)?;
            let actual_mem = buf.len().saturating_sub(MEM_HEAD_RESERVED);
            let count = to_int(&buf, COUNT_POS);
            (buf, count, actual_mem)
        };

        let capacity = actual_mem_size / KEY_LENGTH;

        Ok(Self {
            inner: Mutex::new(MemHashBlockInner {
                buf,
                count,
                capacity,
                dirty: false,
            }),
            path: file_path,
        })
    }

    fn offset(key_hash: i32, capacity: usize) -> usize {
        let bucket_pos = (key_hash as u32 & 0x7FFFFFFF) as usize % capacity;
        KEY_LENGTH * bucket_pos + MEM_HEAD_RESERVED
    }

    /// Get the long5 value at the hash bucket for key_hash.
    pub fn get(&self, key_hash: i32) -> i64 {
        let inner = self.inner.lock().unwrap();
        let pos = Self::offset(key_hash, inner.capacity);
        to_long5(&inner.buf, pos)
    }

    /// Put a long5 value at the hash bucket for key_hash.
    pub fn put(&self, key_hash: i32, value: i64) {
        let mut inner = self.inner.lock().unwrap();
        let pos = Self::offset(key_hash, inner.capacity);

        // Increment count if bucket was empty
        if to_long5(&inner.buf, pos) == 0 {
            inner.count += 1;
            let count = inner.count;
            set_int(&mut inner.buf, COUNT_POS, count);
        }

        let bytes = to_bytes5(value);
        inner.buf[pos..pos + KEY_LENGTH].copy_from_slice(&bytes);
        inner.dirty = true;
    }

    pub fn get_count(&self) -> i32 {
        self.inner.lock().unwrap().count
    }

    /// Flush buffer to disk.
    pub fn flush(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty {
            fs::write(&self.path, &inner.buf)?;
            inner.dirty = false;
        }
        Ok(())
    }
}

impl Drop for MemHashBlock {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// Read a big-endian i32 from buffer at offset.
fn to_int(buf: &[u8], pos: usize) -> i32 {
    i32::from_be_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]])
}

/// Write a big-endian i32 into buffer at offset.
fn set_int(buf: &mut [u8], pos: usize, v: i32) {
    let bytes = v.to_be_bytes();
    buf[pos..pos + 4].copy_from_slice(&bytes);
}

/// Read a long5 (5-byte signed) from buffer at offset.
fn to_long5(buf: &[u8], pos: usize) -> i64 {
    let b0 = buf[pos] as i8 as i64;
    (b0 << 32)
        | ((buf[pos + 1] as i64) << 24)
        | ((buf[pos + 2] as i64) << 16)
        | ((buf[pos + 3] as i64) << 8)
        | (buf[pos + 4] as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_put_and_get() {
        let path = "/tmp/scouter_test_mhb";
        let _ = fs::remove_file(format!("{}.hfile", path));

        let mhb = MemHashBlock::open(path, 1024 * 1024).unwrap();

        mhb.put(42, 12345);
        assert_eq!(mhb.get(42), 12345);
        assert_eq!(mhb.get_count(), 1);

        // Update same bucket
        mhb.put(42, 67890);
        assert_eq!(mhb.get(42), 67890);
        assert_eq!(mhb.get_count(), 1); // count doesn't increase for overwrites

        // Different key
        mhb.put(100, 99999);
        assert_eq!(mhb.get(100), 99999);
        assert_eq!(mhb.get_count(), 2);

        // Flush and reopen
        mhb.flush().unwrap();
        drop(mhb);

        let mhb2 = MemHashBlock::open(path, 1024 * 1024).unwrap();
        assert_eq!(mhb2.get(42), 67890);
        assert_eq!(mhb2.get(100), 99999);
        assert_eq!(mhb2.get_count(), 2);

        let _ = fs::remove_file(format!("{}.hfile", path));
    }

    #[test]
    fn test_empty_bucket_returns_zero() {
        let path = "/tmp/scouter_test_mhb_empty";
        let _ = fs::remove_file(format!("{}.hfile", path));

        let mhb = MemHashBlock::open(path, 1024 * 1024).unwrap();
        assert_eq!(mhb.get(999), 0);

        let _ = fs::remove_file(format!("{}.hfile", path));
    }
}
