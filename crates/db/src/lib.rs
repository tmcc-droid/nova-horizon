//! Database layer for Nova Horizon.
//!
//! PostgreSQL is the sole durable source of truth for accounts, ships, cargo,
//! markets, sessions, and jump transfer tokens (design Key Decision #8).

mod auth_repo;
mod economy;
mod models;

pub use auth_repo::*;
pub use economy::*;
pub use models::*;

use std::env;
use std::path::{Path, PathBuf};

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database not configured: set DATABASE_URL")]
    NotConfigured,
    #[error("migrations directory not found (set MIGRATIONS_DIR or run from repo root)")]
    MigrationsNotFound,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("database error: {0}")]
    Other(String),
}

/// Runtime database configuration (from env).
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub database_url: String,
}

impl DbConfig {
    pub fn from_env() -> Result<Self, DbError> {
        let database_url = env::var("DATABASE_URL").map_err(|_| DbError::NotConfigured)?;
        Ok(Self { database_url })
    }
}

/// Resolve path to SQL migrations directory.
pub fn migrations_dir() -> Result<PathBuf, DbError> {
    if let Ok(p) = env::var("MIGRATIONS_DIR") {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return Ok(pb);
        }
        return Err(DbError::MigrationsNotFound);
    }

    let candidates = [
        PathBuf::from("migrations"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../migrations"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../migrations"),
    ];
    for c in candidates {
        if c.is_dir() {
            return Ok(c
                .canonicalize()
                .unwrap_or(c));
        }
    }
    Err(DbError::MigrationsNotFound)
}

/// Connect with sensible pool defaults for a single-process game-server.
pub async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Apply all migrations from [`migrations_dir`].
pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    let dir = migrations_dir()?;
    info!(path = %dir.display(), "running database migrations");
    sqlx::migrate::Migrator::new(Path::new(&dir))
        .await?
        .run(pool)
        .await?;
    info!("migrations complete");
    Ok(())
}

/// Connect + migrate in one step (CLI `game-server migrate`).
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, DbError> {
    let pool = connect(database_url).await?;
    migrate(&pool).await?;
    Ok(pool)
}

/// Trade lock order (binding, design doc): character → ship → cargo → market.
/// Call inside an open transaction before mutating trade state.
pub async fn lock_trade_rows(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
    ship_id: Uuid,
    station_id: &str,
    commodity_id: &str,
) -> Result<(), DbError> {
    // character
    sqlx::query("SELECT id FROM characters WHERE id = $1 FOR UPDATE")
        .bind(character_id)
        .fetch_optional(&mut **tx)
        .await?;
    // ship
    sqlx::query("SELECT id FROM ships WHERE id = $1 FOR UPDATE")
        .bind(ship_id)
        .fetch_optional(&mut **tx)
        .await?;
    // cargo row may not exist yet; lock ship cargo set via ship already held
    let _ = commodity_id;
    // market
    sqlx::query(
        "SELECT station_id FROM station_markets \
         WHERE station_id = $1 AND commodity_id = $2 FOR UPDATE",
    )
    .bind(station_id)
    .bind(commodity_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(())
}

/// Lightweight health check used by tests / future readiness probe.
pub async fn ping(pool: &PgPool) -> Result<(), DbError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(pool)
        .await?;
    Ok(())
}

/// Count seeded market rows (smoke after migrate).
pub async fn count_market_rows(pool: &PgPool) -> Result<i64, DbError> {
    let n = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM station_markets")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_dir_resolves_in_workspace() {
        let dir = migrations_dir().expect("migrations dir");
        assert!(
            dir.join("0001_init.sql").is_file(),
            "missing 0001_init.sql in {}",
            dir.display()
        );
        assert!(
            dir.join("0002_seed_mvp_markets.sql").is_file(),
            "missing seed migration in {}",
            dir.display()
        );
    }

    /// Requires Postgres: `docker compose up -d postgres` and DATABASE_URL set.
    #[tokio::test]
    async fn migrate_against_postgres() {
        let Ok(url) = env::var("DATABASE_URL") else {
            eprintln!("skip migrate_against_postgres: DATABASE_URL not set");
            return;
        };
        let pool = connect_and_migrate(&url)
            .await
            .expect("migrate should succeed");
        ping(&pool).await.expect("ping");
        let n = count_market_rows(&pool).await.expect("count markets");
        assert!(n >= 4, "expected seeded market rows, got {n}");
    }
}
