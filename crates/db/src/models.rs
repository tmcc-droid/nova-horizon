//! Row types shared by auth and persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct AccountRow {
    pub id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub banned_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CharacterRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub name: String,
    pub credits: i64,
    pub active_ship_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ShipRow {
    pub id: Uuid,
    pub character_id: Uuid,
    pub def_id: String,
    pub name: Option<String>,
    pub system_id: String,
    pub pos_x: f64,
    pub pos_y: f64,
    pub rot: f32,
    pub shield: f32,
    pub armor: f32,
    pub energy: f32,
    pub fuel: i32,
    pub loadout: serde_json::Value,
    pub docked_station: Option<String>,
    pub last_docked_station: Option<String>,
    pub jump_state: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub character_id: Option<Uuid>,
    pub refresh_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub credits: i64,
    pub active_ship_id: Option<Uuid>,
}
