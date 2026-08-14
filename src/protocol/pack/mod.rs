use std::io;

use crate::error::{ScouterError, Result};
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::value::{MapValue, Value};

// Pack type codes (matching Java PackEnum)
pub const PACK_MAP: u8 = 10;
pub const PACK_XLOG: u8 = 21;
pub const PACK_DROPPED_XLOG: u8 = 22;
pub const PACK_XLOG_PROFILE: u8 = 26;
pub const PACK_XLOG_PROFILE2: u8 = 27;
pub const PACK_SPAN: u8 = 31;
pub const PACK_SPAN_CONTAINER: u8 = 32;
pub const PACK_TEXT: u8 = 50;
pub const PACK_PERF_COUNTER: u8 = 60;
pub const PACK_PERF_STATUS: u8 = 61;
pub const PACK_STACK: u8 = 62;
pub const PACK_SUMMARY: u8 = 63;
pub const PACK_BATCH: u8 = 64;
pub const PACK_PERF_INTERACTION_COUNTER: u8 = 65;
pub const PACK_ALERT: u8 = 70;
pub const PACK_OBJECT: u8 = 80;

#[derive(Debug, Clone)]
pub enum Pack {
    Map(MapPack),
    XLog(XLogPack),
    DroppedXLog(DroppedXLogPack),
    XLogProfile(XLogProfilePack),
    XLogProfile2(XLogProfilePack2),
    Text(TextPack),
    PerfCounter(PerfCounterPack),
    Status(StatusPack),
    Stack(StackPack),
    Summary(SummaryPack),
    Batch(BatchPack),
    InteractionPerfCounter(InteractionPerfCounterPack),
    Alert(AlertPack),
    Object(ObjectPack),
    Span(SpanPack),
    SpanContainer(SpanContainerPack),
}

impl Pack {
    pub fn type_code(&self) -> u8 {
        match self {
            Pack::Map(_) => PACK_MAP,
            Pack::XLog(_) => PACK_XLOG,
            Pack::DroppedXLog(_) => PACK_DROPPED_XLOG,
            Pack::XLogProfile(_) => PACK_XLOG_PROFILE,
            Pack::XLogProfile2(_) => PACK_XLOG_PROFILE2,
            Pack::Text(_) => PACK_TEXT,
            Pack::PerfCounter(_) => PACK_PERF_COUNTER,
            Pack::Status(_) => PACK_PERF_STATUS,
            Pack::Stack(_) => PACK_STACK,
            Pack::Summary(_) => PACK_SUMMARY,
            Pack::Batch(_) => PACK_BATCH,
            Pack::InteractionPerfCounter(_) => PACK_PERF_INTERACTION_COUNTER,
            Pack::Alert(_) => PACK_ALERT,
            Pack::Object(_) => PACK_OBJECT,
            Pack::Span(_) => PACK_SPAN,
            Pack::SpanContainer(_) => PACK_SPAN_CONTAINER,
        }
    }

