//! Offline tool: bake a fixed MMO galaxy catalog.
//!
//!   cargo run -p content --bin gen-galaxy
//!
//! Writes `content/galaxy/catalog.json`. Re-running reshapes the universe —
//! only do that intentionally (all shards share this file).

use std::collections::HashSet;

use content::{
    apply_galaxy, content_dir, load_generator_config, load_hand_seeds, write_catalog,
    ContentRegistry,
};

fn main() {
    let root = content_dir().expect("content dir (run from repo root or set CONTENT_DIR)");
    println!("content root: {}", root.display());

    let cfg = load_generator_config(&root);
    println!(
        "generator: seed={} systems={} max_links={} populated={}",
        cfg.seed, cfg.system_count, cfg.max_links, cfg.populated_fraction
    );

    let mut reg = ContentRegistry::empty();
    load_hand_seeds(&mut reg, &root).expect("load content/systems + stations");
    let hand_stations: HashSet<String> = reg.stations.keys().cloned().collect();
    println!(
        "hand seeds: {} systems, {} stations",
        reg.systems.len(),
        reg.stations.len()
    );

    apply_galaxy(&mut reg, &cfg);

    let path = write_catalog(&reg, &root, &hand_stations).expect("write catalog.json");
    println!(
        "wrote {}\n  systems: {}\n  catalog stations (generated): {}\n  hand stations (toml only): {}",
        path.display(),
        reg.systems.len(),
        reg.stations.len() - hand_stations.len(),
        hand_stations.len()
    );
    println!("Commit catalog.json — every server loads the same fixed galaxy.");
}
