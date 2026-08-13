# Protocol overview (PR-02)

Flat JSON over WebSocket text frames. See `crates/protocol` and `protocol/fixtures/`.

## Client → server

| `t` | Purpose |
|-----|---------|
| `AuthHello` | Bind WS to session + content/protocol version check |
| `ClockSyncRequest` | RTT / clock offset sample |
| `InputFrame` | Thrust/turn/fire only — **no pose** |
| `DockRequest` / `UndockRequest` | Station docking RPC |
| `TradeExecute` | Buy/sell at docked station |
| `RefuelRequest` | Station fuel service (not cargo) |
| `HyperspaceRequest` | Start jump FSM |
| `ChatSend` | Stretch |

## Server → client

| `t` | Purpose |
|-----|---------|
| `AuthOk` / `AuthFail` | Join result |
| `ClockSyncResponse` | Clock sample reply |
| `SelfState` | 20 Hz authoritative self + `last_processed_input_seq` |
| `EntitySpawn` / `EntityDespawn` / `EntitySnapshot` | World replication |
| `EventCombat` | Hits / death / fire denied |
| `DockResult` / `StationMenu` | Dock UX |
| `TradeResult` / `RefuelResult` | Economy RPC results |
| `JumpCountdown` / `JumpArrive` / `JumpRejected` | Hyperspace |
| `ServerNotice` | Generic notices |

RPC correlation uses `request_id` (UUID). Prediction uses `input_seq` only on input/self-state.
