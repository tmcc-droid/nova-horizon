//! WebSocket gateway — full MVP gameplay loop.

use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use protocol::{
    AuthFail, AuthFailCode, AuthOk, ClockSyncRequest, ClockSyncResponse, CombatEventKind,
    DespawnReason, EntityDespawn, EntityKind, EntitySnapshot, EntitySpawn, EventCombat, InputFrame,
    JumpArrive, RefuelMode, SelfState, ServerNotice, SnapshotEntity, TradeSide, WireMessage,
    PROTOCOL_VERSION,
};
use sim::{CombatKind, ControlInput};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::gameplay;
use crate::sim_hub::SimEvent;
use crate::state::AppState;
use crate::tokens::tokens_match;

pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let ip = crate::rate_limit::ip_key(Some(addr.ip()));
    if !state.ws_limiter.check_and_record(&ip) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "rate limited",
        )
            .into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state, addr))
}

async fn handle_socket(socket: WebSocket, state: AppState, addr: SocketAddr) {
    let (mut sink, mut stream) = socket.split();

    let hello = tokio::time::timeout(Duration::from_secs(10), stream.next()).await;
    let Some(Ok(Message::Text(text))) = hello.ok().flatten() else {
        fail_auth(&mut sink, AuthFailCode::InvalidSession, "expected AuthHello").await;
        return;
    };
    let Ok(WireMessage::AuthHello(hello)) = protocol::decode_frame(&text) else {
        fail_auth(&mut sink, AuthFailCode::InvalidSession, "bad AuthHello").await;
        return;
    };
    if hello.client_protocol_v != PROTOCOL_VERSION {
        fail_auth(&mut sink, AuthFailCode::ProtocolMismatch, "protocol").await;
        return;
    }
    if hello.client_content_version != state.config.content_version {
        fail_auth(&mut sink, AuthFailCode::ContentMismatch, "content").await;
        return;
    }

    let session = match db::find_session(&state.pool, hello.session_id).await {
        Ok(Some(s)) => s,
        _ => {
            fail_auth(&mut sink, AuthFailCode::InvalidSession, "session").await;
            return;
        }
    };
    if session.revoked_at.is_some()
        || session.expires_at < chrono::Utc::now()
        || !tokens_match(&hello.connect_ticket, &session.refresh_hash)
    {
        fail_auth(&mut sink, AuthFailCode::InvalidSession, "ticket").await;
        return;
    }
    let Some(character_id) = session.character_id else {
        fail_auth(&mut sink, AuthFailCode::InvalidSession, "play first").await;
        return;
    };
    let account = match db::find_account_by_id(&state.pool, session.account_id).await {
        Ok(Some(a)) => a,
        _ => {
            fail_auth(&mut sink, AuthFailCode::InvalidSession, "account").await;
            return;
        }
    };
    if db::is_banned(account.banned_until) {
        fail_auth(&mut sink, AuthFailCode::Banned, "banned").await;
        return;
    }
    let character = match db::find_character(&state.pool, character_id).await {
        Ok(Some(c)) => c,
        _ => {
            fail_auth(&mut sink, AuthFailCode::InvalidSession, "character").await;
            return;
        }
    };
    let ship_id = match character.active_ship_id {
        Some(id) => id,
        None => {
            fail_auth(&mut sink, AuthFailCode::InvalidSession, "no ship").await;
            return;
        }
    };
    let ship = match db::find_ship(&state.pool, ship_id).await {
        Ok(Some(s)) => s,
        _ => {
            fail_auth(&mut sink, AuthFailCode::InvalidSession, "ship").await;
            return;
        }
    };

    let (conn_id, mut kick_rx) = state.claim_character_connection(character_id);
    let _ = state.sim.despawn_player(character_id).await;

    let spawn_req = sim::SpawnPlayer {
        ship_id,
        character_id,
        pilot_name: character.name.clone(),
        def_id: ship.def_id.clone(),
        system_id: ship.system_id.clone(),
        x: ship.pos_x,
        y: ship.pos_y,
        rot: ship.rot,
        shield: ship.shield,
        armor: ship.armor,
        energy: ship.energy,
        fuel: ship.fuel.max(0) as u32,
        docked_station: ship.docked_station.clone(),
        force_undock: ship.docked_station.is_some(),
    };

    let self_view = match state.sim.spawn_player(spawn_req).await {
        Ok(v) => v,
        Err(e) => {
            state.release_character_connection(character_id, conn_id);
            fail_auth(&mut sink, AuthFailCode::AlreadyOnline, &e).await;
            return;
        }
    };

    if send_msg(
        &mut sink,
        &WireMessage::AuthOk(AuthOk {
            v: PROTOCOL_VERSION,
            character_id,
            ship_id,
            system_id: self_view.system_id.clone(),
            content_version: state.config.content_version.clone(),
            server_protocol_v: PROTOCOL_VERSION,
            server_time_ms: now_ms(),
        }),
    )
    .await
    .is_err()
    {
        cleanup(&state, character_id, conn_id, ship_id).await;
        return;
    }

    // Galaxy chart is HTTP GET /galaxy (too large for a single WS text frame on Godot).

    // Seed stations in current system
    for st in state.sim.content.stations.values() {
        if st.system != self_view.system_id {
            continue;
        }
        let _ = send_msg(
            &mut sink,
            &WireMessage::EntitySpawn(EntitySpawn {
                v: PROTOCOL_VERSION,
                id: protocol::station_wire_id(&st.id),
                kind: EntityKind::Station,
                def_id: st.id.clone(),
                x: st.x,
                y: st.y,
                rot: 0.0,
                pilot_name: None,
                faction_id: None,
            }),
        )
        .await;
    }

    for other in state.sim.list_ships(Some(character_id)).await {
        let _ = send_msg(
            &mut sink,
            &WireMessage::EntitySpawn(EntitySpawn {
                v: PROTOCOL_VERSION,
                id: other.ship_id,
                kind: EntityKind::Ship,
                def_id: other.def_id,
                x: other.x,
                y: other.y,
                rot: other.rot,
                pilot_name: Some(other.pilot_name),
                faction_id: None,
            }),
        )
        .await;
    }

    info!(%addr, %character_id, %ship_id, "ws in world");

    let mut events = state.sim.subscribe();
    let mut last_snapshot_tick = 0u64;
    let mut last_persist = Instant::now();
    let mut last_system = self_view.system_id.clone();
    let mut credits = character.credits.max(0) as u64;
    let mut known_ships: std::collections::HashSet<uuid::Uuid> =
        [ship_id].into_iter().collect();

    loop {
        tokio::select! {
            kick = kick_rx.recv() => {
                let reason = kick.map(|k| k.reason).unwrap_or_else(|_| "replaced".into());
                let _ = send_msg(&mut sink, &notice("session_replaced", &reason)).await;
                break;
            }
            ev = events.recv() => {
                match ev {
                    Ok(SimEvent::Tick { tick, ships, combat }) => {
                        let me = ships.iter().find(|s| s.character_id == Some(character_id));
                        if let Some(me) = me {
                            // System change → JumpArrive + clear AOI + reseed stations
                            if me.system_id != last_system {
                                last_system = me.system_id.clone();
                                // Drop remote awareness from previous system
                                for old_id in known_ships.drain() {
                                    if old_id == ship_id {
                                        continue;
                                    }
                                    let _ = send_msg(
                                        &mut sink,
                                        &WireMessage::EntityDespawn(EntityDespawn {
                                            v: PROTOCOL_VERSION,
                                            id: old_id,
                                            reason: DespawnReason::Aoi,
                                        }),
                                    )
                                    .await;
                                }
                                known_ships.insert(ship_id);
                                let _ = send_msg(&mut sink, &WireMessage::JumpArrive(JumpArrive {
                                    v: PROTOCOL_VERSION,
                                    system_id: me.system_id.clone(),
                                    x: me.x, y: me.y, rot: me.rot,
                                })).await;
                                for st in state.sim.content.stations.values() {
                                    if st.system != me.system_id { continue; }
                                    let _ = send_msg(&mut sink, &WireMessage::EntitySpawn(EntitySpawn {
                                        v: PROTOCOL_VERSION,
                                        id: protocol::station_wire_id(&st.id),
                                        kind: EntityKind::Station,
                                        def_id: st.id.clone(),
                                        x: st.x, y: st.y, rot: 0.0,
                                        pilot_name: None, faction_id: None,
                                    })).await;
                                }
                            }

                            if send_msg(&mut sink, &WireMessage::SelfState(SelfState {
                                v: PROTOCOL_VERSION,
                                tick,
                                last_processed_input_seq: me.last_processed_input_seq,
                                x: me.x, y: me.y, rot: me.rot,
                                vx: me.vx, vy: me.vy, omega: me.omega,
                                shield: me.shield, armor: me.armor, energy: me.energy,
                                fuel: me.fuel,
                                credits: Some(credits),
                                docked_station_id: me.docked_station.as_ref().map(|s| protocol::station_wire_id(s)),
                                invuln: me.invuln,
                                flags: me.flags,
                            })).await.is_err() { break; }

                            // Death cargo loss once when armor recovered after death event
                            for c in &combat {
                                if c.kind == CombatKind::Death && c.target_id == Some(ship_id) {
                                    let _ = db::apply_death_cargo_loss(&state.pool, ship_id).await;
                                }
                            }
                        }

                        for c in combat {
                            let kind = match c.kind {
                                CombatKind::Hit => CombatEventKind::Hit,
                                CombatKind::Miss => CombatEventKind::Miss,
                                CombatKind::Death => CombatEventKind::Death,
                                CombatKind::FireDenied => CombatEventKind::FireDenied,
                            };
                            let _ = send_msg(&mut sink, &WireMessage::EventCombat(EventCombat {
                                v: PROTOCOL_VERSION,
                                kind,
                                source_id: c.source_id,
                                target_id: c.target_id,
                                weapon_def: c.weapon_def,
                                x: c.x, y: c.y,
                                damage: c.damage,
                                reason: c.reason,
                            })).await;
                        }

                        if tick.saturating_sub(last_snapshot_tick) >= 2 {
                            last_snapshot_tick = tick;
                            let interest = if let Some(me) = me {
                                ships.iter().filter(|s| s.system_id == me.system_id && s.ship_id != ship_id).cloned().collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            };
                            // Spawn unknown
                            for s in &interest {
                                if known_ships.insert(s.ship_id) {
                                    let _ = send_msg(&mut sink, &WireMessage::EntitySpawn(EntitySpawn {
                                        v: PROTOCOL_VERSION,
                                        id: s.ship_id,
                                        kind: EntityKind::Ship,
                                        def_id: s.def_id.clone(),
                                        x: s.x, y: s.y, rot: s.rot,
                                        pilot_name: Some(s.pilot_name.clone()),
                                        faction_id: None,
                                    })).await;
                                }
                            }
                            let entities: Vec<SnapshotEntity> = interest.iter().map(|s| SnapshotEntity {
                                id: s.ship_id,
                                x: s.x, y: s.y, rot: s.rot,
                                vx: Some(s.vx), vy: Some(s.vy),
                                shield_frac: Some((s.shield / 80.0).clamp(0.0, 1.0)),
                                flags: s.flags,
                            }).collect();
                            if !entities.is_empty() {
                                let _ = send_msg(&mut sink, &WireMessage::EntitySnapshot(EntitySnapshot {
                                    v: PROTOCOL_VERSION, tick, entities,
                                })).await;
                            }
                        }

                        if last_persist.elapsed() > Duration::from_secs(10) {
                            last_persist = Instant::now();
                            if let Some(me) = me {
                                let _ = db::persist_ship_state(
                                    &state.pool, ship_id, &me.system_id,
                                    me.x, me.y, me.rot, me.shield, me.armor, me.energy, me.fuel,
                                    me.docked_station.as_deref(),
                                    me.docked_station.as_deref(),
                                ).await;
                            }
                        }
                    }
                    Ok(SimEvent::Spawned(view)) => {
                        if view.character_id == Some(character_id) { continue; }
                        known_ships.insert(view.ship_id);
                        let _ = send_msg(&mut sink, &WireMessage::EntitySpawn(EntitySpawn {
                            v: PROTOCOL_VERSION,
                            id: view.ship_id,
                            kind: EntityKind::Ship,
                            def_id: view.def_id,
                            x: view.x, y: view.y, rot: view.rot,
                            pilot_name: Some(view.pilot_name),
                            faction_id: None,
                        })).await;
                    }
                    Ok(SimEvent::Despawned { ship_id: id, .. }) => {
                        known_ships.remove(&id);
                        let _ = send_msg(&mut sink, &WireMessage::EntityDespawn(EntityDespawn {
                            v: PROTOCOL_VERSION, id, reason: DespawnReason::Aoi,
                        })).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match protocol::decode_frame(&text) {
                            Ok(WireMessage::InputFrame(InputFrame { input_seq, thrust, turn, fire_mask, target_id, .. })) => {
                                state.sim.apply_input(character_id, ControlInput {
                                    input_seq, thrust, turn, fire_mask, target_id,
                                }).await;
                            }
                            Ok(WireMessage::ClockSyncRequest(ClockSyncRequest { client_send_ms, .. })) => {
                                let srv = now_ms();
                                let _ = send_msg(&mut sink, &WireMessage::ClockSyncResponse(ClockSyncResponse {
                                    v: PROTOCOL_VERSION, client_send_ms, server_recv_ms: srv, server_send_ms: now_ms(),
                                })).await;
                            }
                            Ok(WireMessage::DockRequest(d)) => {
                                let msg = gameplay::handle_dock(&state, character_id, d.request_id, d.station_id).await;
                                if let WireMessage::DockResult(ref r) = msg {
                                    if r.ok {
                                        if let Some(cid) = gameplay::station_content_from_wire(&state, d.station_id) {
                                            if let Some(menu) = gameplay::build_station_menu(&state, &cid).await {
                                                let _ = send_msg(&mut sink, &WireMessage::StationMenu(menu)).await;
                                            }
                                        }
                                    }
                                }
                                let _ = send_msg(&mut sink, &msg).await;
                            }
                            Ok(WireMessage::UndockRequest(u)) => {
                                let msg = gameplay::handle_undock(&state, character_id, u.request_id).await;
                                let _ = send_msg(&mut sink, &msg).await;
                            }
                            Ok(WireMessage::TradeExecute(t)) => {
                                let buy = matches!(t.side, TradeSide::Buy);
                                let msg = gameplay::handle_trade(
                                    &state, character_id, ship_id, t.request_id, t.station_id,
                                    t.commodity_id, buy, t.quantity,
                                ).await;
                                if let WireMessage::TradeResult(ref r) = msg {
                                    if let Some(c) = r.credits { credits = c; }
                                }
                                let _ = send_msg(&mut sink, &msg).await;
                            }
                            Ok(WireMessage::RefuelRequest(r)) => {
                                let fill = matches!(r.mode, RefuelMode::Fill);
                                let msg = gameplay::handle_refuel(
                                    &state, character_id, ship_id, r.request_id, r.station_id, fill, r.quantity,
                                ).await;
                                if let WireMessage::RefuelResult(ref rr) = msg {
                                    if let Some(c) = rr.credits { credits = c; }
                                }
                                let _ = send_msg(&mut sink, &msg).await;
                            }
                            Ok(WireMessage::HyperspaceRequest(h)) => {
                                for m in gameplay::handle_jump(&state, character_id, h.request_id, h.dest_system_id).await {
                                    let _ = send_msg(&mut sink, &m).await;
                                }
                            }
                            Ok(other) => debug!(msg = other.type_name(), "unhandled"),
                            Err(e) => warn!(error = %e, "bad frame"),
                        }
                    }
                    Some(Ok(Message::Ping(p))) => { let _ = sink.send(Message::Pong(p)).await; }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => { warn!(error = %e, "ws"); break; }
                }
            }
        }
    }

    cleanup(&state, character_id, conn_id, ship_id).await;
    info!(%character_id, "ws disconnected");
}

