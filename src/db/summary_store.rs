use std::io;
use std::path::Path;

use crate::db::io::index_key_file::IndexKeyFile;
use crate::db::io::real_data_file::RealDataFile;
use crate::protocol::pack::{SUMMARY_APP, SUMMARY_SQL, SUMMARY_IP, SUMMARY_USER_AGENT, SUMMARY_SERVICE_ERROR};

/// Summary store with 4 index files (app, sql, enduser, other) and a shared data file.
/// Matches Java SummaryWR structure.
pub struct SummaryStore {
    data_file: RealDataFile,
    app_index: IndexKeyFile,
    sql_index: IndexKeyFile,
    enduser_index: IndexKeyFile,
    other_index: IndexKeyFile,
}

impl SummaryStore {
    pub fn open(dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let dir_str = dir.to_str().unwrap_or(".");

        let data_file = RealDataFile::open(&format!("{}/summary.sum", dir_str))?;
        let app_index = IndexKeyFile::open(&format!("{}/summary_app", dir_str), 1)?;
        let sql_index = IndexKeyFile::open(&format!("{}/summary_sql", dir_str), 1)?;
        let enduser_index = IndexKeyFile::open(&format!("{}/summary_enduser", dir_str), 1)?;
        let other_index = IndexKeyFile::open(&format!("{}/summary_other", dir_str), 1)?;

        Ok(Self {
            data_file,
            app_index,
            sql_index,
            enduser_index,
            other_index,
        })
    }

    /// Write a summary record.
    /// key = [hhmm(4B) + id_hash(4B)], data = serialized summary record bytes.
    pub fn write(&self, stype: u8, hhmm: i32, id_hash: i32, data: &[u8]) -> io::Result<()> {
        // Write data to shared data file: length(4B) + data
        let len_bytes = (data.len() as i32).to_be_bytes();
        let offset = self.data_file.write(&len_bytes)?;
        self.data_file.write(data)?;

        // Build key = hhmm(4B) + id_hash(4B)
        let mut key = Vec::with_capacity(8);
        key.extend_from_slice(&hhmm.to_be_bytes());
        key.extend_from_slice(&id_hash.to_be_bytes());

        // Store offset as data_pos
        let data_pos = offset.to_be_bytes();

        let index = self.index_for_stype(stype);
        index.put(&key, &data_pos)?;

        Ok(())
    }

    /// Read all records for a given summary type.
    pub fn read_by_type(&self, stype: u8, mut handler: impl FnMut(&[u8], Vec<u8>)) -> io::Result<()> {
        let index = self.index_for_stype(stype);
        index.read_all(|key, data_pos_bytes| {
            if data_pos_bytes.len() >= 8 {
                let offset = i64::from_be_bytes(data_pos_bytes[..8].try_into().unwrap_or([0; 8]));
                if let Ok(len_bytes) = self.data_file.read(offset, 4) {
                    let len = i32::from_be_bytes(len_bytes[..4].try_into().unwrap_or([0; 4])) as usize;
                    if len > 0 && len < 1_000_000 {
                        if let Ok(data) = self.data_file.read(offset + 4, len) {
                            handler(key, data);
                        }
                    }
                }
            }
        })?;
        Ok(())
    }

    fn index_for_stype(&self, stype: u8) -> &IndexKeyFile {
        match stype {
            SUMMARY_APP | SUMMARY_SERVICE_ERROR => &self.app_index,
            SUMMARY_SQL => &self.sql_index,
            SUMMARY_IP | SUMMARY_USER_AGENT => &self.enduser_index,
            _ => &self.other_index,
        }
    }

    pub fn flush(&self) -> io::Result<()> {
        self.data_file.flush()?;
        self.app_index.flush()?;
        self.sql_index.flush()?;
        self.enduser_index.flush()?;
        self.other_index.flush()?;
        Ok(())
    }
}
