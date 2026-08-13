//! Account / session / character persistence for PR-04.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{AccountRow, CharacterRow, CharacterSummary, SessionRow, ShipRow};
use crate::DbError;

/// Starter ship defaults (aligned with content/ships/shuttle.toml).
pub struct StarterShipSpec {
    pub def_id: &'static str,
    pub system_id: &'static str,
    pub station_id: &'static str,
    pub pos_x: f64,
    pub pos_y: f64,
    pub shield: f32,
    pub armor: f32,
    pub energy: f32,
    pub fuel: i32,
    pub loadout: serde_json::Value,
    pub starting_credits: i64,
}

pub fn default_starter_ship() -> StarterShipSpec {
    StarterShipSpec {
        def_id: "ship.shuttle",
        system_id: "sys.sol",
        station_id: "st.earth_orbit",
        pos_x: 5000.0,
        pos_y: 0.0,
        shield: 140.0,
        armor: 110.0,
        energy: 120.0,
        fuel: 10,
        loadout: serde_json::json!({
            "weapon": ["weapon.light_cannon"],
            "engine": ["engine.basic"]
        }),
        starting_credits: 5_000,
    }
}

pub async fn create_account(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
) -> Result<AccountRow, DbError> {
    let id = Uuid::now_v7();
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        INSERT INTO accounts (id, email, password_hash)
        VALUES ($1, $2, $3)
        RETURNING id, email::text AS email, password_hash, created_at, banned_until
        "#,
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await
    .map_err(map_unique)?;
    Ok(row)
}

