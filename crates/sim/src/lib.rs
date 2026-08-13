//! Authoritative simulation — flight, dock, combat, NPCs, AOI, jump (MVP complete).

use std::collections::HashMap;

use content::{ContentRegistry, ShipDef};
use uuid::Uuid;

pub const TICK_HZ: u32 = 20;
pub const DT: f32 = 1.0 / TICK_HZ as f32;
pub const MAX_PLAYERS_PRE_AOI: usize = 16;
pub const MAX_NPCS_SYSTEM: usize = 24;
pub const INPUT_SEQ_JUMP_MAX: u32 = 40;
pub const DOCK_RANGE_WU: f64 = 400.0;
pub const DOCK_MAX_SPEED: f64 = 80.0;
pub const AOI_CELL_WU: f64 = 3000.0;
pub const WEAPON_COOLDOWN_S: f32 = 0.25;
pub const HYPERSPACE_CHANNEL_S: f32 = 4.0;
pub const RESPAWN_INVULN_S: f32 = 6.0;
/// NPC weapons deal this fraction of listed damage (longer TTK).
pub const NPC_DAMAGE_SCALE: f32 = 0.55;
/// Max simultaneous pirates in a system (random encounters, not a farm).
pub const MAX_PIRATES_SPAWN: usize = 1;
/// Min distance from any station before an encounter can roll.
pub const ENCOUNTER_MIN_STATION_DIST: f64 = 3500.0;
/// Cooldown after a pirate dies before another encounter can spawn.
pub const ENCOUNTER_AFTER_KILL_CD: f32 = 75.0;
/// Base seconds between encounter rolls while eligible.
pub const ENCOUNTER_ROLL_CD: f32 = 35.0;
pub const DAILY_TRADE_CAP: i64 = 50_000;

