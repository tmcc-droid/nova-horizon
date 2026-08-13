# Protocol JSON (source of truth notes)

MVP wire format: **one JSON object per WebSocket text frame**, flat envelope:

```json
{ "t": "MessageType", "v": 1, ...fields }
```

- `t` — message discriminant (PascalCase, matches Rust `WireMessage` variant names)
- `v` — protocol version (`1`)
- `request_id` — UUID on RPCs (dock/trade/refuel/jump); client may retry; server should be idempotent where noted
- `input_seq` — only on `InputFrame` / echoed as `SelfState.last_processed_input_seq`

## Station IDs

| Layer | Form | Example |
|-------|------|---------|
| Content / DB | string | `st.earth_orbit` |
| Wire | UUID v5(`STATION_NAMESPACE`, content_id) | see `protocol::station_wire_id` |

`STATION_NAMESPACE` = `6ba7b810-9dad-11d1-80b4-00c04fd430c8` (DNS namespace UUID; project may pin a custom v4 later — keep constant in code).

## Fixtures

Golden examples live in `../fixtures/*.json`. Rust tests in `crates/protocol` must round-trip all of them.

## Godot

Hand-synced encode/decode in `client/scripts/net/codec.gd`. Keep field names identical to fixtures.
