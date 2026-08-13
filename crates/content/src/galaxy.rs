//! Fixed MMO galaxy catalog + offline generator.
//!
//! Runtime loads `content/galaxy/catalog.json` (committed, identical for every
//! shard). Hand-authored `content/systems/*.toml` and `stations/*.toml` overlay
//! the catalog so story systems can be edited without regenerating the mesh.
//!
//! To rebuild the catalog (rare — breaks player maps if done casually):
//!   cargo run -p content --bin gen-galaxy

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{ContentError, ContentRegistry, StationDef, SystemDef, SystemLink};

/// Offline generator params (`content/galaxy.toml`). Not used at runtime.
#[derive(Debug, Clone, Deserialize)]
pub struct GalaxyConfig {
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default = "default_count")]
    pub system_count: usize,
    #[serde(default = "default_max_links")]
    pub max_links: usize,
    #[serde(default = "default_populated")]
    pub populated_fraction: f64,
    #[serde(default = "default_map_radius")]
    pub map_radius: f64,
    #[serde(default = "default_min_spacing")]
    pub min_spacing: f64,
}

fn default_seed() -> u64 {
    42
}
fn default_count() -> usize {
    1000
}
fn default_max_links() -> usize {
    5
}
fn default_populated() -> f64 {
    0.28
}
fn default_map_radius() -> f64 {
    1200.0
}
fn default_min_spacing() -> f64 {
    16.0
}

impl Default for GalaxyConfig {
    fn default() -> Self {
        Self {
            seed: default_seed(),
            system_count: default_count(),
            max_links: default_max_links(),
            populated_fraction: default_populated(),
            map_radius: default_map_radius(),
            min_spacing: default_min_spacing(),
        }
    }
}

/// Committed galaxy data — same on every server boot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalaxyCatalog {
    pub version: u32,
    #[serde(default)]
    pub notes: String,
    pub systems: Vec<SystemDef>,
    #[serde(default)]
    pub stations: Vec<StationDef>,
}

impl GalaxyCatalog {
    pub fn path(content_root: &Path) -> std::path::PathBuf {
        content_root.join("galaxy").join("catalog.json")
    }
}

/// Load fixed catalog into the registry. Existing ids are **not** overwritten
/// (hand-authored TOML loaded later can override, or load catalog first then overlay).
pub fn load_catalog(reg: &mut ContentRegistry, content_root: &Path) -> Result<(), ContentError> {
    let path = GalaxyCatalog::path(content_root);
    if !path.is_file() {
        return Err(ContentError::Parse {
            path: path.display().to_string(),
            msg: "missing fixed galaxy catalog — run: cargo run -p content --bin gen-galaxy"
                .into(),
        });
    }
    let text = std::fs::read_to_string(&path)?;
    let catalog: GalaxyCatalog = serde_json::from_str(&text).map_err(|e| ContentError::Parse {
        path: path.display().to_string(),
        msg: e.to_string(),
    })?;

    let mut sys_added = 0usize;
    let mut st_added = 0usize;
    for s in catalog.systems {
        let id = s.id.clone();
        if reg.systems.contains_key(&id) {
            // Prefer hand-authored entry already present; still merge map/links if empty?
            // Hand systems loaded after catalog in load_from_dir — here catalog is first.
            reg.systems.insert(id, s);
            sys_added += 1;
        } else {
            reg.systems.insert(id, s);
            sys_added += 1;
        }
    }
    for st in catalog.stations {
        let id = st.id.clone();
        reg.stations.insert(id, st);
        st_added += 1;
    }

    let max_deg = reg
        .systems
        .values()
        .map(|s| s.links.len())
        .max()
        .unwrap_or(0);
    let connected = is_connected(reg);
    info!(
        systems = reg.systems.len(),
        stations = reg.stations.len(),
        catalog_systems = sys_added,
        catalog_stations = st_added,
        max_degree = max_deg,
        connected,
        "fixed galaxy catalog loaded"
    );
    if max_deg > 5 {
        tracing::warn!(max_deg, "galaxy catalog has degree > 5");
    }
    if !connected {
        tracing::warn!("galaxy catalog is not fully connected");
    }
    Ok(())
}