    pub fn read<R: io::Read>(type_code: u8, din: &mut DataInputX<R>) -> Result<Pack> {
        match type_code {
            PACK_MAP => Ok(Pack::Map(MapPack::read(din)?)),
            PACK_XLOG => Ok(Pack::XLog(XLogPack::read(din)?)),
            PACK_DROPPED_XLOG => Ok(Pack::DroppedXLog(DroppedXLogPack::read(din)?)),
            PACK_XLOG_PROFILE => Ok(Pack::XLogProfile(XLogProfilePack::read(din)?)),
            PACK_XLOG_PROFILE2 => Ok(Pack::XLogProfile2(XLogProfilePack2::read(din)?)),
            PACK_TEXT => Ok(Pack::Text(TextPack::read(din)?)),
            PACK_PERF_COUNTER => Ok(Pack::PerfCounter(PerfCounterPack::read(din)?)),
            PACK_PERF_STATUS => Ok(Pack::Status(StatusPack::read(din)?)),
            PACK_STACK => Ok(Pack::Stack(StackPack::read(din)?)),
            PACK_SUMMARY => Ok(Pack::Summary(SummaryPack::read(din)?)),
            PACK_BATCH => Ok(Pack::Batch(BatchPack::read(din)?)),
            PACK_PERF_INTERACTION_COUNTER => Ok(Pack::InteractionPerfCounter(InteractionPerfCounterPack::read(din)?)),
            PACK_ALERT => Ok(Pack::Alert(AlertPack::read(din)?)),
            PACK_OBJECT => Ok(Pack::Object(ObjectPack::read(din)?)),
            PACK_SPAN => Ok(Pack::Span(SpanPack::read(din)?)),
            PACK_SPAN_CONTAINER => Ok(Pack::SpanContainer(SpanContainerPack::read(din)?)),
            _ => Err(ScouterError::UnknownPackType(type_code)),
        }
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        match self {
            Pack::Map(p) => p.write(dout),
            Pack::XLog(p) => p.write(dout),
            Pack::DroppedXLog(p) => p.write(dout),
            Pack::XLogProfile(p) => p.write(dout),
            Pack::XLogProfile2(p) => p.write(dout),
            Pack::Text(p) => p.write(dout),
            Pack::PerfCounter(p) => p.write(dout),
            Pack::Status(p) => p.write(dout),
            Pack::Stack(p) => p.write(dout),
            Pack::Summary(p) => p.write(dout),
            Pack::Batch(p) => p.write(dout),
            Pack::InteractionPerfCounter(p) => p.write(dout),
            Pack::Alert(p) => p.write(dout),
            Pack::Object(p) => p.write(dout),
            Pack::Span(p) => p.write(dout),
            Pack::SpanContainer(p) => p.write(dout),
        }
    }
}

impl std::fmt::Display for Pack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pack::Map(p) => write!(f, "MapPack[{}]", p.table.len()),
            Pack::XLog(p) => write!(f, "XLog obj={:#x} svc={:#x} txid={:#x} elapsed={}ms err={}", p.obj_hash, p.service, p.txid, p.elapsed, p.error),
            Pack::DroppedXLog(p) => write!(f, "DroppedXLog txid={:#x}", p.txid),
            Pack::XLogProfile(_) => write!(f, "XLogProfile"),
            Pack::XLogProfile2(_) => write!(f, "XLogProfile2"),
            Pack::Text(p) => write!(f, "Text type={} hash={:#x} text={}", p.xtype, p.hash, p.text),
            Pack::PerfCounter(p) => write!(f, "PerfCounter obj={} timetype={} data={}", p.obj_name, p.timetype, p.data.table.len()),
            Pack::Status(p) => write!(f, "Status obj={:#x} key={}", p.obj_hash, p.key),
            Pack::Stack(_) => write!(f, "Stack"),
            Pack::Summary(_) => write!(f, "Summary"),
            Pack::Batch(_) => write!(f, "Batch"),
            Pack::InteractionPerfCounter(_) => write!(f, "InteractionPerfCounter"),
            Pack::Alert(p) => write!(f, "Alert level={} title={}", p.level, p.title),
            Pack::Object(p) => write!(f, "Object type={} name={} hash={:#x} alive={}", p.obj_type, p.obj_name, p.obj_hash, p.alive),
            Pack::Span(_) => write!(f, "Span"),
            Pack::SpanContainer(_) => write!(f, "SpanContainer"),
        }
    }
}

// --- MapPack ---

#[derive(Debug, Clone, Default)]
pub struct MapPack {
    pub table: std::collections::HashMap<String, Value>,
}

impl MapPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let count = din.read_decimal().map_err(ScouterError::Io)? as usize;
        let mut table = std::collections::HashMap::with_capacity(count);
        for _ in 0..count {
            let key = din.read_text().map_err(ScouterError::Io)?;
            let value = din.read_value()?;
            table.insert(key, value);
        }
        Ok(MapPack { table })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_decimal(self.table.len() as i64)?;
        for (key, value) in &self.table {
            dout.write_text(key)?;
            dout.write_value(value)?;
        }
        Ok(())
    }
}

// --- PerfCounterPack ---

#[derive(Debug, Clone)]
pub struct PerfCounterPack {
    pub time: i64,
    pub obj_name: String,
    pub timetype: i8,
    pub data: MapValue,
}

impl Default for PerfCounterPack {
    fn default() -> Self {
        Self {
            time: 0,
            obj_name: String::new(),
            timetype: 0,
            data: MapValue::new(),
        }
    }
}

