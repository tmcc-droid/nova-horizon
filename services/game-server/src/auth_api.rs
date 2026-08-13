//! REST auth + character APIs (PR-04).

use axum::extract::{ConnectInfo, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

// ConnectInfo is required on register/login for rate-limit keys.

use crate::jwt_util::{issue_access_token, parse_access_token};
use crate::password::{hash_password, verify_password};
use crate::state::AppState;
use crate::tokens::{hash_token, mint_opaque_token, tokens_match};

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub session_id: Uuid,
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCharacterRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct PlayRequest {
    pub session_id: Uuid,
    pub refresh_token: String,
    pub character_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub session_id: Uuid,
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct PlayResponse {
    pub session_id: Uuid,
    pub connect_ticket: String,
    pub character_id: Uuid,
    pub ship_id: Uuid,
    pub system_id: String,
    pub content_version: String,
    pub ws_path: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub error: String,
    pub message: String,
}

pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Conflict(String),
    Forbidden(String),
    RateLimited,
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m),
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m),
            Self::Conflict(m) => (StatusCode::CONFLICT, "conflict", m),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many requests".into(),
            ),
            Self::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", m),
        };
        (
            status,
            Json(ErrorBody {
                error: error.into(),
                message,
            }),
        )
            .into_response()
    }
}

