# Auth & WebSocket API (PR-04 / PR-05)

Base URL default: `http://127.0.0.1:8080`  
WebSocket: `ws://127.0.0.1:8080/ws`

## REST

| Method | Path | Auth | Body | Result |
|--------|------|------|------|--------|
| POST | `/auth/register` | — | `{ email, password }` | tokens + session |
| POST | `/auth/login` | — | `{ email, password }` | tokens + session |
| POST | `/auth/refresh` | — | `{ session_id, refresh_token }` | rotated tokens |
| POST | `/characters` | Bearer | `{ name }` | character + starter ship |
| GET | `/characters` | Bearer | — | list |
| POST | `/auth/play` | refresh | `{ session_id, refresh_token, character_id }` | WS connect ticket |
| GET | `/health` | — | — | DB ping |
| GET | `/content/manifest` | — | — | content + protocol version |

Password: min 8 chars, argon2id.  
Access JWT: ~15 min (REST only).  
Session: server-side row; WS authorized by session + connect ticket hash, not JWT.

## WebSocket

1. Client connects to `/ws`
2. First text frame within 10s: `AuthHello`
3. Server replies `AuthOk` or `AuthFail`
4. Client may send `InputFrame`, `ClockSyncRequest`
5. Server sends `SelfState` at 20 Hz (stub integration until PR-07)

Rate limits: 5 WS connects / min / IP; 20 auth POSTs / min / IP.  
Dual login: new connection kicks previous for the same character.
