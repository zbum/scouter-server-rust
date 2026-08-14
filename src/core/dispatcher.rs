use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::debug;

use crate::config::Config;
use crate::core::agent_manager::AgentManager;
use crate::core::alert_core::AlertCore;
use crate::core::perf_count_core::PerfCountCore;
use crate::core::profile_core::{ProfileCore, ProfilePack};
use crate::core::status_core::StatusCore;
use crate::core::summary_core::SummaryCore;
use crate::core::text_core::TextCore;
use crate::core::xlog_core::XLogCore;
use crate::protocol::pack::{Pack, XLogPack};
use crate::protocol::value::Value;
use crate::util::hash;

/// Central dispatcher that routes decoded Packs to appropriate core processors.
pub struct Dispatcher {
    #[allow(dead_code)]
    config: Arc<Config>,
    agent_manager: Arc<AgentManager>,
    perf_count_core: Arc<PerfCountCore>,
    xlog_core: Arc<XLogCore>,
    text_core: Arc<TextCore>,
    alert_core: Arc<AlertCore>,
    profile_core: Arc<ProfileCore>,
    status_core: Arc<StatusCore>,
    summary_core: Arc<SummaryCore>,
}

impl Dispatcher {
    pub fn new(
        config: Arc<Config>,
        agent_manager: Arc<AgentManager>,
        perf_count_core: Arc<PerfCountCore>,
        xlog_core: Arc<XLogCore>,
        text_core: Arc<TextCore>,
        alert_core: Arc<AlertCore>,
        profile_core: Arc<ProfileCore>,
        status_core: Arc<StatusCore>,
        summary_core: Arc<SummaryCore>,
    ) -> Self {
        Self {
            config,
            agent_manager,
            perf_count_core,
            xlog_core,
            text_core,
            alert_core,
            profile_core,
            status_core,
            summary_core,
        }
    }

    /// Route a decoded pack to the appropriate core processor.
    pub async fn dispatch(&self, pack: Pack, addr: &SocketAddr) {
        match pack {
            Pack::PerfCounter(mut p) => {
                let obj_hash = hash::hash(&p.obj_name);
                if p.time == 0 {
                    p.time = current_millis();
                }
                if p.timetype == 0 {
                    p.timetype = 1; // REALTIME
                }
                // Add objHash and time into data map (matches Java behavior)
                p.data.put("_objHash_", Value::Decimal(obj_hash as i64));
                p.data.put("_time_", Value::Decimal(p.time));

                self.perf_count_core.add(p).await;
            }
            Pack::XLog(p) => {
                self.xlog_core.add(p).await;
            }
            Pack::DroppedXLog(p) => {
                // Convert to a minimal XLogPack
                let mut xlog = XLogPack::default();
                xlog.gxid = p.gxid;
                xlog.txid = p.txid;
                self.xlog_core.add(xlog).await;
            }
            Pack::XLogProfile(p) => {
                self.profile_core.add(ProfilePack::V1(p)).await;
            }
            Pack::XLogProfile2(p) => {
                self.profile_core.add(ProfilePack::V2(p)).await;
            }
            Pack::Text(p) => {
                self.text_core.add(p).await;
            }
            Pack::Alert(p) => {
                self.alert_core.add(p).await;
            }
            Pack::Object(mut p) => {
                if p.address.is_empty() {
                    p.address = addr.ip().to_string();
                }
                self.agent_manager.active(p);
            }
            Pack::Status(p) => {
                self.status_core.add(p).await;
            }
            Pack::Summary(p) => {
                self.summary_core.add(p).await;
            }
            Pack::Stack(_) => {
                debug!("Stack pack received (processing deferred to Phase 4)");
            }
            Pack::Batch(_) => {
                debug!("Batch pack received (processing deferred to Phase 4)");
            }
            Pack::InteractionPerfCounter(_) => {
                debug!("InteractionPerfCounter received (processing deferred)");
            }
            Pack::SpanContainer(_) => {
                debug!("SpanContainer received (processing deferred)");
            }
            Pack::Map(_) => {
                debug!("MapPack received");
            }
            Pack::Span(_) => {
                debug!("SpanPack received");
            }
        }
    }
}

fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
