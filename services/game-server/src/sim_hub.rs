//! In-process 20 Hz sim + commands (flight, dock, combat, jump).

use std::sync::Arc;
use std::time::Duration;

use content::ContentRegistry;
use sim::{
    CombatEvent, ControlInput, DockFail, ShipView, SpawnPlayer, World, DT, TICK_HZ,
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug)]
pub enum SimCommand {
    Spawn {
        req: SpawnPlayer,
        reply: oneshot::Sender<Result<ShipView, String>>,
    },
    Despawn {
        character_id: Uuid,
        reply: oneshot::Sender<Option<ShipView>>,
    },
    Input {
        character_id: Uuid,
        input: ControlInput,
    },
    Dock {
        character_id: Uuid,
        station_content_id: String,
        reply: oneshot::Sender<DockFail>,
    },
    Undock {
        character_id: Uuid,
        reply: oneshot::Sender<bool>,
    },
    Jump {
        character_id: Uuid,
        dest: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Snapshot {
        character_id: Option<Uuid>,
        reply: oneshot::Sender<Vec<ShipView>>,
    },
    View {
        character_id: Uuid,
        reply: oneshot::Sender<Option<ShipView>>,
    },
    SetFuel {
        character_id: Uuid,
        fuel: u32,
    },
}

#[derive(Debug, Clone)]
pub enum SimEvent {
    Tick {
        tick: u64,
        ships: Vec<ShipView>,
        combat: Vec<CombatEvent>,
    },
    Spawned(ShipView),
    Despawned {
        ship_id: Uuid,
        #[allow(dead_code)]
        character_id: Option<Uuid>,
    },
}

#[derive(Clone)]
pub struct SimHub {
    pub cmd_tx: mpsc::Sender<SimCommand>,
    pub events: broadcast::Sender<SimEvent>,
    pub content: Arc<ContentRegistry>,
}

impl SimHub {
    pub async fn spawn_player(&self, req: SpawnPlayer) -> Result<ShipView, String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(SimCommand::Spawn { req, reply: tx })
            .await
            .map_err(|_| "sim stopped".to_string())?;
        rx.await.map_err(|_| "sim dropped".to_string())?
    }

    pub async fn despawn_player(&self, character_id: Uuid) -> Option<ShipView> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .cmd_tx
            .send(SimCommand::Despawn {
                character_id,
                reply: tx,
            })
            .await;
        rx.await.ok().flatten()
    }

    pub async fn apply_input(&self, character_id: Uuid, input: ControlInput) {
        let _ = self
            .cmd_tx
            .send(SimCommand::Input {
                character_id,
                input,
            })
            .await;
    }

    pub async fn dock(&self, character_id: Uuid, station_content_id: String) -> DockFail {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(SimCommand::Dock {
                character_id,
                station_content_id,
                reply: tx,
            })
            .await
            .is_err()
        {
            return DockFail::Dead;
        }
        rx.await.unwrap_or(DockFail::Dead)
    }

    pub async fn undock(&self, character_id: Uuid) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .cmd_tx
            .send(SimCommand::Undock {
                character_id,
                reply: tx,
            })
            .await
            .is_err()
        {
            return false;
        }
        rx.await.unwrap_or(false)
    }

    pub async fn jump(&self, character_id: Uuid, dest: String) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(SimCommand::Jump {
                character_id,
                dest,
                reply: tx,
            })
            .await
            .map_err(|_| "sim stopped".to_string())?;
        rx.await.map_err(|_| "sim dropped".to_string())?
    }

    pub async fn view(&self, character_id: Uuid) -> Option<ShipView> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .cmd_tx
            .send(SimCommand::View {
                character_id,
                reply: tx,
            })
            .await;
        rx.await.ok().flatten()
    }

    pub async fn list_ships(&self, character_id: Option<Uuid>) -> Vec<ShipView> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .cmd_tx
            .send(SimCommand::Snapshot {
                character_id,
                reply: tx,
            })
            .await;
        rx.await.unwrap_or_default()
    }

    pub async fn set_fuel(&self, character_id: Uuid, fuel: u32) {
        let _ = self
            .cmd_tx
            .send(SimCommand::SetFuel { character_id, fuel })
            .await;
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SimEvent> {
        self.events.subscribe()
    }
}

pub fn start_sim(content: ContentRegistry) -> SimHub {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SimCommand>(512);
    let (events, _) = broadcast::channel::<SimEvent>(128);
    let events_task = events.clone();
    let content_arc = Arc::new(content.clone());

    tokio::spawn(async move {
        let mut world = World::new(content);
        info!(
            stations = world.stations().len(),
            tick_hz = TICK_HZ,
            "sim loop started (full MVP)"
        );
        let mut ticker = tokio::time::interval(Duration::from_secs_f32(DT));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        handle_cmd(&mut world, cmd, &events_task);
                    }
                    world.step();
                    let combat = world.take_combat_log();
                    let ships = world.all_ship_views();
                    // Only despawn entities actually removed (pirates). Player deaths respawn in-place.
                    for c in &combat {
                        if matches!(c.kind, sim::CombatKind::Death) {
                            if let Some(id) = c.target_id {
                                if !ships.iter().any(|s| s.ship_id == id) {
                                    let _ = events_task.send(SimEvent::Despawned {
                                        ship_id: id,
                                        character_id: None,
                                    });
                                }
                            }
                        }
                    }
                    let _ = events_task.send(SimEvent::Tick {
                        tick: world.tick,
                        ships,
                        combat,
                    });
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => handle_cmd(&mut world, cmd, &events_task),
                        None => {
                            warn!("sim channel closed");
                            break;
                        }
                    }
                }
            }
        }
    });

    SimHub {
        cmd_tx,
        events,
        content: content_arc,
    }
}

fn handle_cmd(world: &mut World, cmd: SimCommand, events: &broadcast::Sender<SimEvent>) {
    match cmd {
        SimCommand::Spawn { req, reply } => {
            let r = world.spawn_player(req).map_err(|e| format!("{e:?}"));
            if let Ok(ref v) = r {
                let _ = events.send(SimEvent::Spawned(v.clone()));
            }
            let _ = reply.send(r);
        }
        SimCommand::Despawn {
            character_id,
            reply,
        } => {
            let v = world.despawn_player(character_id);
            if let Some(ref view) = v {
                let _ = events.send(SimEvent::Despawned {
                    ship_id: view.ship_id,
                    character_id: Some(character_id),
                });
            }
            let _ = reply.send(v);
        }
        SimCommand::Input {
            character_id,
            input,
        } => {
            let _ = world.apply_input(character_id, input);
        }
        SimCommand::Dock {
            character_id,
            station_content_id,
            reply,
        } => {
            let _ = reply.send(world.try_dock(character_id, &station_content_id));
        }
        SimCommand::Undock {
            character_id,
            reply,
        } => {
            let _ = reply.send(world.try_undock(character_id));
        }
        SimCommand::Jump {
            character_id,
            dest,
            reply,
        } => {
            let r = world
                .try_jump_start(character_id, &dest)
                .map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        SimCommand::Snapshot {
            character_id,
            reply,
        } => {
            let ships = match character_id {
                Some(cid) => world.interest_for(cid),
                None => world.all_ship_views(),
            };
            let _ = reply.send(ships);
        }
        SimCommand::View {
            character_id,
            reply,
        } => {
            let _ = reply.send(world.ship_view_for_character(character_id));
        }
        SimCommand::SetFuel { character_id, fuel } => {
            let _ = world.set_fuel(character_id, fuel);
        }
    }
}
