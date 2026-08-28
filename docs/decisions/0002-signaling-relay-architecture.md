# ADR-0002: A dumb central relay, not a smart signaling server

Date: 2026-08-28
Status: accepted (documents a decision in place since the pilot)

## Context

WebRTC needs a signaling channel for peers to exchange session
descriptions and ICE candidates before a direct connection exists. That
channel could be anything from a thin message pipe to a stateful server
that models rooms, calls, and media state authoritatively.

## Decision

The server side of signaling is intentionally minimal. `crates/signaling`
holds an in-memory `Registry` mapping a room code to connected peers and
relaying a message from one named peer to another. It has no opinion about
what an "offer" or an "ice candidate" *means* — that interpretation lives
entirely in the browser (`apps/web/src/session`).

The server does own the things that genuinely need a trusted, shared
vantage point:

- room existence, membership, and the 10-member cap;
- `argon2` password hashing/verification;
- per-client wrong-password rate limiting (keyed by `Fly-Client-IP`, not
  the client-controlled `device_id`);
- the roster / sharer / watcher broadcast fan-out;
- short-lived TURN credential minting.

There is no host role — every member is equal. A room is removed only
when its last member leaves (after a short grace period so a reload does
not lose the code).

## Consequences

- The protocol (`crates/protocol`) is one pair of enums shared verbatim
  by both sides; it cannot drift.
- New signaling behavior is split the same way: wire format + routing on
  the server, meaning + behavior on the client.
- The server holds no media state and no per-call state machine, so it
  stays small and cheap to test with plain integration tests.
- Scaling past one process would require externalizing the registry
  (out of scope for the pilot).
