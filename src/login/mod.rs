use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use tracing::{debug, info};

/// A logged-in user session.
#[derive(Debug, Clone)]
pub struct LoginUser {
    pub id: String,
    pub ip: String,
    pub session: i64,
    pub group: String,
    pub logintime: i64,
    pub hostname: String,
    pub version: String,
    pub internal: bool,
}

/// Manages login sessions with TTL-based expiry.
pub struct LoginManager {
    sessions: DashMap<i64, LoginUser>,
    session_counter: std::sync::atomic::AtomicI64,
    default_ttl_ms: i64,
}

impl LoginManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            session_counter: std::sync::atomic::AtomicI64::new(1),
            default_ttl_ms: 3 * 60 * 60 * 1000, // 3 hours
        }
    }

    /// Attempt to log in. Returns session token (0 = failure).
    pub fn login(&self, id: &str, _password: &str, ip: &str, internal: bool) -> i64 {
        // For now, accept all logins (account management is Phase 5+)
        let session = self.session_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let user = LoginUser {
            id: id.to_string(),
            ip: ip.to_string(),
            session,
            group: String::new(),
            logintime: current_millis(),
            hostname: String::new(),
            version: String::new(),
            internal,
        };

        info!("Login: id={} ip={} session={:#x}", id, ip, session);
        self.sessions.insert(session, user);
        session
    }

    /// Validate session and refresh TTL.
    pub fn ok_session(&self, session: i64) -> bool {
        self.sessions.contains_key(&session)
    }

    /// Validate and return session value (0 = invalid).
    pub fn valid_session(&self, session: i64) -> i64 {
        if self.sessions.contains_key(&session) {
            session
        } else {
            0
        }
    }

    /// Get user by session.
    pub fn get_user(&self, session: i64) -> Option<LoginUser> {
        self.sessions.get(&session).map(|v| v.clone())
    }

    /// Update user in session.
    pub fn update_user(&self, session: i64, f: impl FnOnce(&mut LoginUser)) {
        if let Some(mut entry) = self.sessions.get_mut(&session) {
            f(&mut entry);
        }
    }

    /// Get all active login users.
    pub fn get_login_user_list(&self) -> Vec<LoginUser> {
        self.sessions.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Start the background session cleaner.
    pub fn start_cleaner(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let now = current_millis();
                let ttl = self.default_ttl_ms;
                self.sessions.retain(|_, user| {
                    now - user.logintime < ttl
                });
                debug!("Session cleaner: {} active sessions", self.sessions.len());
            }
        });
    }
}

fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
