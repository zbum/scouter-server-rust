use std::collections::HashMap;
use std::io;

use crate::error::{ScouterError, Result};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;

// Value type codes (matching Java ValueEnum)
pub const VALUE_NULL: u8 = 0;
pub const VALUE_BOOLEAN: u8 = 10;
pub const VALUE_DECIMAL: u8 = 20;
pub const VALUE_FLOAT: u8 = 30;
pub const VALUE_DOUBLE: u8 = 40;
pub const VALUE_DOUBLE_SUMMARY: u8 = 45;
pub const VALUE_LONG_SUMMARY: u8 = 46;
pub const VALUE_TEXT: u8 = 50;
pub const VALUE_TEXT_HASH: u8 = 51;
pub const VALUE_BLOB: u8 = 60;
pub const VALUE_IP4ADDR: u8 = 61;
pub const VALUE_LIST: u8 = 70;
pub const VALUE_ARRAY_INT: u8 = 71;
pub const VALUE_ARRAY_FLOAT: u8 = 72;
pub const VALUE_ARRAY_TEXT: u8 = 73;
pub const VALUE_ARRAY_LONG: u8 = 74;
pub const VALUE_MAP: u8 = 80;
const MAX_COLLECTION_LENGTH: usize = u16::MAX as usize;

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Boolean(bool),
    Decimal(i64),
    Float(f32),
    Double(f64),
    DoubleSummary { count: i32, sum: f64, min: f64, max: f64 },
    LongSummary { count: i32, sum: i64, min: i64, max: i64 },
    Text(String),
    TextHash(i32),
    Blob(Vec<u8>),
    Ip4(Vec<u8>),
    List(Vec<Value>),
    IntArray(Vec<i32>),
    FloatArray(Vec<f32>),
    TextArray(Vec<String>),
    LongArray(Vec<i64>),
    Map(MapValue),
}

/// MapValue wraps a HashMap<String, Value> matching Java's MapValue
#[derive(Debug, Clone, Default)]
pub struct MapValue {
    pub table: HashMap<String, Value>,
}

impl MapValue {
    pub fn new() -> Self {
        Self { table: HashMap::new() }
    }

    pub fn put(&mut self, key: impl Into<String>, value: Value) {
        self.table.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.table.get(key)
    }

    pub fn size(&self) -> usize {
        self.table.len()
    }

    pub fn get_int(&self, key: &str) -> i32 {
        match self.table.get(key) {
            Some(Value::Decimal(v)) => *v as i32,
            Some(Value::Float(v)) => *v as i32,
            Some(Value::Double(v)) => *v as i32,
            _ => 0,
        }
    }

    pub fn get_long(&self, key: &str) -> i64 {
        match self.table.get(key) {
            Some(Value::Decimal(v)) => *v,
            Some(Value::Float(v)) => *v as i64,
            Some(Value::Double(v)) => *v as i64,
            _ => 0,
        }
    }

    pub fn get_text(&self, key: &str) -> Option<&str> {
        match self.table.get(key) {
            Some(Value::Text(ref s)) => Some(s),
            _ => None,
        }
    }

    pub fn get_float(&self, key: &str) -> f32 {
        match self.table.get(key) {
            Some(Value::Float(v)) => *v,
            Some(Value::Decimal(v)) => *v as f32,
            Some(Value::Double(v)) => *v as f32,
            _ => 0.0,
        }
    }
}

impl Value {
    pub fn type_code(&self) -> u8 {
        match self {
            Value::Null => VALUE_NULL,
            Value::Boolean(_) => VALUE_BOOLEAN,
            Value::Decimal(_) => VALUE_DECIMAL,
            Value::Float(_) => VALUE_FLOAT,
            Value::Double(_) => VALUE_DOUBLE,
            Value::DoubleSummary { .. } => VALUE_DOUBLE_SUMMARY,
            Value::LongSummary { .. } => VALUE_LONG_SUMMARY,
            Value::Text(_) => VALUE_TEXT,
            Value::TextHash(_) => VALUE_TEXT_HASH,
            Value::Blob(_) => VALUE_BLOB,
            Value::Ip4(_) => VALUE_IP4ADDR,
            Value::List(_) => VALUE_LIST,
            Value::IntArray(_) => VALUE_ARRAY_INT,
            Value::FloatArray(_) => VALUE_ARRAY_FLOAT,
            Value::TextArray(_) => VALUE_ARRAY_TEXT,
            Value::LongArray(_) => VALUE_ARRAY_LONG,
            Value::Map(_) => VALUE_MAP,
        }
    }