fn validate_email(email: &str) -> Result<(), ApiError> {
    let e = email.trim();
    if e.len() < 3 || !e.contains('@') {
        return Err(ApiError::BadRequest("invalid email".into()));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), ApiError> {
    if password.len() < 8 {
        return Err(ApiError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    let n = name.trim();
    if n.len() < 2 || n.len() > 24 {
        return Err(ApiError::BadRequest(
            "name must be 2–24 characters".into(),
        ));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ApiError::BadRequest(
            "name may only contain A–Z, 0–9, _, -".into(),
        ));
    }
    Ok(())
}

pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let ip = crate::rate_limit::ip_key(Some(addr.ip()));
    if !state.auth_limiter.check_and_record(&ip) {
        return Err(ApiError::RateLimited);
    }
    validate_email(&body.email)?;
    validate_password(&body.password)?;

    let hash = hash_password(&body.password).map_err(ApiError::Internal)?;
    match db::create_account(&state.pool, body.email.trim(), &hash).await {
        Ok(account) => {
            let (access, session_id, refresh) = issue_session(&state, account.id).await?;
            Ok((
                StatusCode::CREATED,
                Json(TokenResponse {
                    access_token: access,
                    token_type: "Bearer",
                    expires_in: state.config.access_ttl_secs,
                    session_id,
                    refresh_token: refresh,
                }),
            ))
        }
        Err(db::DbError::Other(s)) if s == "unique_violation" => {
            Err(ApiError::Conflict("email already registered".into()))
        }
        Err(e) => Err(ApiError::Internal(e.to_string())),
    }
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let ip = crate::rate_limit::ip_key(Some(addr.ip()));
    if !state.auth_limiter.check_and_record(&ip) {
        return Err(ApiError::RateLimited);
    }
    validate_email(&body.email)?;

    let account = db::find_account_by_email(&state.pool, body.email.trim())
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Unauthorized("invalid credentials".into()))?;

    if db::is_banned(account.banned_until) {
        return Err(ApiError::Forbidden("account banned".into()));
    }

    let Some(ref ph) = account.password_hash else {
        return Err(ApiError::Unauthorized("invalid credentials".into()));
    };
    if !verify_password(&body.password, ph) {
        return Err(ApiError::Unauthorized("invalid credentials".into()));
    }

    let (access, session_id, refresh) = issue_session(&state, account.id).await?;
    Ok(Json(TokenResponse {
        access_token: access,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        session_id,
        refresh_token: refresh,
    }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let session = db::find_session(&state.pool, body.session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Unauthorized("invalid session".into()))?;

    if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
        return Err(ApiError::Unauthorized("session expired".into()));
    }
    if !tokens_match(&body.refresh_token, &session.refresh_hash) {
        return Err(ApiError::Unauthorized("invalid refresh token".into()));
    }

    let account = db::find_account_by_id(&state.pool, session.account_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Unauthorized("account missing".into()))?;
    if db::is_banned(account.banned_until) {
        return Err(ApiError::Forbidden("account banned".into()));
    }

    let new_refresh = mint_opaque_token();
    let new_hash = hash_token(&new_refresh);
    db::rotate_session_refresh(
        &state.pool,
        session.id,
        &new_hash,
        state.config.session_ttl_hours,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let access = issue_access_token(
        session.account_id,
        &state.config.jwt_secret,
        state.config.access_ttl_secs,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(TokenResponse {
        access_token: access,
        token_type: "Bearer",
        expires_in: state.config.access_ttl_secs,
        session_id: session.id,
        refresh_token: new_refresh,
    }))
}

pub async fn list_characters(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<db::CharacterSummary>>, ApiError> {
    let account_id = require_bearer(&state, &headers)?;
    let list = db::list_characters(&state.pool, account_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(list))
}

pub async fn create_character(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateCharacterRequest>,
) -> Result<(StatusCode, Json<db::CharacterSummary>), ApiError> {
    let account_id = require_bearer(&state, &headers)?;
    validate_name(&body.name)?;

    let starter = db::default_starter_ship();
    match db::create_character_with_ship(&state.pool, account_id, body.name.trim(), &starter)
        .await
    {
        Ok((c, _ship)) => Ok((
            StatusCode::CREATED,
            Json(db::CharacterSummary {
                id: c.id,
                name: c.name,
                credits: c.credits,
                active_ship_id: c.active_ship_id,
            }),
        )),
        Err(db::DbError::Other(s)) if s == "unique_violation" => {
            Err(ApiError::Conflict("character name taken".into()))
        }
        Err(e) => Err(ApiError::Internal(e.to_string())),
    }
}

/// Bind session to a character and return WS connect credentials.
pub async fn play(
    State(state): State<AppState>,
    Json(body): Json<PlayRequest>,
) -> Result<Json<PlayResponse>, ApiError> {
    let session = db::find_session(&state.pool, body.session_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Unauthorized("invalid session".into()))?;

    if session.revoked_at.is_some() || session.expires_at < chrono::Utc::now() {
        return Err(ApiError::Unauthorized("session expired".into()));
    }
    if !tokens_match(&body.refresh_token, &session.refresh_hash) {
        return Err(ApiError::Unauthorized("invalid refresh token".into()));
    }

    let character = db::find_character(&state.pool, body.character_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest("character not found".into()))?;

    if character.account_id != session.account_id {
        return Err(ApiError::Forbidden("character not owned by account".into()));
    }

    let ship_id = character
        .active_ship_id
        .ok_or_else(|| ApiError::Internal("character has no active ship".into()))?;
    let ship = db::find_ship(&state.pool, ship_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Internal("active ship missing".into()))?;

    db::bind_session_character(&state.pool, session.id, character.id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // connect_ticket is the current refresh token (hashed check on WS).
    Ok(Json(PlayResponse {
        session_id: session.id,
        connect_ticket: body.refresh_token,
        character_id: character.id,
        ship_id,
        system_id: ship.system_id,
        content_version: state.config.content_version.clone(),
        ws_path: "/ws",
    }))
}

/// Fixed MMO galaxy for the jump map — served over HTTP (not WS; ~700KB).
pub async fn galaxy_chart(State(state): State<AppState>) -> Json<serde_json::Value> {
    let systems: Vec<serde_json::Value> = state
        .sim
        .content
        .galaxy_snapshot()
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "map_x": s.map_x,
                "map_y": s.map_y,
                "kind": s.kind,
                "links": s.links.iter().map(|l| serde_json::json!({
                    "to": l.to,
                    "fuel_cost": l.fuel_cost,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Json(serde_json::json!({
        "t": "GalaxyMap",
        "v": protocol::PROTOCOL_VERSION,
        "systems": systems,
    }))
}

pub async fn content_manifest(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": state.config.content_version,
        "protocol_v": protocol::PROTOCOL_VERSION,
    }))
}

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match db::ping(&state.pool).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        ),
    }
}

async fn issue_session(
    state: &AppState,
    account_id: Uuid,
) -> Result<(String, Uuid, String), ApiError> {
    let refresh = mint_opaque_token();
    let refresh_hash = hash_token(&refresh);
    let session = db::create_session(
        &state.pool,
        account_id,
        &refresh_hash,
        state.config.session_ttl_hours,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let access = issue_access_token(
        account_id,
        &state.config.jwt_secret,
        state.config.access_ttl_secs,
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok((access, session.id, refresh))
}

fn require_bearer(state: &AppState, headers: &axum::http::HeaderMap) -> Result<Uuid, ApiError> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization".into()))?;
    let token = auth
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("expected Bearer token".into()))?;
    parse_access_token(token, &state.config.jwt_secret)
        .map_err(|_| ApiError::Unauthorized("invalid access token".into()))
}
