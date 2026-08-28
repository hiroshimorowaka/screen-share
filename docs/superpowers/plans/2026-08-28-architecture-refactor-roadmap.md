# Architecture Refactor Roadmap

> **For agentic workers:** this is the master roadmap, not an executable plan.
> Each phase below has its own detailed, bite-sized plan document
> (`docs/superpowers/plans/2026-08-28-refactor-phase-N-*.md`), written just
> before that phase is executed so it reflects the tree the previous phase
> produced. Execute phases in order; do not start phase N+1 until phase N's
> acceptance gate is green and committed.

**Goal:** Restructure the single `screen_share` crate into a Cargo workspace
with explicit domain / protocol / signaling / web-app / desktop boundaries,
relocate tests out of source files, and pull WebRTC + signaling wiring out
of the Leptos components into a testable `RoomSession` — without changing
any runtime behavior.

**Architecture:** A Cargo workspace. Pure domain types in `crates/core`;
the client↔server wire protocol in `crates/protocol` (depends only on
`core` + `serde`); the WebSocket relay server in `crates/signaling`
(depends on `core` + `protocol`); the Leptos isomorphic app in `apps/web`
(depends on `core` + `protocol` + `signaling`, still built by
`cargo-leptos`); the Electron shell reorganized under `desktop/` by
concern (`main` / `features` / `ipc` / `platform`), with
`desktop/native/windows-audio` kept as a standalone napi crate outside the
Rust workspace.

**Tech stack:** Rust 2021, Cargo workspace (`resolver = "2"`), Leptos 0.8 +
`leptos_axum`, Axum 0.8, Tokio, `wasm-bindgen` / `web-sys`, `cargo-leptos`
0.3.x, Electron 43 + TypeScript, napi-rs.

---

## Global Constraints

- **Language policy (CLAUDE.md §1):** English for all code, identifiers,
  comments, commit messages, branch names, docs. pt-BR only in
  conversation with the maintainer.
- **No behavior change.** This is a pure refactor. Every phase ends with
  the full existing test suite green and the app manually sanity-checked
  in a browser where the phase touched `apps/web` UI code.
- **`cargo-leptos` is the build authority for `apps/web`.** Never run the
  compiled binary directly to validate the web app; use
  `cargo leptos build` / `cargo leptos watch`. The server binary and the
  WASM bundle must agree on `LEPTOS_OUTPUT_NAME` (`screen_share`) — keep
  that value stable across the whole refactor so `.cargo/config.toml`,
  the `Dockerfile`, and `main.rs`'s `shell` wiring do not need to change.
- **Package name stays `screen_share`** even though its directory becomes
  `apps/web`. Directory name and Cargo package name are independent;
  keeping the package name avoids touching every `use screen_share::…`
  path and the `target/release/screen_share` binary name in the
  `Dockerfile` until a phase deliberately renames it.
- **Test command:** `cargo test --features ssr` from the workspace root
  must run every crate's tests. `cargo test -p <crate>` scopes to one.
- **Lint gate (CLAUDE.md §"Dependencies and lints"):**
  `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass
  before any phase is considered done. `#[allow]` only at item level with
  a one-line reason.
- **Dependency direction is an invariant, not a preference** — see
  "Dependency invariants" below. A phase that would violate it is wrong,
  not a judgment call.
- **Commit frequently**, one commit per completed step-group as each
  phase plan specifies. Every commit builds and passes tests.
- Co-author trailer on every commit:
  `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>`.

---

## Current state (baseline, 2026-08-28)

Single crate `screen_share` (`crate-type = ["cdylib", "rlib"]`), ~7.5k
lines of Rust (≈50% tests).

