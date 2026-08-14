use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::db::io::index_key_file::IndexKeyFile;
use crate::db::io::real_data_file::RealDataFile;
use crate::protocol::data_output::to_bytes5;

// Time type constants (matching Java DailyCounterUtils)
pub const TIME_TYPE_ONE_MIN: i8 = 1;
pub const TIME_TYPE_FIVE_MIN: i8 = 5;
pub const TIME_TYPE_TEN_MIN: i8 = 10;
pub const TIME_TYPE_HOUR: i8 = 60;

// Value type constants
pub const VALUE_TYPE_FLOAT: u8 = 5;
pub const VALUE_TYPE_DOUBLE: u8 = 9;
pub const VALUE_TYPE_DECIMAL: u8 = 10;

/// Build a counter key from obj_hash and counter name.
/// Key = [obj_hash(4 bytes BE) + counter_name_hash(4 bytes BE)]
pub fn make_counter_key(obj_hash: i32, counter_name: &str) -> [u8; 8] {
    let name_hash = crate::util::hash::hash(counter_name);
    let mut key = [0u8; 8];
    key[0..4].copy_from_slice(&obj_hash.to_be_bytes());
    key[4..8].copy_from_slice(&name_hash.to_be_bytes());
    key
}

/// Counter storage with index + data file.
/// Matches Java DailyCounterWR/DailyCounterData/DailyCounterIndex.
pub struct CounterStore {
    index: IndexKeyFile,
    data_file: RealDataFile,
    /// Path for random-access reads/writes to data file
    data_path: String,
}

impl CounterStore {
    /// Open or create counter storage.
    /// Files: {date}5m.index.{hfile,kfile} and {date}5m.data
    pub fn open(dir: &Path, date: &str, index_mb: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;

        let index_path = dir.join(format!("{}5m.index", date));
        let data_path = dir.join(format!("{}5m.data", date));
        let data_path_str = data_path.to_str().unwrap().to_string();

        let index = IndexKeyFile::open(index_path.to_str().unwrap(), index_mb)?;
        let data_file = RealDataFile::open(&data_path_str)?;

        Ok(Self {
            index,
            data_file,
            data_path: data_path_str,
        })
    }

    /// Write a counter value at a specific time bucket.
    /// `key` is typically `[obj_hash_bytes + counter_name_bytes]`.
    /// `hhmm` is 0-2359.
    /// `value` is the counter value as f64.
    /// `time_type` controls bucket granularity.
    pub fn write(
        &self,
        key: &[u8],
        hhmm: i32,
        value: f64,
        time_type: i8,
    ) -> io::Result<()> {
        let offset_bytes = self.index.get(key)?;

        match offset_bytes {
            Some(ob) => {
                // Update existing record in-place
                let offset = to_long5_from_bytes(&ob);
                self.update_value(offset, hhmm, value, time_type)?;
            }
            None => {
                // Create new record
                let offset = self.write_new_record(hhmm, value, time_type)?;
                let offset_5 = to_bytes5(offset);
                self.index.put(key, &offset_5)?;
            }
        }

        Ok(())
    }

    /// Create a new counter data record.
    /// Format: [valueType:1B][timeType:1B][values: N * value_size bytes]
    fn write_new_record(
        &self,
        hhmm: i32,
        value: f64,
        time_type: i8,
    ) -> io::Result<i64> {
        let bucket_count = get_bucket_count(time_type);
        let value_size = 8; // Always use f64 (DOUBLE)
        let record_size = 2 + bucket_count * value_size; // header + values

        let mut record = vec![0u8; record_size];
        record[0] = VALUE_TYPE_DOUBLE;
        record[1] = time_type as u8;

        // Set the value at the correct bucket position
        let bucket_idx = hhmm_to_bucket(hhmm, time_type);
        if bucket_idx < bucket_count {
            let pos = 2 + bucket_idx * value_size;
            let bytes = value.to_be_bytes();
            record[pos..pos + 8].copy_from_slice(&bytes);
        }

        self.data_file.write(&record)
    }

    /// Update an existing counter record's value at the given time bucket.
    fn update_value(
        &self,
        record_offset: i64,
        hhmm: i32,
        value: f64,
        time_type: i8,
    ) -> io::Result<()> {
        self.data_file.flush()?;

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.data_path)?;

        // Read header to get value type
        file.seek(SeekFrom::Start(record_offset as u64))?;
        let mut header = [0u8; 2];
        file.read_exact(&mut header)?;

        let value_size = match header[0] {
            VALUE_TYPE_FLOAT => 4,
            VALUE_TYPE_DOUBLE => 8,
            VALUE_TYPE_DECIMAL => 8,
            _ => 8,
        };

