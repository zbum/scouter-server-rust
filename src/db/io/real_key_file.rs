use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;

const MAGIC: [u8; 2] = [0xCA, 0xFE];
const HEADER_SIZE: i64 = 2;

/// A key record read from the key file.
#[derive(Debug, Clone)]
pub struct KeyRecord {
    pub deleted: bool,
    pub prev_pos: i64,
    pub key: Vec<u8>,
    pub data_pos: Vec<u8>,
    /// File offset after this record (start of next record).
    pub next_offset: i64,
}

/// Append-only key file with collision chaining.
/// Matches Java RealKeyFile.
///
/// File format:
///   [0xCA, 0xFE]  (2 bytes magic)
///   [records...]
///
/// Record format:
///   [deleted: 1B bool][prevPos: 5B long5][key: 2B len + data][dataPos: blob]
pub struct RealKeyFile {
    raf: Mutex<File>,
    #[allow(dead_code)]
    path: String,
}

impl RealKeyFile {
    pub fn open(path: &str) -> io::Result<Self> {
        let file_path = format!("{}.kfile", path);
        let p = Path::new(&file_path);
        let is_new = !p.exists() || p.metadata().map(|m| m.len()).unwrap_or(0) == 0;

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&file_path)?;

        if is_new {
            file.write_all(&MAGIC)?;
            file.flush()?;
        }

        Ok(Self {
            raf: Mutex::new(file),
            path: file_path,
        })
    }

    /// Read a full record at the given position.
    pub fn get_record(&self, pos: i64) -> io::Result<KeyRecord> {
        let mut raf = self.raf.lock().unwrap();
        raf.seek(SeekFrom::Start(pos as u64))?;

        // Read entire record into buffer, then parse
        // Record: deleted(1) + prevPos(5) + shortBytes(2+N) + blob(1+N or 3+N or 5+N)
        // We read generously and parse
        let record_start = pos;
        let file_len = raf.seek(SeekFrom::End(0))?;
        raf.seek(SeekFrom::Start(pos as u64))?;

        let remaining = (file_len - pos as u64) as usize;
        let mut buf = vec![0u8; remaining.min(65536)];
        let read_count = raf.read(&mut buf)?;
        buf.truncate(read_count);

        let mut din = DataInputX::from_bytes(buf);
        let deleted = din.read_boolean()?;
        let prev_pos = din.read_long5()?;
        let key = din.read_short_bytes()?;
        let data_pos = din.read_blob()?;

        Ok(KeyRecord {
            deleted,
            prev_pos,
            key,
            data_pos,
            next_offset: record_start + din.offset() as i64,
        })
    }

    /// Check if the record at pos is deleted.
    pub fn is_deleted(&self, pos: i64) -> io::Result<bool> {
        let mut raf = self.raf.lock().unwrap();
        raf.seek(SeekFrom::Start(pos as u64))?;
        let mut buf = [0u8; 1];
        raf.read_exact(&mut buf)?;
        Ok(buf[0] != 0)
    }

    /// Read the prevPos field (5 bytes at offset +1).
    pub fn get_prev_pos(&self, pos: i64) -> io::Result<i64> {
        let mut raf = self.raf.lock().unwrap();
        raf.seek(SeekFrom::Start((pos + 1) as u64))?;
        let mut buf = [0u8; 5];
        raf.read_exact(&mut buf)?;
        Ok(to_long5(&buf))
    }

    /// Read the key field.
    pub fn get_key(&self, pos: i64) -> io::Result<Vec<u8>> {
        let mut raf = self.raf.lock().unwrap();
        raf.seek(SeekFrom::Start((pos + 1 + 5) as u64))?;
        let mut len_buf = [0u8; 2];
        raf.read_exact(&mut len_buf)?;
        let len = i16::from_be_bytes(len_buf) as usize;
        let mut key = vec![0u8; len];
        raf.read_exact(&mut key)?;
        Ok(key)
    }

    /// Read the dataPos field (blob after key).
    pub fn get_data_pos(&self, pos: i64) -> io::Result<Vec<u8>> {
        let mut raf = self.raf.lock().unwrap();
        raf.seek(SeekFrom::Start((pos + 1 + 5) as u64))?;
        // Skip shortBytes(key)
        let mut len_buf = [0u8; 2];
        raf.read_exact(&mut len_buf)?;
        let key_len = i16::from_be_bytes(len_buf) as usize;
        let mut skip = vec![0u8; key_len];
        raf.read_exact(&mut skip)?;
        // Read blob
        read_blob_from_file(&mut *raf)
    }

    /// Append a record at the end. Returns the position of the new record.
    pub fn append(&self, prev_pos: i64, index_key: &[u8], data_pos: &[u8]) -> io::Result<i64> {
        let mut raf = self.raf.lock().unwrap();
        let pos = raf.seek(SeekFrom::End(0))? as i64;

        let mut dout = DataOutputX::new();
        dout.write_boolean(false)?; // not deleted
        dout.write_long5(prev_pos)?;
        dout.write_short_bytes(index_key)?;
        dout.write_blob(data_pos)?;

        raf.write_all(dout.as_bytes())?;
        Ok(pos)
    }

    /// Mark a record as deleted.
    pub fn set_delete(&self, pos: i64, deleted: bool) -> io::Result<()> {
        let mut raf = self.raf.lock().unwrap();
        raf.seek(SeekFrom::Start(pos as u64))?;
        raf.write_all(&[if deleted { 1 } else { 0 }])?;
        Ok(())
    }

    /// Get the first valid record position (after magic header).
    pub fn get_first_pos(&self) -> i64 {
        HEADER_SIZE
    }

    /// Get the file length.
    pub fn get_length(&self) -> io::Result<i64> {
        let mut raf = self.raf.lock().unwrap();
        Ok(raf.seek(SeekFrom::End(0))? as i64)
    }

    pub fn flush(&self) -> io::Result<()> {
        self.raf.lock().unwrap().flush()
    }
}