/// Overlay hand-authored system/station defs on top of the catalog (story edits).
pub fn overlay_hand_authored(reg: &mut ContentRegistry, content_root: &Path) -> Result<(), ContentError> {
    let mut hand_systems: HashMap<String, SystemDef> = HashMap::new();
    let mut hand_stations: HashMap<String, StationDef> = HashMap::new();
    load_toml_dir_into(content_root.join("systems"), &mut hand_systems, |s| s.id.clone())?;
    load_toml_dir_into(content_root.join("stations"), &mut hand_stations, |s| s.id.clone())?;

    for (id, mut hand) in hand_systems {
        if let Some(base) = reg.systems.get(&id) {
            // Story fields from hand; geography + jump graph stay fixed from catalog
            // (so Sol lore edits never orphan the rest of the MMO mesh).
            hand.links = base.links.clone();
            hand.map_x = base.map_x;
            hand.map_y = base.map_y;
            if hand.kind.is_empty() {
                hand.kind = base.kind.clone();
            }
            if hand.stations.is_empty() {
                hand.stations = base.stations.clone();
            }
            if hand.lore.is_empty() && !base.lore.is_empty() {
                hand.lore = base.lore.clone();
            }
        }
        reg.systems.insert(id, hand);
    }
    for (id, st) in hand_stations {
        reg.stations.insert(id, st);
    }
    Ok(())
}

fn load_toml_dir_into<T, F>(
    dir: std::path::PathBuf,
    out: &mut HashMap<String, T>,
    id_of: F,
) -> Result<(), ContentError>
where
    T: for<'de> Deserialize<'de>,
    F: Fn(&T) -> String,
{
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let typed: T = toml::from_str(&text).map_err(|e| ContentError::Parse {
            path: path.display().to_string(),
            msg: e.to_string(),
        })?;
        let id = id_of(&typed);
        out.insert(id, typed);
    }
    Ok(())
}

// ── Offline generator (fixed catalog authoring) ─────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }
    fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
    fn range_usize(&mut self, lo: usize, hi: usize) -> usize {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() as usize % (hi - lo))
    }
}

const PREFIXES: &[&str] = &[
    "Nova", "Vega", "Orion", "Lyra", "Kepler", "Helios", "Astra", "Cygnus", "Rigel", "Sirius",
    "Procyon", "Altair", "Deneb", "Polaris", "Mira", "Titan", "Oberon", "Io", "Callisto", "Enceladus",
    "Phoenix", "Hydra", "Draco", "Perseus", "Andromeda", "Cassiopeia", "Centauri", "Arcturus",
    "Spica", "Antares", "Aldebaran", "Betelgeuse", "Fomalhaut", "Achernar", "Canopus", "Capella",
    "Zuben", "Thuban", "Nunki", "Sadal", "Markab", "Scheat", "Algol", "Regulus", "Castor", "Pollux",
    "Hadar", "Acrux", "Gacrux", "Mimosa", "Shaula", "Kaus", "Rasal", "Unuk", "Sargas", "Wezen",
];

const SUFFIXES: &[&str] = &[
    "Prime", "Minor", "Reach", "Gate", "Expanse", "Drift", "Well", "Hollow", "Marches", "Rim",
    "Deep", "Shoals", "Belt", "Cluster", "Verge", "Abyss", "Span", "Crossing", "Anchor", "Haven",
    "Outpost", "Null", "Fringe", "Core", "Arc", "Wedge", "Fold", "Rift", "Corridor", "Pass",
    "Station", "Point", "Node", "Terminus", "Boundary", "Watch", "Signal", "Beacon", "Forge", "Yard",
];

const KINDS_POP: &[&str] = &["populated", "hub"];
const KIND_TRANSIT: &str = "transit";

pub fn load_generator_config(root: &Path) -> GalaxyConfig {
    let path = root.join("galaxy.toml");
    if !path.is_file() {
        return GalaxyConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|_| GalaxyConfig::default()),
        Err(_) => GalaxyConfig::default(),
    }
}

