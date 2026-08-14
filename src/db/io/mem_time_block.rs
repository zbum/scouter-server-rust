use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;

use crate::protocol::data_output::to_bytes5;
use crate::util::date;

const COUNT_POS: usize = 4;
const MEM_HEAD_RESERVED: usize = 1024;
const KEY_LENGTH: usize = 5;
/// 3600 * 24 * 2 = 172,800 entries (48 hours at 500ms intervals)
const CAPACITY: usize = 3600 * 24 * 2;
const MEM_BUFFER_SIZE: usize = CAPACITY * KEY_LENGTH; // 864,000 bytes

/// In-memory time-bucket block backed by an .hfile.
/// Matches Java MemTimeBlock.
///
/// Uses 500ms time buckets, supporting a 48-hour time window per day.
pub struct MemTimeBlock {
    inner: Mutex<MemTimeBlockInner>,
    path: String,
}

struct MemTimeBlockInner {
    buf: Vec<u8>,
    count: i32,
    dirty: bool,
}

impl MemTimeBlock {
    pub fn open(path: &str) -> io::Result<Self> {
        let file_path = format!("{}.hfile", path);
        let p = Path::new(&file_path);
        let is_new = !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) < MEM_HEAD_RESERVED as u64;

        let (buf, count) = if is_new {
            let mut buf = vec![0u8; MEM_HEAD_RESERVED + MEM_BUFFER_SIZE];
            buf[0] = 0xCA;
            buf[1] = 0xFE;
            (buf, 0)
        } else {
            let buf = fs::read(&file_path)?;
            let count = to_int(&buf, COUNT_POS);
            (buf, count)
        };

        Ok(Self {
            inner: Mutex::new(MemTimeBlockInner {
                buf,
                count,
                dirty: false,
            }),
            path: file_path,
        })
    }

    /// Compute the buffer offset for a given time in milliseconds.
    fn offset(time_ms: i64) -> usize {
        let millis_in_day = date::get_date_millis(time_ms) as usize;
        let seconds_500 = millis_in_day / 500;
        let hash = seconds_500 % CAPACITY;
        KEY_LENGTH * hash + MEM_HEAD_RESERVED
    }

    /// Get the long5 value at the time bucket.
    pub fn get(&self, time_ms: i64) -> i64 {
        let inner = self.inner.lock().unwrap();
        let pos = Self::offset(time_ms);
        if pos + KEY_LENGTH > inner.buf.len() {
            return 0;
        }
        to_long5(&inner.buf, pos)
    }

    /// Put a long5 value at the time bucket.
    pub fn put(&self, time_ms: i64, value: i64) {
        let mut inner = self.inner.lock().unwrap();
        let pos = Self::offset(time_ms);
        if pos + KEY_LENGTH > inner.buf.len() {
            return;
        }

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

    pub fn add_count(&self, n: i32) {
        let mut inner = self.inner.lock().unwrap();
        inner.count += n;
        let count = inner.count;
        set_int(&mut inner.buf, COUNT_POS, count);
    }

    pub fn get_count(&self) -> i32 {
        self.inner.lock().unwrap().count
    }

    pub fn flush(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty {
            fs::write(&self.path, &inner.buf)?;
            inner.dirty = false;
        }
        Ok(())
    }
}

impl Drop for MemTimeBlock {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn to_int(buf: &[u8], pos: usize) -> i32 {
    i32::from_be_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]])
}

fn set_int(buf: &mut [u8], pos: usize, v: i32) {
    buf[pos..pos + 4].copy_from_slice(&v.to_be_bytes());
}

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
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_put_and_get() {
        let path = "/tmp/scouter_test_mtb";
        let _ = fs::remove_file(format!("{}.hfile", path));

        let mtb = MemTimeBlock::open(path).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        mtb.put(now, 42);
        assert_eq!(mtb.get(now), 42);

        // Different time (500ms later should be a different bucket)
        mtb.put(now + 500, 99);
        assert_eq!(mtb.get(now + 500), 99);

        // Flush and verify persistence
        mtb.flush().unwrap();
        drop(mtb);

        let mtb2 = MemTimeBlock::open(path).unwrap();
        assert_eq!(mtb2.get(now), 42);
        assert_eq!(mtb2.get(now + 500), 99);

        let _ = fs::remove_file(format!("{}.hfile", path));
    }
}
