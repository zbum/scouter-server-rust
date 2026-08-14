use std::io;

pub const INT3_MIN_VALUE: i32 = 0xff800000_u32 as i32;
pub const INT3_MAX_VALUE: i32 = 0x007fffff;
pub const LONG5_MIN_VALUE: i64 = 0xffffff8000000000_u64 as i64;
pub const LONG5_MAX_VALUE: i64 = 0x0000007fffffffff_u64 as i64;

pub struct DataOutputX {
    buf: Vec<u8>,
}

impl DataOutputX {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self { buf: Vec::with_capacity(cap) }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    pub fn size(&self) -> usize {
        self.buf.len()
    }

    pub fn write_raw(&mut self, b: &[u8]) -> io::Result<()> {
        self.buf.extend_from_slice(b);
        Ok(())
    }

    pub fn write_boolean(&mut self, v: bool) -> io::Result<()> {
        self.buf.push(if v { 1 } else { 0 });
        Ok(())
    }

    pub fn write_byte(&mut self, v: i32) -> io::Result<()> {
        self.buf.push(v as u8);
        Ok(())
    }

    pub fn write_short(&mut self, v: i32) -> io::Result<()> {
        let s = v as i16;
        self.buf.extend_from_slice(&s.to_be_bytes());
        Ok(())
    }

    pub fn write_int(&mut self, v: i32) -> io::Result<()> {
        self.buf.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    pub fn write_int3(&mut self, v: i32) -> io::Result<()> {
        self.buf.push(((v >> 16) & 0xFF) as u8);
        self.buf.push(((v >> 8) & 0xFF) as u8);
        self.buf.push((v & 0xFF) as u8);
        Ok(())
    }

    pub fn write_long(&mut self, v: i64) -> io::Result<()> {
        self.buf.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    pub fn write_long5(&mut self, v: i64) -> io::Result<()> {
        self.buf.push(((v >> 32) & 0xFF) as u8);
        self.buf.push(((v >> 24) & 0xFF) as u8);
        self.buf.push(((v >> 16) & 0xFF) as u8);
        self.buf.push(((v >> 8) & 0xFF) as u8);
        self.buf.push((v & 0xFF) as u8);
        Ok(())
    }

    pub fn write_float(&mut self, v: f32) -> io::Result<()> {
        self.buf.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    pub fn write_double(&mut self, v: f64) -> io::Result<()> {
        self.buf.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    /// Variable-length decimal encoding matching Java DataOutputX.writeDecimal
    pub fn write_decimal(&mut self, v: i64) -> io::Result<()> {
        if v == 0 {
            self.write_byte(0)?;
        } else if (i8::MIN as i64) <= v && v <= (i8::MAX as i64) {
            self.write_byte(1)?;
            self.write_byte(v as i32)?;
        } else if (i16::MIN as i64) <= v && v <= (i16::MAX as i64) {
            self.write_byte(2)?;
            self.write_short(v as i32)?;
        } else if (INT3_MIN_VALUE as i64) <= v && v <= (INT3_MAX_VALUE as i64) {
            self.write_byte(3)?;
            self.write_int3(v as i32)?;
        } else if (i32::MIN as i64) <= v && v <= (i32::MAX as i64) {
            self.write_byte(4)?;
            self.write_int(v as i32)?;
        } else if LONG5_MIN_VALUE <= v && v <= LONG5_MAX_VALUE {
            self.write_byte(5)?;
            self.write_long5(v)?;
        } else {
            self.write_byte(8)?;
            self.write_long(v)?;
        }
        Ok(())
    }

    /// Blob encoding: 0x00=empty, 1-253=inline, 0xFF=2-byte len, 0xFE=4-byte len
    pub fn write_blob(&mut self, value: &[u8]) -> io::Result<()> {
        if value.is_empty() {
            self.write_byte(0)?;
        } else {
            let len = value.len();
            if len <= 253 {
                self.write_byte(len as i32)?;
                self.write_raw(value)?;
            } else if len <= 65535 {
                self.write_byte(255)?;
                self.write_short(len as i32)?;
                self.write_raw(value)?;
            } else {
                self.write_byte(254)?;
                self.write_int(len as i32)?;
                self.write_raw(value)?;
            }
        }
        Ok(())
    }

    pub fn write_text(&mut self, s: &str) -> io::Result<()> {
        self.write_blob(s.as_bytes())
    }

    pub fn write_text_opt(&mut self, s: &Option<String>) -> io::Result<()> {
        match s {
            Some(ref text) => self.write_text(text),
            None => self.write_byte(0),
        }
    }

    pub fn write_int_bytes(&mut self, b: &[u8]) -> io::Result<()> {
        self.write_int(b.len() as i32)?;
        self.write_raw(b)
    }

    pub fn write_short_bytes(&mut self, b: &[u8]) -> io::Result<()> {
        self.write_short(b.len() as i32)?;
        self.write_raw(b)
    }

    pub fn write_value(&mut self, value: &crate::protocol::value::Value) -> io::Result<()> {
        self.write_byte(value.type_code() as i32)?;
        value.write(self)
    }

    pub fn write_pack(&mut self, pack: &crate::protocol::pack::Pack) -> io::Result<()> {
        self.write_byte(pack.type_code() as i32)?;
        pack.write(self)
    }
}

// Static conversion helpers (matching Java DataOutputX)

pub fn to_bytes_short(v: i16) -> [u8; 2] {
    v.to_be_bytes()
}

pub fn to_bytes_int(v: i32) -> [u8; 4] {
    v.to_be_bytes()
}

pub fn to_bytes_long(v: i64) -> [u8; 8] {
    v.to_be_bytes()
}

pub fn to_bytes3(v: i32) -> [u8; 3] {
    [
        ((v >> 16) & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        (v & 0xFF) as u8,
    ]
}

pub fn to_bytes5(v: i64) -> [u8; 5] {
    [
        ((v >> 32) & 0xFF) as u8,
        ((v >> 24) & 0xFF) as u8,
        ((v >> 16) & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        (v & 0xFF) as u8,
    ]
}