/// Offline baker: expand hand seeds into a full fixed galaxy mesh.
pub fn apply_galaxy(reg: &mut ContentRegistry, cfg: &GalaxyConfig) {
    let max_links = cfg.max_links.clamp(1, 8);
    let target = cfg.system_count.max(reg.systems.len()).max(2);
    let mut rng = Rng::new(cfg.seed);

    if let Some(sol) = reg.systems.get_mut("sys.sol") {
        sol.map_x = 0.0;
        sol.map_y = 0.0;
        sol.kind = "hub".into();
    }
    if let Some(ac) = reg.systems.get_mut("sys.alpha_centauri") {
        ac.map_x = 48.0;
        ac.map_y = 12.0;
        if ac.kind.is_empty() {
            ac.kind = "populated".into();
        }
    }

    let need = target.saturating_sub(reg.systems.len());
    let grid_n = ((need as f64).sqrt().ceil() as usize).max(1) + 2;
    let cell = (cfg.map_radius * 2.0) / grid_n as f64;
    let spacing = cfg.min_spacing.max(cell * 0.55);
    let mut slots: Vec<(f64, f64)> = Vec::with_capacity(grid_n * grid_n);
    for gy in 0..grid_n {
        for gx in 0..grid_n {
            let base_x = -cfg.map_radius + (gx as f64 + 0.5) * cell;
            let base_y = -cfg.map_radius + (gy as f64 + 0.5) * cell;
            let jx = (rng.next_f64() - 0.5) * cell * 0.7;
            let jy = (rng.next_f64() - 0.5) * cell * 0.7;
            let x = base_x + jx;
            let y = base_y + jy;
            if dist(x, y, 0.0, 0.0) < spacing * 1.5 {
                continue;
            }
            if dist(x, y, 48.0, 12.0) < spacing {
                continue;
            }
            slots.push((x, y));
        }
    }
    for i in (1..slots.len()).rev() {
        let j = rng.range_usize(0, i + 1);
        slots.swap(i, j);
    }

    let hand_station_ids: HashSet<String> = reg.stations.keys().cloned().collect();

    let mut gen_i = 0usize;
    let mut slot_i = 0usize;
    while reg.systems.len() < target {
        gen_i += 1;
        let id = format!("sys.g{:04}", gen_i);
        if reg.systems.contains_key(&id) {
            continue;
        }
        let (mx, my) = if slot_i < slots.len() {
            let p = slots[slot_i];
            slot_i += 1;
            p
        } else {
            let ang = gen_i as f64 * 2.399963;
            let r = spacing * (1.0 + (gen_i as f64).sqrt());
            (ang.cos() * r, ang.sin() * r)
        };

        let name = format!(
            "{} {}",
            PREFIXES[rng.range_usize(0, PREFIXES.len())],
            SUFFIXES[rng.range_usize(0, SUFFIXES.len())]
        );
        let populated = rng.next_f64() < cfg.populated_fraction;
        let kind = if populated {
            KINDS_POP[rng.range_usize(0, KINDS_POP.len())].to_string()
        } else {
            KIND_TRANSIT.into()
        };
        let radius = if populated {
            rng.range_f64(60_000.0, 110_000.0)
        } else {
            rng.range_f64(40_000.0, 80_000.0)
        };

        let mut stations = Vec::new();
        if populated {
            let st_id = format!("st.g{:04}", gen_i);
            stations.push(st_id.clone());
            let angle = rng.range_f64(0.0, std::f64::consts::TAU);
            let d = rng.range_f64(2500.0, 8000.0);
            reg.stations.insert(
                st_id.clone(),
                StationDef {
                    id: st_id,
                    system: id.clone(),
                    x: angle.cos() * d,
                    y: angle.sin() * d,
                    dock_range_wu: 400.0,
                    safe_radius_wu: 2200.0,
                    fuel_price_per_unit: 35 + rng.range_usize(0, 50) as u32,
                },
            );
        }

        reg.systems.insert(
            id.clone(),
            SystemDef {
                id,
                display_name: name,
                radius_wu: radius,
                stations,
                links: Vec::new(),
                spawn_tables: if populated {
                    vec!["npc.pirate_raider".into()]
                } else {
                    Vec::new()
                },
                map_x: mx,
                map_y: my,
                kind,
                lore: String::new(),
            },
        );
    }

    let mut ids: Vec<String> = reg.systems.keys().cloned().collect();
    ids.sort();
    let n = ids.len();
    let index: HashMap<String, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let positions: Vec<(f64, f64)> = ids
        .iter()
        .map(|id| {
            let s = &reg.systems[id];
            (s.map_x, s.map_y)
        })
        .collect();

    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n];
    let seed_links: Vec<(String, String)> = reg
        .systems
        .iter()
        .flat_map(|(from, sys)| {
            sys.links
                .iter()
                .map(|l| (from.clone(), l.to.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    for (a, b) in seed_links {
        if let (Some(&ia), Some(&ib)) = (index.get(&a), index.get(&b)) {
            try_add(&mut adj, ia, ib, max_links);
        }
    }

    let k = (max_links + 4).min(12).min(n.saturating_sub(1));
    let mut candidates: Vec<(f64, usize, usize)> = Vec::with_capacity(n * k);
    for i in 0..n {
        let (ax, ay) = positions[i];
        let mut nearest: Vec<(f64, usize)> = Vec::with_capacity(n);
        for j in 0..n {
            if i == j {
                continue;
            }
            let (bx, by) = positions[j];
            nearest.push((dist(ax, ay, bx, by), j));
        }
        nearest.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Less));
        for &(d, j) in nearest.iter().take(k) {
            if i < j {
                candidates.push((d, i, j));
            } else {
                candidates.push((d, j, i));
            }
        }
    }
    candidates.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Less)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    candidates.dedup_by(|a, b| a.1 == b.1 && a.2 == b.2);

    let mut uf = UnionFind::new(n);
    for &(_d, i, j) in &candidates {
        if uf.find(i) != uf.find(j) && try_add(&mut adj, i, j, max_links) {
            uf.union(i, j);
        }
    }
    for &(_d, i, j) in &candidates {
        if adj[i].len() >= max_links || adj[j].len() >= max_links {
            continue;
        }
        if adj[i].contains(&j) {
            continue;
        }
        if rng.next_f64() < 0.55 {
            try_add(&mut adj, i, j, max_links);
        }
    }

    for sys in reg.systems.values_mut() {
        sys.links.clear();
    }
    let mut edge_count = 0usize;
    for i in 0..n {
        for &j in &adj[i] {
            if i >= j {
                continue;
            }
            let (ax, ay) = positions[i];
            let (bx, by) = positions[j];
            let fuel = fuel_cost(dist(ax, ay, bx, by));
            let a_id = &ids[i];
            let b_id = &ids[j];
            if let Some(sa) = reg.systems.get_mut(a_id) {
                sa.links.push(SystemLink {
                    to: b_id.clone(),
                    fuel_cost: fuel,
                });
            }
            if let Some(sb) = reg.systems.get_mut(b_id) {
                sb.links.push(SystemLink {
                    to: a_id.clone(),
                    fuel_cost: fuel,
                });
            }
            edge_count += 1;
        }
    }

    let populated = reg
        .systems
        .values()
        .filter(|s| s.kind != KIND_TRANSIT)
        .count();
    let max_deg = reg
        .systems
        .values()
        .map(|s| s.links.len())
        .max()
        .unwrap_or(0);

    info!(
        systems = reg.systems.len(),
        stations = reg.stations.len(),
        generated_stations = reg.stations.len().saturating_sub(hand_station_ids.len()),
        edges = edge_count,
        populated,
        max_degree = max_deg,
        connected = is_connected(reg),
        seed = cfg.seed,
        "fixed galaxy generated (write catalog to commit)"
    );
}

