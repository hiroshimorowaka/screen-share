# Architecture overview

A persistent room where a small group shares screens peer-to-peer. The
server only introduces peers; video never flows through it.

## Boundaries

```
                 ┌─────────────┐
                 │    core     │   pure domain types (ids, nick, color,
                 └──────▲──────┘   browser-agnostic room rules) — serde only
                        │
                 ┌──────┴──────┐
                 │  protocol   │   client↔server wire enums
                 └──────▲──────┘   (ClientMessage / ServerMessage / info)
                        │
                 ┌──────┴──────┐
                 │  signaling  │   in-memory registry + WebSocket relay +
                 └──────▲──────┘   argon2 auth + TURN credential minting (Axum/Tokio)
                        │
              ┌─────────┴─────────┐
              │     apps/web      │   Leptos isomorphic app: SSR bin + WASM
              │  (Leptos + WebRTC)│   hydrate. Owns RoomSession (signaling +
              └─────────┬─────────┘   peer connections + local media).
                        │  wire protocol only
              ┌─────────┴─────────┐
              │     desktop       │   Electron shell around apps/web.
              │  (Electron + TS)  │   Platform audio backends behind one
              └───────────────────┘   interface. native/windows-audio: napi/WASAPI.
```

Dependencies point downward only. See `CLAUDE.md` §"Dependency
invariants" for the enforced rules.

## Key decisions

| Topic | ADR |
|-------|-----|
| Why a Cargo workspace with these crate seams | [0001](../decisions/0001-workspace-crate-split.md) |
| Why a dumb central relay instead of a smart server | [0002](../decisions/0002-signaling-relay-architecture.md) |
| Why peer-to-peer WebRTC, and the `RoomSession` seam | [0003](../decisions/0003-webrtc-p2p-and-roomsession.md) |
| Why Electron (not Tauri), and Rust/napi for Windows audio | [0004](../decisions/0004-desktop-electron-and-windows-native-audio.md) |
| Layered automated tests, mutation testing, CI as the gate | [0005](../decisions/0005-quality-gate.md) |

## Runtime shape

- One process serves both the SSR HTML (`leptos_axum`) and the `/ws` +
  `/api/rooms/:code` signaling endpoints (`main.rs` merges the routers).
- The registry is in-memory. A room is reservable for a short grace
  period after its last member leaves, then dropped.
- ICE uses STUN by default; a self-hosted `coturn` runs alongside the app
  in the same container and is used only when `TURN_SECRET` /
  `TURN_EXTERNAL_IP` are set (`docker-entrypoint.sh`).
- All runtime config comes from environment variables at process start;
  nothing is bundled in the deployed artifact.