impl PerfCounterPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let time = din.read_long().map_err(ScouterError::Io)?;
        let obj_name = din.read_text().map_err(ScouterError::Io)?;
        let timetype = din.read_byte().map_err(ScouterError::Io)?;
        let data = din.read_value()?.into_map_value()
            .unwrap_or_default();
        Ok(PerfCounterPack { time, obj_name, timetype, data })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_long(self.time)?;
        dout.write_text(&self.obj_name)?;
        dout.write_byte(self.timetype as i32)?;
        dout.write_value(&Value::Map(self.data.clone()))
    }
}

// --- XLogPack ---

#[derive(Debug, Clone, Default)]
pub struct XLogPack {
    pub end_time: i64,
    pub obj_hash: i32,
    pub service: i32,
    pub txid: i64,
    pub thread_name_hash: i32,
    pub caller: i64,
    pub gxid: i64,
    pub elapsed: i32,
    pub error: i32,
    pub cpu: i32,
    pub sql_count: i32,
    pub sql_time: i32,
    pub ipaddr: Vec<u8>,
    pub kbytes: i32,
    pub status: i32,
    pub userid: i64,
    pub user_agent: i32,
    pub referer: i32,
    pub group: i32,
    pub apicall_count: i32,
    pub apicall_time: i32,
    pub country_code: String,
    pub city: i32,
    pub x_type: i8,
    pub login: i32,
    pub desc: i32,
    pub web_hash: i32,
    pub web_time: i32,
    pub has_dump: i8,
    pub text1: String,
    pub text2: String,
    pub queuing_host_hash: i32,
    pub queuing_time: i32,
    pub queuing_2nd_host_hash: i32,
    pub queuing_2nd_time: i32,
    pub text3: String,
    pub text4: String,
    pub text5: String,
    pub profile_count: i32,
    pub b3_mode: bool,
    pub profile_size: i32,
    pub discard_type: i8,
    pub ignore_global_consequent_sampling: bool,
}

impl XLogPack {
    /// Read with blob-envelope pattern for forward compatibility
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        let mut d = DataInputX::from_bytes(blob);
        let mut p = XLogPack::default();

        p.end_time = d.read_decimal().map_err(ScouterError::Io)?;
        p.obj_hash = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.service = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.txid = d.read_long().map_err(ScouterError::Io)?;
        p.caller = d.read_long().map_err(ScouterError::Io)?;
        p.gxid = d.read_long().map_err(ScouterError::Io)?;
        p.elapsed = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.error = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.cpu = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.sql_count = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.sql_time = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.ipaddr = d.read_blob().map_err(ScouterError::Io)?;
        p.kbytes = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.status = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.userid = d.read_decimal().map_err(ScouterError::Io)?;
        p.user_agent = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.referer = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.group = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.apicall_count = d.read_decimal().map_err(ScouterError::Io)? as i32;
        p.apicall_time = d.read_decimal().map_err(ScouterError::Io)? as i32;

        if d.available() > 0 {
            p.country_code = d.read_text().map_err(ScouterError::Io)?;
            p.city = d.read_decimal().map_err(ScouterError::Io)? as i32;
        }
        if d.available() > 0 {
            p.x_type = d.read_byte().map_err(ScouterError::Io)?;
        }
        if d.available() > 0 {
            p.login = d.read_decimal().map_err(ScouterError::Io)? as i32;
            p.desc = d.read_decimal().map_err(ScouterError::Io)? as i32;
        }
        if d.available() > 0 {
            p.web_hash = d.read_decimal().map_err(ScouterError::Io)? as i32;
            p.web_time = d.read_decimal().map_err(ScouterError::Io)? as i32;
        }
        if d.available() > 0 {
            p.has_dump = d.read_byte().map_err(ScouterError::Io)?;
        }
        if d.available() > 0 {
            p.thread_name_hash = d.read_decimal().map_err(ScouterError::Io)? as i32;
        }
        if d.available() > 0 {
            p.text1 = d.read_text().map_err(ScouterError::Io)?;
            p.text2 = d.read_text().map_err(ScouterError::Io)?;
        }
        if d.available() > 0 {
            p.queuing_host_hash = d.read_decimal().map_err(ScouterError::Io)? as i32;
            p.queuing_time = d.read_decimal().map_err(ScouterError::Io)? as i32;
            p.queuing_2nd_host_hash = d.read_decimal().map_err(ScouterError::Io)? as i32;
            p.queuing_2nd_time = d.read_decimal().map_err(ScouterError::Io)? as i32;
        }
        if d.available() > 0 {
            p.text3 = d.read_text().map_err(ScouterError::Io)?;
            p.text4 = d.read_text().map_err(ScouterError::Io)?;
            p.text5 = d.read_text().map_err(ScouterError::Io)?;
        }
        if d.available() > 0 {
            p.profile_count = d.read_decimal().map_err(ScouterError::Io)? as i32;
        }
        if d.available() > 0 {
            p.b3_mode = d.read_boolean().map_err(ScouterError::Io)?;
        }
        if d.available() > 0 {
            p.profile_size = d.read_decimal().map_err(ScouterError::Io)? as i32;
            p.discard_type = d.read_byte().map_err(ScouterError::Io)?;
            p.ignore_global_consequent_sampling = d.read_boolean().map_err(ScouterError::Io)?;
        }