async fn cleanup(state: &AppState, character_id: uuid::Uuid, conn_id: uuid::Uuid, ship_id: uuid::Uuid) {
    if let Some(view) = state.sim.despawn_player(character_id).await {
        let _ = db::persist_ship_state(
            &state.pool,
            ship_id,
            &view.system_id,
            view.x,
            view.y,
            view.rot,
            view.shield,
            view.armor,
            view.energy,
            view.fuel,
            view.docked_station.as_deref(),
            view.docked_station.as_deref(),
        )
        .await;
    }
    state.release_character_connection(character_id, conn_id);
}

fn notice(code: &str, message: &str) -> WireMessage {
    WireMessage::ServerNotice(ServerNotice {
        v: PROTOCOL_VERSION,
        level: protocol::NoticeLevel::Warn,
        code: code.into(),
        message: message.into(),
    })
}

async fn fail_auth(
    sink: &mut (impl SinkExt<Message> + Unpin),
    code: AuthFailCode,
    message: &str,
) {
    let _ = send_msg(
        sink,
        &WireMessage::AuthFail(AuthFail {
            v: PROTOCOL_VERSION,
            code,
            message: message.into(),
        }),
    )
    .await;
}

async fn send_msg(
    sink: &mut (impl SinkExt<Message> + Unpin),
    msg: &WireMessage,
) -> Result<(), ()> {
    let text = protocol::encode_frame(msg).map_err(|_| ())?;
    sink.send(Message::Text(text.into())).await.map_err(|_| ())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
