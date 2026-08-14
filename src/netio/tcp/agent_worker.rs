use std::io;
use std::time::Instant;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::pack::{MapPack, Pack};
use crate::protocol::tcp_flag;

/// Protocol version for agent connections.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentProtocol {
    V1, // TCP_AGENT (0xCAFE1001)
    V2, // TCP_AGENT_V2 (0xCAFE1002)
}

/// A persistent TCP connection to an agent.
pub struct TcpAgentWorker {
    pub obj_hash: i32,
    pub protocol: AgentProtocol,
    reader: Mutex<BufReader<OwnedReadHalf>>,
    writer: Mutex<BufWriter<OwnedWriteHalf>>,
    pub last_write_time: Mutex<Instant>,
    pub closed: std::sync::atomic::AtomicBool,
}

impl TcpAgentWorker {
    pub fn new(
        obj_hash: i32,
        protocol: AgentProtocol,
        read_half: OwnedReadHalf,
        write_half: OwnedWriteHalf,
    ) -> Self {
        Self {
            obj_hash,
            protocol,
            reader: Mutex::new(BufReader::new(read_half)),
            writer: Mutex::new(BufWriter::new(write_half)),
            last_write_time: Mutex::new(Instant::now()),
            closed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Write a command + pack to the agent.
    pub async fn write(&self, cmd: &str, pack: &Pack) -> io::Result<()> {
        if self.is_closed() {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Agent connection closed"));
        }

        let mut dout = DataOutputX::new();
        dout.write_text(cmd)?;
        dout.write_pack(pack)?;
        let payload = dout.to_bytes();

        let mut writer = self.writer.lock().await;

        match self.protocol {
            AgentProtocol::V1 => {
                writer.write_all(&payload).await?;
            }
            AgentProtocol::V2 => {
                // Length-prefixed
                let len = payload.len() as i32;
                writer.write_all(&len.to_be_bytes()).await?;
                writer.write_all(&payload).await?;
            }
        }
        writer.flush().await?;

        *self.last_write_time.lock().await = Instant::now();
        Ok(())
    }

    /// Read a pack from the agent's response.
    pub async fn read_pack(&self) -> io::Result<Pack> {
        let mut reader = self.reader.lock().await;

        match self.protocol {
            AgentProtocol::V1 => {
                // Read pack directly from stream
                let mut header = [0u8; 1];
                reader.read_exact(&mut header).await?;
                // Skip flag byte, read pack
                let mut buf = Vec::new();
                // For V1, we need to buffer - simplified approach
                reader.read_buf(&mut buf).await?;
                let mut din = DataInputX::from_bytes(buf);
                din.read_pack().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
            }
            AgentProtocol::V2 => {
                let mut len_buf = [0u8; 4];
                reader.read_exact(&mut len_buf).await?;
                let len = i32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf).await?;
                let mut din = DataInputX::from_bytes(buf);
                din.read_pack().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
            }
        }
    }

    /// Read flag byte from agent response.
    pub async fn read_flag(&self) -> io::Result<u8> {
        let mut reader = self.reader.lock().await;
        let mut flag = [0u8; 1];
        reader.read_exact(&mut flag).await?;
        Ok(flag[0])
    }

    /// Send KEEP_ALIVE and read response.
    pub async fn send_keep_alive(&self) -> io::Result<()> {
        let empty_map = Pack::Map(MapPack { table: std::collections::HashMap::new() });
        self.write("KEEP_ALIVE", &empty_map).await?;

        // Read response until NoNEXT
        loop {
            let flag = self.read_flag().await?;
            match flag {
                tcp_flag::HAS_NEXT => {
                    // Read and discard response pack
                    let _pack = self.read_pack().await?;
                }
                tcp_flag::NO_NEXT => break,
                _ => break,
            }
        }
        Ok(())
    }

    pub fn close(&self) {
        self.closed.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
