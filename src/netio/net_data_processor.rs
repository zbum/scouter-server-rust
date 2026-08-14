use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::core::dispatcher::Dispatcher;
use crate::protocol::data_input::DataInputX;
use crate::protocol::net_cafe;
use crate::protocol::pack::Pack;
use crate::netio::multi_packet::MultiPacketProcessor;

pub struct NetData {
    pub data: Vec<u8>,
    pub addr: SocketAddr,
}

pub struct NetDataProcessor {
    config: Arc<Config>,
    multi_packet: Arc<MultiPacketProcessor>,
    dispatcher: Arc<Dispatcher>,
}

impl NetDataProcessor {
    pub fn new(config: Arc<Config>, dispatcher: Arc<Dispatcher>) -> Self {
        Self {
            config,
            multi_packet: Arc::new(MultiPacketProcessor::new()),
            dispatcher,
        }
    }

    pub fn start(self: Arc<Self>, mut rx: mpsc::Receiver<NetData>) {
        let worker_count = self.config.net_udp_worker_thread_count;
        info!("Starting {} UDP data processor workers", worker_count);

        let processor = self.clone();
        tokio::spawn(async move {
            while let Some(net_data) = rx.recv().await {
                let proc = processor.clone();
                tokio::spawn(async move {
                    if let Err(e) = proc.process(net_data).await {
                        warn!("Error processing UDP packet: {}", e);
                    }
                });
            }
        });
    }

    async fn process(&self, p: NetData) -> crate::error::Result<()> {
        let mut din = DataInputX::from_bytes(p.data);
        let cafe = din.read_int().map_err(crate::error::ScouterError::Io)?;

        match cafe {
            net_cafe::UDP_CAFE | net_cafe::UDP_JAVA => {
                self.process_cafe(&mut din, &p.addr).await?;
            }
            net_cafe::UDP_CAFE_N | net_cafe::UDP_JAVA_N => {
                self.process_cafe_n(&mut din, &p.addr).await?;
            }
            net_cafe::UDP_CAFE_MTU | net_cafe::UDP_JAVA_MTU => {
                self.process_cafe_mtu(&mut din, &p.addr).await?;
            }
            _ => {
                warn!("Received unknown packet magic: {:#010x} from {}", cafe, p.addr);
            }
        }
        Ok(())
    }

    async fn process_cafe(
        &self,
        din: &mut DataInputX<std::io::Cursor<Vec<u8>>>,
        addr: &SocketAddr,
    ) -> crate::error::Result<()> {
        let pack = din.read_pack()?;
        self.process_pack(pack, addr).await;
        Ok(())
    }

    async fn process_cafe_n(
        &self,
        din: &mut DataInputX<std::io::Cursor<Vec<u8>>>,
        addr: &SocketAddr,
    ) -> crate::error::Result<()> {
        let n = din.read_short().map_err(crate::error::ScouterError::Io)?;
        for _ in 0..n {
            let pack = din.read_pack()?;
            self.process_pack(pack, addr).await;
        }
        Ok(())
    }

    async fn process_cafe_mtu(
        &self,
        din: &mut DataInputX<std::io::Cursor<Vec<u8>>>,
        addr: &SocketAddr,
    ) -> crate::error::Result<()> {
        let _obj_hash = din.read_int().map_err(crate::error::ScouterError::Io)?;
        let packet_id = din.read_long().map_err(crate::error::ScouterError::Io)?;
        let total = din.read_short().map_err(crate::error::ScouterError::Io)?;
        let num = din.read_short().map_err(crate::error::ScouterError::Io)?;
        let data = din.read_blob().map_err(crate::error::ScouterError::Io)?;

        if let Some(assembled) = self.multi_packet.add(packet_id, total, num, data).await {
            let mut d = DataInputX::from_bytes(assembled);
            let pack = d.read_pack()?;
            self.process_pack(pack, addr).await;

            if self.config.log_udp_multipacket {
                info!("Reassembled multi-packet: total={} from {}", total, addr);
            }
        }
        Ok(())
    }

    async fn process_pack(&self, pack: Pack, addr: &SocketAddr) {
        // Log if enabled
        if self.config.log_udp_packet {
            info!("UDP packet from {}: {}", addr, pack);
        }

        match &pack {
            Pack::PerfCounter(p) => {
                if self.config.log_udp_counter {
                    debug!("COUNTER: obj={} data={}", p.obj_name, p.data.table.len());
                }
            }
            Pack::XLog(p) => {
                if self.config.log_udp_xlog {
                    debug!("XLOG: obj={:#x} svc={:#x} elapsed={}ms err={}", p.obj_hash, p.service, p.elapsed, p.error);
                }
            }
            Pack::XLogProfile(_) | Pack::XLogProfile2(_) => {
                if self.config.log_udp_profile {
                    debug!("PROFILE: {}", pack);
                }
            }
            Pack::DroppedXLog(p) => {
                if self.config.log_udp_xlog {
                    debug!("DROPPED XLOG: txid={:#x}", p.txid);
                }
            }
            Pack::Text(p) => {
                if self.config.log_udp_text {
                    debug!("TEXT: type={} hash={:#x} text={}", p.xtype, p.hash, p.text);
                }
            }
            Pack::Alert(p) => {
                if self.config.log_udp_alert {
                    debug!("ALERT: level={} title={} msg={}", p.level, p.title, p.message);
                }
            }
            Pack::Object(p) => {
                if self.config.log_udp_object {
                    debug!("OBJECT: type={} name={} hash={:#x}", p.obj_type, p.obj_name, p.obj_hash);
                }
            }
            Pack::Status(_) => {
                if self.config.log_udp_status {
                    debug!("STATUS: {}", pack);
                }
            }
            Pack::Summary(_) => {
                if self.config.log_udp_summary {
                    debug!("SUMMARY: {}", pack);
                }
            }
            _ => {}
        }

        // Dispatch to core processors
        self.dispatcher.dispatch(pack, addr).await;
    }
}
