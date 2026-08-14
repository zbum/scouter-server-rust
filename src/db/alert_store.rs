use std::fs;
use std::io;
use std::path::Path;

use crate::db::io::index_time_file::IndexTimeFile;
use crate::db::io::real_data_file::RealDataFile;

/// Alert storage with data file + time index.
/// Matches Java AlertWR/AlertRD/AlertIndex.
pub struct AlertStore {
    data_file: RealDataFile,
    time_index: IndexTimeFile,
}

impl AlertStore {
    pub fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;

        let data_path = dir.join("alert.data");
        let time_path = dir.join("alert_tim");

        let data_file = RealDataFile::open(data_path.to_str().unwrap())?;
        let time_index = IndexTimeFile::open(time_path.to_str().unwrap())?;

        Ok(Self {
            data_file,
            time_index,
        })
    }

    /// Write an alert entry.
    pub fn write(&self, time_ms: i64, data: &[u8]) -> io::Result<()> {
        // Write: short(len) + bytes(data)
        let len = data.len() as i16;
        let offset = self.data_file.write_short(len)?;
        self.data_file.write(data)?;

        // Index by time
        self.time_index.put(time_ms, offset)?;

        Ok(())
    }

    /// Read alerts in a time range.
    pub fn read_by_time<F>(&self, from_ms: i64, to_ms: i64, mut handler: F) -> io::Result<()>
    where
        F: FnMut(i64, Vec<u8>),
    {
        self.time_index.read_with_reader(
            from_ms,
            to_ms,
            |time, data| {
                handler(time, data);
            },
            |offset| {
                let len_bytes = self.data_file.read(offset, 2)?;
                let len = i16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
                self.data_file.read(offset + 2, len)
            },
        )
    }

    pub fn flush(&self) -> io::Result<()> {
        self.data_file.flush()?;
        self.time_index.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cleanup(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_and_read() {
        let dir = "/tmp/scouter_test_alert_store";
        cleanup(dir);

        let store = AlertStore::open(Path::new(dir)).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        store.write(now, b"alert1").unwrap();
        store.write(now + 1000, b"alert2").unwrap();

        let mut results = Vec::new();
        store
            .read_by_time(now - 1000, now + 2000, |time, data| {
                results.push((time, data));
            })
            .unwrap();

        assert!(results.len() >= 2);

        cleanup(dir);
    }
}