pub type DurableShipId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId {
    pub index: u32,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Transform2D {
    pub x: f64,
    pub y: f64,
    pub rot: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Velocity {
    pub vx: f64,
    pub vy: f64,
    pub omega: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ControlInput {
    pub input_seq: u32,
    pub thrust: f32,
    pub turn: f32,
    pub fire_mask: u32,
    pub target_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilotKind {
    Player,
    NpcPirate,
}

#[derive(Debug, Clone)]
pub struct ShipBody {
    pub entity: EntityId,
    pub ship_id: DurableShipId,
    pub character_id: Option<Uuid>,
    pub pilot_name: String,
    pub pilot: PilotKind,
    pub def_id: String,
    pub system_id: String,
    pub transform: Transform2D,
    pub velocity: Velocity,
    pub shield: f32,
    pub armor: f32,
    pub energy: f32,
    pub energy_max: f32,
    pub energy_regen: f32,
    pub fuel: u32,
    pub max_fuel: u32,
    pub thrust: f32,
    pub max_speed: f32,
    pub turn_rate: f32,
    pub hull_radius_wu: f32,
    pub docked_station: Option<String>,
    pub last_docked_station: Option<String>,
    pub last_input: ControlInput,
    pub last_processed_input_seq: u32,
    pub weapon_cooldown: f32,
    pub invuln_ticks: u32,
    pub dead: bool,
    pub jump_channel: Option<JumpChannel>,
    pub npc_target: Option<DurableShipId>,
    pub npc_retarget_cd: f32,
}

#[derive(Debug, Clone)]
pub struct JumpChannel {
    pub dest_system: String,
    pub fuel_cost: u32,
    pub ticks_left: u32,
}

#[derive(Debug, Clone)]
pub struct StationBody {
    pub content_id: String,
    pub system_id: String,
    pub transform: Transform2D,
    pub safe_radius_wu: f64,
    pub dock_range_wu: f64,
    pub fuel_price_per_unit: u32,
}

#[derive(Debug, Clone)]
pub struct ShipView {
    pub ship_id: DurableShipId,
    pub character_id: Option<Uuid>,
    pub pilot_name: String,
    pub def_id: String,
    pub system_id: String,
    pub x: f64,
    pub y: f64,
    pub rot: f32,
    pub vx: f64,
    pub vy: f64,
    pub omega: f32,
    pub shield: f32,
    pub armor: f32,
    pub energy: f32,
    pub fuel: u32,
    pub last_processed_input_seq: u32,
    pub docked_station: Option<String>,
    pub invuln: bool,
    pub flags: u32,
    pub dead: bool,
}

#[derive(Debug, Clone)]
pub struct CombatEvent {
    pub kind: CombatKind,
    pub source_id: Option<Uuid>,
    pub target_id: Option<Uuid>,
    pub weapon_def: Option<String>,
    pub x: f64,
    pub y: f64,
    pub damage: Option<f32>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatKind {
    Hit,
    Miss,
    Death,
    FireDenied,
}

#[derive(Debug, Clone)]
pub struct SpawnPlayer {
    pub ship_id: DurableShipId,
    pub character_id: Uuid,
    pub pilot_name: String,
    pub def_id: String,
    pub system_id: String,
    pub x: f64,
    pub y: f64,
    pub rot: f32,
    pub shield: f32,
    pub armor: f32,
    pub energy: f32,
    pub fuel: u32,
    pub docked_station: Option<String>,
    pub force_undock: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockFail {
    Ok,
    TooFar,
    TooFast,
    AlreadyDocked,
    Dead,
    InvalidStation,
    WrongSystem,
}

#[derive(Debug)]
pub enum SimError {
    Full,
    UnknownShipDef(String),
    AlreadyPresent,
    NotFound,
}

struct Slot {
    generation: u32,
    ship: Option<ShipBody>,
}

pub struct World {
    pub tick: u64,
    content: ContentRegistry,
    slots: Vec<Slot>,
    free: Vec<u32>,
    by_ship: HashMap<DurableShipId, EntityId>,
    by_character: HashMap<Uuid, EntityId>,
    stations: Vec<StationBody>,
    combat_log: Vec<CombatEvent>,
    /// Time until next random-encounter roll is allowed.
    encounter_roll_cd: f32,
    /// Extra lockout after a pirate is destroyed (stops kill→instant respawn).
    encounter_kill_cd: f32,
}

impl World {
    pub fn new(content: ContentRegistry) -> Self {
        let mut w = Self {
            tick: 0,
            content,
            slots: Vec::new(),
            free: Vec::new(),
            by_ship: HashMap::new(),
            by_character: HashMap::new(),
            stations: Vec::new(),
            combat_log: Vec::new(),
            encounter_roll_cd: 20.0, // grace after server boot
            encounter_kill_cd: 0.0,
        };
        w.boot_stations();
        w
    }

    fn boot_stations(&mut self) {
        for st in self.content.stations.values() {
            self.stations.push(StationBody {
                content_id: st.id.clone(),
                system_id: st.system.clone(),
                transform: Transform2D {
                    x: st.x,
                    y: st.y,
                    rot: 0.0,
                },
                safe_radius_wu: st.safe_radius_wu,
                dock_range_wu: st.dock_range_wu,
                fuel_price_per_unit: st.fuel_price_per_unit,
            });
        }
    }

    pub fn content(&self) -> &ContentRegistry {
        &self.content
    }

    pub fn stations(&self) -> &[StationBody] {
        &self.stations
    }

    pub fn player_count(&self) -> usize {
        self.by_character.len()
    }

    pub fn take_combat_log(&mut self) -> Vec<CombatEvent> {
        std::mem::take(&mut self.combat_log)
    }

    pub fn spawn_player(&mut self, req: SpawnPlayer) -> Result<ShipView, SimError> {
        if self.by_character.contains_key(&req.character_id)
            || self.by_ship.contains_key(&req.ship_id)
        {
            return Err(SimError::AlreadyPresent);
        }
        if self.player_count() >= MAX_PLAYERS_PRE_AOI {
            return Err(SimError::Full);
        }
        let def = self
            .content
            .ship(&req.def_id)
            .map_err(|_| SimError::UnknownShipDef(req.def_id.clone()))?
            .clone();

        let mut docked = req.docked_station.clone();
        let mut x = req.x;
        let mut y = req.y;
        let mut rot = req.rot;
        if req.force_undock {
            if let Some(ref st_id) = docked {
                if let Some(st) = self.stations.iter().find(|s| s.content_id == *st_id) {
                    x = st.transform.x + st.dock_range_wu + 80.0;
                    y = st.transform.y;
                    rot = 0.0;
                }
            }
            docked = None;
        }

        let entity = self.alloc();
        let ship = ShipBody {
            entity,
            ship_id: req.ship_id,
            character_id: Some(req.character_id),
            pilot_name: req.pilot_name,
            pilot: PilotKind::Player,
            def_id: req.def_id,
            system_id: req.system_id,
            transform: Transform2D { x, y, rot },
            velocity: Velocity::default(),
            shield: req.shield,
            armor: req.armor,
            energy: req.energy,
            energy_max: def.base_energy,
            energy_regen: def.energy_regen,
            fuel: req.fuel,
            max_fuel: def.max_fuel.max(0) as u32,
            thrust: def.thrust,
            max_speed: def.max_speed,
            turn_rate: def.turn_rate,
            hull_radius_wu: def.hull_radius_wu,
            docked_station: docked.clone(),
            last_docked_station: docked.or(Some("st.earth_orbit".into())),
            last_input: ControlInput::default(),
            last_processed_input_seq: 0,
            weapon_cooldown: 0.0,
            invuln_ticks: 0,
            dead: false,
            jump_channel: None,
            npc_target: None,
            npc_retarget_cd: 0.0,
        };
        let view = ship_view(&ship);
        self.by_ship.insert(ship.ship_id, entity);
        self.by_character.insert(req.character_id, entity);
        self.slots[entity.index as usize].ship = Some(ship);
        Ok(view)
    }

    pub fn despawn_player(&mut self, character_id: Uuid) -> Option<ShipView> {
        let entity = self.by_character.remove(&character_id)?;
        self.free_entity(entity)
    }

    pub fn ship_view_for_character(&self, character_id: Uuid) -> Option<ShipView> {
        let e = *self.by_character.get(&character_id)?;
        self.ship_ref(e).map(ship_view)
    }

    pub fn set_fuel(&mut self, character_id: Uuid, fuel: u32) -> bool {
        let Some(entity) = self.by_character.get(&character_id).copied() else {
            return false;
        };
        let Some(ship) = self.ship_mut(entity) else {
            return false;
        };
        ship.fuel = fuel.min(ship.max_fuel);
        true
    }

    pub fn apply_input(&mut self, character_id: Uuid, input: ControlInput) -> bool {
        let Some(entity) = self.by_character.get(&character_id).copied() else {
            return false;
        };
        let Some(ship) = self.ship_mut(entity) else {
            return false;
        };
        if ship.dead {
            return false;
        }
        if input.input_seq <= ship.last_processed_input_seq {
            return false;
        }
        // Large seq gaps happen after hyperspace (client keeps sending while channel
        // freezes sim input). Resync instead of soft-locking the pilot at the exit.
        if input
            .input_seq
            .saturating_sub(ship.last_processed_input_seq)
            > INPUT_SEQ_JUMP_MAX
        {
            ship.last_processed_input_seq = input.input_seq.saturating_sub(1);
        }
        // During jump channel: still ack seq so the gap does not explode, but no flight.
        if ship.jump_channel.is_some() {
            ship.last_processed_input_seq = input.input_seq;
            ship.last_input = ControlInput {
                input_seq: input.input_seq,
                thrust: 0.0,
                turn: 0.0,
                fire_mask: 0,
                target_id: None,
            };
            return true;
        }
        ship.last_input = ControlInput {
            input_seq: input.input_seq,
            thrust: input.thrust.clamp(-1.0, 1.0),
            turn: input.turn.clamp(-1.0, 1.0),
            fire_mask: input.fire_mask,
            target_id: input.target_id,
        };
        true
    }

    pub fn try_dock(&mut self, character_id: Uuid, station_content_id: &str) -> DockFail {
        let Some(entity) = self.by_character.get(&character_id).copied() else {
            return DockFail::Dead;
        };
        let (x, y, vel, system_id) = {
            let Some(ship) = self.ship_ref(entity) else {
                return DockFail::Dead;
            };
            if ship.dead {
                return DockFail::Dead;
            }
            if ship.docked_station.is_some() {
                return DockFail::AlreadyDocked;
            }
            (
                ship.transform.x,
                ship.transform.y,
                ship.velocity,
                ship.system_id.clone(),
            )
        };
        let Some(st) = self
            .stations
            .iter()
            .find(|s| s.content_id == station_content_id)
            .cloned()
        else {
            return DockFail::InvalidStation;
        };
        if st.system_id != system_id {
            return DockFail::WrongSystem;
        }
        let dx = x - st.transform.x;
        let dy = y - st.transform.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > st.dock_range_wu.max(DOCK_RANGE_WU) {
            return DockFail::TooFar;
        }
        let speed = (vel.vx * vel.vx + vel.vy * vel.vy).sqrt();
        if speed > DOCK_MAX_SPEED {
            return DockFail::TooFast;
        }
        if let Some(ship) = self.ship_mut(entity) {
            ship.docked_station = Some(station_content_id.into());
            ship.last_docked_station = Some(station_content_id.into());
            ship.transform.x = st.transform.x;
            ship.transform.y = st.transform.y;
            ship.velocity = Velocity::default();
            ship.jump_channel = None;
        }
        DockFail::Ok
    }

    pub fn try_undock(&mut self, character_id: Uuid) -> bool {
        let Some(entity) = self.by_character.get(&character_id).copied() else {
            return false;
        };
        let st_id = {
            let Some(ship) = self.ship_mut(entity) else {
                return false;
            };
            ship.docked_station.take()
        };
        let Some(st_id) = st_id else {
            return false;
        };
        let st = self
            .stations
            .iter()
            .find(|s| s.content_id == st_id)
            .map(|s| (s.transform.x, s.transform.y, s.dock_range_wu));
        let Some(ship) = self.ship_mut(entity) else {
            return false;
        };
        if let Some((sx, sy, range)) = st {
            ship.transform.x = sx + range + 60.0;
            ship.transform.y = sy;
            ship.velocity = Velocity {
                vx: 40.0,
                vy: 0.0,
                omega: 0.0,
            };
        }
        true
    }

    pub fn try_jump_start(&mut self, character_id: Uuid, dest: &str) -> Result<(), &'static str> {
        let Some(entity) = self.by_character.get(&character_id).copied() else {
            return Err("not_found");
        };
        let (system_id, fuel, docked, jumping, dead) = {
            let ship = self.ship_ref(entity).ok_or("not_found")?;
            (
                ship.system_id.clone(),
                ship.fuel,
                ship.docked_station.is_some(),
                ship.jump_channel.is_some(),
                ship.dead,
            )
        };
        if dead {
            return Err("dead");
        }
        if jumping {
            return Err("already");
        }
        let link_cost = self
            .content
            .system(&system_id)
            .ok()
            .and_then(|s| s.links.iter().find(|l| l.to == dest).map(|l| l.fuel_cost))
            .ok_or("no_link")?;
        if fuel < link_cost {
            return Err("no_fuel");
        }
        // Reserve is trivial in monolith (always accept).
        let ticks = (HYPERSPACE_CHANNEL_S * TICK_HZ as f32) as u32;
        if let Some(ship) = self.ship_mut(entity) {
            // Auto-undock when plotting hyperspace from a station berth
            if docked {
                ship.docked_station = None;
            }
            ship.jump_channel = Some(JumpChannel {
                dest_system: dest.into(),
                fuel_cost: link_cost,
                ticks_left: ticks,
            });
            ship.velocity = Velocity::default();
        }
        Ok(())
    }

    pub fn step(&mut self) -> Vec<(Uuid, String, f64, f64, f32)> {
        self.tick = self.tick.wrapping_add(1);
        self.combat_log.clear();
        let dt = DT;
        self.npc_ai(dt);
        self.maybe_spawn_npcs(dt);

        // Collect fire intents then resolve (avoid borrow issues).
        let mut fire_jobs: Vec<(EntityId, ControlInput)> = Vec::new();
        let mut jump_complete: Vec<(EntityId, String, u32)> = Vec::new();

        for slot in &mut self.slots {
            let Some(ship) = slot.ship.as_mut() else {
                continue;
            };
            if ship.invuln_ticks > 0 {
                ship.invuln_ticks -= 1;
            }
            if ship.weapon_cooldown > 0.0 {
                ship.weapon_cooldown = (ship.weapon_cooldown - dt).max(0.0);
            }
            ship.energy = (ship.energy + ship.energy_regen * dt).clamp(0.0, ship.energy_max.max(1.0));

            if let Some(ref mut jc) = ship.jump_channel {
                jc.ticks_left = jc.ticks_left.saturating_sub(1);
                ship.last_processed_input_seq = ship.last_input.input_seq;
                if jc.ticks_left == 0 {
                    let dest = jc.dest_system.clone();
                    let cost = jc.fuel_cost;
                    jump_complete.push((ship.entity, dest, cost));
                }
                continue;
            }

            if ship.dead || ship.docked_station.is_some() {
                ship.velocity = Velocity::default();
                ship.last_processed_input_seq = ship.last_input.input_seq;
                continue;
            }

            let input = ship.last_input;
            ship.velocity.omega = ship.turn_rate * input.turn;
            ship.transform.rot += ship.velocity.omega * dt;
            let (sin, cos) = ship.transform.rot.sin_cos();
            let thrust_acc = ship.thrust * input.thrust;
            ship.velocity.vx += cos as f64 * thrust_acc as f64 * dt as f64;
            ship.velocity.vy += sin as f64 * thrust_acc as f64 * dt as f64;
            let speed =
                (ship.velocity.vx * ship.velocity.vx + ship.velocity.vy * ship.velocity.vy).sqrt();
            let max = ship.max_speed as f64;
            if speed > max && speed > 0.0 {
                let s = max / speed;
                ship.velocity.vx *= s;
                ship.velocity.vy *= s;
            }
            ship.transform.x += ship.velocity.vx * dt as f64;
            ship.transform.y += ship.velocity.vy * dt as f64;
            ship.last_processed_input_seq = input.input_seq;

            if input.fire_mask != 0 {
                fire_jobs.push((ship.entity, input));
            }
        }

        for (entity, dest, cost) in jump_complete {
            self.finish_jump(entity, &dest, cost);
        }
        for (entity, input) in fire_jobs {
            self.resolve_fire(entity, input);
        }

        // Return completed jumps for gateway (character_id, system, x,y,rot)
        Vec::new()
    }

    fn finish_jump(&mut self, entity: EntityId, dest: &str, cost: u32) {
        let spawn = self
            .stations
            .iter()
            .find(|s| s.system_id == dest)
            .map(|s| (s.transform.x, s.transform.y, s.dock_range_wu));
        let Some(ship) = self.ship_mut(entity) else {
            return;
        };
        ship.jump_channel = None;
        ship.docked_station = None;
        if ship.fuel < cost {
            return;
        }
        ship.fuel -= cost;
        ship.system_id = dest.into();
        if let Some((sx, sy, range)) = spawn {
            ship.transform.x = sx + range + 120.0;
            ship.transform.y = sy;
        } else {
            ship.transform.x = 0.0;
            ship.transform.y = 0.0;
        }
        ship.velocity = Velocity::default();
        // Fresh control state at exit — do not keep pre-channel thrust/fire
        let seq = ship.last_input.input_seq.max(ship.last_processed_input_seq);
        ship.last_input = ControlInput {
            input_seq: seq,
            thrust: 0.0,
            turn: 0.0,
            fire_mask: 0,
            target_id: None,
        };
        ship.last_processed_input_seq = seq;
    }

    fn in_safe_zone(&self, system_id: &str, x: f64, y: f64) -> bool {
        self.stations.iter().any(|st| {
            st.system_id == system_id && {
                let dx = x - st.transform.x;
                let dy = y - st.transform.y;
                (dx * dx + dy * dy).sqrt() <= st.safe_radius_wu
            }
        })
    }

    fn resolve_fire(&mut self, shooter_e: EntityId, input: ControlInput) {
        let shooter = match self.ship_ref(shooter_e).cloned() {
            Some(s) => s,
            None => return,
        };
        if shooter.dead || shooter.docked_station.is_some() || shooter.weapon_cooldown > 0.0 {
            return;
        }
        if self.in_safe_zone(
            &shooter.system_id,
            shooter.transform.x,
            shooter.transform.y,
        ) {
            self.combat_log.push(CombatEvent {
                kind: CombatKind::FireDenied,
                source_id: Some(shooter.ship_id),
                target_id: None,
                weapon_def: Some("weapon.light_cannon".into()),
                x: shooter.transform.x,
                y: shooter.transform.y,
                damage: None,
                reason: Some("safe_zone".into()),
            });
            return;
        }
        let wpn = self.content.weapons.get("weapon.light_cannon");
        let (range, mut damage, energy_cost, mut cooldown) = match wpn {
            Some(w) => (w.range_wu as f64, w.damage, w.energy_cost, w.cooldown_s),
            None => (2200.0, 4.0, 6.0, 0.45),
        };
        if shooter.pilot == PilotKind::NpcPirate {
            damage *= NPC_DAMAGE_SCALE;
            cooldown = cooldown.max(0.75); // pirates cannot machine-gun
        }
        if shooter.energy < energy_cost {
            self.combat_log.push(CombatEvent {
                kind: CombatKind::FireDenied,
                source_id: Some(shooter.ship_id),
                target_id: input.target_id,
                weapon_def: Some("weapon.light_cannon".into()),
                x: shooter.transform.x,
                y: shooter.transform.y,
                damage: None,
                reason: Some("no_energy".into()),
            });
            return;
        }

        // Apply energy/cooldown on shooter
        if let Some(s) = self.ship_mut(shooter_e) {
            s.energy -= energy_cost;
            s.weapon_cooldown = cooldown;
        }

        // Hitscan along facing; pick closest ship in cone/range (or target_id)
        let (sin, cos) = shooter.transform.rot.sin_cos();
        let mut best: Option<(EntityId, f64, Uuid)> = None;
        for slot in &self.slots {
            let Some(t) = slot.ship.as_ref() else {
                continue;
            };
            if t.entity == shooter_e || t.dead || t.system_id != shooter.system_id {
                continue;
            }
            if t.docked_station.is_some() {
                continue;
            }
            if self.in_safe_zone(&t.system_id, t.transform.x, t.transform.y) {
                continue;
            }
            if t.invuln_ticks > 0 {
                continue;
            }
            if let Some(tid) = input.target_id {
                if t.ship_id != tid {
                    continue;
                }
            }
            let dx = t.transform.x - shooter.transform.x;
            let dy = t.transform.y - shooter.transform.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > range + t.hull_radius_wu as f64 {
                continue;
            }
            // facing: dot with forward
            let nx = dx / dist.max(1e-6);
            let ny = dy / dist.max(1e-6);
            let dot = nx * cos as f64 + ny * sin as f64;
            if dot < 0.7 && input.target_id.is_none() {
                continue;
            }
            if best.map(|b| dist < b.1).unwrap_or(true) {
                best = Some((t.entity, dist, t.ship_id));
            }
        }

        if let Some((te, _dist, tid)) = best {
            // Impact point = target hull (not shooter) so clients don't show "self hits"
            let (ix, iy) = self
                .ship_ref(te)
                .map(|t| (t.transform.x, t.transform.y))
                .unwrap_or((shooter.transform.x, shooter.transform.y));
            let killed = self.apply_damage(te, damage, shooter.ship_id);
            self.combat_log.push(CombatEvent {
                kind: if killed {
                    CombatKind::Death
                } else {
                    CombatKind::Hit
                },
                source_id: Some(shooter.ship_id),
                target_id: Some(tid),
                weapon_def: Some("weapon.light_cannon".into()),
                x: ix,
                y: iy,
                damage: Some(damage),
                reason: None,
            });
        } else {
            // Miss marker slightly ahead of shooter (tracer end)
            let (sin, cos) = shooter.transform.rot.sin_cos();
            let reach = 400.0_f64;
            self.combat_log.push(CombatEvent {
                kind: CombatKind::Miss,
                source_id: Some(shooter.ship_id),
                target_id: input.target_id,
                weapon_def: Some("weapon.light_cannon".into()),
                x: shooter.transform.x + cos as f64 * reach,
                y: shooter.transform.y + sin as f64 * reach,
                damage: None,
                reason: None,
            });
        }
    }

    fn apply_damage(&mut self, target_e: EntityId, dmg: f32, _src: Uuid) -> bool {
        let outcome = {
            let Some(ship) = self.ship_mut(target_e) else {
                return false;
            };
            if ship.invuln_ticks > 0 || ship.dead {
                return false;
            }
            let mut left = dmg;
            if ship.shield > 0.0 {
                let take = ship.shield.min(left);
                ship.shield -= take;
                left -= take;
            }
            if left > 0.0 {
                ship.armor -= left;
            }
            if ship.armor > 0.0 {
                return false;
            }
            ship.armor = 0.0;
            (
                ship.pilot,
                ship.entity,
                ship.ship_id,
                ship.last_docked_station
                    .clone()
                    .or_else(|| Some("st.earth_orbit".into())),
                ship.energy_max,
                ship.def_id.clone(),
            )
        };
        let (pilot, entity, ship_id, last_station, energy_max, def_id) = outcome;

        // Pirates: remove from world + long encounter lockout (no instant replacements)
        if pilot == PilotKind::NpcPirate {
            let _ = self.free_entity(entity);
            let _ = ship_id;
            self.encounter_kill_cd = ENCOUNTER_AFTER_KILL_CD;
            self.encounter_roll_cd = ENCOUNTER_ROLL_CD.max(20.0);
            return true;
        }

        // Players: respawn free near last station
        let respawn = last_station.as_ref().and_then(|st_id| {
            self.stations.iter().find(|s| &s.content_id == st_id).map(|s| {
                (
                    s.system_id.clone(),
                    s.transform.x,
                    s.transform.y,
                    s.content_id.clone(),
                )
            })
        });
        let def_shield = self
            .content
            .ship(&def_id)
            .map(|d| d.base_shield)
            .unwrap_or(140.0);
        let def_armor = self
            .content
            .ship(&def_id)
            .map(|d| d.base_armor)
            .unwrap_or(110.0);
        let Some(ship) = self.ship_mut(target_e) else {
            return false;
        };
        if let Some((sys, x, y, st_id)) = respawn {
            ship.system_id = sys;
            ship.transform.x = x + 180.0;
            ship.transform.y = y + 40.0;
            ship.docked_station = None;
            ship.last_docked_station = Some(st_id);
        }
        ship.velocity = Velocity::default();
        ship.shield = def_shield;
        ship.armor = def_armor;
        ship.energy = energy_max.max(def_shield * 0.8);
        ship.dead = false;
        ship.invuln_ticks = (RESPAWN_INVULN_S * TICK_HZ as f32) as u32;
        ship.last_input = ControlInput::default();
        true
    }

    fn npc_ai(&mut self, dt: f32) {
        // Collect player positions per system for targeting
        let mut players: Vec<(String, f64, f64, DurableShipId, bool)> = Vec::new();
        for slot in &self.slots {
            if let Some(s) = &slot.ship {
                if s.pilot == PilotKind::Player && !s.dead {
                    // Skip invulnerable players (post-death grace)
                    if s.invuln_ticks > 0 {
                        continue;
                    }
                    if s.docked_station.is_some() {
                        continue;
                    }
                    players.push((
                        s.system_id.clone(),
                        s.transform.x,
                        s.transform.y,
                        s.ship_id,
                        true,
                    ));
                }
            }
        }
        for slot in &mut self.slots {
            let Some(ship) = slot.ship.as_mut() else {
                continue;
            };
            if ship.pilot != PilotKind::NpcPirate || ship.dead {
                continue;
            }
            ship.npc_retarget_cd -= dt;
            if ship.npc_retarget_cd <= 0.0 || ship.npc_target.is_none() {
                ship.npc_retarget_cd = 2.0;
                let mut best = None;
                for (sys, x, y, id, _) in &players {
                    if *sys != ship.system_id {
                        continue;
                    }
                    let dx = x - ship.transform.x;
                    let dy = y - ship.transform.y;
                    let d = dx * dx + dy * dy;
                    // Prefer engagement band ~800–2000 wu (not point-blank swarm)
                    if best.map(|b: (f64, _)| d < b.0).unwrap_or(true) {
                        best = Some((d, *id));
                    }
                }
                ship.npc_target = best.map(|b| b.1);
            }
            if let Some(tid) = ship.npc_target {
                if let Some((_, px, py, _, _)) = players.iter().find(|p| p.3 == tid) {
                    let dx = px - ship.transform.x;
                    let dy = py - ship.transform.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let ang = dy.atan2(dx) as f32;
                    let mut diff = ang - ship.transform.rot;
                    while diff > std::f32::consts::PI {
                        diff -= std::f32::consts::TAU;
                    }
                    while diff < -std::f32::consts::PI {
                        diff += std::f32::consts::TAU;
                    }
                    ship.last_input.turn = (diff * 1.4).clamp(-1.0, 1.0);
                    // Hold range: ease off if too close, approach if far
                    if dist < 600.0 {
                        ship.last_input.thrust = -0.15;
                    } else if dist > 1800.0 {
                        ship.last_input.thrust = 0.55;
                    } else {
                        ship.last_input.thrust = 0.25;
                    }
                    // Stricter aim + only fire when roughly lined up
                    ship.last_input.fire_mask = if diff.abs() < 0.18 && dist < 2000.0 {
                        1
                    } else {
                        0
                    };
                    ship.last_input.input_seq = ship.last_input.input_seq.wrapping_add(1);
                    ship.last_input.target_id = Some(tid);
                }
            }
        }
    }

    fn maybe_spawn_npcs(&mut self, dt: f32) {
        // Random deep-space encounters — not a kill-respawn farm.
        self.encounter_roll_cd = (self.encounter_roll_cd - dt).max(0.0);
        self.encounter_kill_cd = (self.encounter_kill_cd - dt).max(0.0);
        if self.encounter_roll_cd > 0.0 || self.encounter_kill_cd > 0.0 {
            return;
        }

        let pirate_count = self
            .slots
            .iter()
            .filter(|s| {
                s.ship
                    .as_ref()
                    .map(|sh| sh.pilot == PilotKind::NpcPirate)
                    .unwrap_or(false)
            })
            .count();
        if pirate_count >= MAX_PIRATES_SPAWN {
            // Already an active encounter — only roll again much later
            self.encounter_roll_cd = ENCOUNTER_ROLL_CD;
            return;
        }

        // Find a player in open space (far from stations) to ambush nearby
        let mut candidate: Option<(String, f64, f64)> = None;
        for slot in &self.slots {
            let Some(s) = slot.ship.as_ref() else {
                continue;
            };
            if s.pilot != PilotKind::Player || s.dead || s.docked_station.is_some() {
                continue;
            }
            if s.invuln_ticks > 0 {
                continue;
            }
            let near_station = self.stations.iter().any(|st| {
                st.system_id == s.system_id && {
                    let dx = s.transform.x - st.transform.x;
                    let dy = s.transform.y - st.transform.y;
                    (dx * dx + dy * dy).sqrt() < ENCOUNTER_MIN_STATION_DIST
                }
            });
            if near_station {
                continue;
            }
            candidate = Some((s.system_id.clone(), s.transform.x, s.transform.y));
            break;
        }

        // Always consume a roll interval so we don't spin every tick
        self.encounter_roll_cd = ENCOUNTER_ROLL_CD;

        let Some((sys, px, py)) = candidate else {
            return; // no eligible player in deep space
        };

        // ~40% chance when eligible (feels random, not a conveyor belt)
        // Use tick as cheap entropy
        if (self.tick.wrapping_mul(1103515245).wrapping_add(12345) % 100) >= 40 {
            return;
        }

        // Spawn off to the side of the player (not on top of them)
        let ang = (self.tick as f64 * 0.17) % std::f64::consts::TAU;
        let dist = 900.0 + (self.tick % 400) as f64;
        let x = px + ang.cos() * dist;
        let y = py + ang.sin() * dist;
        let _ = self.spawn_npc("ship.raider", &sys, x, y, "Pirate");
    }

    fn spawn_npc(
        &mut self,
        def_id: &str,
        system_id: &str,
        x: f64,
        y: f64,
        name: &str,
    ) -> Result<ShipView, SimError> {
        let def: ShipDef = self
            .content
            .ship(def_id)
            .map_err(|_| SimError::UnknownShipDef(def_id.into()))?
            .clone();
        let entity = self.alloc();
        let ship_id = Uuid::now_v7();
        let ship = ShipBody {
            entity,
            ship_id,
            character_id: None,
            pilot_name: name.into(),
            pilot: PilotKind::NpcPirate,
            def_id: def_id.into(),
            system_id: system_id.into(),
            transform: Transform2D { x, y, rot: 0.0 },
            velocity: Velocity::default(),
            shield: def.base_shield,
            armor: def.base_armor,
            energy: def.base_energy,
            energy_max: def.base_energy,
            energy_regen: def.energy_regen,
            fuel: def.starting_fuel.max(0) as u32,
            max_fuel: def.max_fuel.max(0) as u32,
            thrust: def.thrust,
            max_speed: def.max_speed,
            turn_rate: def.turn_rate,
            hull_radius_wu: def.hull_radius_wu,
            docked_station: None,
            last_docked_station: None,
            last_input: ControlInput::default(),
            last_processed_input_seq: 0,
            weapon_cooldown: 0.0,
            invuln_ticks: 0,
            dead: false,
            jump_channel: None,
            npc_target: None,
            npc_retarget_cd: 0.0,
        };
        let view = ship_view(&ship);
        self.by_ship.insert(ship_id, entity);
        self.slots[entity.index as usize].ship = Some(ship);
        Ok(view)
    }

    /// Ships relevant to observer (same system + AOI grid 3×3), always-relevant if few.
    pub fn interest_for(&self, character_id: Uuid) -> Vec<ShipView> {
        let Some(me) = self.ship_view_for_character(character_id) else {
            return self.all_ship_views();
        };
        let all = self.all_ship_views();
        if all.len() <= 16 {
            return all
                .into_iter()
                .filter(|s| s.system_id == me.system_id)
                .collect();
        }
        let cx = (me.x / AOI_CELL_WU).floor() as i32;
        let cy = (me.y / AOI_CELL_WU).floor() as i32;
        all.into_iter()
            .filter(|s| {
                if s.system_id != me.system_id {
                    return false;
                }
                let sx = (s.x / AOI_CELL_WU).floor() as i32;
                let sy = (s.y / AOI_CELL_WU).floor() as i32;
                (sx - cx).abs() <= 1 && (sy - cy).abs() <= 1
            })
            .collect()
    }

    pub fn all_ship_views(&self) -> Vec<ShipView> {
        self.slots
            .iter()
            .filter_map(|s| s.ship.as_ref().map(ship_view))
            .collect()
    }

    fn alloc(&mut self) -> EntityId {
        if let Some(index) = self.free.pop() {
            let gen = self.slots[index as usize].generation;
            EntityId {
                index,
                generation: gen,
            }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {
                generation: 1,
                ship: None,
            });
            EntityId {
                index,
                generation: 1,
            }
        }
    }

    fn free_entity(&mut self, entity: EntityId) -> Option<ShipView> {
        let slot = self.slots.get_mut(entity.index as usize)?;
        if slot.generation != entity.generation {
            return None;
        }
        let ship = slot.ship.take()?;
        self.by_ship.remove(&ship.ship_id);
        if let Some(cid) = ship.character_id {
            self.by_character.remove(&cid);
        }
        let view = ship_view(&ship);
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(entity.index);
        Some(view)
    }

    fn ship_mut(&mut self, entity: EntityId) -> Option<&mut ShipBody> {
        let slot = self.slots.get_mut(entity.index as usize)?;
        if slot.generation != entity.generation {
            return None;
        }
        slot.ship.as_mut()
    }

    fn ship_ref(&self, entity: EntityId) -> Option<&ShipBody> {
        let slot = self.slots.get(entity.index as usize)?;
        if slot.generation != entity.generation {
            return None;
        }
        slot.ship.as_ref()
    }
}

fn ship_view(ship: &ShipBody) -> ShipView {
    ShipView {
        ship_id: ship.ship_id,
        character_id: ship.character_id,
        pilot_name: ship.pilot_name.clone(),
        def_id: ship.def_id.clone(),
        system_id: ship.system_id.clone(),
        x: ship.transform.x,
        y: ship.transform.y,
        rot: ship.transform.rot,
        vx: ship.velocity.vx,
        vy: ship.velocity.vy,
        omega: ship.velocity.omega,
        shield: ship.shield,
        armor: ship.armor,
        energy: ship.energy,
        fuel: ship.fuel,
        last_processed_input_seq: ship.last_processed_input_seq,
        docked_station: ship.docked_station.clone(),
        invuln: ship.invuln_ticks > 0,
        flags: if ship.last_input.thrust.abs() > 0.01 {
            1
        } else {
            0
        } | if ship.jump_channel.is_some() { 2 } else { 0 },
        dead: ship.dead,
    }
}

pub fn predict_step(
    x: f64,
    y: f64,
    rot: f32,
    vx: f64,
    vy: f64,
    thrust_axis: f32,
    turn_axis: f32,
    thrust: f32,
    max_speed: f32,
    turn_rate: f32,
    dt: f32,
) -> (f64, f64, f32, f64, f64, f32) {
    let turn_axis = turn_axis.clamp(-1.0, 1.0);
    let thrust_axis = thrust_axis.clamp(-1.0, 1.0);
    let omega = turn_rate * turn_axis;
    let rot = rot + omega * dt;
    let (sin, cos) = rot.sin_cos();
    let mut vx = vx + cos as f64 * (thrust * thrust_axis) as f64 * dt as f64;
    let mut vy = vy + sin as f64 * (thrust * thrust_axis) as f64 * dt as f64;
    let speed = (vx * vx + vy * vy).sqrt();
    let max = max_speed as f64;
    if speed > max && speed > 0.0 {
        let s = max / speed;
        vx *= s;
        vy *= s;
    }
    (x + vx * dt as f64, y + vy * dt as f64, rot, vx, vy, omega)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content() -> ContentRegistry {
        content::load_default().expect("content")
    }

    #[test]
    fn tick_and_stations() {
        let mut w = World::new(content());
        assert!(w.stations().len() >= 2);
        w.step();
        assert_eq!(w.tick, 1);
    }

    #[test]
    fn dock_and_undock() {
        let mut w = World::new(content());
        let cid = Uuid::now_v7();
        w.spawn_player(SpawnPlayer {
            ship_id: Uuid::now_v7(),
            character_id: cid,
            pilot_name: "T".into(),
            def_id: "ship.shuttle".into(),
            system_id: "sys.sol".into(),
            x: 5000.0,
            y: 0.0,
            rot: 0.0,
            shield: 80.0,
            armor: 60.0,
            energy: 100.0,
            fuel: 10,
            docked_station: None,
            force_undock: false,
        })
        .unwrap();
        assert_eq!(w.try_dock(cid, "st.earth_orbit"), DockFail::Ok);
        assert!(w.try_undock(cid));
    }

    #[test]
    fn thrust_moves() {
        let mut w = World::new(content());
        let cid = Uuid::now_v7();
        w.spawn_player(SpawnPlayer {
            ship_id: Uuid::now_v7(),
            character_id: cid,
            pilot_name: "T".into(),
            def_id: "ship.shuttle".into(),
            system_id: "sys.sol".into(),
            x: 0.0,
            y: 0.0,
            rot: 0.0,
            shield: 80.0,
            armor: 60.0,
            energy: 100.0,
            fuel: 10,
            docked_station: None,
            force_undock: false,
        })
        .unwrap();
        w.apply_input(
            cid,
            ControlInput {
                input_seq: 1,
                thrust: 1.0,
                turn: 0.0,
                fire_mask: 0,
                target_id: None,
            },
        );
        for _ in 0..20 {
            w.step();
        }
        assert!(w.ship_view_for_character(cid).unwrap().x > 1.0);
    }

    #[test]
    fn jump_channel() {
        let mut w = World::new(content());
        let cid = Uuid::now_v7();
        w.spawn_player(SpawnPlayer {
            ship_id: Uuid::now_v7(),
            character_id: cid,
            pilot_name: "T".into(),
            def_id: "ship.shuttle".into(),
            system_id: "sys.sol".into(),
            x: 0.0,
            y: 0.0,
            rot: 0.0,
            shield: 80.0,
            armor: 60.0,
            energy: 100.0,
            fuel: 10,
            docked_station: None,
            force_undock: false,
        })
        .unwrap();
        // Simulate client still sending frames during channel (seq races ahead)
        w.apply_input(
            cid,
            ControlInput {
                input_seq: 1,
                thrust: 1.0,
                turn: 0.0,
                fire_mask: 0,
                target_id: None,
            },
        );
        w.try_jump_start(cid, "sys.alpha_centauri").unwrap();
        for i in 2..90u32 {
            assert!(
                w.apply_input(
                    cid,
                    ControlInput {
                        input_seq: i,
                        thrust: 1.0,
                        turn: 0.0,
                        fire_mask: 0,
                        target_id: None,
                    },
                ),
                "input during jump channel must ack seq"
            );
            w.step();
        }
        let v = w.ship_view_for_character(cid).unwrap();
        assert_eq!(v.system_id, "sys.alpha_centauri");
        assert!(v.fuel < 10);
        // After exit, flight inputs must still apply (no tether)
        assert!(w.apply_input(
            cid,
            ControlInput {
                input_seq: 90,
                thrust: 1.0,
                turn: 0.0,
                fire_mask: 0,
                target_id: None,
            },
        ));
        let x0 = w.ship_view_for_character(cid).unwrap().x;
        for _ in 0..40 {
            w.step();
        }
        let x1 = w.ship_view_for_character(cid).unwrap().x;
        assert!(
            (x1 - x0).abs() > 1.0,
            "ship must move after jump, was tethered at {x0}"
        );
    }
}
