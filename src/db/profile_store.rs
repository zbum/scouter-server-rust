use std::fs;
use std::io;
use std::path::Path;

use crate::db::io::index_key_file::IndexKeyFile;
use crate::db::io::real_data_file::RealDataFile;
use crate::protocol::data_output::{to_bytes5, to_bytes_long};

/// Profile storage with data file + txid index.
/// Matches Java XLogProfileWR.
pub struct ProfileStore {
    data_file: RealDataFile,
    txid_index: IndexKeyFile,
}

impl ProfileStore {
    pub fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;

        let data_path = dir.join("profile.data");
        let txid_path = dir.join("profile");

        let data_file = RealDataFile::open(data_path.to_str().unwrap())?;
        let txid_index = IndexKeyFile::open(txid_path.to_str().unwrap(), 1)?;

        Ok(Self {
            data_file,
            txid_index,
        })
    }

    /// Write profile data indexed by txid.
    pub fn write(&self, txid: i64, data: &[u8]) -> io::Result<()> {
        let offset = self.data_file.write(data)?;

        let txid_key = to_bytes_long(txid);
        let offset_bytes = to_bytes5(offset);
        self.txid_index.put(&txid_key, &offset_bytes)?;

        Ok(())
    }

    /// Read profile data by txid.
    pub fn read_by_txid(&self, txid: i64) -> io::Result<Option<Vec<u8>>> {
        let txid_key = to_bytes_long(txid);
        if let Some(offset_bytes) = self.txid_index.get(&txid_key)? {
            let offset = to_long5_from_bytes(&offset_bytes);
            // Profile data is stored as raw bytes without length prefix
            // We need to read until the next entry or end
            // For simplicity, store with length prefix
            let len_bytes = self.data_file.read(offset, 4)?;
            let len = i32::from_be_bytes([
                len_bytes[0],
                len_bytes[1],
                len_bytes[2],
                len_bytes[3],
            ]) as usize;
            let data = self.data_file.read(offset + 4, len)?;
            return Ok(Some(data));
        }
        Ok(None)
    }

    /// Write profile data with a 4-byte length prefix for safe reading.
    pub fn write_with_length(&self, txid: i64, data: &[u8]) -> io::Result<()> {
        let len = data.len() as i32;
        let offset = self.data_file.write_int(len)?;
        self.data_file.write(data)?;

        let txid_key = to_bytes_long(txid);
        let offset_bytes = to_bytes5(offset);
        self.txid_index.put(&txid_key, &offset_bytes)?;

        Ok(())
    }

    pub fn flush(&self) -> io::Result<()> {
        self.data_file.flush()?;
        self.txid_index.flush()?;
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

    fn cleanup(dir: &str) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_write_and_read() {
        let dir = "/tmp/scouter_test_profile_store";
        cleanup(dir);

        let store = ProfileStore::open(Path::new(dir)).unwrap();

        store
            .write_with_length(12345, b"profile_data_here")
            .unwrap();

        let data = store.read_by_txid(12345).unwrap();
        assert_eq!(data, Some(b"profile_data_here".to_vec()));

        let none = store.read_by_txid(99999).unwrap();
        assert_eq!(none, None);

        cleanup(dir);
    }
}
