//! Baseline rate limits (design Security section).

use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Sliding-window limiter: max `limit` events per `window`.
pub struct SlidingWindow {
    window: Duration,
    limit: usize,
    hits: DashMap<String, Mutex<VecDeque<Instant>>>,
}

impl SlidingWindow {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            window,
            limit,
            hits: DashMap::new(),
        }
    }

    /// Returns true if the event is allowed.
    pub fn check_and_record(&self, key: &str) -> bool {
        let now = Instant::now();
        let entry = self.hits.entry(key.to_string()).or_insert_with(|| {
            Mutex::new(VecDeque::new())
        });
        let mut q = entry.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(front) = q.front() {
            if now.duration_since(*front) > self.window {
                q.pop_front();
            } else {
                break;
            }
        }
        if q.len() >= self.limit {
            return false;
        }
        q.push_back(now);
        true
    }
}

/// WS connect: 5 / min / IP (design).
pub fn ws_connect_limiter() -> SlidingWindow {
    SlidingWindow::new(5, Duration::from_secs(60))
}

/// Auth login/register: 20 / min / IP (generous for friends alpha).
pub fn auth_ip_limiter() -> SlidingWindow {
    SlidingWindow::new(20, Duration::from_secs(60))
}

pub fn ip_key(ip: Option<IpAddr>) -> String {
    ip.map(|i| i.to_string()).unwrap_or_else(|| "unknown".into())
}
