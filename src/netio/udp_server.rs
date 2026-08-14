use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::config::Config;
use crate::netio::net_data_processor::NetData;

pub async fn start_udp_server(
    config: Arc<Config>,
    tx: mpsc::Sender<NetData>,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let bind_addr = format!(
        "{}:{}",
        config.net_udp_listen_ip, config.net_udp_listen_port
    );
    let socket = UdpSocket::bind(&bind_addr).await?;

    // Set receive buffer size if possible
    if let Err(e) = socket.set_broadcast(true) {
        tracing::warn!("Failed to set broadcast: {}", e);
    }

    info!(
        "UDP server started on {} (buffer_size={})",
        bind_addr, config.net_udp_packet_buffer_size
    );

    let mut buf = vec![0u8; config.net_udp_packet_buffer_size];

    loop {
        let received = tokio::select! {
            _ = shutdown.cancelled() => {
                info!("UDP server stopping");
                break;
            }
            received = socket.recv_from(&mut buf) => received,
        };

        match received {
            Ok((len, addr)) => {
                let data = buf[..len].to_vec();
                if tx.send(NetData { data, addr }).await.is_err() {
                    error!("UDP processor channel closed, stopping UDP server");
                    break;
                }
            }
            Err(e) => {
                error!("UDP recv error: {}", e);
            }
        }
    }

    Ok(())
}
