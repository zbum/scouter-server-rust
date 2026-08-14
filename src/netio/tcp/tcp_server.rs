use std::sync::Arc;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::netio::service::handler_registry::HandlerRegistry;
use crate::netio::tcp::agent_worker::{AgentProtocol, TcpAgentWorker};
use crate::netio::tcp::service_worker;
use crate::netio::tcp::tcp_agent_manager::TcpAgentManager;
use crate::protocol::net_cafe;

/// Start the TCP server that handles both agent and client connections.
pub async fn start_tcp_server(
    config: Arc<Config>,
    registry: Arc<HandlerRegistry>,
    agent_manager: Arc<TcpAgentManager>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let addr = format!(
        "{}:{}",
        config.net_tcp_listen_ip, config.net_tcp_listen_port
    );
    let listener = TcpListener::bind(&addr).await?;
    info!("TCP server listening on {}", addr);

    loop {
        let accepted = tokio::select! {
            _ = shutdown.cancelled() => {
                info!("TCP server stopping");
                break;
            }
            accepted = listener.accept() => accepted,
        };

        match accepted {
            Ok((stream, addr)) => {
                let registry = registry.clone();
                let agent_mgr = agent_manager.clone();

                tokio::spawn(async move {
                    // Read magic bytes to identify connection type
                    let mut buf = [0u8; 4];
                    let (mut read_half, write_half) = stream.into_split();

                    if let Err(e) = read_half.read_exact(&mut buf).await {
                        debug!("Failed to read magic bytes from {}: {}", addr, e);
                        return;
                    }

                    let magic = i32::from_be_bytes(buf);

                    match magic {
                        net_cafe::TCP_AGENT => {
                            // V1 agent connection
                            let mut hash_buf = [0u8; 4];
                            if read_half.read_exact(&mut hash_buf).await.is_err() {
                                return;
                            }
                            let obj_hash = i32::from_be_bytes(hash_buf);

                            let worker = Arc::new(TcpAgentWorker::new(
                                obj_hash,
                                AgentProtocol::V1,
                                read_half,
                                write_half,
                            ));
                            let count = agent_mgr.add(obj_hash, worker);
                            info!(
                                "TCP agent V1 connected: {:#x} from {} (pool={})",
                                obj_hash, addr, count
                            );
                        }
                        net_cafe::TCP_AGENT_V2 => {
                            // V2 agent connection (length-prefixed)
                            let mut hash_buf = [0u8; 4];
                            if read_half.read_exact(&mut hash_buf).await.is_err() {
                                return;
                            }
                            let obj_hash = i32::from_be_bytes(hash_buf);

                            let worker = Arc::new(TcpAgentWorker::new(
                                obj_hash,
                                AgentProtocol::V2,
                                read_half,
                                write_half,
                            ));
                            let count = agent_mgr.add(obj_hash, worker);
                            info!(
                                "TCP agent V2 connected: {:#x} from {} (pool={})",
                                obj_hash, addr, count
                            );
                        }
                        net_cafe::TCP_CLIENT => {
                            // Reassemble TcpStream from the split halves
                            let stream = read_half.reunite(write_half).expect("reunite failed");
                            service_worker::handle_client(stream, addr, registry).await;
                        }
                        _ => {
                            warn!("Unknown TCP magic: {:#010x} from {}", magic, addr);
                        }
                    }
                });
            }
            Err(e) => {
                error!("TCP accept error: {}", e);
            }
        }
    }

    Ok(())
}
