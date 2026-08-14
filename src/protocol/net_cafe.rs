// Magic bytes for UDP packets
pub const UDP_CAFE: i32 = 0x43414645_u32 as i32; // "CAFE"
pub const UDP_CAFE_N: i32 = 0x4341464E_u32 as i32; // "CAFN" - multiple packs
pub const UDP_CAFE_MTU: i32 = 0x4341464D_u32 as i32; // "CAFM" - MTU fragmented
pub const UDP_JAVA: i32 = 0x4A415641_u32 as i32; // "JAVA"
pub const UDP_JAVA_N: i32 = 0x4A41564E_u32 as i32; // "JAVN"
pub const UDP_JAVA_MTU: i32 = 0x4A4D5455_u32 as i32; // "JMTU"

// TCP connection types
pub const TCP_AGENT: i32 = 0xCAFE1001_u32 as i32;
pub const TCP_AGENT_V2: i32 = 0xCAFE1002_u32 as i32;
pub const TCP_AGENT_REQ: i32 = 0xCAFE1011_u32 as i32;
pub const TCP_CLIENT: i32 = 0xCAFE2001_u32 as i32;
pub const TCP_SHUTDOWN: i32 = 0xCAFE1999_u32 as i32;