/// Serialize registry systems + non-hand stations to catalog path.
pub fn write_catalog(
    reg: &ContentRegistry,
    content_root: &Path,
    hand_station_ids: &HashSet<String>,
) -> Result<std::path::PathBuf, ContentError> {
    let mut systems: Vec<SystemDef> = reg.systems.values().cloned().collect();
    systems.sort_by(|a, b| a.id.cmp(&b.id));
    let mut stations: Vec<StationDef> = reg
        .stations
        .values()
        .filter(|s| !hand_station_ids.contains(&s.id))
        .cloned()
        .collect();
    stations.sort_by(|a, b| a.id.cmp(&b.id));

    let catalog = GalaxyCatalog {
        version: 1,
        notes: "Fixed MMO galaxy. Edit story systems via content/systems/*.toml overlays. \
                Only regenerate with gen-galaxy when intentionally reshaping the universe."
            .into(),
        systems,
        stations,
    };

    let dir = content_root.join("galaxy");
    std::fs::create_dir_all(&dir)?;
    let path = GalaxyCatalog::path(content_root);
    let json = serde_json::to_string_pretty(&catalog).map_err(|e| ContentError::Parse {
        path: path.display().to_string(),
        msg: e.to_string(),
    })?;
    std::fs::write(&path, json)?;
    Ok(path)
}

fn try_add(adj: &mut [HashSet<usize>], i: usize, j: usize, max_links: usize) -> bool {
    if i == j {
        return false;
    }
    if adj[i].len() >= max_links || adj[j].len() >= max_links {
        return false;
    }
    if adj[i].contains(&j) {
        return false;
    }
    adj[i].insert(j);
    adj[j].insert(i);
    true
}

fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = ax - bx;
    let dy = ay - by;
    (dx * dx + dy * dy).sqrt()
}

fn fuel_cost(map_dist: f64) -> u32 {
    let c = 1 + (map_dist / 55.0).floor() as u32;
    c.clamp(1, 4)
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let mut ra = self.find(a);
        let mut rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        if self.rank[ra] == self.rank[rb] {
            self.rank[ra] += 1;
        }
    }
}

fn is_connected(reg: &ContentRegistry) -> bool {
    if reg.systems.is_empty() {
        return true;
    }
    let start = reg.systems.keys().next().cloned().unwrap_or_default();
    let mut seen = HashSet::new();
    let mut q = VecDeque::new();
    q.push_back(start.clone());
    seen.insert(start);
    while let Some(cur) = q.pop_front() {
        if let Some(sys) = reg.systems.get(&cur) {
            for l in &sys.links {
                if seen.insert(l.to.clone()) {
                    q.push_back(l.to.clone());
                }
            }
        }
    }
    seen.len() == reg.systems.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_default;

    #[test]
    fn fixed_catalog_loads_stable() {
        let reg = load_default().expect("content");
        assert!(reg.systems.len() >= 100);
        let max_deg = reg.systems.values().map(|s| s.links.len()).max().unwrap();
        assert!(max_deg <= 5, "max degree {max_deg}");
        assert!(is_connected(&reg));
        assert!(reg.systems.contains_key("sys.sol"));
        assert!(reg.systems.contains_key("sys.alpha_centauri"));
    }
}
