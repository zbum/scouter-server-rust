use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

/// Append-only data file matching Java RealDataFile.
/// Writes are buffered (8KB). Reads use a separate file handle with seeking.
pub struct RealDataFile {
    writer: Mutex<BufWriter<File>>,
    offset: Mutex<i64>,
    path: String,
}

impl RealDataFile {
    pub fn open(path: &str) -> io::Result<Self> {
        let p = Path::new(path);
        let current_len = if p.exists() { p.metadata()?.len() as i64 } else { 0 };

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            writer: Mutex::new(BufWriter::with_capacity(8192, file)),
            offset: Mutex::new(current_len),
            path: path.to_string(),
        })
    }

    /// Write raw bytes, returns the offset before writing.
    pub fn write(&self, data: &[u8]) -> io::Result<i64> {
        let mut writer = self.writer.lock().unwrap();
        let mut offset = self.offset.lock().unwrap();
        let idx = *offset;
        writer.write_all(data)?;
        *offset += data.len() as i64;
        Ok(idx)
    }

    /// Write a big-endian i16, returns the offset before writing.
    pub fn write_short(&self, v: i16) -> io::Result<i64> {
        self.write(&v.to_be_bytes())
    }

    /// Write a big-endian i32, returns the offset before writing.
    pub fn write_int(&self, v: i32) -> io::Result<i64> {
        self.write(&v.to_be_bytes())
    }

    /// Read bytes at a given offset. Opens a separate file handle for reading.
    pub fn read(&self, offset: i64, len: usize) -> io::Result<Vec<u8>> {
        self.flush()?;
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut buf = vec![0u8; len];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Current write offset (file size after pending writes).
    pub fn get_offset(&self) -> i64 {
        *self.offset.lock().unwrap()
    }

    pub fn flush(&self) -> io::Result<()> {
        self.writer.lock().unwrap().flush()
    }
}

impl Drop for RealDataFile {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_write_and_read() {
        let path = "/tmp/scouter_test_real_data_file.data";
        let _ = fs::remove_file(path);

        let df = RealDataFile::open(path).unwrap();
        let off1 = df.write(b"hello").unwrap();
        assert_eq!(off1, 0);

        let off2 = df.write(b"world").unwrap();
        assert_eq!(off2, 5);

        let data = df.read(0, 5).unwrap();
        assert_eq!(&data, b"hello");

        let data2 = df.read(5, 5).unwrap();
        assert_eq!(&data2, b"world");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_write_short_int() {
        let path = "/tmp/scouter_test_real_data_file_si.data";
        let _ = fs::remove_file(path);

        let df = RealDataFile::open(path).unwrap();
        let off1 = df.write_short(1234).unwrap();
        assert_eq!(off1, 0);
        assert_eq!(df.get_offset(), 2);

        let off2 = df.write_int(56789).unwrap();
        assert_eq!(off2, 2);
        assert_eq!(df.get_offset(), 6);

        let _ = fs::remove_file(path);
    }
}
