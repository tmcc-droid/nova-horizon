# Playtest guide (MVP complete)

## Prerequisites

- Docker Desktop (Postgres)
- Rust + MSVC Build Tools
- Godot 4.2+

## Start

```powershell
cd C:\Users\User\Documents\nova-horizon
.\scripts\dev_up.ps1
. .\scripts\env_msvc.ps1
$env:DATABASE_URL = "postgres://nova:nova@127.0.0.1:5432/nova_horizon"
$env:JWT_ACCESS_SECRET = "dev-secret"
cargo run -p game-server
```

Open `client/` in Godot → F5 → Register / Login & Play.

## Controls

| Input | Action |
|-------|--------|
| Arrows | Turn / thrust |
| Space | Fire hitscan |
| D | Dock nearest station |
| U | Undock |
| J | Jump Sol → Alpha Centauri |

## What to try

1. Fly between Earth Orbit and Mars Depot stations  
2. Dock (D) → buy/sell commodities → refuel → undock  
3. Fight orange pirates near Mars (Space)  
4. Jump (J) to Alpha Centauri, dock Proxima Port  
5. Second Godot client with another email = multiplayer  

## Automated

```powershell
cargo test --workspace
```
