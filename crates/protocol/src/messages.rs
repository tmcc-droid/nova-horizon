//! MVP message catalog (flat `t` + `v` envelope).
//! Field units: positions/radii in **wu**, angles in **radians**, credits integer.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Client → server ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthHello {
    pub v: u16,
    pub session_id: Uuid,
    /// Opaque session / refresh material from REST auth (PR-04).
    pub connect_ticket: String,
    pub client_content_version: String,
    pub client_protocol_v: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockSyncRequest {
    pub v: u16,
    pub client_send_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputFrame {
    pub v: u16,
    pub input_seq: u32,
    /// Thrust in [-1, 1].
    pub thrust: f32,
    /// Turn in [-1, 1].
    pub turn: f32,
    /// Bit0 = primary fire group, etc.
    pub fire_mask: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<Uuid>,
    // NO position, velocity, or damage fields — server owns transform.
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockRequest {
    pub v: u16,
    pub request_id: Uuid,
    /// Wire station id (UUID v5 of content id).
    pub station_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndockRequest {
    pub v: u16,
    pub request_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradeExecute {
    pub v: u16,
    pub request_id: Uuid,
    /// Wire station UUID v5; server maps → content string.
    pub station_id: Uuid,
    pub commodity_id: String,
    pub side: TradeSide,
    /// Must be > 0.
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefuelMode {
    Fill,
    Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefuelRequest {
    pub v: u16,
    pub request_id: Uuid,
    pub station_id: Uuid,
    pub mode: RefuelMode,
    /// Required when `mode == Quantity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyperspaceRequest {
    pub v: u16,
    pub request_id: Uuid,
    pub dest_system_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSend {
    pub v: u16,
    pub request_id: Uuid,
    pub channel: String,
    /// Max 256 chars (enforced server-side).
    pub text: String,
}

// ─── Server → client ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AuthFailCode {
    InvalidSession,
    Banned,
    AlreadyOnline,
    ContentMismatch,
    ProtocolMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthOk {
    pub v: u16,
    pub character_id: Uuid,
    pub ship_id: Uuid,
    pub system_id: String,
    pub content_version: String,
    pub server_protocol_v: u16,
    pub server_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthFail {
    pub v: u16,
    pub code: AuthFailCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockSyncResponse {
    pub v: u16,
    pub client_send_ms: u64,
    pub server_recv_ms: u64,
    pub server_send_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfState {
    pub v: u16,
    pub tick: u64,
    pub last_processed_input_seq: u32,
    /// Position wu.
    pub x: f64,
    pub y: f64,
    /// Radians.
    pub rot: f32,
    pub vx: f64,
    pub vy: f64,
    pub omega: f32,
    pub shield: f32,
    pub armor: f32,
    pub energy: f32,
    pub fuel: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docked_station_id: Option<Uuid>,
    pub invuln: bool,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Ship,
    Station,
    Projectile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySpawn {
    pub v: u16,
    pub id: Uuid,
    pub kind: EntityKind,
    pub def_id: String,
    pub x: f64,
    pub y: f64,
    pub rot: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pilot_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DespawnReason {
    Aoi,
    Death,
    Dock,
    Jump,
    Lifetime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDespawn {
    pub v: u16,
    pub id: Uuid,
    pub reason: DespawnReason,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotEntity {
    pub id: Uuid,
    pub x: f64,
    pub y: f64,
    pub rot: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vx: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vy: Option<f64>,
    /// 0..1 for remote ships.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield_frac: Option<f32>,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub v: u16,
    pub tick: u64,
    pub entities: Vec<SnapshotEntity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CombatEventKind {
    Hit,
    Miss,
    Death,
    ShieldBreak,
    FireDenied,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventCombat {
    pub v: u16,
    pub kind: CombatEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_def: Option<String>,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DockResultCode {
    Ok,
    TooFar,
    TooFast,
    AlreadyDocked,
    Dead,
    InvalidStation,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockResult {
    pub v: u16,
    pub request_id: Uuid,
    pub ok: bool,
    pub code: DockResultCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketRow {
    pub commodity_id: String,
    pub stock: i32,
    pub buy_price: i32,
    pub sell_price: i32,
    pub mass_per_unit: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StationMenu {
    pub v: u16,
    pub station_id: Uuid,
    /// Content id, e.g. `st.earth_orbit`.
    pub def_id: String,
    pub services: Vec<String>,
    pub fuel_price_per_unit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_max_purchase: Option<u32>,
    pub market: Vec<MarketRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TradeResultCode {
    Ok,
    InsufficientFunds,
    InsufficientStock,
    InsufficientCargo,
    NotDocked,
    InvalidItem,
    RateLimited,
    DailyCap,
    StaleMarket,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoStackWire {
    pub commodity_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeResult {
    pub v: u16,
    pub request_id: Uuid,
    pub ok: bool,
    pub code: TradeResultCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo: Option<Vec<CargoStackWire>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_mass_used: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_capacity_mass: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_version: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum RefuelResultCode {
    Ok,
    NotDocked,
    InsufficientFunds,
    Full,
    Unavailable,
    InvalidStation,
    RateLimited,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefuelResult {
    pub v: u16,
    pub request_id: Uuid,
    pub ok: bool,
    pub code: RefuelResultCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units_bought: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JumpCountdown {
    pub v: u16,
    pub request_id: Uuid,
    pub dest_system_id: String,
    pub seconds: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JumpArrive {
    pub v: u16,
    pub system_id: String,
    pub x: f64,
    pub y: f64,
    pub rot: f32,
}

/// Full galaxy graph for the jump map (sent once at join).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalaxyMap {
    pub v: u16,
    pub current_system_id: String,
    pub systems: Vec<GalaxySystemWire>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalaxySystemWire {
    pub id: String,
    pub name: String,
    pub map_x: f32,
    pub map_y: f32,
    /// `hub` | `populated` | `transit`
    pub kind: String,
    pub links: Vec<GalaxyLinkWire>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GalaxyLinkWire {
    pub to: String,
    pub fuel_cost: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum JumpRejectedCode {
    NoFuel,
    NoLink,
    Full,
    DestDown,
    AlreadyJumping,
    Docked,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JumpRejected {
    pub v: u16,
    pub request_id: Uuid,
    pub code: JumpRejectedCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerNotice {
    pub v: u16,
    pub level: NoticeLevel,
    pub code: String,
    pub message: String,
}

// ─── Tagged union ────────────────────────────────────────────────────────────

/// All MVP WebSocket messages. Serde tagged via field `t`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum WireMessage {
    // Client → server
    AuthHello(AuthHello),
    ClockSyncRequest(ClockSyncRequest),
    InputFrame(InputFrame),
    DockRequest(DockRequest),
    UndockRequest(UndockRequest),
    TradeExecute(TradeExecute),
    RefuelRequest(RefuelRequest),
    HyperspaceRequest(HyperspaceRequest),
    ChatSend(ChatSend),
    // Server → client
    AuthOk(AuthOk),
    AuthFail(AuthFail),
    ClockSyncResponse(ClockSyncResponse),
    SelfState(SelfState),
    EntitySpawn(EntitySpawn),
    EntityDespawn(EntityDespawn),
    EntitySnapshot(EntitySnapshot),
    EventCombat(EventCombat),
    DockResult(DockResult),
    StationMenu(StationMenu),
    TradeResult(TradeResult),
    RefuelResult(RefuelResult),
    JumpCountdown(JumpCountdown),
    JumpArrive(JumpArrive),
    JumpRejected(JumpRejected),
    GalaxyMap(GalaxyMap),
    ServerNotice(ServerNotice),
}

impl WireMessage {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::AuthHello(_) => "AuthHello",
            Self::ClockSyncRequest(_) => "ClockSyncRequest",
            Self::InputFrame(_) => "InputFrame",
            Self::DockRequest(_) => "DockRequest",
            Self::UndockRequest(_) => "UndockRequest",
            Self::TradeExecute(_) => "TradeExecute",
            Self::RefuelRequest(_) => "RefuelRequest",
            Self::HyperspaceRequest(_) => "HyperspaceRequest",
            Self::ChatSend(_) => "ChatSend",
            Self::AuthOk(_) => "AuthOk",
            Self::AuthFail(_) => "AuthFail",
            Self::ClockSyncResponse(_) => "ClockSyncResponse",
            Self::SelfState(_) => "SelfState",
            Self::EntitySpawn(_) => "EntitySpawn",
            Self::EntityDespawn(_) => "EntityDespawn",
            Self::EntitySnapshot(_) => "EntitySnapshot",
            Self::EventCombat(_) => "EventCombat",
            Self::DockResult(_) => "DockResult",
            Self::StationMenu(_) => "StationMenu",
            Self::TradeResult(_) => "TradeResult",
            Self::RefuelResult(_) => "RefuelResult",
            Self::JumpCountdown(_) => "JumpCountdown",
            Self::JumpArrive(_) => "JumpArrive",
            Self::JumpRejected(_) => "JumpRejected",
            Self::GalaxyMap(_) => "GalaxyMap",
            Self::ServerNotice(_) => "ServerNotice",
        }
    }
}