        Ok(p)
    }

    pub fn write(&self, out: &mut DataOutputX) -> io::Result<()> {
        let mut o = DataOutputX::new();
        o.write_decimal(self.end_time)?;
        o.write_decimal(self.obj_hash as i64)?;
        o.write_decimal(self.service as i64)?;
        o.write_long(self.txid)?;
        o.write_long(self.caller)?;
        o.write_long(self.gxid)?;
        o.write_decimal(self.elapsed as i64)?;
        o.write_decimal(self.error as i64)?;
        o.write_decimal(self.cpu as i64)?;
        o.write_decimal(self.sql_count as i64)?;
        o.write_decimal(self.sql_time as i64)?;
        o.write_blob(&self.ipaddr)?;
        o.write_decimal(self.kbytes as i64)?;
        o.write_decimal(self.status as i64)?;
        o.write_decimal(self.userid)?;
        o.write_decimal(self.user_agent as i64)?;
        o.write_decimal(self.referer as i64)?;
        o.write_decimal(self.group as i64)?;
        o.write_decimal(self.apicall_count as i64)?;
        o.write_decimal(self.apicall_time as i64)?;
        o.write_text(&self.country_code)?;
        o.write_decimal(self.city as i64)?;
        o.write_byte(self.x_type as i32)?;
        o.write_decimal(self.login as i64)?;
        o.write_decimal(self.desc as i64)?;
        o.write_decimal(self.web_hash as i64)?;
        o.write_decimal(self.web_time as i64)?;
        o.write_byte(self.has_dump as i32)?;
        o.write_decimal(self.thread_name_hash as i64)?;
        o.write_text(&self.text1)?;
        o.write_text(&self.text2)?;
        o.write_decimal(self.queuing_host_hash as i64)?;
        o.write_decimal(self.queuing_time as i64)?;
        o.write_decimal(self.queuing_2nd_host_hash as i64)?;
        o.write_decimal(self.queuing_2nd_time as i64)?;
        o.write_text(&self.text3)?;
        o.write_text(&self.text4)?;
        o.write_text(&self.text5)?;
        o.write_decimal(self.profile_count as i64)?;
        o.write_boolean(self.b3_mode)?;
        o.write_decimal(self.profile_size as i64)?;
        o.write_byte(self.discard_type as i32)?;
        o.write_boolean(self.ignore_global_consequent_sampling)?;
        out.write_blob(&o.to_bytes())
    }

    pub fn is_driving(&self) -> bool {
        self.gxid == self.txid || self.gxid == 0
    }
}

// --- DroppedXLogPack ---

#[derive(Debug, Clone, Default)]
pub struct DroppedXLogPack {
    pub txid: i64,
    pub gxid: i64,
}

impl DroppedXLogPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        let mut d = DataInputX::from_bytes(blob);
        let txid = d.read_long().map_err(ScouterError::Io)?;
        let gxid = d.read_long().map_err(ScouterError::Io)?;
        Ok(DroppedXLogPack { txid, gxid })
    }

    pub fn write(&self, out: &mut DataOutputX) -> io::Result<()> {
        let mut o = DataOutputX::new();
        o.write_long(self.txid)?;
        o.write_long(self.gxid)?;
        out.write_blob(&o.to_bytes())
    }
}

// --- TextPack ---

#[derive(Debug, Clone, Default)]
pub struct TextPack {
    pub xtype: String,
    pub hash: i32,
    pub text: String,
}

