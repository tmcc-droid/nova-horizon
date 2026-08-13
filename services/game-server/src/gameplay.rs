//! WS gameplay handlers: dock, trade, refuel, jump, shipyard.

use protocol::{
    CargoStackWire, DockResult, DockResultCode, JumpCountdown, JumpRejected, JumpRejectedCode,
    MarketRow, RefuelResult, RefuelResultCode, StationMenu, TradeResult, TradeResultCode,
    WireMessage, PROTOCOL_VERSION,
};
use sim::DockFail;
use uuid::Uuid;

use crate::state::AppState;

pub fn station_content_from_wire(state: &AppState, wire_id: Uuid) -> Option<String> {
    for id in state.sim.content.stations.keys() {
        if protocol::station_wire_id(id) == wire_id {
            return Some(id.clone());
        }
    }
    None
}

pub async fn handle_dock(
    state: &AppState,
    character_id: Uuid,
    request_id: Uuid,
    station_wire: Uuid,
) -> WireMessage {
    let Some(content_id) = station_content_from_wire(state, station_wire) else {
        return WireMessage::DockResult(DockResult {
            v: PROTOCOL_VERSION,
            request_id,
            ok: false,
            code: DockResultCode::InvalidStation,
            station_id: None,
        });
    };
    let fail = state.sim.dock(character_id, content_id.clone()).await;
    let (ok, code) = match fail {
        DockFail::Ok => (true, DockResultCode::Ok),
        DockFail::TooFar => (false, DockResultCode::TooFar),
        DockFail::TooFast => (false, DockResultCode::TooFast),
        DockFail::AlreadyDocked => (false, DockResultCode::AlreadyDocked),
        DockFail::Dead => (false, DockResultCode::Dead),
        DockFail::InvalidStation | DockFail::WrongSystem => (false, DockResultCode::InvalidStation),
    };
    WireMessage::DockResult(DockResult {
        v: PROTOCOL_VERSION,
        request_id,
        ok,
        code,
        station_id: ok.then_some(station_wire),
    })
}

pub async fn handle_undock(state: &AppState, character_id: Uuid, request_id: Uuid) -> WireMessage {
    let ok = state.sim.undock(character_id).await;
    WireMessage::DockResult(DockResult {
        v: PROTOCOL_VERSION,
        request_id,
        ok,
        code: if ok {
            DockResultCode::Ok
        } else {
            DockResultCode::Internal
        },
        station_id: None,
    })
}

pub async fn build_station_menu(state: &AppState, station_content_id: &str) -> Option<StationMenu> {
    let st = state.sim.content.stations.get(station_content_id)?;
    let market_db = db::list_market(&state.pool, station_content_id)
        .await
        .ok()?;
    let market: Vec<MarketRow> = market_db
        .into_iter()
        .map(|m| {
            let mass = state
                .sim
                .content
                .commodities
                .get(&m.commodity_id)
                .map(|c| c.mass_per_unit)
                .unwrap_or(1.0);
            MarketRow {
                commodity_id: m.commodity_id,
                stock: m.stock,
                buy_price: m.buy_price,
                sell_price: m.sell_price,
                mass_per_unit: mass,
            }
        })
        .collect();
    Some(StationMenu {
        v: PROTOCOL_VERSION,
        station_id: protocol::station_wire_id(station_content_id),
        def_id: station_content_id.into(),
        services: vec![
            "market".into(),
            "shipyard".into(),
            "refuel".into(),
        ],
        fuel_price_per_unit: st.fuel_price_per_unit,
        fuel_max_purchase: None,
        market,
    })
}

pub async fn handle_trade(
    state: &AppState,
    character_id: Uuid,
    ship_id: Uuid,
    request_id: Uuid,
    station_wire: Uuid,
    commodity_id: String,
    side_buy: bool,
    quantity: u32,
) -> WireMessage {
    let view = state.sim.view(character_id).await;
    let Some(view) = view else {
        return trade_fail(request_id, TradeResultCode::Internal);
    };
    let Some(docked) = view.docked_station.clone() else {
        return trade_fail(request_id, TradeResultCode::NotDocked);
    };
    let Some(st_content) = station_content_from_wire(state, station_wire) else {
        return trade_fail(request_id, TradeResultCode::InvalidItem);
    };
    if st_content != docked {
        return trade_fail(request_id, TradeResultCode::NotDocked);
    }
    let mass = state
        .sim
        .content
        .commodities
        .get(&commodity_id)
        .map(|c| c.mass_per_unit)
        .unwrap_or(1.0);
    let cap = state
        .sim
        .content
        .ship(&view.def_id)
        .map(|s| s.cargo_capacity_mass as f64)
        .unwrap_or(25.0);

    match db::execute_trade(
        &state.pool,
        character_id,
        ship_id,
        &st_content,
        &commodity_id,
        side_buy,
        quantity as i32,
        mass,
        cap,
        sim::DAILY_TRADE_CAP,
    )
    .await
    {
        Ok((credits, cargo, mass_used)) => WireMessage::TradeResult(TradeResult {
            v: PROTOCOL_VERSION,
            request_id,
            ok: true,
            code: TradeResultCode::Ok,
            credits: Some(credits as u64),
            cargo: Some(
                cargo
                    .into_iter()
                    .map(|c| CargoStackWire {
                        commodity_id: c.commodity_id,
                        quantity: c.quantity as u32,
                    })
                    .collect(),
            ),
            cargo_mass_used: Some(mass_used),
            cargo_capacity_mass: Some(cap),
            market_version: None,
        }),
        Err(db::DbError::Other(s)) => {
            let code = match s.as_str() {
                "insufficient_funds" => TradeResultCode::InsufficientFunds,
                "insufficient_stock" => TradeResultCode::InsufficientStock,
                "insufficient_cargo" => TradeResultCode::InsufficientCargo,
                "daily_cap" => TradeResultCode::DailyCap,
                "invalid_item" => TradeResultCode::InvalidItem,
                _ => TradeResultCode::Internal,
            };
            trade_fail(request_id, code)
        }
        Err(_) => trade_fail(request_id, TradeResultCode::Internal),
    }
}

