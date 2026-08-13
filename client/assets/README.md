# Client art assets

| Path | Use |
|------|-----|
| `ships/player_shuttle.jpg` | Player + ally ships (chroma key magenta) |
| `ships/pirate_raider.jpg` | Pirate NPCs |
| `stations/ring_station.jpg` | Stations |
| `ui/panel_frame.jpg` | HUD / login / station panels (9-slice) |

Loaded at runtime via `scripts/visuals/asset_loader.gd` which strips hot-pink/magenta backgrounds to alpha (no external image tools required).

Style: top-down orthographic, neon cyan / mint sci-fi, flat key background.