    pub fn read<R: std::io::Read>(type_code: u8, din: &mut DataInputX<R>) -> Result<Value> {
        match type_code {
            VALUE_NULL => Ok(Value::Null),
            VALUE_BOOLEAN => {
                let v = din.read_boolean().map_err(ScouterError::Io)?;
                Ok(Value::Boolean(v))
            }
            VALUE_DECIMAL => {
                let v = din.read_decimal().map_err(ScouterError::Io)?;
                Ok(Value::Decimal(v))
            }
            VALUE_FLOAT => {
                let v = din.read_float().map_err(ScouterError::Io)?;
                Ok(Value::Float(v))
            }
            VALUE_DOUBLE => {
                let v = din.read_double().map_err(ScouterError::Io)?;
                Ok(Value::Double(v))
            }
            VALUE_DOUBLE_SUMMARY => {
                let count = din.read_int().map_err(ScouterError::Io)?;
                let sum = din.read_double().map_err(ScouterError::Io)?;
                let min = din.read_double().map_err(ScouterError::Io)?;
                let max = din.read_double().map_err(ScouterError::Io)?;
                Ok(Value::DoubleSummary { count, sum, min, max })
            }
            VALUE_LONG_SUMMARY => {
                let count = din.read_int().map_err(ScouterError::Io)?;
                let sum = din.read_long().map_err(ScouterError::Io)?;
                let min = din.read_long().map_err(ScouterError::Io)?;
                let max = din.read_long().map_err(ScouterError::Io)?;
                Ok(Value::LongSummary { count, sum, min, max })
            }
            VALUE_TEXT => {
                let v = din.read_text().map_err(ScouterError::Io)?;
                Ok(Value::Text(v))
            }
            VALUE_TEXT_HASH => {
                let v = din.read_int().map_err(ScouterError::Io)?;
                Ok(Value::TextHash(v))
            }
            VALUE_BLOB => {
                let v = din.read_blob().map_err(ScouterError::Io)?;
                Ok(Value::Blob(v))
            }
            VALUE_IP4ADDR => {
                let mut v = vec![0; 4];
                din.read_fully(&mut v).map_err(ScouterError::Io)?;
                Ok(Value::Ip4(v))
            }
            VALUE_LIST => {
                let count = read_collection_length(din)?;
                let mut list = Vec::with_capacity(count);
                for _ in 0..count {
                    list.push(din.read_value()?);
                }
                Ok(Value::List(list))
            }
            VALUE_ARRAY_INT => {
                let count = read_array_length(din)?;
                let mut arr = Vec::with_capacity(count);
                for _ in 0..count {
                    arr.push(din.read_int().map_err(ScouterError::Io)?);
                }
                Ok(Value::IntArray(arr))
            }
            VALUE_ARRAY_FLOAT => {
                let count = read_array_length(din)?;
                let mut arr = Vec::with_capacity(count);
                for _ in 0..count {
                    arr.push(din.read_float().map_err(ScouterError::Io)?);
                }
                Ok(Value::FloatArray(arr))
            }
            VALUE_ARRAY_TEXT => {
                let count = read_array_length(din)?;
                let mut arr = Vec::with_capacity(count);
                for _ in 0..count {
                    arr.push(din.read_text().map_err(ScouterError::Io)?);
                }
                Ok(Value::TextArray(arr))
            }
            VALUE_ARRAY_LONG => {
                let count = read_array_length(din)?;
                let mut arr = Vec::with_capacity(count);
                for _ in 0..count {
                    arr.push(din.read_long().map_err(ScouterError::Io)?);
                }
                Ok(Value::LongArray(arr))
            }
            VALUE_MAP => {
                let count = read_collection_length(din)?;
                let mut map = MapValue::new();
                for _ in 0..count {
                    let key = din.read_text().map_err(ScouterError::Io)?;
                    let value = din.read_value()?;
                    map.put(key, value);
                }
                Ok(Value::Map(map))
            }
            _ => Err(ScouterError::UnknownValueType(type_code)),
        }
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        match self {
            Value::Null => { /* no data */ }
            Value::Boolean(v) => { dout.write_boolean(*v)?; }
            Value::Decimal(v) => { dout.write_decimal(*v)?; }
            Value::Float(v) => { dout.write_float(*v)?; }
            Value::Double(v) => { dout.write_double(*v)?; }
            Value::DoubleSummary { count, sum, min, max } => {
                dout.write_int(*count)?;
                dout.write_double(*sum)?;
                dout.write_double(*min)?;
                dout.write_double(*max)?;
            }
            Value::LongSummary { count, sum, min, max } => {
                dout.write_int(*count)?;
                dout.write_long(*sum)?;
                dout.write_long(*min)?;
                dout.write_long(*max)?;
            }
            Value::Text(v) => { dout.write_text(v)?; }
            Value::TextHash(v) => { dout.write_int(*v)?; }
            Value::Blob(v) => { dout.write_blob(v)?; }
            Value::Ip4(v) => {
                if v.len() != 4 {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput,
                        format!("IPv4 value must contain exactly 4 bytes, got {}", v.len())));
                }
                dout.write_raw(v)?;
            }
            Value::List(list) => {
                validate_collection_length(list.len())?;
                dout.write_decimal(list.len() as i64)?;
                for v in list {
                    dout.write_value(v)?;
                }
            }
            Value::IntArray(arr) => {
                write_array_length(dout, arr.len())?;
                for v in arr {
                    dout.write_int(*v)?;
                }
            }
            Value::FloatArray(arr) => {
                write_array_length(dout, arr.len())?;
                for v in arr {
                    dout.write_float(*v)?;
                }
            }
            Value::TextArray(arr) => {
                write_array_length(dout, arr.len())?;
                for v in arr {
                    dout.write_text(v)?;
                }
            }
            Value::LongArray(arr) => {
                write_array_length(dout, arr.len())?;
                for v in arr {
                    dout.write_long(*v)?;
                }
            }
            Value::Map(map) => {
                validate_collection_length(map.table.len())?;
                dout.write_decimal(map.table.len() as i64)?;
                for (key, value) in &map.table {
                    dout.write_text(key)?;
                    dout.write_value(value)?;
                }
            }
        }
        Ok(())
    }

    pub fn as_map_value(&self) -> Option<&MapValue> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn into_map_value(self) -> Option<MapValue> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }
}