```
src/
├── lib.rs                     # pub mod signaling; pub mod ui; hydrate()
├── main.rs                    # #[cfg(ssr)] Axum bootstrap
├── signaling/
│   ├── mod.rs                 # protocol always; auth/registry/rooms_status/state/turn/ws are #[cfg(ssr)]
│   ├── protocol.rs   346 ln   # ClientMessage/ServerMessage/MemberInfo/… + 193 ln tests
│   ├── registry.rs   909 ln   # Registry + Room/Member + free fns + 505 ln tests
│   ├── auth.rs        42 ln   # argon2 hash/verify + tests
│   ├── rooms_status.rs        # GET /api/rooms/:code handler
│   ├── state.rs       24 ln   # SignalingState { registry, turn }
│   ├── turn.rs        93 ln   # TurnConfig::from_env + HMAC credential mint + tests
│   └── ws.rs         174 ln   # WebSocket upgrade + per-connection loop
└── ui/
    ├── app.rs, mod.rs
    ├── client/                # #[cfg(hydrate)] browser infra
    │   ├── webrtc.rs 397 ln   ├── socket.rs ├── storage.rs 200 ln
    │   ├── session.rs         ├── rooms_api.rs
    ├── components/            # generic: color_picker, icons, palette, status*
    ├── pages/
    │   ├── home/  (mod, create_room 190, join_room, recent_rooms)
    │   └── room/  (mod 369, connection 204, share 227, watch 143,
    │               member_card 389, grid 264, media_controls 257,
    │               message_handler 389, quality 509, latency, invite,
    │               room_check, dev_preview 395)
    ├── profile.rs, quick_share.rs
tests/
├── rooms_status.rs   68 ln    # integration: HTTP + WS
└── signaling_ws.rs  257 ln    # integration: WS relay

desktop/                       # Electron + TS, plus native/windows-audio napi crate
```

**Problems this refactor addresses:**

1. `signaling/protocol.rs` drags the whole `signaling/` module tree into
   the `wasm32` build graph; the browser only needs the wire enums.
2. `message_handler.rs` calls `new_peer_connection` / `create_offer` /
   `create_answer` directly inside the `ServerMessage` match arm;
   `room/mod.rs` declares ~15 `RwSignal`s inline. Peer-connection
   lifecycle, signaling, and view state are braided together in
   components — the real complexity, and untestable as-is.
3. Tests inflate their source files (`registry.rs` 909 ln → ~400 real;
   `protocol.rs` 346 → ~150 real; `quality.rs` 509 → ~397 real).
4. No stated dependency-direction rule, so an agent will eventually wire a
   component straight to the registry "because it works".
5. `desktop/src/` is a flat pile with ad-hoc `shared-types.ts` and
   `process.platform === "win32"` branches inline.

**Non-problems (do not "fix" these):**

- `desktop/release/`, `desktop/dist/` — already git-ignored
  (`desktop/.gitignore`). Leave alone.
- `desktop/native/windows-audio/Cargo.lock` — correct to keep; it is a
  standalone binary napi crate built by `electron-builder`, not a
  workspace member.
