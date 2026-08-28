# Signaling

Two browsers never exchange video through the server. They exchange a
handful of small JSON messages (session descriptions, ICE candidates)
over a WebSocket — just enough for each side to open a direct WebRTC
connection to the other.

## Split of responsibility

- **Wire format and routing** live on the server (`crates/protocol` for
  the enums, `crates/signaling` for the relay). The server has no opinion
  about what an "offer" or an "ice candidate" *means*.
- **Meaning and behavior** live in the browser (`apps/web/src/session`).
  The client constructs and reacts to these messages.

The message shapes are one pair of Rust enums (`ClientMessage`,
`ServerMessage`) shared verbatim by both sides, so the protocol cannot
drift.

## The relay

`crates/signaling::Registry` is an in-memory map: room code → connected
peers, plus per-room sharer and watcher sets, latency samples, and a
wrong-password attempt log. It relays a message from one named peer to
another and broadcasts roster/sharer/watcher changes. It also:

- hashes and verifies room passwords with `argon2`;
- rate-limits wrong-password attempts **per client** (keyed by
  `Fly-Client-IP`, not the client-supplied `device_id`), so one attacker
  cannot lock out everyone joining the same room;
- keeps an emptied room reservable for a 30s grace period so a page
  reload does not lose the code;
- mints short-lived TURN credentials (HMAC over an expiry, `coturn`
  `use-auth-secret` scheme).

## Endpoints

| Route | Purpose |
|-------|---------|
| `GET /ws` | WebSocket upgrade; carries the `ClientMessage` / `ServerMessage` protocol |
| `GET /api/rooms/:code` | Plain HTTP room-existence check, used before showing the nick/password form |

See [ADR-0002](../decisions/0002-signaling-relay-architecture.md).