fn read_array_length<R: std::io::Read>(din: &mut DataInputX<R>) -> Result<usize> {
    let length = din.read_short().map_err(ScouterError::Io)?;
    usize::try_from(length).map_err(|_| ScouterError::Io(io::Error::new(
        io::ErrorKind::InvalidData, format!("negative array length {length}"))))
}

fn read_collection_length<R: std::io::Read>(din: &mut DataInputX<R>) -> Result<usize> {
    let signed = din.read_decimal().map_err(ScouterError::Io)?;
    let length = usize::try_from(signed).map_err(|_| ScouterError::Io(io::Error::new(
        io::ErrorKind::InvalidData, format!("negative collection length {signed}"))))?;
    if length > MAX_COLLECTION_LENGTH {
        return Err(ScouterError::Io(io::Error::new(io::ErrorKind::InvalidData,
            format!("collection length {length} exceeds {MAX_COLLECTION_LENGTH}"))));
    }
    Ok(length)
}

fn write_array_length(dout: &mut DataOutputX, length: usize) -> io::Result<()> {
    let length = i16::try_from(length).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput,
        format!("array length exceeds Java protocol maximum: {length}")))?;
    dout.write_short(length as i32)
}

fn validate_collection_length(length: usize) -> io::Result<()> {
    if length > MAX_COLLECTION_LENGTH {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            format!("collection length {length} exceeds {MAX_COLLECTION_LENGTH}")));
    }
    Ok(())
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Boolean(v) => write!(f, "{}", v),
            Value::Decimal(v) => write!(f, "{}", v),
            Value::Float(v) => write!(f, "{}", v),
            Value::Double(v) => write!(f, "{}", v),
            Value::Text(v) => write!(f, "{}", v),
            Value::TextHash(v) => write!(f, "#{:x}", v),
            Value::Blob(v) => write!(f, "blob[{}]", v.len()),
            Value::Ip4(v) => {
                if v.len() == 4 {
                    write!(f, "{}.{}.{}.{}", v[0], v[1], v[2], v[3])
                } else {
                    write!(f, "ip4[{}]", v.len())
                }
            }
            Value::List(list) => write!(f, "list[{}]", list.len()),
            Value::Map(map) => write!(f, "map[{}]", map.table.len()),
            Value::IntArray(arr) => write!(f, "int[{}]", arr.len()),
            Value::FloatArray(arr) => write!(f, "float[{}]", arr.len()),
            Value::TextArray(arr) => write!(f, "text[{}]", arr.len()),
            Value::LongArray(arr) => write!(f, "long[{}]", arr.len()),
            Value::DoubleSummary { count, sum, .. } => write!(f, "dsum(n={},s={})", count, sum),
            Value::LongSummary { count, sum, .. } => write!(f, "lsum(n={},s={})", count, sum),
        }
    }
}
