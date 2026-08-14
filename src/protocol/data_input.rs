use std::io::{self, Cursor, Read};

use crate::error::{ScouterError, Result};
use crate::protocol::value::Value;
use crate::protocol::pack::Pack;

pub struct DataInputX<R: Read> {
    inner: R,
    offset: usize,
}

impl DataInputX<Cursor<Vec<u8>>> {
    pub fn from_bytes(buf: Vec<u8>) -> Self {
        Self {
            inner: Cursor::new(buf),
            offset: 0,
        }
    }

    pub fn available(&self) -> usize {
        let pos = self.inner.position() as usize;
        let len = self.inner.get_ref().len();
        if pos >= len { 0 } else { len - pos }
    }
}

impl<R: Read> DataInputX<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            offset: 0,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    fn read_n(&mut self, len: usize) -> io::Result<Vec<u8>> {
        self.offset += len;
        let mut buf = vec![0u8; len];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn read_fully(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.offset += buf.len();
        self.inner.read_exact(buf)
    }

    pub fn read_boolean(&mut self) -> io::Result<bool> {
        self.offset += 1;
        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;
        Ok(buf[0] != 0)
    }

    pub fn read_byte(&mut self) -> io::Result<i8> {
        self.offset += 1;
        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;
        Ok(buf[0] as i8)
    }

