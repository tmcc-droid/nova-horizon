# Nova Horizon — MVP Complete

Top-down 2D Escape Velocity–style multiplayer (MMO-capable architecture).

**Design:** [`docs/design/design-doc-v0.3.md`](docs/design/design-doc-v0.3.md)

## Stack

| Layer | Choice |
|-------|--------|
| Client | Godot 4 (prediction + REST/WS) |
| Server | Rust modular monolith |
| Protocol | JSON WebSocket |
| DB | PostgreSQL |

## Features (MVP)

- Auth (register/login/session) + characters + starter shuttle  
- 20 Hz authoritative flight, multi-client visibility  
- Dock / undock, station market buy/sell, refuel  
- Hitscan combat, safe zones, death → docked respawn + 50% cargo loss  
- Pirate NPCs near Mars  
- AOI interest (same system; grid when crowded)  
- Hyperspace Sol ↔ Alpha Centauri (fuel cost)  
- Ship position persistence (10 s + disconnect)  
- Metrics stub at `/metrics`  

## Run

```powershell
cd C:\Users\User\Documents\nova-horizon
copy .env.example .env
.\scripts\dev_up.ps1
. .\scripts\env_msvc.ps1
$env:DATABASE_URL = "postgres://nova:nova@127.0.0.1:5432/nova_horizon"
$env:JWT_ACCESS_SECRET = "dev-secret"
cargo run -p game-server
```

Godot 4: open `client/` → F5 → Login & Play.

### Controls

| Key | Action |
|-----|--------|
| Arrows | Turn / thrust |
| Space | Fire |
| D | Dock nearest station |
| U | Undock |
| G / J | Galaxy map (pick linked system to jump) |
| M | Cycle radar range |

Galaxy: **fixed** 1000-system MMO map (`content/galaxy/catalog.json`, max 5 links). Open map, select a cyan neighbor, Jump. Story systems overlay via `content/systems/*.toml`.

### Tests

```powershell
. .\scripts\env_msvc.ps1
cargo test --workspace
```

## Layout

```text
client/          Godot
content/         TOML defs (2 systems, ships, weapons, commodities)
protocol/        JSON fixtures
crates/          protocol, content, db, sim
services/game-server/
migrations/
docs/
```

## PR status

| PR | Status |
|----|--------|
| 00–07 | Done (stack → multiplayer flight) |
| 08–10 | Done (dock, trade/refuel, persist) |
| 11–13 | Done (combat, NPCs, AOI) |
| 14–15 | Partial shipyard API in DB; jump done |
| 16–18 | Metrics + loadtest stub + this polish |

MIT — original spiritual successor only (not affiliated with Ambrosia / EV Nova).