- `RUST_GUIDELINES.md` (untracked) — generic boilerplate that contradicts
  CLAUDE.md (it mandates Pico CSS / vanilla JS / "you will be fined
  $100"). Not authoritative. Recommend deleting it in Phase 0; do not
  follow it.

---

## Target layout

```
screenshare/
├── Cargo.toml                 # [workspace] only
├── Cargo.lock
├── .cargo/config.toml         # unchanged (workspace-wide)
├── rust-toolchain.toml        # (optional, add in Phase 1 to pin)
├── CLAUDE.md
├── README.md
│
├── crates/
│   ├── core/                  # pure domain — no async, no I/O, no framework
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ids.rs         # RoomCode, PeerId, DeviceId newtypes
│   │       ├── member.rs      # Nick, Color, Member view
│   │       ├── room.rs        # Room aggregate rules that are browser-agnostic
│   │       └── error.rs
│   │
│   ├── protocol/              # wire format — depends on: core, serde
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs      # ClientMessage
│   │       ├── server.rs      # ServerMessage
│   │       ├── media.rs       # QualityLevel, TurnCredentials
│   │       └── info.rs        # MemberInfo, WatcherInfo, LatencyInfo, RoomStatus, MAX_MEMBERS
│   │
│   └── signaling/             # relay server — depends on: core, protocol, axum, tokio, argon2, hmac…
│       └── src/
│           ├── lib.rs
│           ├── registry.rs    # Registry + Room/Member (server-side, mpsc senders)
│           ├── auth.rs
│           ├── turn.rs
│           ├── state.rs       # SignalingState
│           ├── rooms_status.rs
│           └── ws.rs
│
├── apps/
│   └── web/                   # Leptos isomorphic app — package name stays `screen_share`
│       ├── Cargo.toml         # [package.metadata.leptos] lives here OR in workspace root array
│       ├── public/            # moved from repo-root public/
│       ├── src/
│       │   ├── lib.rs         # hydrate()
│       │   ├── main.rs        # #[cfg(ssr)] Axum bootstrap, wires crates/signaling routes
│       │   ├── app.rs
│       │   ├── components/    # generic only: button/modal/status/color_picker/icons/palette
│       │   ├── infra/         # #[cfg(hydrate)] browser infra (was ui/client/)
│       │   │   ├── socket.rs  ├── storage.rs  ├── rooms_api.rs
│       │   │   └── webrtc.rs  # thin web-sys wrappers only
│       │   ├── session/       # NEW — RoomSession: owns signaling + peers + local media
│       │   │   ├── mod.rs     ├── peers.rs  ├── media.rs  └── handler.rs
│       │   └── features/
│       │       ├── home/      (page, create_room, join_room, recent_rooms)
│       │       ├── room/      (page, connection, share, watch, member_card,
│       │       │              grid, media_controls, quality, latency, invite,
│       │       │              room_check, dev_preview)
│       │       └── profile/
│       └── tests/             # per-crate integration tests for the web app (SSR-testable bits)
│
├── desktop/
│   ├── src/
│   │   ├── main/              (index, lifecycle, windows, tray)
│   │   ├── features/
│   │   │   ├── screen-share/  (service, picker, display-media)
│   │   │   └── audio-share/   (service, loopback, process-filter)
│   │   ├── ipc/               (channels, handlers, types)
│   │   └── platform/
│   │       ├── windows/       (audio native binding, loopback, process-filter)
│   │       └── linux/         (pipewire, process-filter)
│   └── native/
│       └── windows-audio/     # unchanged; standalone napi crate, own Cargo.lock
│
├── tests/                     # workspace-level cross-crate integration only
│   ├── signaling_ws.rs
│   └── rooms_status.rs
│
└── docs/
    ├── architecture/          # overview, signaling, webrtc, desktop
    ├── decisions/             # ADRs (0001…)
    └── superpowers/{specs,plans}/   # unchanged
```

### Notes on the target

- **`apps/web` is the only Cargo-workspace member with two build modes.**
  The `ssr` bin and the `hydrate` cdylib are still one package
  (`screen_share`). `cargo-leptos` config moves to a
  `[[workspace.metadata.leptos]]` array entry in the root `Cargo.toml`
  with `bin-package = "screen_share"` and `lib-package = "screen_share"`.
- **`crates/core` `Room` vs `crates/signaling` `Room` are different
  types.** `core::Room` holds only browser-agnostic rules (member cap,
  code shape, sharer/watcher set invariants). `signaling::Registry`'s
  internal `Room`/`Member` keep their `UnboundedSender<ServerMessage>` and
  `tokio::time::Instant` fields — those never move to `core`. If, when
  extracting, there turns out to be almost nothing browser-agnostic to
  lift, `core` shrinks to just the id/nick/color newtypes and that is an
  acceptable outcome — do not invent domain logic to justify the crate.
- **`desktop/native/windows-audio` stays OUT of the workspace.**
  Adding it as a member would force `wasm`/`ssr` feature unification
  headaches and it is Windows-only. Keep its `Cargo.lock`.

---

## Dependency invariants

These get added to `CLAUDE.md` verbatim in Phase 0 and are enforced by
`cargo`'s dependency graph thereafter.

```
core        →  (serde only)
protocol    →  core
signaling   →  core, protocol
apps/web    →  core, protocol, signaling
desktop     →  (nothing Rust in this workspace; talks to apps/web over the wire)
```

Rules (CLAUDE.md wording):

> **Dependency direction.** Dependencies point toward lower-level
> abstractions only. `core` depends on nothing but `serde`. `protocol`
> may depend on `core`, never the reverse. Domain and protocol code
> (`crates/core`, `crates/protocol`) must never depend on Axum, Tokio,
> `web-sys`, `wasm-bindgen`, Leptos, Electron, or any OS API.
>
> **UI components never do I/O.** A Leptos `#[component]` must not open a
> `WebSocket`, construct an `RTCPeerConnection`, call `getDisplayMedia`,
> touch `localStorage`, or reach into `crates/signaling`. It calls a
> method on a `RoomSession` (or a plain helper) and renders signals.
> Networking, signaling, and WebRTC lifecycle live in `apps/web/src/infra`
> and `apps/web/src/session`.
>
> **Platform code is isolated behind a platform-independent interface.**
> `process.platform` / `#[cfg(target_os)]` branching lives only under a
> `platform/` module that exposes one interface; the rest of the code
> depends on the interface, not the branch.

---

## Phase sequence

Each phase is a separate plan doc, executed and committed before the next
begins. Phases 1–4 are mechanical and low-risk (no behavior change, only
`cargo test` + `cargo leptos build` gate them). Phase 5 is the one with
real design content. Phase 6 is independent of the Rust side and could be
done in parallel by another person.

| # | Subsystem | Risk | Gate |
|---|-----------|------|------|
| 0 | Docs & invariants: CLAUDE.md dependency rules, `docs/architecture` + `docs/decisions` skeleton with ADR-0001..0004, delete `RUST_GUIDELINES.md` | trivial | `cargo fmt --check`; docs render |
| 1 | Cargo workspace + move Leptos crate to `apps/web/` (package name unchanged), move `public/`, wire `[[workspace.metadata.leptos]]` | low, mechanical | `cargo test --features ssr` green; `cargo leptos build` succeeds; app loads in browser |
| 2 | Extract `crates/protocol` (move the wire enums out of `signaling/protocol.rs`), repoint ~35 `use` sites | low | full suite green; `cargo tree -p screen_share` shows `apps/web` no longer pulls `signaling` for the wasm build |
| 3 | Extract `crates/signaling` (lib crate; `apps/web` ssr bin depends on it); move the WS/HTTP integration tests into it. **`crates/core` was evaluated and deferred** — no shared browser-agnostic domain logic; see ADR-0001's Update. | medium | full suite green; `main.rs` wires `signaling::ws::ws_handler` from the new crate | ✅ done (`42de1b2`) |
| 4 | Relocate tests: promote tested items to `pub(crate)`/`pub`, move `#[cfg(test)]` modules into each crate's `tests/` dir (or `src/<mod>/tests.rs` where private access is unavoidable), delete inline test modules | low | identical test count before/after (`cargo test -- --list \| wc -l`); full suite green | ✅ done (`2f5bbe2`) |
| 5a | Flatten `apps/web/src/ui/` → `app.rs` + `components/` + `infra/` (was `ui/client/`) + `features/` (was `ui/pages/`) | low, mechanical | full suite green; `cargo leptos build` + HTTP smoke | ✅ done (`8ad8194`) |
| 5b.1–5b.4 | Move the room runtime into `apps/web/src/session/`: `connection.rs`→`mod.rs` (`RoomConnection`→`RoomSession`), `message_handler.rs`→`handler.rs`, `share.rs`→`media.rs`, `quality.rs`+`latency.rs`→`session/`. Each step byte-for-byte, each with a 2-tab browser GATE. | medium | full suite green + maintainer GATE per step | ✅ done (`b942c13`, `d51def8`, `e93c739`, `d916fae`) |
| 5b.5 | RoomSession-via-context + method API replacing the free-fn shims + `RoomSignals`; `PeerConnectionManager` (one teardown path); `SharingState` enum | medium | full suite green; full 2-tab GATE | ⏸ **deferred** — see `2026-08-28-refactor-phase-5-roomsession.md` §5b.5 |
| 6 | Desktop reorg: `desktop/src` → `main/ features/ ipc/ platform/{windows,linux}`; kill inline `process.platform` branches behind a `platform` interface; give `shared-types.ts` contents real owners; `docs/architecture/desktop.md` | medium | `pnpm --dir desktop build` (tsc) clean; `pnpm --dir desktop start` launches; screen-share + audio-share manually verified on Linux (and Windows if available) | ✅ done (`a501572`) — needs maintainer Electron-launch check |

### Ordering rationale

- **0 before everything** so the invariants guide every later edit and
  reviews can cite them.
- **1 before 2–5** because the final directory layout must exist before
  moving code into it; doing the workspace move once, mechanically, is
  safer than shifting files repeatedly.
- **2 before 3** because `protocol` is the clean, obvious cut and it
  shrinks `signaling` before `signaling` itself is lifted.
- **3 before 4** so tests are relocated once, against the final crate
  boundaries, not moved twice.
- **4 before 5** so Phase 5 starts from source files that are already
  free of test bulk and easier to hold in context.
- **6 anytime after 0** — no dependency on the Rust phases.

---

## Per-phase specs

> Full bite-sized step lists are written into
> `2026-08-28-refactor-phase-N-*.md` immediately before executing phase N.
> The detailed Phase 1 plan already exists:
> `2026-08-28-refactor-phase-1-workspace.md`.

### Phase 0 — Docs & invariants

**Files:**
- Modify: `CLAUDE.md` — add a "Dependency invariants" subsection under
  §"Rust and Leptos coding practices" with the three rules from
  "Dependency invariants" above, plus a "Feature ownership" rule: *code
  belongs to the feature that owns it, not the technology it uses —
  `features/room/member_card.rs`, not `components/room_member_card.rs`.*
- Modify: `CLAUDE.md` §"Commands" — replace the single-crate command
  examples with the workspace forms (`cargo test --features ssr` from
  root still works; add `cargo test -p <crate>`; `cargo leptos …` is run
  from repo root).
- Create: `docs/architecture/overview.md` — one page: the four Rust
  boundaries + the desktop boundary + the dependency arrows (copy the
  ASCII graph from this roadmap).
- Create: `docs/architecture/{signaling,webrtc,desktop}.md` — stubs with
  a one-paragraph summary each and a "see ADR-000X" pointer.
- Create: `docs/decisions/0001-workspace-crate-split.md`,
  `0002-signaling-relay-architecture.md`,
  `0003-webrtc-p2p-and-roomsession.md`,
  `0004-desktop-electron-and-windows-native-audio.md` — short ADRs
  (Context / Decision / Consequences), capturing decisions already made
  (why Electron over Tauri — there is prior art in
  `docs/superpowers/specs/2026-08-21-tauri-*`; why P2P WebRTC; why a dumb
  central relay; why Rust/napi for Windows loopback audio).
- Delete: `RUST_GUIDELINES.md` (untracked; contradicts CLAUDE.md). Confirm
  with maintainer first since it is untracked and may be intentional.

**Acceptance:** `cargo fmt --check` still clean (no code touched);
CLAUDE.md and the new docs read coherently; `git status` shows only doc
changes.

### Phase 2 — Extract `crates/protocol`

**Files:**
- Create: `crates/protocol/Cargo.toml` (deps: `serde` with `derive`;
  `screen-share-core` path dep — added in Phase 3, so for Phase 2 use no
  `core` dep yet and keep the raw `String` fields as they are today).
- Create: `crates/protocol/src/lib.rs`, `client.rs`, `server.rs`,
  `media.rs`, `info.rs` — move the contents of `signaling/protocol.rs`
  verbatim, split by the grouping in "Target layout". Keep `MAX_MEMBERS`
  in `info.rs`, re-export at crate root.
- Delete: `src/signaling/protocol.rs`; remove `pub mod protocol;` from
  `signaling/mod.rs`.
- Modify: root `Cargo.toml` — add `crates/protocol` to `members`; add
  `screen-share-protocol` as a path dependency of `apps/web` and (later)
  `crates/signaling`.
- Modify: every `use crate::signaling::protocol::…` (15 files in
  `apps/web`, listed in the Phase 2 plan) → `use screen_share_protocol::…`.
- Modify: `tests/*.rs` — `screen_share::signaling::protocol::…` →
  `screen_share_protocol::…`.
- Move the 193 lines of `#[cfg(test)]` in the old `protocol.rs` into
  `crates/protocol/src/*` for now (Phase 4 relocates them to
  `crates/protocol/tests/`).

**Interfaces produced:** crate `screen_share_protocol` exporting
`ClientMessage`, `ServerMessage`, `MemberInfo`, `WatcherInfo`,
`LatencyInfo`, `TurnCredentials`, `QualityLevel`, `RoomStatus`,
`MAX_MEMBERS` — same names, same serde representations (verify with the
existing round-trip tests).

**Acceptance:** `cargo test --features ssr` green; `cargo leptos build`
succeeds; `cargo tree -p screen_share --target wasm32-unknown-unknown`
no longer lists `tokio`/`axum` pulled via a protocol path.

### Phase 3 — Extract `crates/core` + `crates/signaling`

**Files:**
- Create: `crates/core/{Cargo.toml,src/lib.rs,src/ids.rs,src/member.rs,
  src/room.rs,src/error.rs}`. Move id/nick/color newtypes here (today
  they are bare `String`s in `protocol` and `registry` — introduce
  `RoomCode`, `PeerId`, `DeviceId`, `Nick`, `HexColor` as
  `#[derive(Serialize, Deserialize)]` newtypes). `protocol` gains a
  `core` dep and switches its `String` fields to these types **only if**
  the serde representation is unchanged (`#[serde(transparent)]`
  newtypes). If that risks wire drift, keep `protocol` on `String` and
  have `core` newtypes used only server-side — decide in the Phase 3 plan
  with a round-trip test as the arbiter.
- Create: `crates/signaling/{Cargo.toml,src/lib.rs}` + move
  `auth.rs`, `registry.rs`, `rooms_status.rs`, `state.rs`, `turn.rs`,
  `ws.rs` from `src/signaling/` into `crates/signaling/src/`.
  `crates/signaling` deps: `core`, `protocol`, `axum`, `tokio`,
  `argon2`, `hmac`, `sha1`, `base64`, `uuid`, `rand`, `futures-util`.
- Delete: `src/signaling/` entirely; remove `pub mod signaling;` from
  `apps/web/src/lib.rs`.
- Modify: `apps/web/src/main.rs` — `use screen_share::signaling::…` →
  `use screen_share_signaling::…`. `apps/web` gains a
  `screen-share-signaling` path dep (behind the `ssr` feature).
- Modify: root `Cargo.toml` `members`; move the `ssr`-only server deps
  out of `apps/web` into `crates/signaling` (they are no longer needed by
  the web package directly except `axum`/`leptos_axum` for routing).
- Modify: `tests/*.rs` at repo root — repoint to `screen_share_signaling`.

**Acceptance:** `cargo test --features ssr` green; `cargo leptos build`
succeeds; `cargo build -p screen-share-core` compiles with only `serde`
in its dependency tree (`cargo tree -p screen-share-core` shows no
`tokio`/`axum`/`leptos`).

### Phase 4 — Relocate tests

**Approach (maintainer's choice: promote to `pub(crate)` + move to
`tests/`):**
- For each `#[cfg(test)] mod tests` in `crates/*` and `apps/web`:
  1. Identify items the tests touch that are currently private.
  2. Widen the minimum necessary: `pub(crate)` for same-crate needs;
     `pub` only for items an integration test in `tests/` must reach.
  3. Move the test module body to `crates/<name>/tests/<mod>.rs` (or
     `apps/web/tests/…`). Where a test genuinely must see a private
     internal (e.g. `Registry`'s private `lock_rooms`) and widening would
     leak implementation, keep *that* test in-crate but in a sibling file
     via `#[cfg(test)] #[path = "registry_tests.rs"] mod tests;` — noted
     per-case in the Phase 4 plan, not the default.
  4. Delete the now-empty inline module.
- Files with inline tests to process: `registry.rs` (25 tests),
  `protocol` (14), `quality.rs` (11), `grid.rs`, `member_card.rs`,
  `auth.rs`, `turn.rs`, `join_room.rs`, `palette.rs`, plus any in
  `webrtc.rs`.

**Acceptance:** test count identical before/after
(`cargo test --features ssr -- --list | grep -c ': test$'` matches the
baseline captured at the start of the phase); full suite green; no
`#[cfg(test)] mod tests` blocks remain inside `src/` files longer than a
single `#[path]` include line.

### Phase 5 — `RoomSession` extraction

**Files:**
- Create: `apps/web/src/session/mod.rs` — `RoomSession` struct owning:
  `signaling: SignalingClient` (wraps the `socket.rs` wrapper),
  `peers: PeerConnectionManager`, `local_media: LocalMedia`,
  and the reactive room state (roster, sharers, watchers, latency,
  quality) as fields it exposes read-only. Public API (names are
  binding for later phases):
  - `RoomSession::connect(room: RoomCode, auth: JoinAuth) -> RoomSession`
  - `fn start_sharing(&self) -> impl Future<Output = Result<(), ShareError>>`
  - `fn stop_sharing(&self)`
  - `fn watch(&self, sharer: PeerId)` / `fn unwatch(&self, sharer: PeerId)`
  - `fn set_quality(&self, sharer: PeerId, level: QualityLevel)`
  - read accessors: `fn members(&self) -> Signal<Vec<RoomMember>>`,
    `fn sharers(&self) -> Signal<HashSet<PeerId>>`, etc.
- Create: `apps/web/src/session/peers.rs` — `PeerConnectionManager`:
  the `(sharer, viewer)`-pair connection map, offer/answer/ICE handling,
  the single teardown path. Absorbs the WebRTC calls currently inline in
  `message_handler.rs`.
- Create: `apps/web/src/session/media.rs` — `LocalMedia`:
  `getDisplayMedia`, track ended-event handling, the `SharingState` enum
  (replacing `is_sharing: bool` + separate stream handle).
- Create: `apps/web/src/session/handler.rs` — the `ServerMessage`
  dispatch (was `message_handler.rs`), now calling `PeerConnectionManager`
  / `LocalMedia` methods instead of raw `web-sys`.
- Modify: `apps/web/src/features/room/*` — components drop their inline
  `RwSignal` soup and WebRTC/socket calls; they take a `RoomSession`
  (via context or prop) and render its accessors. `room/page.rs` (was
  `room/mod.rs`) constructs the `RoomSession` once.
- Move/rename: `ui/client/` → `apps/web/src/infra/`
  (`webrtc.rs` keeps ONLY thin `web-sys` wrappers — anything stateful
  moves to `session/`); `ui/components/` → `apps/web/src/components/`
  (generic only); `ui/pages/` → `apps/web/src/features/`.
- Keep the `#[cfg(hydrate)]` / `#[cfg(not(hydrate))]` paired-function
  pattern for every browser call `RoomSession` makes, so
  `crates/signaling`'s `ssr` build of the same package still compiles.

**Interfaces consumed:** `screen_share_protocol` messages;
`screen-share-core` newtypes.

**Acceptance:** `cargo test --features ssr` green; `cargo clippy
--all-targets -- -D warnings` clean; **manual browser test** (CLAUDE.md
"Testing approach" — no automation harness for this layer): two tabs,
one room; each shares; each watches the other; stop sharing via the
in-app button and via the browser's own "stop sharing" control; reload
one tab mid-session. Roster, watch buttons, teardown, and reconnect
behave exactly as on `main`. Record the check in the phase plan's
acceptance checklist.

### Phase 6 — Desktop reorg

**Files:**
- Restructure `desktop/src/` per "Target layout": `main/` (from
  `main.ts`, `lifecycle.ts`, `main-window.ts`, `tray.ts`), `features/
  screen-share/` (from `picker.ts`, `display-media-handler.ts`,
  `share.ts`), `features/audio-share/` (from `audio/*`), `ipc/` (from
  `audio/ipc-handlers.ts` + a new `channels.ts` enumerating every channel
  string), `platform/windows/` + `platform/linux/` (from `audio/windows/*`
  and `audio/pipewire.ts` / `audio/process-identity.ts`).
- Introduce `platform/index.ts` exposing one `AudioBackend` interface;
  `features/audio-share/service.ts` selects `windows` vs `linux`
  implementation once, at startup. Remove every other
  `process.platform === …` branch.
- Dissolve `shared-types.ts`: IPC payload types → `ipc/types.ts`; audio
  types → `features/audio-share/types.ts`.
- Update `desktop/package.json` `build.files` globs and `tsconfig.json`
  paths for the new tree. `native/windows-audio` unchanged.
- Create: `docs/architecture/desktop.md` — the Electron/native boundary
  diagram.

**Acceptance:** `pnpm --dir desktop build` (tsc) clean; `pnpm --dir
desktop start` launches the shell; screen-share and (Linux) audio-share
manually verified; Windows path verified if a Windows machine is
available, otherwise noted as untested in the phase plan.

---

## Risks & mitigations

| Risk | Mitigation |
|------|------------|
| `cargo-leptos` misbehaves in a workspace (wrong `site-root`, wasm name) | Phase 1 does the workspace move in isolation with `cargo leptos build` + browser load as the gate, nothing else. `LEPTOS_OUTPUT_NAME` held constant. Revert is `git revert` of one mechanical commit. |
| Wire-format drift when introducing `core` newtypes into `protocol` | The existing serde round-trip tests in `protocol` are the arbiter. If `#[serde(transparent)]` newtypes don't produce byte-identical JSON, `protocol` stays on `String` and newtypes are server-only. |
| Phase 5 changes behavior subtly (teardown races, ICE ordering) | Phase 5 is the only phase with a mandatory manual multi-tab browser checklist in its acceptance gate. Keep the single teardown path (CLAUDE.md "same per-connection teardown path") as an explicit invariant in the `PeerConnectionManager` doc comment. |
| Big-bang churn makes review impossible | Six phases, each its own PR/commit range, each independently green. No phase starts before the previous is merged. Phases 1–4 are mechanical (diff is mostly moves). |
| `core` turns out near-empty | Accepted outcome — see "Notes on the target". Do not fabricate domain logic. If it would hold only 4 newtypes, fold them into `protocol` and drop `core` (decide in Phase 3 plan). |
| Desktop reorg conflicts with in-flight audio work | Phase 6 is independent; schedule it when `desktop/` has no open branch, or do it first. |

---

## Self-review

- **Spec coverage:** every point the maintainer raised maps to a phase —
  workspace + `apps/`/`crates/` (1,3), `protocol` crate (2), `core`
  crate (3), `signaling` as app (3), feature-oriented web (5),
  dependency-direction rules in CLAUDE.md (0), tests split from source
  (4), `RoomSession` / WebRTC-out-of-UI (5), desktop `main/features/ipc/
  platform` (6), platform-isolated audio backend (6), `shared-types.ts`
  cleanup (6), ADRs (0), `release/` in git (non-issue, noted). Full
  `apps/`+`crates/` layout and `pub(crate)`+`tests/` test strategy and
  RoomSession-now and desktop-included all reflect the maintainer's four
  answers on 2026-08-28.
- **No placeholders:** per-phase specs name concrete files and the
  binding public API for `RoomSession`; step-level detail is deferred to
  each phase's own plan doc by design (stated at the top).
- **Type consistency:** `RoomSession` method names here
  (`start_sharing`, `stop_sharing`, `watch`, `unwatch`, `set_quality`)
  are the contract Phase 5's plan must use verbatim. Crate names:
  `screen-share-core` (package) / `screen_share_core` (import),
  `screen-share-protocol` / `screen_share_protocol`,
  `screen-share-signaling` / `screen_share_signaling`, `screen_share`
  (web package, unchanged).
