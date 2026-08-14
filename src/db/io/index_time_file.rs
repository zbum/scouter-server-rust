use std::io;

use crate::db::io::mem_time_block::MemTimeBlock;
use crate::db::io::real_key_file::RealKeyFile;
use crate::protocol::data_output::{to_bytes5, to_bytes_long};

const SECONDS_PER_DAY_X2: i64 = 86400 * 2;

/// Time-based index combining MemTimeBlock + RealKeyFile.
/// Matches Java IndexTimeFile.
///
/// Uses 500ms time buckets. Entries within the same bucket are
/// chained via RealKeyFile's prevPos mechanism.
pub struct IndexTimeFile {
    time_block: MemTimeBlock,
    key_file: RealKeyFile,
}

impl IndexTimeFile {
    pub fn open(path: &str) -> io::Result<Self> {
        let time_block = MemTimeBlock::open(path)?;
        let key_file = RealKeyFile::open(path)?;
        Ok(Self { time_block, key_file })
    }

    /// Insert a time → data_pos mapping.
    /// `data_pos` is stored as a long5 (5 bytes).
    pub fn put(&self, time_ms: i64, data_pos: i64) -> io::Result<()> {
        if time_ms <= 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid time"));
        }

        let prev_pos = self.time_block.get(time_ms);
        let time_key = to_bytes_long(time_ms);
        let data_pos_bytes = to_bytes5(data_pos);
        let new_pos = self.key_file.append(prev_pos, &time_key, &data_pos_bytes)?;
        self.time_block.put(time_ms, new_pos);
        self.time_block.add_count(1);

        Ok(())
    }

    /// Read all entries in a time range [from_ms, to_ms], stepping 500ms forward.
    /// Calls handler(time_ms, data_pos_bytes) for each entry.
    pub fn read<F>(&self, from_ms: i64, to_ms: i64, mut handler: F) -> io::Result<()>
    where
        F: FnMut(i64, &[u8]),
    {
        let mut t = from_ms;
        let mut i: i64 = 0;

        while i < SECONDS_PER_DAY_X2 && t <= to_ms {
            let entries = self.get_bucket_entries(t)?;
            // Forward order (sorted by time)
            for entry in &entries {
                handler(entry.time, &entry.data_pos);
            }
            i += 1;
            t += 500;
        }

        Ok(())
    }

    /// Read all entries in a time range, calling handler with resolved data.
    /// handler(time_ms, data_bytes) where data_bytes is read by the reader function.
    pub fn read_with_reader<F, R>(
        &self,
        from_ms: i64,
        to_ms: i64,
        mut handler: F,
        reader: R,
    ) -> io::Result<()>
    where
        F: FnMut(i64, Vec<u8>),
        R: Fn(i64) -> io::Result<Vec<u8>>,
    {
        let mut t = from_ms;
        let mut i: i64 = 0;

        while i < SECONDS_PER_DAY_X2 && t <= to_ms {
            let entries = self.get_bucket_entries(t)?;
            for entry in &entries {
                if entry.time >= from_ms && entry.time <= to_ms {
                    let offset = to_long5_from_slice(&entry.data_pos);
                    if let Ok(data) = reader(offset) {
                        handler(entry.time, data);
                    }
                }
            }
            i += 1;
            t += 500;
        }

        Ok(())
    }

    /// Read in reverse time order.
    pub fn read_from_end<F>(&self, from_ms: i64, to_ms: i64, mut handler: F) -> io::Result<()>
    where
        F: FnMut(i64, &[u8]),
    {
        let mut t = to_ms;
        let mut i: i64 = 0;

        while i < SECONDS_PER_DAY_X2 && from_ms <= t {
            let mut entries = self.get_bucket_entries(t)?;
            entries.reverse();
            for entry in &entries {
                handler(entry.time, &entry.data_pos);
            }
            i += 1;
            t -= 500;
        }

        Ok(())
    }

    /// Get the start and end data positions in a time range.
    pub fn get_start_end_data_pos(
        &self,
        from_ms: i64,
        to_ms: i64,
    ) -> io::Result<(Option<Vec<u8>>, Option<Vec<u8>>)> {
        let start = self.get_data_pos_first(from_ms, to_ms)?;
        let end = self.get_data_pos_last(from_ms, to_ms)?;
        Ok((start, end))
    }

    /// Get all entries in a single time bucket, sorted by time.
    fn get_bucket_entries(&self, time_ms: i64) -> io::Result<Vec<TimeToData>> {
        let mut entries = Vec::new();
        let mut pos = self.time_block.get(time_ms);

        while pos > 0 {
            if !self.key_file.is_deleted(pos)? {
                let record = self.key_file.get_record(pos)?;
                let time = i64::from_be_bytes([
                    record.key[0],
                    record.key[1],
                    record.key[2],
                    record.key[3],
                    record.key[4],
                    record.key[5],
                    record.key[6],
                    record.key[7],
                ]);
                entries.push(TimeToData {
                    time,
                    data_pos: record.data_pos,
                });
            }
            pos = self.key_file.get_prev_pos(pos)?;
        }

        // Sort by time (ascending)
        entries.sort_by_key(|e| e.time);
        Ok(entries)
    }

    /// Find the first data position in a time range.
    fn get_data_pos_first(&self, from_ms: i64, to_ms: i64) -> io::Result<Option<Vec<u8>>> {
        let mut t = from_ms;
        let mut i: i64 = 0;
        while i < SECONDS_PER_DAY_X2 && t <= to_ms {
            let mut pos = self.time_block.get(t);
            // Walk to the end of the chain (first inserted)
            while pos > 0 {
                let prev = self.key_file.get_prev_pos(pos)?;
                if prev == 0 {
                    return Ok(Some(self.key_file.get_data_pos(pos)?));
                }
                pos = prev;
            }
            i += 1;
            t += 500;
        }
        Ok(None)
    }

    /// Find the last data position in a time range.
    fn get_data_pos_last(&self, from_ms: i64, to_ms: i64) -> io::Result<Option<Vec<u8>>> {
        let mut t = to_ms;
        let mut i: i64 = 0;
        while i < SECONDS_PER_DAY_X2 && from_ms <= t {
            let pos = self.time_block.get(t);
            if pos > 0 {
                return Ok(Some(self.key_file.get_data_pos(pos)?));
            }
            i += 1;
            t -= 500;
        }
        Ok(None)
    }

    pub fn flush(&self) -> io::Result<()> {
        self.time_block.flush()?;
        self.key_file.flush()?;
        Ok(())
    }
}