pub async fn find_account_by_email(pool: &PgPool, email: &str) -> Result<Option<AccountRow>, DbError> {
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        SELECT id, email::text AS email, password_hash, created_at, banned_until
        FROM accounts WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_account_by_id(pool: &PgPool, id: Uuid) -> Result<Option<AccountRow>, DbError> {
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        SELECT id, email::text AS email, password_hash, created_at, banned_until
        FROM accounts WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn create_session(
    pool: &PgPool,
    account_id: Uuid,
    refresh_hash: &str,
    ttl_hours: i64,
) -> Result<SessionRow, DbError> {
    let id = Uuid::now_v7();
    let expires_at = Utc::now() + Duration::hours(ttl_hours);
    let row = sqlx::query_as::<_, SessionRow>(
        r#"
        INSERT INTO sessions (id, account_id, character_id, refresh_hash, expires_at)
        VALUES ($1, $2, NULL, $3, $4)
        RETURNING id, account_id, character_id, refresh_hash, expires_at, revoked_at, created_at
        "#,
    )
    .bind(id)
    .bind(account_id)
    .bind(refresh_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn find_session(pool: &PgPool, session_id: Uuid) -> Result<Option<SessionRow>, DbError> {
    let row = sqlx::query_as::<_, SessionRow>(
        r#"
        SELECT id, account_id, character_id, refresh_hash, expires_at, revoked_at, created_at
        FROM sessions WHERE id = $1
        "#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn rotate_session_refresh(
    pool: &PgPool,
    session_id: Uuid,
    new_refresh_hash: &str,
    ttl_hours: i64,
) -> Result<SessionRow, DbError> {
    let expires_at = Utc::now() + Duration::hours(ttl_hours);
    let row = sqlx::query_as::<_, SessionRow>(
        r#"
        UPDATE sessions
        SET refresh_hash = $2, expires_at = $3, revoked_at = NULL
        WHERE id = $1 AND revoked_at IS NULL
        RETURNING id, account_id, character_id, refresh_hash, expires_at, revoked_at, created_at
        "#,
    )
    .bind(session_id)
    .bind(new_refresh_hash)
    .bind(expires_at)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::Other("session not found or revoked".into()))?;
    Ok(row)
}

pub async fn revoke_session(pool: &PgPool, session_id: Uuid) -> Result<(), DbError> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Revoke other live sessions for this character (kick previous policy).
pub async fn revoke_other_character_sessions(
    pool: &PgPool,
    character_id: Uuid,
    keep_session_id: Uuid,
) -> Result<u64, DbError> {
    let res = sqlx::query(
        r#"
        UPDATE sessions
        SET revoked_at = now()
        WHERE character_id = $1
          AND id <> $2
          AND revoked_at IS NULL
        "#,
    )
    .bind(character_id)
    .bind(keep_session_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn bind_session_character(
    pool: &PgPool,
    session_id: Uuid,
    character_id: Uuid,
) -> Result<SessionRow, DbError> {
    // Clear unique live-session conflicts by revoking others first.
    let _ = revoke_other_character_sessions(pool, character_id, session_id).await?;

    let row = sqlx::query_as::<_, SessionRow>(
        r#"
        UPDATE sessions
        SET character_id = $2
        WHERE id = $1 AND revoked_at IS NULL AND expires_at > now()
        RETURNING id, account_id, character_id, refresh_hash, expires_at, revoked_at, created_at
        "#,
    )
    .bind(session_id)
    .bind(character_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::Other("session not bindable".into()))?;
    Ok(row)
}

pub async fn list_characters(
    pool: &PgPool,
    account_id: Uuid,
) -> Result<Vec<CharacterSummary>, DbError> {
    let rows = sqlx::query_as::<_, CharacterRow>(
        r#"
        SELECT id, account_id, name::text AS name, credits, active_ship_id, created_at
        FROM characters WHERE account_id = $1 ORDER BY created_at
        "#,
    )
    .bind(account_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|c| CharacterSummary {
            id: c.id,
            name: c.name,
            credits: c.credits,
            active_ship_id: c.active_ship_id,
        })
        .collect())
}

pub async fn find_character(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<Option<CharacterRow>, DbError> {
    let row = sqlx::query_as::<_, CharacterRow>(
        r#"
        SELECT id, account_id, name::text AS name, credits, active_ship_id, created_at
        FROM characters WHERE id = $1
        "#,
    )
    .bind(character_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn create_character_with_ship(
    pool: &PgPool,
    account_id: Uuid,
    name: &str,
    starter: &StarterShipSpec,
) -> Result<(CharacterRow, ShipRow), DbError> {
    let mut tx = pool.begin().await?;

    let character_id = Uuid::now_v7();
    let ship_id = Uuid::now_v7();

    sqlx::query(
        r#"
        INSERT INTO characters (id, account_id, name, credits, active_ship_id)
        VALUES ($1, $2, $3, $4, NULL)
        "#,
    )
    .bind(character_id)
    .bind(account_id)
    .bind(name)
    .bind(starter.starting_credits)
    .execute(&mut *tx)
    .await
    .map_err(map_unique)?;

    sqlx::query(
        r#"
        INSERT INTO ships (
            id, character_id, def_id, name, system_id,
            pos_x, pos_y, rot, shield, armor, energy, fuel, loadout,
            docked_station, last_docked_station
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, 0, $8, $9, $10, $11, $12,
            $13, $13
        )
        "#,
    )
    .bind(ship_id)
    .bind(character_id)
    .bind(starter.def_id)
    .bind(name)
    .bind(starter.system_id)
    .bind(starter.pos_x)
    .bind(starter.pos_y)
    .bind(starter.shield)
    .bind(starter.armor)
    .bind(starter.energy)
    .bind(starter.fuel)
    .bind(&starter.loadout)
    .bind(starter.station_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE characters SET active_ship_id = $1 WHERE id = $2")
        .bind(ship_id)
        .bind(character_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let character = find_character(pool, character_id)
        .await?
        .ok_or_else(|| DbError::Other("character missing after create".into()))?;
    let ship = find_ship(pool, ship_id)
        .await?
        .ok_or_else(|| DbError::Other("ship missing after create".into()))?;
    Ok((character, ship))
}

pub async fn find_ship(pool: &PgPool, ship_id: Uuid) -> Result<Option<ShipRow>, DbError> {
    let row = sqlx::query_as::<_, ShipRow>(
        r#"
        SELECT id, character_id, def_id, name, system_id,
               pos_x, pos_y, rot, shield, armor, energy, fuel, loadout,
               docked_station, last_docked_station, jump_state
        FROM ships WHERE id = $1
        "#,
    )
    .bind(ship_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub fn is_banned(banned_until: Option<DateTime<Utc>>) -> bool {
    match banned_until {
        Some(t) => t > Utc::now(),
        None => false,
    }
}

fn map_unique(e: sqlx::Error) -> DbError {
    if let sqlx::Error::Database(db) = &e {
        if db.constraint().is_some() || db.code().as_deref() == Some("23505") {
            return DbError::Other("unique_violation".into());
        }
    }
    DbError::Sqlx(e)
}