    pub fn read_unsigned_byte(&mut self) -> io::Result<u8> {
        self.offset += 1;
        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    pub fn read_short(&mut self) -> io::Result<i16> {
        self.offset += 2;
        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;
        Ok(i16::from_be_bytes(buf))
    }

    pub fn read_unsigned_short(&mut self) -> io::Result<u16> {
        self.offset += 2;
        let mut buf = [0u8; 2];
        self.inner.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    pub fn read_int(&mut self) -> io::Result<i32> {
        self.offset += 4;
        let mut buf = [0u8; 4];
        self.inner.read_exact(&mut buf)?;
        Ok(i32::from_be_bytes(buf))
    }

    /// Read a 3-byte signed integer with sign extension.
    /// Java: ((ch1 << 24) + (ch2 << 16) + (ch3 << 8)) >> 8
    pub fn read_int3(&mut self) -> io::Result<i32> {
        let buf = self.read_n(3)?;
        Ok(to_int3(&buf, 0))
    }

    pub fn read_long(&mut self) -> io::Result<i64> {
        self.offset += 8;
        let mut buf = [0u8; 8];
        self.inner.read_exact(&mut buf)?;
        Ok(i64::from_be_bytes(buf))
    }

    /// Read a 5-byte signed long with sign extension.
    pub fn read_long5(&mut self) -> io::Result<i64> {
        let buf = self.read_n(5)?;
        Ok(to_long5(&buf, 0))
    }

    pub fn read_float(&mut self) -> io::Result<f32> {
        self.offset += 4;
        let mut buf = [0u8; 4];
        self.inner.read_exact(&mut buf)?;
        Ok(f32::from_be_bytes(buf))
    }

    pub fn read_double(&mut self) -> io::Result<f64> {
        self.offset += 8;
        let mut buf = [0u8; 8];
        self.inner.read_exact(&mut buf)?;
        Ok(f64::from_be_bytes(buf))
    }

    /// Variable-length decimal encoding (Java compat).
    /// len byte: 0=zero, 1=i8, 2=i16, 3=int3, 4=i32, 5=long5, 8=i64
    pub fn read_decimal(&mut self) -> io::Result<i64> {
        let len = self.read_byte()?;
        match len {
            0 => Ok(0),
            1 => Ok(self.read_byte()? as i64),
            2 => Ok(self.read_short()? as i64),
            3 => Ok(self.read_int3()? as i64),
            4 => Ok(self.read_int()? as i64),
            5 => Ok(self.read_long5()?),
            _ => self.read_long(),
        }
    }

    /// Blob encoding: 0x00=empty, 1-253=inline len, 0xFF=2-byte len, 0xFE=4-byte len
    pub fn read_blob(&mut self) -> io::Result<Vec<u8>> {
        let base_len = self.read_unsigned_byte()?;
        match base_len {
            0 => Ok(vec![]),
            255 => {
                let len = self.read_unsigned_short()? as usize;
                self.read_n(len)
            }
            254 => {
                let len = self.read_int()? as usize;
                self.read_n(len)
            }
            n => self.read_n(n as usize),
        }
    }

    pub fn read_text(&mut self) -> io::Result<String> {
        let buf = self.read_blob()?;
        String::from_utf8(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn read_int_bytes(&mut self) -> io::Result<Vec<u8>> {
        let len = self.read_int()? as usize;
        self.read_n(len)
    }

    pub fn read_short_bytes(&mut self) -> io::Result<Vec<u8>> {
        let len = self.read_unsigned_short()? as usize;
        self.read_n(len)
    }

    pub fn read_value(&mut self) -> Result<Value> {
        let type_code = self.read_unsigned_byte()
            .map_err(ScouterError::Io)?;
        Value::read(type_code, self)
    }

    pub fn read_pack(&mut self) -> Result<Pack> {
        let type_code = self.read_unsigned_byte()
            .map_err(ScouterError::Io)?;
        Pack::read(type_code, self)
    }

    pub fn skip_bytes(&mut self, n: usize) -> io::Result<()> {
        self.offset += n;
        let mut buf = vec![0u8; n];
        self.inner.read_exact(&mut buf)
    }
}

// Static conversion helpers (matching Java DataInputX)

pub fn to_int3(buf: &[u8], pos: usize) -> i32 {
    let ch1 = buf[pos] as u32 & 0xff;
    let ch2 = buf[pos + 1] as u32 & 0xff;
    let ch3 = buf[pos + 2] as u32 & 0xff;
    (((ch1 << 24) + (ch2 << 16) + (ch3 << 8)) as i32) >> 8
}

pub fn to_int(buf: &[u8], pos: usize) -> i32 {
    let ch1 = buf[pos] as u32 & 0xff;
    let ch2 = buf[pos + 1] as u32 & 0xff;
    let ch3 = buf[pos + 2] as u32 & 0xff;
    let ch4 = buf[pos + 3] as u32 & 0xff;
    ((ch1 << 24) + (ch2 << 16) + (ch3 << 8) + ch4) as i32
}

pub fn to_long(buf: &[u8], pos: usize) -> i64 {
    // buf[pos] must sign-extend (Java byte is signed)
    ((buf[pos] as i8 as i64) << 56)
        | (((buf[pos + 1] as u64 & 0xff) as i64) << 48)
        | (((buf[pos + 2] as u64 & 0xff) as i64) << 40)
        | (((buf[pos + 3] as u64 & 0xff) as i64) << 32)
        | (((buf[pos + 4] as u64 & 0xff) as i64) << 24)
        | (((buf[pos + 5] as u64 & 0xff) as i64) << 16)
        | (((buf[pos + 6] as u64 & 0xff) as i64) << 8)
        | ((buf[pos + 7] as u64 & 0xff) as i64)
}

pub fn to_long5(buf: &[u8], pos: usize) -> i64 {
    // buf[pos] must sign-extend (Java byte is signed)
    ((buf[pos] as i8 as i64) << 32)
        | (((buf[pos + 1] as u64 & 0xff) as i64) << 24)
        | (((buf[pos + 2] as u64 & 0xff) as i64) << 16)
        | (((buf[pos + 3] as u64 & 0xff) as i64) << 8)
        | ((buf[pos + 4] as u64 & 0xff) as i64)
}

pub fn to_short(buf: &[u8], pos: usize) -> i16 {
    let ch1 = buf[pos] as u16 & 0xff;
    let ch2 = buf[pos + 1] as u16 & 0xff;
    ((ch1 << 8) + ch2) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int3_sign_extension() {
        // Positive: 0x007FFF = 32767
        assert_eq!(to_int3(&[0x00, 0x7F, 0xFF], 0), 0x007FFF);
        // Negative: 0xFF8000 should sign-extend to -32768
        assert_eq!(to_int3(&[0xFF, 0x80, 0x00], 0), -32768);
        // Zero
        assert_eq!(to_int3(&[0x00, 0x00, 0x00], 0), 0);
    }

    #[test]
    fn test_long5_sign_extension() {
        // Positive
        assert_eq!(to_long5(&[0x00, 0x00, 0x00, 0x00, 0x01], 0), 1);
        // Negative: top byte 0xFF sign-extends
        assert_eq!(to_long5(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF], 0), -1);
    }

    #[test]
    fn test_decimal_round_trip() {
        use crate::protocol::data_output::DataOutputX;

        let values: Vec<i64> = vec![
            0, 1, -1, 127, -128,
            128, -129, 32767, -32768,
            32768, -32769,
            i32::MAX as i64, i32::MIN as i64,
            i64::MAX, i64::MIN,
        ];

        for &v in &values {
            let mut out = DataOutputX::new();
            out.write_decimal(v).unwrap();
            let bytes = out.to_bytes();
            let mut inp = DataInputX::from_bytes(bytes);
            let read_v = inp.read_decimal().unwrap();
            assert_eq!(v, read_v, "Failed for value {}", v);
        }
    }

    #[test]
    fn test_blob_round_trip() {
        use crate::protocol::data_output::DataOutputX;

        // Empty
        let mut out = DataOutputX::new();
        out.write_blob(&[]).unwrap();
        let mut inp = DataInputX::from_bytes(out.to_bytes());
        assert_eq!(inp.read_blob().unwrap(), vec![0u8; 0]);

        // Small (< 254)
        let data: Vec<u8> = (0..100).collect();
        let mut out = DataOutputX::new();
        out.write_blob(&data).unwrap();
        let mut inp = DataInputX::from_bytes(out.to_bytes());
        assert_eq!(inp.read_blob().unwrap(), data);

        // Medium (254..65535)
        let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
        let mut out = DataOutputX::new();
        out.write_blob(&data).unwrap();
        let mut inp = DataInputX::from_bytes(out.to_bytes());
        assert_eq!(inp.read_blob().unwrap(), data);
    }

    #[test]
    fn test_text_round_trip() {
        use crate::protocol::data_output::DataOutputX;

        let text = "Hello, Scouter! 한글 테스트";
        let mut out = DataOutputX::new();
        out.write_text(text).unwrap();
        let mut inp = DataInputX::from_bytes(out.to_bytes());
        assert_eq!(inp.read_text().unwrap(), text);
    }
}
