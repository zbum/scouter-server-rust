use std::io::{self, BufReader, BufWriter, Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::netio::service::handler_registry::HandlerRegistry;
use crate::protocol::data_input::DataInputX;
use crate::protocol::data_output::DataOutputX;
use crate::protocol::request_cmd;
use crate::protocol::tcp_flag;

/// Handles a TCP_CLIENT connection with a blocking command loop.
/// Uses spawn_blocking to avoid blocking the async runtime.
pub async fn handle_client(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    registry: Arc<HandlerRegistry>,
) {
    info!("TCP client connected: {}", addr);

    // Convert to std blocking TcpStream
    let std_stream = match stream.into_std() {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to convert TcpStream for {}: {}", addr, e);
            return;
        }
    };

    if let Err(e) = std_stream.set_read_timeout(Some(Duration::from_secs(60))) {
        warn!("Failed to set read timeout for {}: {}", addr, e);
        return;
    }
    if let Err(e) = std_stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY for {}: {}", addr, e);
    }

    let result = tokio::task::spawn_blocking(move || {
        run_command_loop(std_stream, addr, &registry)
    }).await;

    match result {
        Ok(Ok(())) => info!("TCP client disconnected: {}", addr),
        Ok(Err(e)) => debug!("TCP client {} error: {}", addr, e),
        Err(e) => warn!("TCP client {} task panicked: {}", addr, e),
    }
}

fn run_command_loop(
    stream: std::net::TcpStream,
    addr: SocketAddr,
    registry: &HandlerRegistry,
) -> io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);
    let mut writer = BufWriter::new(stream);

    let boxed_reader: Box<dyn Read + Send> = Box::new(reader);
    let mut din = DataInputX::new(boxed_reader);

    let mut session_ok = false;

    loop {
        // Read command text (blob-encoded)
        let cmd = match din.read_text() {
            Ok(cmd) => cmd,
            Err(e) => {
                if e.kind() == io::ErrorKind::UnexpectedEof
                    || e.kind() == io::ErrorKind::WouldBlock
                    || e.kind() == io::ErrorKind::TimedOut
                {
                    debug!("Client {} disconnected: {}", addr, e);
                } else {
                    debug!("Client {} read error: {}", addr, e);
                }
                break;
            }
        };

        // Check for CLOSE command
        if cmd == request_cmd::CLOSE {
            debug!("Client {} sent CLOSE", addr);
            break;
        }

        // Read session token (8 bytes, big-endian)
        let session = match din.read_long() {
            Ok(s) => s,
            Err(e) => {
                debug!("Failed to read session from {}: {}", addr, e);
                break;
            }
        };

        // Validate session for non-free commands
        if !session_ok && !request_cmd::is_free_cmd(&cmd) {
            session_ok = registry.context.login_manager.ok_session(session);
            if !session_ok {
                warn!("Invalid session from {}: cmd={} session={:#x}", addr, cmd, session);
                writer.write_all(&[tcp_flag::INVALID_SESSION])?;
                writer.flush()?;
                break;
            }
        }

        debug!("TCP cmd={} session={:#x} from {}", cmd, session, addr);

        // Process command - handler reads remaining data from din (TCP stream)
        let mut dout = DataOutputX::new();
        registry.process(&cmd, &mut din, &mut dout, session_ok);

        // Write response buffer
        let response = dout.to_bytes();
        if !response.is_empty() {
            writer.write_all(&response)?;
        }

        // Write NoNEXT to signal end of response
        writer.write_all(&[tcp_flag::NO_NEXT])?;
        writer.flush()?;
    }

    Ok(())
}
