# Fixed MMO galaxy

| File | Role |
|------|------|
| `catalog.json` | **Authoritative geography** — 1000 systems, jump links (≤5), generated stations. Same on every server. |
| `../galaxy.toml` | Parameters for the offline baker only (not used at runtime). |
| `../systems/*.toml` | **Story overlays** — hand-authored systems (names, lore, stations). Merged over the catalog. |
| `../stations/*.toml` | Hand-authored stations (Earth Orbit, Mars Depot, …). |

## Do not randomize at runtime

This is an MMO: players, wiki, and missions all need a stable map.  
The server **loads** `catalog.json`; it does not regenerate systems on boot.

## When to regenerate

Only when you intentionally redesign the whole mesh:

```powershell
cd C:\Users\User\Documents\nova-horizon
cargo run -p content --bin gen-galaxy
# review + commit content/galaxy/catalog.json
```

Adding lore or a special system usually means editing `content/systems/<name>.toml` (overlay), not regenerating.