impl TextPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let xtype = din.read_text().map_err(ScouterError::Io)?;
        let hash = din.read_int().map_err(ScouterError::Io)?;
        let text = din.read_text().map_err(ScouterError::Io)?;
        Ok(TextPack { xtype, hash, text })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_text(&self.xtype)?;
        dout.write_int(self.hash)?;
        dout.write_text(&self.text)
    }
}

// --- AlertPack ---

#[derive(Debug, Clone, Default)]
pub struct AlertPack {
    pub time: i64,
    pub obj_type: String,
    pub obj_hash: i32,
    pub level: i8,
    pub title: String,
    pub message: String,
    pub tags: MapValue,
}

impl AlertPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let time = din.read_long().map_err(ScouterError::Io)?;
        let level = din.read_byte().map_err(ScouterError::Io)?;
        let obj_type = din.read_text().map_err(ScouterError::Io)?;
        let obj_hash = din.read_int().map_err(ScouterError::Io)?;
        let title = din.read_text().map_err(ScouterError::Io)?;
        let message = din.read_text().map_err(ScouterError::Io)?;
        let tags = din.read_value()?.into_map_value().unwrap_or_default();
        Ok(AlertPack { time, obj_type, obj_hash, level, title, message, tags })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_long(self.time)?;
        dout.write_byte(self.level as i32)?;
        dout.write_text(&self.obj_type)?;
        dout.write_int(self.obj_hash)?;
        dout.write_text(&self.title)?;
        dout.write_text(&self.message)?;
        dout.write_value(&Value::Map(self.tags.clone()))
    }
}

// --- ObjectPack ---

#[derive(Debug, Clone)]
pub struct ObjectPack {
    pub obj_type: String,
    pub obj_hash: i32,
    pub obj_name: String,
    pub address: String,
    pub version: String,
    pub alive: bool,
    pub wakeup: i64,
    pub tags: MapValue,
}

impl Default for ObjectPack {
    fn default() -> Self {
        Self {
            obj_type: String::new(),
            obj_hash: 0,
            obj_name: String::new(),
            address: String::new(),
            version: String::new(),
            alive: true,
            wakeup: 0,
            tags: MapValue::new(),
        }
    }
}

impl ObjectPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let obj_type = din.read_text().map_err(ScouterError::Io)?;
        let obj_hash = din.read_decimal().map_err(ScouterError::Io)? as i32;
        let obj_name = din.read_text().map_err(ScouterError::Io)?;
        let address = din.read_text().map_err(ScouterError::Io)?;
        let version = din.read_text().map_err(ScouterError::Io)?;
        let alive = din.read_boolean().map_err(ScouterError::Io)?;
        let wakeup = din.read_decimal().map_err(ScouterError::Io)?;
        let tags = din.read_value()?.into_map_value().unwrap_or_default();
        Ok(ObjectPack { obj_type, obj_hash, obj_name, address, version, alive, wakeup, tags })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_text(&self.obj_type)?;
        dout.write_decimal(self.obj_hash as i64)?;
        dout.write_text(&self.obj_name)?;
        dout.write_text(&self.address)?;
        dout.write_text(&self.version)?;
        dout.write_boolean(self.alive)?;
        dout.write_decimal(self.wakeup)?;
        dout.write_value(&Value::Map(self.tags.clone()))
    }
}

// --- StatusPack ---

#[derive(Debug, Clone, Default)]
pub struct StatusPack {
    pub time: i64,
    pub obj_type: String,
    pub obj_hash: i32,
    pub key: String,
    pub data: MapValue,
}

impl StatusPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let time = din.read_decimal().map_err(ScouterError::Io)?;
        let obj_type = din.read_text().map_err(ScouterError::Io)?;
        let obj_hash = din.read_decimal().map_err(ScouterError::Io)? as i32;
        let key = din.read_text().map_err(ScouterError::Io)?;
        let data = din.read_value()?.into_map_value().unwrap_or_default();
        Ok(StatusPack { time, obj_type, obj_hash, key, data })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_decimal(self.time)?;
        dout.write_text(&self.obj_type)?;
        dout.write_decimal(self.obj_hash as i64)?;
        dout.write_text(&self.key)?;
        dout.write_value(&Value::Map(self.data.clone()))
    }
}

// --- XLogProfilePack ---