        let bucket_idx = hhmm_to_bucket(hhmm, time_type);
        let value_offset = record_offset + 2 + (bucket_idx * value_size) as i64;

        file.seek(SeekFrom::Start(value_offset as u64))?;
        let bytes = value.to_be_bytes();
        file.write_all(&bytes[..value_size.min(8)])?;

        Ok(())
    }

    /// Read a counter record by key.
    pub fn read(&self, key: &[u8]) -> io::Result<Option<CounterRecord>> {
        let offset_bytes = self.index.get(key)?;
        match offset_bytes {
            Some(ob) => {
                let offset = to_long5_from_bytes(&ob);
                self.read_record(offset)
            }
            None => Ok(None),
        }
    }

    fn read_record(&self, offset: i64) -> io::Result<Option<CounterRecord>> {
        self.data_file.flush()?;

        let header = self.data_file.read(offset, 2)?;
        let value_type = header[0];
        let time_type = header[1] as i8;

        let bucket_count = get_bucket_count(time_type);
        let value_size = match value_type {
            VALUE_TYPE_FLOAT => 4,
            VALUE_TYPE_DOUBLE => 8,
            VALUE_TYPE_DECIMAL => 8,
            _ => 8,
        };

        let data = self.data_file.read(offset + 2, bucket_count * value_size)?;

        let mut values = Vec::with_capacity(bucket_count);
        for i in 0..bucket_count {
            let pos = i * value_size;
            let v = match value_type {
                VALUE_TYPE_FLOAT => {
                    let b: [u8; 4] = data[pos..pos + 4].try_into().unwrap();
                    f32::from_be_bytes(b) as f64
                }
                _ => {
                    let b: [u8; 8] = data[pos..pos + 8].try_into().unwrap();
                    f64::from_be_bytes(b)
                }
            };
            values.push(v);
        }

        Ok(Some(CounterRecord {
            value_type,
            time_type,
            values,
        }))
    }

    pub fn flush(&self) -> io::Result<()> {
        self.data_file.flush()?;
        self.index.flush()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CounterRecord {
    pub value_type: u8,
    pub time_type: i8,
    pub values: Vec<f64>,
}

/// Get the number of buckets for a time type.
fn get_bucket_count(time_type: i8) -> usize {
    match time_type {
        TIME_TYPE_ONE_MIN => 1440,
        TIME_TYPE_FIVE_MIN => 288,
        TIME_TYPE_TEN_MIN => 144,
        TIME_TYPE_HOUR => 24,
        _ => 1, // DAY
    }
}

/// Convert HHMM to bucket index for a given time type.
fn hhmm_to_bucket(hhmm: i32, time_type: i8) -> usize {
    let hours = hhmm / 100;
    let minutes = hhmm % 100;
    let total_minutes = hours * 60 + minutes;

    match time_type {
        TIME_TYPE_ONE_MIN => total_minutes as usize,
        TIME_TYPE_FIVE_MIN => (total_minutes / 5) as usize,
        TIME_TYPE_TEN_MIN => (total_minutes / 10) as usize,
        TIME_TYPE_HOUR => hours as usize,
        _ => 0,
    }
}

fn to_long5_from_bytes(buf: &[u8]) -> i64 {
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

    fn cleanup(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_and_read() {
        let dir = "/tmp/scouter_test_counter_store";
        cleanup(dir);

        let store = CounterStore::open(Path::new(dir), "20260227", 1).unwrap();

        let key = b"test_counter";
        store.write(key, 1030, 42.5, TIME_TYPE_FIVE_MIN).unwrap();

        let record = store.read(key).unwrap().unwrap();
        assert_eq!(record.time_type, TIME_TYPE_FIVE_MIN);

        // Bucket for 10:30 at 5min granularity = (10*60+30)/5 = 126
        let bucket = hhmm_to_bucket(1030, TIME_TYPE_FIVE_MIN);
        assert_eq!(bucket, 126);
        assert!((record.values[bucket] - 42.5).abs() < 0.001);

        cleanup(dir);
    }

    #[test]
    fn test_update_value() {
        let dir = "/tmp/scouter_test_counter_update";
        cleanup(dir);

        let store = CounterStore::open(Path::new(dir), "20260227", 1).unwrap();

        let key = b"cpu_usage";
        store.write(key, 1000, 50.0, TIME_TYPE_HOUR).unwrap();
        store.write(key, 1100, 60.0, TIME_TYPE_HOUR).unwrap();

        let record = store.read(key).unwrap().unwrap();
        assert!((record.values[10] - 50.0).abs() < 0.001); // 10:00 → bucket 10
        assert!((record.values[11] - 60.0).abs() < 0.001); // 11:00 → bucket 11

        cleanup(dir);
    }
}