fn trade_fail(request_id: Uuid, code: TradeResultCode) -> WireMessage {
    WireMessage::TradeResult(TradeResult {
        v: PROTOCOL_VERSION,
        request_id,
        ok: false,
        code,
        credits: None,
        cargo: None,
        cargo_mass_used: None,
        cargo_capacity_mass: None,
        market_version: None,
    })
}

pub async fn handle_refuel(
    state: &AppState,
    character_id: Uuid,
    ship_id: Uuid,
    request_id: Uuid,
    station_wire: Uuid,
    fill: bool,
    quantity: Option<u32>,
) -> WireMessage {
    let view = match state.sim.view(character_id).await {
        Some(v) => v,
        None => {
            return refuel_fail(request_id, RefuelResultCode::Internal);
        }
    };
    let Some(docked) = view.docked_station.clone() else {
        return refuel_fail(request_id, RefuelResultCode::NotDocked);
    };
    let Some(st_content) = station_content_from_wire(state, station_wire) else {
        return refuel_fail(request_id, RefuelResultCode::InvalidStation);
    };
    if st_content != docked {
        return refuel_fail(request_id, RefuelResultCode::NotDocked);
    }
    let price = state
        .sim
        .content
        .stations
        .get(&st_content)
        .map(|s| s.fuel_price_per_unit)
        .unwrap_or(50);
    let max_fuel = state
        .sim
        .content
        .ship(&view.def_id)
        .map(|s| s.max_fuel.max(0) as u32)
        .unwrap_or(10);
    if view.fuel >= max_fuel {
        return refuel_fail(request_id, RefuelResultCode::Full);
    }
    let units = if fill {
        max_fuel.saturating_sub(view.fuel)
    } else {
        quantity.unwrap_or(0).min(max_fuel.saturating_sub(view.fuel))
    };
    if units == 0 {
        return refuel_fail(request_id, RefuelResultCode::Full);
    }
    let new_fuel = view.fuel + units;
    match db::refuel(
        &state.pool,
        character_id,
        ship_id,
        price,
        units,
        new_fuel,
    )
    .await
    {
        Ok((credits, fuel)) => {
            // Best-effort: persist already updated fuel; sim fuel via despawn/spawn is heavy —
            // client sees SelfState fuel from sim; update by jump/dock persist path.
            // Apply by re-reading: soft set through undock not available — patch view by re-jump.
            // For MVP, update DB and note sim fuel lags until reconnect unless we add SetFuel.
            state.sim.set_fuel(character_id, fuel).await;
            WireMessage::RefuelResult(RefuelResult {
                v: PROTOCOL_VERSION,
                request_id,
                ok: true,
                code: RefuelResultCode::Ok,
                fuel: Some(fuel),
                credits: Some(credits as u64),
                units_bought: Some(units),
            })
        }
        Err(db::DbError::Other(s)) if s == "insufficient_funds" => {
            refuel_fail(request_id, RefuelResultCode::InsufficientFunds)
        }
        Err(_) => refuel_fail(request_id, RefuelResultCode::Internal),
    }
}

fn refuel_fail(request_id: Uuid, code: RefuelResultCode) -> WireMessage {
    WireMessage::RefuelResult(RefuelResult {
        v: PROTOCOL_VERSION,
        request_id,
        ok: false,
        code,
        fuel: None,
        credits: None,
        units_bought: None,
    })
}

pub async fn handle_jump(
    state: &AppState,
    character_id: Uuid,
    request_id: Uuid,
    dest: String,
) -> Vec<WireMessage> {
    match state.sim.jump(character_id, dest.clone()).await {
        Ok(()) => vec![WireMessage::JumpCountdown(JumpCountdown {
            v: PROTOCOL_VERSION,
            request_id,
            dest_system_id: dest,
            seconds: sim::HYPERSPACE_CHANNEL_S,
        })],
        Err(e) => {
            let code = match e.as_str() {
                "no_fuel" => JumpRejectedCode::NoFuel,
                "no_link" => JumpRejectedCode::NoLink,
                "docked" => JumpRejectedCode::Docked,
                "already" => JumpRejectedCode::AlreadyJumping,
                _ => JumpRejectedCode::Cancelled,
            };
            vec![WireMessage::JumpRejected(JumpRejected {
                v: PROTOCOL_VERSION,
                request_id,
                code,
            })]
        }
    }
}
