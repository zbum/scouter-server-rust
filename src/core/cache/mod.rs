pub mod counter_cache;
pub mod xlog_cache;
pub mod text_cache;
pub mod alert_cache;
pub mod object_cache;
pub mod status_cache;

use crate::config::Config;

pub struct CacheManager {
    pub counter: counter_cache::CounterCache,
    pub xlog: xlog_cache::XLogCache,
    pub text: text_cache::TextCache,
    pub alert: alert_cache::AlertCache,
    pub object: object_cache::ObjectCache,
    pub status: status_cache::StatusCache,
}

impl CacheManager {
    pub fn new(_config: &Config) -> Self {
        Self {
            counter: counter_cache::CounterCache::new(),
            xlog: xlog_cache::XLogCache::new(20000),
            text: text_cache::TextCache::new(),
            alert: alert_cache::AlertCache::new(2000),
            object: object_cache::ObjectCache::new(),
            status: status_cache::StatusCache::new(),
        }
    }
}
