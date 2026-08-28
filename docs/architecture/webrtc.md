# WebRTC and RoomSession

## Model

Sharing and watching are decoupled, Discord-style. Starting a share opens
no connections — it flips a flag every member sees. A peer connection
between a sharer and a viewer exists only while that specific viewer has
chosen to watch that specific sharer. A room with several sharers and
several watchers has as many independent connections as there are
`(sharer, viewer)` pairs currently watching — not one mesh per sharer.

One teardown path handles every way a session ends — stopping
deliberately, using the browser's own screen-share control, or the
connection dropping — so there is a single place that owns "what happens
when this sharer stops".

## RoomSession seam (target after refactor Phase 5)

Leptos components do not touch `web-sys` WebRTC APIs. They hold a
`RoomSession` and render its reactive accessors. `RoomSession` owns:

- `SignalingClient` — the typed WebSocket wrapper;
- `PeerConnectionManager` — the `(sharer, viewer)` connection map,
  offer/answer/ICE handling, the single teardown path;
- `LocalMedia` — `getDisplayMedia`, the track-ended handler, and a
  `SharingState` enum (no `is_sharing: bool` + separate stream handle
  that can fall out of sync).

Component-facing API: `start_sharing()`, `stop_sharing()`, `watch(peer)`,
`unwatch(peer)`, `set_quality(peer, level)`, plus read accessors for the
roster, sharers, watchers, latency, and quality.

Every browser call goes through the `#[cfg(hydrate)]` /
`#[cfg(not(hydrate))]` paired-function pattern so the same package still
compiles for the SSR target.

See [ADR-0003](../decisions/0003-webrtc-p2p-and-roomsession.md).