impl Drop for IndexTimeFile {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

struct TimeToData {
    time: i64,
    data_pos: Vec<u8>,
}

/// Convert a 5-byte slice to i64 (long5 with sign extension).
fn to_long5_from_slice(buf: &[u8]) -> i64 {
    if buf.len() < 5 {
        return 0;
    }
    let b0 = buf[0] as i8 as i64;
    (b0 << 32)
        | ((buf[1] as i64) << 24)
        | ((buf[2] as i64) << 16)
        | ((buf[3] as i64) << 8)
        | (buf[4] as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cleanup(path: &str) {
        let _ = fs::remove_file(format!("{}.hfile", path));
        let _ = fs::remove_file(format!("{}.kfile", path));
    }

    #[test]
    fn test_put_and_read() {
        let path = "/tmp/scouter_test_itf";
        cleanup(path);

        let itf = IndexTimeFile::open(path).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Insert 3 entries at different times
        itf.put(now, 100).unwrap();
        itf.put(now + 1000, 200).unwrap();
        itf.put(now + 2000, 300).unwrap();

        // Read all entries in range
        let mut results = Vec::new();
        itf.read(now - 1000, now + 3000, |time, data_pos| {
            results.push((time, data_pos.to_vec()));
        })
        .unwrap();

        assert!(results.len() >= 3);

        cleanup(path);
    }

    #[test]
    fn test_read_from_end() {
        let path = "/tmp/scouter_test_itf_rev";
        cleanup(path);

        let itf = IndexTimeFile::open(path).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        itf.put(now, 100).unwrap();
        itf.put(now + 1000, 200).unwrap();

        let mut results = Vec::new();
        itf.read_from_end(now - 1000, now + 2000, |time, _data_pos| {
            results.push(time);
        })
        .unwrap();

        // Should have results in reverse order
        assert!(!results.is_empty());

        cleanup(path);
    }
}