impl Drop for RealKeyFile {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// Read a long5 (5-byte big-endian with sign extension) from a buffer.
fn to_long5(buf: &[u8]) -> i64 {
    // Sign extension from first byte
    let b0 = buf[0] as i8 as i64;
    (b0 << 32)
        | ((buf[1] as i64) << 24)
        | ((buf[2] as i64) << 16)
        | ((buf[3] as i64) << 8)
        | (buf[4] as i64)
}

/// Read a blob from a file handle (matching DataInputX blob encoding).
fn read_blob_from_file(file: &mut File) -> io::Result<Vec<u8>> {
    let mut len_byte = [0u8; 1];
    file.read_exact(&mut len_byte)?;
    let b = len_byte[0];

    match b {
        0 => Ok(Vec::new()),
        0xFF => {
            // 2-byte length
            let mut buf = [0u8; 2];
            file.read_exact(&mut buf)?;
            let len = u16::from_be_bytes(buf) as usize;
            let mut data = vec![0u8; len];
            file.read_exact(&mut data)?;
            Ok(data)
        }
        0xFE => {
            // 4-byte length
            let mut buf = [0u8; 4];
            file.read_exact(&mut buf)?;
            let len = i32::from_be_bytes(buf) as usize;
            let mut data = vec![0u8; len];
            file.read_exact(&mut data)?;
            Ok(data)
        }
        n => {
            // Inline length (1-253)
            let len = n as usize;
            let mut data = vec![0u8; len];
            file.read_exact(&mut data)?;
            Ok(data)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_append_and_read() {
        let path = "/tmp/scouter_test_rkf";
        let _ = fs::remove_file(format!("{}.kfile", path));

        let rkf = RealKeyFile::open(path).unwrap();

        // Append first record with no prev
        let pos1 = rkf.append(0, b"key1", b"data1").unwrap();
        assert_eq!(pos1, 2); // After 2-byte magic header

        // Append second record chained to first
        let pos2 = rkf.append(pos1, b"key2", b"data2").unwrap();
        assert!(pos2 > pos1);

        // Read back first record
        let rec1 = rkf.get_record(pos1).unwrap();
        assert!(!rec1.deleted);
        assert_eq!(rec1.prev_pos, 0);
        assert_eq!(rec1.key, b"key1");
        assert_eq!(rec1.data_pos, b"data1");

        // Read back second record
        let rec2 = rkf.get_record(pos2).unwrap();
        assert!(!rec2.deleted);
        assert_eq!(rec2.prev_pos, pos1);
        assert_eq!(rec2.key, b"key2");
        assert_eq!(rec2.data_pos, b"data2");

        // Follow chain
        let prev = rkf.get_prev_pos(pos2).unwrap();
        assert_eq!(prev, pos1);

        // Delete first record
        rkf.set_delete(pos1, true).unwrap();
        assert!(rkf.is_deleted(pos1).unwrap());
        assert!(!rkf.is_deleted(pos2).unwrap());

        let _ = fs::remove_file(format!("{}.kfile", path));
    }
}
