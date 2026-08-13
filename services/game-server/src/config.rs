//! Runtime configuration from environment.

use std::env;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub content_version: String,
    pub jwt_secret: String,
    pub session_ttl_hours: i64,
    pub access_ttl_secs: i64,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind: SocketAddr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid BIND_ADDR: {e}"))?;

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is required (see .env.example)"))?;

        let content_version =
            env::var("CONTENT_VERSION").unwrap_or_else(|_| content::CONTENT_VERSION.to_string());

        let jwt_secret = env::var("JWT_ACCESS_SECRET").unwrap_or_else(|_| {
            tracing::warn!("JWT_ACCESS_SECRET unset; using insecure dev default");
            "dev-only-change-me".into()
        });

        let session_ttl_hours = env::var("SESSION_TTL_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(72);

        let access_ttl_secs = env::var("ACCESS_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(15 * 60);

        Ok(Self {
            bind,
            database_url,
            content_version,
            jwt_secret,
            session_ttl_hours,
            access_ttl_secs,
        })
    }
}
