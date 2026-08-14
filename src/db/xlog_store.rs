use std::fs;
use std::io;
use std::path::Path;

use crate::db::io::index_key_file::IndexKeyFile;
use crate::db::io::index_time_file::IndexTimeFile;
use crate::db::io::real_data_file::RealDataFile;
use crate::protocol::data_output::{to_bytes5, to_bytes_long};

/// XLog storage with data file + 3 indices (time, txid, gxid).
/// Matches Java XLogWR/XLogRD/XLogIndex/XLogDataWriter.
pub struct XLogStore {
    data_file: RealDataFile,
    time_index: IndexTimeFile,
    txid_index: IndexKeyFile,
    gxid_index: IndexKeyFile,
}

impl XLogStore {
    /// Open or create XLog storage in the given directory.
    /// Creates: xlog.service, xlog_tim.{hfile,kfile}, xlog_tid.{hfile,kfile}, xlog_gid.{hfile,kfile}
    pub fn open(dir: &Path, xlog_index_mb: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;

        let data_path = dir.join("xlog.service");
        let time_path = dir.join("xlog_tim");
        let txid_path = dir.join("xlog_tid");
        let gxid_path = dir.join("xlog_gid");

        let data_file = RealDataFile::open(data_path.to_str().unwrap())?;
        let time_index = IndexTimeFile::open(time_path.to_str().unwrap())?;
        let txid_index = IndexKeyFile::open(txid_path.to_str().unwrap(), xlog_index_mb)?;
        let gxid_index = IndexKeyFile::open(gxid_path.to_str().unwrap(), xlog_index_mb)?;

        Ok(Self {
            data_file,
            time_index,
            txid_index,
            gxid_index,
        })
    }

    /// Write an XLog entry.
    /// Data format in file: [short: data_len][bytes: data]
    pub fn write(&self, time_ms: i64, txid: i64, gxid: i64, data: &[u8]) -> io::Result<()> {
        // Write data: short(len) + bytes(data)
        let len = data.len() as i16;
        let offset = self.data_file.write_short(len)?;
        self.data_file.write(data)?;

        // Index by time
        self.time_index.put(time_ms, offset)?;

        // Index by txid
        let txid_key = to_bytes_long(txid);
        let offset_bytes = to_bytes5(offset);
        self.txid_index.put(&txid_key, &offset_bytes)?;

        // Index by gxid (skip if zero)
        if gxid != 0 {
            let gxid_key = to_bytes_long(gxid);
            self.gxid_index.put(&gxid_key, &offset_bytes)?;
        }

        Ok(())
    }

    /// Read XLog data by txid. Returns the raw data bytes.
    pub fn read_by_txid(&self, txid: i64) -> io::Result<Option<Vec<u8>>> {
        let txid_key = to_bytes_long(txid);
        if let Some(offset_bytes) = self.txid_index.get(&txid_key)? {
            let offset = to_long5_from_bytes(&offset_bytes);
            return self.read_data_at(offset);
        }
        Ok(None)
    }

    /// Read all XLog data entries for a gxid.
    pub fn read_by_gxid(&self, gxid: i64) -> io::Result<Vec<Vec<u8>>> {
        let gxid_key = to_bytes_long(gxid);
        let offsets = self.gxid_index.get_all(&gxid_key)?;
        let mut results = Vec::new();
        for offset_bytes in offsets {
            let offset = to_long5_from_bytes(&offset_bytes);
            if let Some(data) = self.read_data_at(offset)? {
                results.push(data);
            }
        }
        Ok(results)
    }

    /// Read XLog entries in a time range.
    /// handler(time_ms, data_bytes)
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
                self.read_data_at(offset)?
                    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "data not found"))
            },
        )
    }

    /// Read the data at a given file offset.
    /// Format: [short: len][bytes: data]
    fn read_data_at(&self, offset: i64) -> io::Result<Option<Vec<u8>>> {
        let len_bytes = self.data_file.read(offset, 2)?;
        let len = i16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
        if len == 0 {
            return Ok(Some(Vec::new()));
        }
        let data = self.data_file.read(offset + 2, len)?;
        Ok(Some(data))
    }

    pub fn flush(&self) -> io::Result<()> {
        self.data_file.flush()?;
        self.time_index.flush()?;
        self.txid_index.flush()?;
        self.gxid_index.flush()?;
        Ok(())
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cleanup(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_and_read_by_txid() {
        let dir = "/tmp/scouter_test_xlog_store";
        cleanup(dir);

        let store = XLogStore::open(Path::new(dir), 1).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        store.write(now, 12345, 0, b"xlog_data_1").unwrap();
        store.write(now + 100, 12346, 67890, b"xlog_data_2").unwrap();

        // Read by txid
        let data1 = store.read_by_txid(12345).unwrap();
        assert_eq!(data1, Some(b"xlog_data_1".to_vec()));

        let data2 = store.read_by_txid(12346).unwrap();
        assert_eq!(data2, Some(b"xlog_data_2".to_vec()));

        let data3 = store.read_by_txid(99999).unwrap();
        assert_eq!(data3, None);

        cleanup(dir);
    }

    #[test]
    fn test_read_by_gxid() {
        let dir = "/tmp/scouter_test_xlog_gxid";
        cleanup(dir);

        let store = XLogStore::open(Path::new(dir), 1).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Two xlogs with same gxid
        store.write(now, 100, 999, b"data_a").unwrap();
        store.write(now + 100, 101, 999, b"data_b").unwrap();

        let results = store.read_by_gxid(999).unwrap();
        assert_eq!(results.len(), 2);

        cleanup(dir);
    }
}
