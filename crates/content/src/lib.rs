//! Content pack loading for Nova Horizon.
//!
//! Packs live under repo `content/`; client and server must share
//! `CONTENT_VERSION` at join.
//!
//! The galaxy is a **fixed** MMO catalog (`content/galaxy/catalog.json`), not
//! random per boot. Story systems overlay via `content/systems/*.toml`.

mod galaxy;

pub use galaxy::{
    apply_galaxy, load_generator_config, write_catalog, GalaxyCatalog, GalaxyConfig,
};

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

/// Manifest version string exchanged at join (must match client).
pub const CONTENT_VERSION: &str = "0.1.0-dev";

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("content version mismatch: server={server}, client={client}")]
    VersionMismatch { server: String, client: String },
    #[error("unknown content id: {0}")]
    UnknownId(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error in {path}: {msg}")]
    Parse { path: String, msg: String },
    #[error("content directory not found (set CONTENT_DIR or run from repo root)")]
    DirNotFound,
}

pub fn check_version(client_version: &str) -> Result<(), ContentError> {
    if client_version == CONTENT_VERSION {
        Ok(())
    } else {
        Err(ContentError::VersionMismatch {
            server: CONTENT_VERSION.into(),
            client: client_version.into(),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShipDef {
    pub id: String,
    pub display_name: String,
    pub mass: f32,
    pub hull_radius_wu: f32,
    pub cargo_capacity_mass: f32,
    pub base_shield: f32,
    pub base_armor: f32,
    pub base_energy: f32,
    pub energy_regen: f32,
    pub thrust: f32,
    pub max_speed: f32,
    pub turn_rate: f32,
    pub starting_fuel: i32,
    pub max_fuel: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationDef {
    pub id: String,
    pub system: String,
    pub x: f64,
    pub y: f64,
    pub dock_range_wu: f64,
    pub safe_radius_wu: f64,
    pub fuel_price_per_unit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemLink {
    pub to: String,
    pub fuel_cost: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDef {
    pub id: String,
    pub display_name: String,
    pub radius_wu: f64,
    #[serde(default)]
    pub stations: Vec<String>,
    #[serde(default)]
    pub links: Vec<SystemLink>,
    #[serde(default)]
    pub spawn_tables: Vec<String>,
    /// Galaxy-map X (arbitrary layout units).
    #[serde(default)]
    pub map_x: f64,
    /// Galaxy-map Y (arbitrary layout units).
    #[serde(default)]
    pub map_y: f64,
    /// `hub` | `populated` | `transit` (pass-through).
    #[serde(default)]
    pub kind: String,
    /// Optional lore blurb for station UI / codex (MMO story hooks).
    #[serde(default)]
    pub lore: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponDef {
    pub id: String,
    pub display_name: String,
    pub fire_group: u32,
    pub range_wu: f32,
    pub damage: f32,
    pub energy_cost: f32,
    pub cooldown_s: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommodityDef {
    pub id: String,
    pub display_name: String,
    pub mass_per_unit: f64,
}

#[derive(Debug, Clone)]
pub struct ContentRegistry {
    pub version: &'static str,
    pub ships: HashMap<String, ShipDef>,
    pub stations: HashMap<String, StationDef>,
    pub systems: HashMap<String, SystemDef>,
    pub weapons: HashMap<String, WeaponDef>,
    pub commodities: HashMap<String, CommodityDef>,
}

impl ContentRegistry {
    pub fn empty() -> Self {
        Self {
            version: CONTENT_VERSION,
            ships: HashMap::new(),
            stations: HashMap::new(),
            systems: HashMap::new(),
            weapons: HashMap::new(),
            commodities: HashMap::new(),
        }
    }

    pub fn load_from_dir(root: &Path) -> Result<Self, ContentError> {
        let mut reg = Self::empty();
        load_toml_dir(root.join("ships"), &mut reg.ships, |s: &ShipDef| {
            s.id.clone()
        })?;
        load_toml_dir(root.join("weapons"), &mut reg.weapons, |s: &WeaponDef| {
            s.id.clone()
        })?;
        load_toml_dir(
            root.join("commodities"),
            &mut reg.commodities,
            |s: &CommodityDef| s.id.clone(),
        )?;
        // Fixed MMO galaxy (identical every boot), then hand-authored story overlays
        galaxy::load_catalog(&mut reg, root)?;
        galaxy::overlay_hand_authored(&mut reg, root)?;
        info!(
            ships = reg.ships.len(),
            stations = reg.stations.len(),
            systems = reg.systems.len(),
            "content pack loaded"
        );
        Ok(reg)
    }

    /// Snapshot for client galaxy map / jump planner.
    pub fn galaxy_snapshot(&self) -> Vec<GalaxySystemView> {
        let mut out: Vec<GalaxySystemView> = self
            .systems
            .values()
            .map(|s| GalaxySystemView {
                id: s.id.clone(),
                name: s.display_name.clone(),
                map_x: s.map_x as f32,
                map_y: s.map_y as f32,
                kind: if s.kind.is_empty() {
                    "transit".into()
                } else {
                    s.kind.clone()
                },
                links: s
                    .links
                    .iter()
                    .map(|l| GalaxyLinkView {
                        to: l.to.clone(),
                        fuel_cost: l.fuel_cost,
                    })
                    .collect(),
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn ship(&self, id: &str) -> Result<&ShipDef, ContentError> {
        self.ships
            .get(id)
            .ok_or_else(|| ContentError::UnknownId(id.into()))
    }

    pub fn station(&self, id: &str) -> Result<&StationDef, ContentError> {
        self.stations
            .get(id)
            .ok_or_else(|| ContentError::UnknownId(id.into()))
    }

    pub fn system(&self, id: &str) -> Result<&SystemDef, ContentError> {
        self.systems
            .get(id)
            .ok_or_else(|| ContentError::UnknownId(id.into()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GalaxySystemView {
    pub id: String,
    pub name: String,
    pub map_x: f32,
    pub map_y: f32,
    pub kind: String,
    pub links: Vec<GalaxyLinkView>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GalaxyLinkView {
    pub to: String,
    pub fuel_cost: u32,
}

fn load_toml_dir<T, F>(
    dir: PathBuf,
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
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let typed: T = toml::from_str(&text).map_err(|e| ContentError::Parse {
            path: path.display().to_string(),
            msg: e.to_string(),
        })?;
        let id = id_of(&typed);
        out.insert(id, typed);
    }
    Ok(())
}

/// Load only hand-authored systems/stations (for offline catalog baking).
pub fn load_hand_seeds(reg: &mut ContentRegistry, root: &Path) -> Result<(), ContentError> {
    load_toml_dir(root.join("systems"), &mut reg.systems, |s: &SystemDef| {
        s.id.clone()
    })?;
    load_toml_dir(root.join("stations"), &mut reg.stations, |s: &StationDef| {
        s.id.clone()
    })?;
    Ok(())
}

/// Resolve content directory (CONTENT_DIR or repo-relative `content/`).
pub fn content_dir() -> Result<PathBuf, ContentError> {
    if let Ok(p) = std::env::var("CONTENT_DIR") {
        let pb = PathBuf::from(p);
        if pb.is_dir() {
            return Ok(pb);
        }
        return Err(ContentError::DirNotFound);
    }
    let candidates = [
        PathBuf::from("content"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content"),
    ];
    for c in candidates {
        if c.is_dir() {
            return Ok(c.canonicalize().unwrap_or(c));
        }
    }
    Err(ContentError::DirNotFound)
}

pub fn load_default() -> Result<ContentRegistry, ContentError> {
    let dir = content_dir()?;
    ContentRegistry::load_from_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_match_ok() {
        assert!(check_version(CONTENT_VERSION).is_ok());
    }

    #[test]
    fn version_mismatch_err() {
        assert!(check_version("0.0.0").is_err());
    }

    #[test]
    fn load_workspace_content() {
        let reg = load_default().expect("load content/");
        assert!(reg.ships.contains_key("ship.shuttle"));
        assert!(reg.stations.contains_key("st.earth_orbit"));
        assert!(reg.systems.contains_key("sys.sol"));
        let shuttle = reg.ship("ship.shuttle").unwrap();
        assert!(shuttle.thrust > 0.0);
        assert!(shuttle.max_speed > 0.0);
    }
}
