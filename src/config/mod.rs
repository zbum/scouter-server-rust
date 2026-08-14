use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server_id: String,

    // Network
    pub net_udp_listen_ip: String,
    pub net_udp_listen_port: u16,
    pub net_tcp_listen_ip: String,
    pub net_tcp_listen_port: u16,
    pub net_http_port: u16,
    pub net_http_server_enabled: bool,
    pub net_udp_packet_buffer_size: usize,
    pub net_udp_so_rcvbuf_size: usize,
    pub net_udp_worker_thread_count: usize,

    // Directories
    pub db_dir: String,
    pub log_dir: String,
    pub conf_dir: String,

    // Queue sizes
    pub xlog_queue_size: usize,
    pub profile_queue_size: usize,
    pub counter_queue_size: usize,
    pub text_queue_size: usize,
    pub alert_queue_size: usize,
    pub status_queue_size: usize,
    pub summary_queue_size: usize,

    // DB settings
    pub db_keep_days: u32,
    pub mgr_xlog_id_index_mb: usize,
    pub mgr_text_db_daily_index_mb: usize,
    pub mgr_counter_index_mb: usize,

    // Logging flags
    pub log_udp_packet: bool,
    pub log_udp_counter: bool,
    pub log_udp_xlog: bool,
    pub log_udp_profile: bool,
    pub log_udp_text: bool,
    pub log_udp_alert: bool,
    pub log_udp_object: bool,
    pub log_udp_status: bool,
    pub log_udp_stack: bool,
    pub log_udp_summary: bool,
    pub log_udp_batch: bool,
    pub log_udp_span: bool,
    pub log_udp_multipacket: bool,
    pub log_udp_interaction_counter: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_id: "SCOUTER-RUST".into(),
            net_udp_listen_ip: "0.0.0.0".into(),
            net_udp_listen_port: 6100,
            net_tcp_listen_ip: "0.0.0.0".into(),
            net_tcp_listen_port: 6100,
            net_http_port: 6180,
            net_http_server_enabled: false,
            net_udp_packet_buffer_size: 65535,
            net_udp_so_rcvbuf_size: 4 * 1024 * 1024,
            net_udp_worker_thread_count: 3,
            db_dir: "./database".into(),
            log_dir: "./logs".into(),
            conf_dir: "./conf".into(),
            xlog_queue_size: 10000,
            profile_queue_size: 10000,
            counter_queue_size: 10000,
            text_queue_size: 10000,
            alert_queue_size: 10000,
            status_queue_size: 10000,
            summary_queue_size: 10000,
            db_keep_days: 30,
            mgr_xlog_id_index_mb: 1,
            mgr_text_db_daily_index_mb: 1,
            mgr_counter_index_mb: 1,
            log_udp_packet: false,
            log_udp_counter: false,
            log_udp_xlog: false,
            log_udp_profile: false,
            log_udp_text: false,
            log_udp_alert: false,
            log_udp_object: false,
            log_udp_status: false,
            log_udp_stack: false,
            log_udp_summary: false,
            log_udp_batch: false,
            log_udp_span: false,
            log_udp_multipacket: false,
            log_udp_interaction_counter: false,
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Self {
        let config_path = Path::new(path);
        if config_path.exists() {
            match std::fs::read_to_string(config_path) {
                Ok(content) => {
                    match toml::from_str::<Config>(&content) {
                        Ok(config) => {
                            tracing::info!("Loaded config from {}", path);
                            return config;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse config {}: {}, using defaults", path, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read config {}: {}, using defaults", path, e);
                }
            }
        } else {
            tracing::info!("Config file not found at {}, using defaults", path);
        }
        Config::default()
    }
}
