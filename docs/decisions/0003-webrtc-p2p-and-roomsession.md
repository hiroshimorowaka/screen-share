# ADR-0003: Peer-to-peer WebRTC, and a RoomSession seam in the web app

Date: 2026-08-28
Status: accepted (P2P since the pilot; RoomSession seam is new)

## Context

Video has to get from each sharer's browser to each viewer's browser. The
options are a media server (SFU/MFU) that every stream passes through, or
direct peer-to-peer connections with the server only introducing peers.

Separately: the web app's room page had grown to hold ~15 inline
`RwSignal`s and call `web-sys` WebRTC APIs directly inside a
`ServerMessage` match arm. Peer-connection lifecycle, signaling, and view
state were braided together in components and untestable.

## Decision

**Transport:** direct peer-to-peer WebRTC. The server only relays
signaling messages. Sharing and watching are decoupled — starting a share
opens no connections; a peer connection exists only while a specific
viewer is watching a specific sharer. A room with N sharers and M
watchers has exactly as many connections as there are watching
`(sharer, viewer)` pairs.

**Structure:** a `RoomSession` type in `apps/web/src/session/` owns the
`SignalingClient`, a `PeerConnectionManager` (the `(sharer, viewer)`
connection map plus the single teardown path), and `LocalMedia` (a
`SharingState` enum, not `is_sharing: bool` + a separate stream handle).
Leptos components call `session.start_sharing()` / `session.watch(peer)`
etc. and render read accessors — they never touch `web-sys` WebRTC APIs.
Browser calls go through the `#[cfg(hydrate)]` / `#[cfg(not(hydrate))]`
paired-function pattern so the package still builds for SSR.

## Consequences

- No media-server cost or bandwidth bottleneck; latency is minimal.
- Bandwidth at each sharer scales with the number of watchers (accepted
  for a small-group tool).
- NAT traversal needs STUN, and TURN as a fallback — hence the
  self-hosted `coturn` (see ADR-0002).
- The teardown path is defined once, in `PeerConnectionManager`, covering
  deliberate stop, the browser's own stop control, and connection drop.
- `RoomSession` is testable in isolation as far as the WASM/browser
  boundary allows; the rest is still hand-verified in a real browser.