#[derive(Debug, Clone, Default)]
pub struct XLogProfilePack {
    pub txid: i64,
    pub obj_hash: i32,
    pub profile: Vec<u8>,
    pub service: i32,
    pub elapsed: i32,
}

impl XLogProfilePack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let txid = din.read_long().map_err(ScouterError::Io)?;
        let obj_hash = din.read_int().map_err(ScouterError::Io)?;
        let profile = din.read_blob().map_err(ScouterError::Io)?;
        Ok(XLogProfilePack { txid, obj_hash, profile, service: 0, elapsed: 0 })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_long(self.txid)?;
        dout.write_int(self.obj_hash)?;
        dout.write_blob(&self.profile)
    }
}

// --- XLogProfilePack2 (extended with service/elapsed) ---

#[derive(Debug, Clone, Default)]
pub struct XLogProfilePack2 {
    pub txid: i64,
    pub obj_hash: i32,
    pub profile: Vec<u8>,
    pub service: i32,
    pub elapsed: i32,
}

impl XLogProfilePack2 {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let txid = din.read_long().map_err(ScouterError::Io)?;
        let obj_hash = din.read_int().map_err(ScouterError::Io)?;
        let service = din.read_int().map_err(ScouterError::Io)?;
        let elapsed = din.read_int().map_err(ScouterError::Io)?;
        let profile = din.read_blob().map_err(ScouterError::Io)?;
        Ok(XLogProfilePack2 { txid, obj_hash, profile, service, elapsed })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_long(self.txid)?;
        dout.write_int(self.obj_hash)?;
        dout.write_int(self.service)?;
        dout.write_int(self.elapsed)?;
        dout.write_blob(&self.profile)
    }
}

// --- Stub implementations for less-used pack types ---
// These store raw bytes for now; full parsing added in later phases.

#[derive(Debug, Clone, Default)]
pub struct StackPack {
    pub raw: Vec<u8>,
}

impl StackPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        Ok(StackPack { raw: blob })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_blob(&self.raw)
    }
}

// Summary type constants
pub const SUMMARY_APP: u8 = 1;
pub const SUMMARY_SQL: u8 = 2;
pub const SUMMARY_APICALL: u8 = 3;
pub const SUMMARY_IP: u8 = 4;
pub const SUMMARY_USER_AGENT: u8 = 5;
pub const SUMMARY_SERVICE_ERROR: u8 = 6;
pub const SUMMARY_ALERT: u8 = 7;

#[derive(Debug, Clone)]
pub struct SummaryPack {
    pub stype: u8,
    pub table: MapValue,
}

impl Default for SummaryPack {
    fn default() -> Self {
        Self {
            stype: 0,
            table: MapValue::new(),
        }
    }
}

impl SummaryPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        let mut d = DataInputX::from_bytes(blob);
        let stype = d.read_byte().map_err(ScouterError::Io)? as u8;
        let table = d.read_value()?.into_map_value().unwrap_or_default();
        Ok(SummaryPack { stype, table })
    }

    pub fn write(&self, out: &mut DataOutputX) -> io::Result<()> {
        let mut o = DataOutputX::new();
        o.write_byte(self.stype as i32)?;
        o.write_value(&Value::Map(self.table.clone()))?;
        out.write_blob(&o.to_bytes())
    }
}

#[derive(Debug, Clone, Default)]
pub struct BatchPack {
    pub raw: Vec<u8>,
}

impl BatchPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        Ok(BatchPack { raw: blob })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_blob(&self.raw)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InteractionPerfCounterPack {
    pub raw: Vec<u8>,
}

impl InteractionPerfCounterPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        Ok(InteractionPerfCounterPack { raw: blob })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_blob(&self.raw)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpanPack {
    pub raw: Vec<u8>,
}

impl SpanPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        Ok(SpanPack { raw: blob })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_blob(&self.raw)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpanContainerPack {
    pub raw: Vec<u8>,
}

impl SpanContainerPack {
    pub fn read<R: io::Read>(din: &mut DataInputX<R>) -> Result<Self> {
        let blob = din.read_blob().map_err(ScouterError::Io)?;
        Ok(SpanContainerPack { raw: blob })
    }

    pub fn write(&self, dout: &mut DataOutputX) -> io::Result<()> {
        dout.write_blob(&self.raw)
    }
}
