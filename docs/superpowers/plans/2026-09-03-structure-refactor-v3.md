# Structure Refactor v3 — Server Crate, `client/` Umbrella, Feature Slices

> **For agentic workers:** REQUIRED SUB-SKILL: use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan phase-by-phase,
> task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> This continues the work on branch
> `worktree-refactor+structure-and-code-design` from commit `6c9a3aa`
> (the branch's steps 1–8 — dispatch split, foldered tests, `crates/domain`
> seed, `RoomGate`/`MemberCard`, `SharingState`, the three step-8 seam
> traits — are **already done**). The rationale, the current-state audit,
> and the target trees are in the v3 review report
> (`claude.ai/code/artifact/50e7803c-4457-4f10-8076-4886f4379047`).

**Goal:** Make "what is the back-end" answerable by a directory, and give
the web UI a React-style `pages/` + feature-slice layout, without changing
any runtime behavior.

**Architecture:** Promote the Axum binary out of `apps/web` into a new
`apps/server` crate, leaving `apps/web` as a pure Leptos UI library.
Inside `apps/web`, gather every browser-only module under one
`#[cfg(feature = "hydrate")]` umbrella (`client/`), split the 564-line
`webrtc` module by responsibility, dissolve the `session/` grab-bag into
`room/` and `home/` feature slices, replace the hand-threaded
`RoomSignals`/`MemberCardSignals` context structs with one
`provide_context`-delivered `RoomState`, move browser-free logic into
`crates/domain`, and introduce typed identifiers in `crates/protocol`.

**Tech stack:** Rust 2021, Cargo workspace (`resolver = "2"`), Leptos 0.8
+ `leptos_axum`, Axum 0.8, Tokio, `wasm-bindgen`/`web-sys`, `cargo-leptos`
0.3.x, `thiserror`.

---

## Global Constraints

Every task's requirements implicitly include this section. Values are
copied verbatim from `CLAUDE.md`, `RUST_GUIDELINES.md`, and
`docs/superpowers/plans/2026-08-28-architecture-refactor-roadmap.md`.

- **No behavior change.** This is a pure refactor. Every phase ends with
  the full existing suite green and the app hand-checked in a browser
  where the phase touched `apps/web` UI code.
- **Build authority is `cargo-leptos`.** Validate the web app only via
  `cargo leptos build` / `cargo leptos watch`, never the bare binary.
- **`LEPTOS_OUTPUT_NAME` stays `screen_share`.** The wasm bundle name is
  wired through `.cargo/config.toml`, `HydrationScripts`, the `Dockerfile`
  and `fly.toml`. Do not change it in any phase.
- **`apps/web` package name stays `screen_share`.** Its directory stays
  `apps/web`. Only the *binary* moves out.
- **Test gate:** `scripts/test-all.sh --no-mutants` must pass before a
  phase is "done" — it runs `cargo fmt --check`, clippy (ssr + hydrate,
  incl. `--tests`), `cargo leptos build`, the Rust suite, the wasm suite,
  and the Playwright suites. Narrow with a target while iterating
  (`scripts/test-all.sh rust`, `… e2e-web`, `… lint`; `--help` lists
  them). **Do not** run individual `cargo test` / `cargo clippy` /
  `playwright test` by hand.
- **Test count must not silently drop.** Removing a test is a deliberate,
  explained change. Record the baseline count at the start of each phase.
- **Lint:** `cargo clippy --workspace --all-targets --features ssr --
  -D warnings` clean; the wasm clippy
  (`-p screen_share --target wasm32-unknown-unknown
  --no-default-features --features hydrate -- -D warnings`) clean; `cargo
  fmt --check` clean. `#[allow]` only at item level with a one-line
  reason. **The `clippy::too_many_lines` / `too_many_arguments`
  allow-list in `apps/web` may only shrink across this plan, never grow.**
- **Tests are mandatory.** Every behavior-preserving move still ships the
  tests that already covered the moved code, relocated with it. Any new
  seam or type ships its own unit tests. The impossible-to-test exception
  (real screen/window capture, system audio, the browser's own share
  controls, two-tab WebRTC teardown races) still applies — state it
  explicitly when invoked.
- **Language:** English for all code, identifiers, comments, commit
  messages, docs. pt-BR only in conversation with the maintainer.
- **Commit trailer** on every commit:

  ```
  Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01P59S2rxh7tTBGbw8Wwi16R
  ```

- **Commits, pushes, merges are maintainer-gated.** "The task is done" is
  not approval to commit. Present the finished, verified change and wait.
- **Work on `worktree-refactor+structure-and-code-design`.** Do not
  branch off it per-phase unless the maintainer asks; commit directly to
  it, one commit per completed task-group as each phase specifies.

---

## Dependency Invariants

Enforced by the graph once the crates exist; a phase that would violate
this is wrong, not a judgment call.

```
crates/protocol   →  serde                        (+ its own `ids` module, Phase 2)
crates/domain     →  protocol                      (gains this dep in Phase 3, for QualityLevel)
crates/signaling  →  protocol, domain
apps/web          →  protocol, domain              (lib only, after Phase 1)
apps/server       →  signaling, protocol, screen_share (= apps/web), leptos_axum, axum, tokio
```

- `crates/domain` must never depend on `web-sys`, `wasm-bindgen`,
  `js-sys`, `axum`, `tokio`, `leptos`, or any OS API. Only `protocol`
  (and, transitively, `serde`).
- `crates/protocol` must never depend on anything but `serde`.
- **Correction to the v3 report:** the typed identifiers land in
  `crates/protocol` (`protocol::ids`), **not** `crates/domain`. Reason:
  the wire structs in `protocol` (`MemberInfo.peer_id`,
  `ClientMessage::JoinRoom.room`, …) are exactly what should carry the
  newtypes, and `protocol` cannot depend on `domain` because `domain`
  already depends on `protocol` (`quality::tier_for` takes
  `protocol::QualityLevel`). `protocol` is the base crate; typed ids are
  serde value types; they belong there. `domain` and `apps/web` both
  re-use `protocol::ids`. The maintainer's intent — "typed identifiers in
  the shared core, not bare `String` in `apps/web`" — is fully honored.
- **`best_column_count` stays in the UI** (`apps/web/src/room/layout.rs`),
  not `domain` — it is grid geometry, a UI concern (maintainer
  correction).

---

## File Structure — end state

After Phase 8. `(NEW)` = created by this plan, `(MOVED)` = relocated,
`(SPLIT)` = one file becomes several, everything else unchanged.

```
Cargo.toml                         # [workspace] members += "apps/server"; [[workspace.metadata.leptos]] bin/lib split
Dockerfile                         # builder copies target/release/screen-share-server
apps/
├── server/                        # (NEW) THE BACK-END
│   ├── Cargo.toml                 # package "screen-share-server", one [[bin]], feature "ssr"
│   └── src/
│       ├── main.rs                # (MOVED from apps/web/src/main.rs) tokio::main → build router → serve
│       ├── router.rs              # (NEW) compose leptos_routes + signaling_router + fallback + DoS guards
│       ├── config.rs             # (NEW) env vars (bind addr, LEPTOS_*, TURN secret) in one struct
│       └── middleware/
│           ├── mod.rs             # (NEW) re-export
│           ├── security.rs        # (MOVED from apps/web/src/http_security.rs)
│           └── limits.rs          # (MOVED from apps/web/src/http_limits.rs)
└── web/
    ├── Cargo.toml                 # loses [[bin]] / bin-only deps (axum, tower, tower-http, leptos_axum, tokio move to apps/server)
    └── src/
        ├── lib.rs                 # module decls + hydrate()   (no more main.rs)
        ├── config.rs             # (NEW) client build-time config (empty stub if none needed)
        ├── error.rs              # (NEW) app-level `enum AppError` (thiserror) — the unified error type
        ├── app/
        │   ├── mod.rs             # (NEW) re-export
        │   ├── app.rs             # (MOVED from src/app.rs) <App>, shell(), <Stylesheet>/<Title>/<meta>
        │   └── router.rs          # (NEW) the two <Routes> variants (was app.rs::app_routes)
        ├── pages/                 # (NEW) one thin component per route
        │   ├── mod.rs
        │   ├── home.rs            # <HomePage> — compose home/ components
        │   ├── room.rs            # <RoomPage> — params + auth gate + provide_context(RoomState)
        │   └── not_found.rs       # (MOVED from features/not_found.rs)
        ├── room/                  # (NEW) the room feature slice (isomorphic)
        │   ├── mod.rs
        │   ├── state.rs           # RoomState + RosterStore/WatchStore/MediaStore/DiagnosticsStore/ConnectionStore
        │   ├── participant.rs     # RoomMember (renamed Participant) + avatar/color helpers  (MOVED from session/mod.rs + palette bits)
        │   ├── layout.rs          # best_column_count + adaptive-grid recompute  (MOVED from features/room/grid)
        │   ├── messages.rs        # ServerMessage dispatch: one fn per message  (MOVED from session/handler)
        │   ├── connection.rs      # setup_room_connection / adopt_pending_session / reconnect wiring  (MOVED from session/mod.rs + session/reconnect)
        │   ├── actions/           # (NEW folder — see "actions/ and effects/ sizing policy")
        │   │   ├── mod.rs         #   re-export only
        │   │   ├── share.rs       #   start/stop own share
        │   │   ├── source.rs      #   switch shared source
        │   │   ├── watch.rs       #   start/stop watching a peer
        │   │   └── leave.rs       #   leave room / stop-watching-or-leave
        │   ├── effects/           # (NEW folder — see sizing policy)
        │   │   ├── mod.rs         #   re-export + install_all(state)
        │   │   ├── audio_selftest.rs
        │   │   ├── outgoing_mute.rs
        │   │   ├── invite_autocopy.rs
        │   │   └── quick_share.rs
        │   └── components/
        │       ├── mod.rs
        │       ├── stage.rs             # the authenticated room view (was features/room/mod.rs::RoomPage body)
        │       ├── gate.rs              # (MOVED from features/room/gate.rs) pre-auth panels
        │       ├── participant.rs       # (MOVED from features/room/member_card/mod.rs) one card
        │       ├── participant_grid.rs  # (MOVED from features/room/grid) the grid container
        │       ├── toolbar.rs           # (NEW) the control bar (was RoomPage's control-bar markup)
        │       └── transmission_menu.rs # (MOVED from components/transmission_menu.rs)
        ├── home/                  # (NEW) the home feature slice
        │   ├── mod.rs
        │   ├── actions/
        │   │   ├── mod.rs
        │   │   ├── create.rs      # create_room handler + after-mount loaders  (MOVED from features/home/create_room.rs)
        │   │   ├── join.rs        # join_room handler  (MOVED from features/home/join_room)
        │   │   └── recent.rs      # recent-rooms load + prune  (MOVED from features/home/recent_rooms.rs)
        │   └── components/
        │       ├── mod.rs
        │       ├── create_panel.rs
        │       ├── join_panel.rs
        │       └── recent_rooms.rs
        ├── profile.rs             # (MOVED from features/profile.rs) — shared by home + room
        ├── components/            # generic, domain-free
        │   ├── mod.rs
        │   ├── ui/                # (MOVED from components/) button, input, dialog, color_picker, icons, status_indicator, status_message
        │   │   └── mod.rs
        │   └── layout/
        │       ├── mod.rs
        │       └── header.rs      # wordmark / lobby bar
        └── client/               # (NEW) BROWSER-ONLY — #[cfg(feature = "hydrate")]
            ├── mod.rs
            ├── webrtc/           # (SPLIT from infra/webrtc/mod.rs, 564 lines)
            │   ├── mod.rs
            │   ├── peer.rs        # new_peer_connection, reserve_audio_mline, teardown
            │   ├── connection.rs  # create_offer/create_answer/accept_answer/add_ice_candidate
            │   ├── media.rs       # play_stream_in, combine_video_and_audio, track wiring
            │   ├── screen_share.rs# capture_display + display_media_constraints
            │   ├── stats.rs       # read getStats → domain::quality::RawReading
            │   └── error.rs       # enum WebRtcError (thiserror)
            ├── seam/             # (MOVED from infra/{signaling_transport,display_capture,peer_link}.rs)
            │   ├── mod.rs
            │   ├── signaling_transport.rs
            │   ├── display_capture.rs
            │   └── peer_link.rs
            ├── socket.rs         # (MOVED from infra/socket.rs) WsClient transport
            ├── storage.rs        # (MOVED from infra/storage/) localStorage/sessionStorage
            ├── dom.rs            # (MOVED from infra/dom/)
            ├── rooms_api.rs      # (MOVED from infra/rooms_api.rs) GET /api/rooms/:code
            ├── pending_session.rs# (MOVED+RENAMED from infra/session.rs)
            └── desktop_bridge.rs # (MOVED) quick_share.rs + infra/webrtc::notify_desktop_* + audio loopback
crates/
├── protocol/
│   └── src/
│       ├── lib.rs               # re-export ids
│       └── ids.rs               # (NEW) PeerId, RoomCode, Nick, HexColor, IdError
├── domain/
│   └── src/
│       ├── lib.rs
│       ├── ids.rs               # (NEW) `pub use screen_share_protocol::ids::*;` re-export for callers that only touch domain
│       ├── backoff/             # unchanged
│       ├── sdp/                 # unchanged
│       ├── quality.rs           # (MOVED from apps/web/src/session/quality/mod.rs — the pure half)
│       └── status.rs            # (MOVED from apps/web/src/components/status.rs) status_meta
└── signaling/                   # unchanged in this plan
public/styles/                   # + @layer directive (Phase 7); files otherwise unchanged
```

---

## `actions/` and `effects/` sizing policy

The maintainer's explicit constraint: these must not become large or
hard to follow. Both are **folders from creation, never single files.**

- **`room/actions/`** — one *user-initiated capability* per file:
  `share.rs`, `source.rs`, `watch.rs`, `leave.rs`. Each file exposes the
  `*_handler` closures the components bind to (e.g.
  `pub(crate) fn share_toggle_handler(state: RoomState) -> impl Fn(MouseEvent) + Clone`).
  `mod.rs` is **re-export only** (`pub(crate) use share::*;` …).
- **`room/effects/`** — one `Effect::new` wiring per file:
  `audio_selftest.rs`, `outgoing_mute.rs`, `invite_autocopy.rs`,
  `quick_share.rs`. Each exposes exactly
  `pub(crate) fn install(state: RoomState)` (or a named `install_x`).
  `mod.rs` re-exports and provides one
  `pub(crate) fn install_all(state: RoomState)` that calls each — so
  `pages/room.rs` has a single line for effect setup.
- **Hard rules:**
  - `mod.rs` in both folders contains no logic — declarations and
    re-exports only.
  - **No `#[allow(clippy::too_many_lines)]` may appear anywhere under
    `room/actions/` or `room/effects/`.** If a file trips the lint, the
    file is doing too much — split the capability further (e.g.
    `share.rs` → `share.rs` + `share_teardown.rs`).
  - Each file has a module doc-comment (`//!`) stating the one capability
    or one effect it owns and *why it reacts to what it reacts to*.
  - A file that needs more than three `use crate::client::…` imports is a
    smell — the logic has leaked across the browser boundary; push the
    `web_sys` part down into `client/`.

---

## Phase overview

| # | Phase | Ends green with | Risk |
|---|-------|-----------------|------|
| 1 | Extract `apps/server` | full gate + `docker build .` + HTTP smoke | low (mechanical) |
| 2 | Typed ids in `protocol::ids` | full gate | low |
| 3 | Finish `crates/domain` (quality machine, status) | full gate; quality tests become native | low |
| 4 | `client/` umbrella + split `webrtc` | full gate | medium |
| 5 | `RoomState` + `provide_context` | full gate | medium |
| 6 | `pages/` + `room/` + `home/` slices | full gate + browser sanity | medium |
| 7 | `desktop_bridge` + CSS `@layer` | full gate | low |
| 8 | Collapse the 4 peer maps into `HashMap<PeerId, PeerLink>` | full gate **+ two-tab manual checklist between sub-steps** | high — no automation harness |

**Phases 4–8 each get their own dated bite-sized plan document, written
just before that phase is executed**, per the roadmap convention
(`2026-08-28-architecture-refactor-roadmap.md`): their exact `git mv`
paths depend on the tree the previous phase produced, and writing literal
per-step commands now would bake in stale paths. This document specifies
each of them to the **file-map + interface + acceptance-gate** level —
enough to expand mechanically. Phases 1–3 are fully bite-sized below.

---

# Phase 1 — Extract `apps/server`

**Goal:** Move the Axum binary and the two SSR-only middleware modules out
of `apps/web` into a new `apps/server` crate. `apps/web` becomes a
library with no `[[bin]]`. Zero behavior change; the deployed artifact is
the same binary under a new name.

**Architecture:** `cargo-leptos` supports `bin-package` ≠ `lib-package`.
`apps/server` is the `bin-package` (`screen-share-server`), `apps/web`
stays the `lib-package` (`screen_share`). `apps/server` depends on
`screen_share` for `shell()` and `App` (SSR fallback rendering). The
`ssr` feature and its deps (`axum`, `tower`, `tower-http`, `leptos_axum`,
`tokio`, `screen-share-signaling`) move from `apps/web/Cargo.toml` to
`apps/server/Cargo.toml`.

**File Structure after this phase:**

```
Cargo.toml                       # members += "apps/server"; [[workspace.metadata.leptos]] bin-package = "screen-share-server"
apps/server/Cargo.toml           # NEW
apps/server/src/main.rs          # MOVED from apps/web/src/main.rs, split: bootstrap here, router in router.rs
apps/server/src/router.rs        # NEW — the Router::new()... chain from the old main.rs
apps/server/src/config.rs        # NEW — get_configuration + TurnConfig::from_env + HandshakeConfig::from_env, one struct
apps/server/src/middleware/mod.rs      # NEW
apps/server/src/middleware/security.rs # MOVED from apps/web/src/http_security.rs
apps/server/src/middleware/limits.rs   # MOVED from apps/web/src/http_limits.rs
apps/web/Cargo.toml              # ssr feature + bin-only deps removed; [[bin]] gone; [lib] stays
apps/web/src/lib.rs              # `pub mod http_limits/http_security` lines removed; no `mod main`
apps/web/src/main.rs             # DELETED
Dockerfile                       # line 27 + 48: screen_share → screen-share-server
```

**Interfaces:**
- Consumes (from `apps/web`, unchanged): `screen_share::app::shell`,
  `screen_share::app::App`.
- Consumes (from `crates/signaling`, unchanged):
  `handshake::HandshakeConfig`, `registry::Registry`,
  `rooms_status::{room_status_handler, RoomStatusLimiter}`,
  `state::SignalingState`, `turn::TurnConfig`, `ws::ws_handler`.
- Produces: `apps/server` binary `screen-share-server`; module
  `screen_share_server::router::build(...)` and
  `screen_share_server::config::ServerConfig`.

### Task 1.1: Capture the baseline

**Files:** none.

- [ ] **Step 1: Record the passing suite + count**

```bash
cd /home/hiroshi/projects/screenshare/fuck_you_janja/.claude/worktrees/refactor+structure-and-code-design
scripts/test-all.sh --no-mutants 2>&1 | tee /tmp/p1-baseline.txt
grep -E 'test result:|Summary|PASS|FAIL' /tmp/p1-baseline.txt
```

Expected: everything green. Note the native test count (209) and wasm
count (62) — later steps must reproduce them.

- [ ] **Step 2: Record a clean production build + the Docker build**

```bash
cargo leptos build --release 2>&1 | tail -10
ls target/site/pkg/          # screen_share.js, screen_share.wasm, *.css
docker build -t screen-share-baseline . 2>&1 | tail -15
```

Expected: both succeed.

### Task 1.2: Scaffold `apps/server`

**Files:**
- Create: `apps/server/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `apps/server/Cargo.toml`**

```toml
[package]
name = "screen-share-server"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "screen-share-server"
path = "src/main.rs"

[dependencies]
screen_share = { path = "../web", default-features = false, features = ["ssr"] }
screen-share-protocol = { path = "../../crates/protocol" }
screen-share-signaling = { path = "../../crates/signaling" }
leptos = { version = "0.8.0", features = ["ssr"] }
leptos_axum = { version = "0.8.0" }
leptos_meta = { version = "0.8.0", features = ["ssr"] }
leptos_router = { version = "0.8.0", features = ["ssr"] }
axum = { version = "0.8.0" }
tower = { version = "0.5", features = ["util", "limit"] }
tower-http = { version = "0.6", features = ["timeout"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
```

- [ ] **Step 2: Add the member to the workspace**

In root `Cargo.toml`, `[workspace] members`:

```toml
members = ["apps/server", "apps/web", "crates/domain", "crates/protocol", "crates/signaling"]
```

- [ ] **Step 3: Point cargo-leptos at the split**

In root `Cargo.toml`, `[[workspace.metadata.leptos]]`:

```toml
name = "screen_share"
bin-package = "screen-share-server"
lib-package = "screen_share"
output-name = "screen_share"
```

(only `bin-package` changes; `output-name` and `lib-package` stay.)

- [ ] **Step 4: Verify the workspace resolves**

```bash
cargo metadata --format-version 1 --no-deps > /dev/null && echo OK
```

Expected: `OK`, no manifest errors. (The build will fail until Task 1.4
— that is expected.)

### Task 1.3: Give `apps/web` the `ssr` feature back as a pass-through

`apps/web` still needs to *compile* under `--features ssr` (its
components render server-side), but it no longer owns the binary or the
HTTP layer.

**Files:**
- Modify: `apps/web/Cargo.toml`

- [ ] **Step 1: Trim `apps/web/Cargo.toml`**

Remove `[[bin]]` if present. In `[dependencies]`, delete `axum`,
`tower`, `tower-http`, `leptos_axum`, `tokio`, and the optional
`screen-share-signaling` (it moves to `apps/server`). Keep everything
`hydrate` needs and everything the components need. Rewrite `[features]`:

```toml
[features]
hydrate = [
    "leptos/hydrate",
    "dep:console_error_panic_hook",
    "dep:wasm-bindgen",
    "dep:wasm-bindgen-futures",
    "dep:js-sys",
    "dep:web-sys",
    "dep:send_wrapper",
]
ssr = [
    "leptos/ssr",
    "leptos/nonce",
    "leptos_meta/ssr",
    "leptos_router/ssr",
]
```

- [ ] **Step 2: `cargo check` the lib under both features**

```bash
cargo check -p screen_share --no-default-features --features ssr 2>&1 | tail -20
cargo check -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate 2>&1 | tail -20
```

Expected: `ssr` check fails only on `http_limits` / `http_security` (they
reference `tower`/`axum`, now gone) and on `main.rs`. That is expected —
Task 1.4 removes them from `apps/web`.

### Task 1.4: Move `main.rs` + middleware into `apps/server`

**Files:**
- Move: `apps/web/src/http_security.rs` → `apps/server/src/middleware/security.rs`
- Move: `apps/web/src/http_limits.rs` → `apps/server/src/middleware/limits.rs`
- Move + split: `apps/web/src/main.rs` → `apps/server/src/{main.rs, router.rs, config.rs}`
- Create: `apps/server/src/middleware/mod.rs`
- Modify: `apps/web/src/lib.rs`
- Delete: `apps/web/src/main.rs`

- [ ] **Step 1: Move the two middleware files with git**

```bash
mkdir -p apps/server/src/middleware
git mv apps/web/src/http_security.rs apps/server/src/middleware/security.rs
git mv apps/web/src/http_limits.rs apps/server/src/middleware/limits.rs
```

- [ ] **Step 2: Create `apps/server/src/middleware/mod.rs`**

```rust
//! SSR-only HTTP layers, moved out of the (now library-only) `apps/web`
//! crate: the CSP + per-request nonce, and the DoS guards
//! (request timeout + global concurrency cap + per-IP rate limit).
pub mod limits;
pub mod security;
```

- [ ] **Step 3: Fix the module paths inside the two moved files**

In `security.rs` and `limits.rs`, replace any `crate::` path that pointed
at the old `apps/web` crate root. They were self-contained
(`http_security` only used `axum`/`tower`/`leptos`), so the likely change
is `use crate::…` → nothing, and the `pub use` of `provide_request_nonce`
now lives at `screen_share_server::middleware::security::provide_request_nonce`.
Grep to be sure:

```bash
grep -n 'crate::' apps/server/src/middleware/security.rs apps/server/src/middleware/limits.rs
```

Repoint each hit at `crate::middleware::…` or an external crate.

- [ ] **Step 4: Create `apps/server/src/config.rs`**

```rust
//! All server runtime configuration, read from the environment at
//! process start (see CLAUDE.md §Configuration — the deployed artifact
//! carries no config file).

use leptos::config::{get_configuration, LeptosOptions};
use screen_share_signaling::handshake::HandshakeConfig;
use screen_share_signaling::turn::TurnConfig;

pub struct ServerConfig {
    pub leptos_options: LeptosOptions,
    pub dev_csp: bool,
    pub turn: TurnConfig,
    pub handshake: HandshakeConfig,
}

impl ServerConfig {
    /// # Errors
    /// Fails if `get_configuration` can't read the Leptos env vars, or
    /// if `TURN_SECRET` is set but malformed (abort rather than run a
    /// relay with a weak secret — finding F13).
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let conf = get_configuration(None)?;
        let leptos_options = conf.leptos_options;
        let dev_csp = !matches!(leptos_options.env, leptos::config::Env::PROD);
        let turn = TurnConfig::from_env()?;
        let handshake = HandshakeConfig::from_env();
        Ok(Self { leptos_options, dev_csp, turn, handshake })
    }
}
```

- [ ] **Step 5: Create `apps/server/src/router.rs`**

Move the `Router::new()…` construction verbatim out of the old
`main.rs`. Signature:

```rust
//! Composes the full Axum service: the Leptos SSR routes (each wrapped
//! to re-publish the per-request CSP nonce), the signaling relay
//! (`/ws`, `/api/rooms/{code}`), and the DoS guards. Merge order
//! matters — see the inline comments moved from the old `main.rs`.

use axum::Router;
use leptos::prelude::LeptosOptions;
use screen_share_signaling::state::SignalingState;

use crate::middleware::{limits, security};

pub fn build(
    leptos_options: LeptosOptions,
    signaling_state: SignalingState,
    handshake: screen_share_signaling::handshake::HandshakeConfig,
    dev_csp: bool,
) -> Router {
    // <-- the body of the old main.rs from `let routes = generate_route_list(App);`
    //     through the final `.layer(axum::middleware::from_fn_with_state(dev_csp, security::apply))`
    //     lives here, with `http_security::` → `security::` and
    //     `http_limits::` → `limits::`.
}
```

Keep every comment from the original (`_with_context`, "Merged after the
DoS guards…", etc.).

- [ ] **Step 6: Rewrite `apps/server/src/main.rs`**

```rust
//! `screen-share-server` — the Axum host. Renders Leptos pages
//! server-side and runs the signaling relay. All meaning of the
//! signaling messages lives in the browser (`screen_share::client`);
//! this binary only routes them.
#![recursion_limit = "512"]
#![warn(clippy::too_many_lines)]
#![warn(clippy::too_many_arguments)]

mod config;
mod middleware;
mod router;

use leptos::logging::log;
use screen_share::app::App;
use screen_share_signaling::registry::Registry;
use screen_share_signaling::rooms_status::RoomStatusLimiter;
use screen_share_signaling::state::SignalingState;

use crate::config::ServerConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = ServerConfig::from_env()?;
    let addr = cfg.leptos_options.site_addr;

    log!(
        "TURN server: {}",
        if cfg.turn.is_some() { "configured" } else { "not configured (STUN-only ICE)" }
    );

    let signaling_state = SignalingState {
        registry: Registry::new(),
        turn: cfg.turn,
        handshake: cfg.handshake.clone(),
        room_status_limiter: RoomStatusLimiter::new(),
    };

    let app = router::build(
        cfg.leptos_options.clone(),
        signaling_state,
        cfg.handshake,
        cfg.dev_csp,
    );

    log!("listening on http://{}", &addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
```

`generate_route_list(App)` moves into `router::build` (that is why `App`
is imported here and passed through, or call `generate_route_list` inside
`build` — pick one and keep it consistent; putting it in `build` keeps
`main.rs` free of `leptos_axum`).

- [ ] **Step 7: Delete `apps/web/src/main.rs` and clean `lib.rs`**

```bash
git rm apps/web/src/main.rs
```

In `apps/web/src/lib.rs`, remove:

```rust
#[cfg(feature = "ssr")]
pub mod http_limits;
#[cfg(feature = "ssr")]
pub mod http_security;
```

- [ ] **Step 8: Move the two SSR integration tests**

`apps/web/tests/http_limits.rs` and `apps/web/tests/http_security.rs`
exercise middleware that now lives in `apps/server`.

```bash
mkdir -p apps/server/tests
git mv apps/web/tests/http_limits.rs apps/server/tests/limits.rs
git mv apps/web/tests/http_security.rs apps/server/tests/security.rs
```

Repoint their `use screen_share::http_security::…` →
`use screen_share_server::middleware::security::…` (and `http_limits` →
`middleware::limits`). `apps/web/tests/ssr_render.rs` stays — it tests
component rendering, which is still `apps/web`.

### Task 1.5: Update the Dockerfile

**Files:**
- Modify: `Dockerfile:27`, `Dockerfile:48`

- [ ] **Step 1: Rename the copied binary**

Line 27: `&& cp target/release/screen_share /app/screen_share.out \`
→ `&& cp target/release/screen-share-server /app/screen_share.out \`

Line 48: unchanged target name is fine (`./screen_share`), but for
clarity rename the intermediate too — or leave `screen_share.out` /
`./screen_share` as the runtime name (`docker-entrypoint.sh` calls
`./screen_share`). **Minimal change:** only line 27's source path.
Confirm `docker-entrypoint.sh` still invokes `./screen_share`:

```bash
grep -n screen_share docker-entrypoint.sh
```

If it does, line 48 and the `ENV` block stay untouched.

### Task 1.6: Build, test, gate

- [ ] **Step 1: Full build**

```bash
cargo leptos build 2>&1 | tail -20
```

Expected: succeeds; `target/site/pkg/screen_share.wasm` present.

- [ ] **Step 2: Full test gate**

```bash
scripts/test-all.sh --no-mutants 2>&1 | tee /tmp/p1-after.txt
diff <(grep -oE 'test result: ok\. [0-9]+' /tmp/p1-baseline.txt) \
     <(grep -oE 'test result: ok\. [0-9]+' /tmp/p1-after.txt)
```

Expected: green; native + wasm counts unchanged (the moved middleware
tests still run, now under `-p screen-share-server`).

- [ ] **Step 3: Docker build**

```bash
docker build -t screen-share-p1 . 2>&1 | tail -15
```

Expected: succeeds.

- [ ] **Step 4: HTTP smoke**

```bash
cargo leptos watch &
sleep 25
for p in / /pkg/screen_share.wasm /styles/home.css '/api/rooms/ZZZZZZ'; do
  printf '%s -> ' "$p"; curl -s -o /dev/null -w '%{http_code}\n' "http://127.0.0.1:3000$p"
done
kill %1
```

Expected: `/` → 200 (HTML, no hydration-mismatch warning in it),
`/pkg/screen_share.wasm` → 200, `/styles/home.css` → 200,
`/api/rooms/ZZZZZZ` → 200 with `{"exists":false}` (or the current
shape).

- [ ] **Step 5: MAINTAINER GATE** — present the diff, the passing gate,
  the Docker build, and the smoke output. On approval, commit:

```
refactor(server): extract apps/server crate from apps/web

apps/web is now a pure Leptos UI library; the Axum host, its router,
config, and the SSR-only CSP/DoS middleware move to a new
screen-share-server binary crate. cargo-leptos bin-package split; the
wasm output name and the deployed runtime binary path are unchanged.
```

---

# Phase 2 — Typed identifiers in `protocol::ids`

**Goal:** Replace bare `String` identifiers (`peer_id`, `room` code,
`nick`, hex `color`) with newtypes, introduced at the wire boundary and
carried through `signaling` and `apps/web`.

**Architecture:** `crates/protocol/src/ids.rs` defines four newtypes,
each a `String` wrapper with a private field, a `parse` constructor
("parse, don't validate"), `as_str`, `Display`, and serde
`try_from`/`into` so the JSON wire format is unchanged. `protocol`'s
message and info structs adopt them. `signaling` and `apps/web` follow
one boundary at a time.

**Interfaces:**
- Produces: `screen_share_protocol::ids::{PeerId, RoomCode, Nick, HexColor, IdError}`.
- `PeerId::parse(impl Into<String>) -> Result<PeerId, IdError>`,
  `PeerId::as_str(&self) -> &str`, `impl Display`, `impl FromStr`.
  Same shape for the other three.
- `IdError`: `#[derive(Debug, thiserror::Error)]` with one variant per
  type (`#[error("invalid peer id")] PeerId`, …).

### Task 2.1: Baseline

- [ ] **Step 1:** `scripts/test-all.sh --no-mutants` green; record counts.

### Task 2.2: Define the newtypes (TDD)

**Files:**
- Create: `crates/protocol/src/ids.rs`
- Create: `crates/protocol/tests/ids.rs`
- Modify: `crates/protocol/src/lib.rs`, `crates/protocol/Cargo.toml`

- [ ] **Step 1: Add `thiserror` to `crates/protocol/Cargo.toml`**

```toml
[dependencies]
serde = { version = "1.0.229", features = ["derive"] }
thiserror = "2"
```

- [ ] **Step 2: Write the failing test `crates/protocol/tests/ids.rs`**

```rust
use screen_share_protocol::ids::{HexColor, IdError, Nick, PeerId, RoomCode};

#[test]
fn peer_id_roundtrips_through_str() {
    let id = PeerId::parse("abc123").unwrap();
    assert_eq!(id.as_str(), "abc123");
    assert_eq!(id.to_string(), "abc123");
}

#[test]
fn peer_id_rejects_empty_and_overlong() {
    assert!(matches!(PeerId::parse(""), Err(IdError::PeerId)));
    assert!(matches!(PeerId::parse("x".repeat(65)), Err(IdError::PeerId)));
}

#[test]
fn room_code_is_six_uppercase_alnum() {
    assert!(RoomCode::parse("ABC123").is_ok());
    assert!(matches!(RoomCode::parse("abc123"), Err(IdError::RoomCode)));
    assert!(matches!(RoomCode::parse("ABC12"), Err(IdError::RoomCode)));
}

#[test]
fn nick_trims_and_bounds_length() {
    assert_eq!(Nick::parse("  Ana ").unwrap().as_str(), "Ana");
    assert!(matches!(Nick::parse("   "), Err(IdError::Nick)));
    assert!(matches!(Nick::parse("n".repeat(33)), Err(IdError::Nick)));
}

#[test]
fn hex_color_requires_hash_and_six_hex() {
    assert!(HexColor::parse("#1a2b3c").is_ok());
    assert!(matches!(HexColor::parse("1a2b3c"), Err(IdError::HexColor)));
    assert!(matches!(HexColor::parse("#zzzzzz"), Err(IdError::HexColor)));
}

#[test]
fn serde_wire_is_a_plain_string() {
    let id = PeerId::parse("p1").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"p1\"");
    let back: PeerId = serde_json::from_str("\"p1\"").unwrap();
    assert_eq!(back, id);
    assert!(serde_json::from_str::<PeerId>("\"\"").is_err());
}
```

- [ ] **Step 3: Run — expect fail** (`cannot find module ids`)

```bash
scripts/test-all.sh rust 2>&1 | grep -A2 'ids.rs'
```

- [ ] **Step 4: Implement `crates/protocol/src/ids.rs`**

The exact bounds come from `crates/protocol/src/validate.rs` and the
maintainer rules in `CLAUDE.md` (6-char code, ≤10 members, nick/color
validated server-side). Match `validate.rs` exactly — do not invent new
limits. Skeleton:

```rust
//! Typed identifiers for the signaling wire. Each is a validated
//! `String` newtype: construct via `parse` at the boundary, and every
//! downstream consumer can then trust the value ("parse, don't
//! validate"). The serde representation is a bare string, so the JSON
//! wire format is byte-identical to the pre-newtype protocol.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdError {
    #[error("invalid peer id")]
    PeerId,
    #[error("invalid room code")]
    RoomCode,
    #[error("invalid nick")]
    Nick,
    #[error("invalid hex color")]
    HexColor,
}

macro_rules! string_newtype {
    ($name:ident, $err:ident, $parse:expr) => {
        #[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// # Errors
            /// Returns [`IdError::$err`] if the input violates the type's invariant.
            pub fn parse(raw: impl Into<String>) -> Result<Self, IdError> {
                let f: fn(String) -> Result<Self, IdError> = $parse;
                f(raw.into())
            }
            #[must_use]
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }
        impl FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, IdError> { Self::parse(s) }
        }
        impl TryFrom<String> for $name {
            type Error = IdError;
            fn try_from(s: String) -> Result<Self, IdError> { Self::parse(s) }
        }
        impl From<$name> for String {
            fn from(v: $name) -> String { v.0 }
        }
    };
}

string_newtype!(PeerId, PeerId, |s| {
    if s.is_empty() || s.len() > 64 { return Err(IdError::PeerId); }
    Ok(PeerId(s))
});
string_newtype!(RoomCode, RoomCode, |s| {
    if s.len() == 6 && s.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
        Ok(RoomCode(s))
    } else { Err(IdError::RoomCode) }
});
string_newtype!(Nick, Nick, |s| {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > 32 { return Err(IdError::Nick); }
    Ok(Nick(t.to_string()))
});
string_newtype!(HexColor, HexColor, |s| {
    let ok = s.len() == 7
        && s.starts_with('#')
        && s[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if ok { Ok(HexColor(s)) } else { Err(IdError::HexColor) }
});
```

Cross-check every bound against `validate.rs`; adjust to match, and if
`validate.rs` has logic these newtypes now duplicate, have `validate.rs`
call `ids::*::parse` instead (removing the duplication is in-scope for
this task).

- [ ] **Step 5: Export from `crates/protocol/src/lib.rs`**

```rust
pub mod ids;
pub use ids::{HexColor, IdError, Nick, PeerId, RoomCode};
```

- [ ] **Step 6: Run — expect pass**

```bash
scripts/test-all.sh rust 2>&1 | grep -E 'protocol.*test result'
```

- [ ] **Step 7: Commit** `feat(protocol): typed identifier newtypes (PeerId, RoomCode, Nick, HexColor)`.

### Task 2.3: Adopt in `protocol`'s own structs

**Files:** `crates/protocol/src/{client,server,info}.rs`,
`crates/protocol/tests/wire.rs`

- [ ] **Step 1:** In `client.rs` / `server.rs` / `info.rs`, change field
  types: `peer_id: String` → `peer_id: PeerId`; the room code field
  (`room` / `code`) → `RoomCode`; `nick: String` → `Nick`;
  `color: String` → `HexColor`. Leave SDP strings, candidate strings,
  status text as `String`.
- [ ] **Step 2:** Run `scripts/test-all.sh rust`. `tests/wire.rs`
  asserts serialized shapes — they must be **unchanged** (the newtypes
  serialize as strings). Fix only compile errors, not the JSON
  expectations. If a JSON expectation changes, the serde attr is wrong —
  fix the newtype, not the test.
- [ ] **Step 3: Commit** `refactor(protocol): carry typed ids in wire structs`.

### Task 2.4: Adopt in `crates/signaling`

**Files:** `crates/signaling/src/*.rs`, `crates/signaling/tests/*.rs`

- [ ] **Step 1:** Follow the compiler. `registry.rs` keys rooms/members
  by these ids — its `HashMap<String, _>` become `HashMap<PeerId, _>` /
  `HashMap<RoomCode, _>`. Construct via `parse` where a raw string
  crosses in (WS handshake, HTTP path param); propagate `IdError` as the
  existing "invalid input" rejection path (`ServerMessage::InvalidInput`
  / a 400), do not `unwrap`.
- [ ] **Step 2:** `scripts/test-all.sh rust` + the signaling mutation
  note in `CLAUDE.md` (§Definition of done) — run
  `cargo mutants --in-diff` locally on the changed area per the roadmap
  before considering it done.
- [ ] **Step 3: Commit** `refactor(signaling): key the registry by typed ids`.

### Task 2.5: Adopt in `apps/web`

**Files:** across `apps/web/src` — `session/mod.rs` (`RoomMember`,
`RoomSignals`), `session/handler/*`, `features/room/*`, `infra/*`.

- [ ] **Step 1:** `RoomMember { peer_id: PeerId, nick: Nick, color: HexColor, … }`.
  The four `RoomSession` maps become `HashMap<PeerId, _>`. `watching`,
  `expanded`, `watchers_by_sharer` sets/maps key by `PeerId`. Leaf UI
  that needs a `&str` for an attribute calls `.as_str()`.
- [ ] **Step 2:** Where a `PeerId` is needed from `localStorage` or a
  route param, `parse` at that read and handle the `Err` (treat a
  malformed stored id as "no stored session").
- [ ] **Step 3:** `scripts/test-all.sh --no-mutants`. Browser sanity:
  `cargo leptos watch`, create/join/share/watch once.
- [ ] **Step 4: MAINTAINER GATE**, then commit
  `refactor(web): use typed ids throughout the room session`.

---

# Phase 3 — Finish `crates/domain`

**Goal:** Move the browser-free half of the adaptive-quality machine and
the status classifier into `crates/domain`, so they are natively tested
and mutation-covered. The `web_sys` half stays in `apps/web`
(`client/webrtc/` after Phase 4; `session/quality` until then).

**Architecture:** `crates/domain` gains a dependency on
`screen-share-protocol` (for `QualityLevel`, used by `tier_for`). New
`domain::quality` holds `Tier`, `AdaptiveQuality`, `RawReading`,
`EncodingPreset`, `Signal`, `preset_for`, `tier_for`, `classify`,
`record_signal`, `step`, `InitialTier`. New `domain::status` holds
`status_meta`. `apps/web` calls into them.

**Interfaces:**
- Produces: `screen_share_domain::quality::{Tier, AdaptiveQuality,
  RawReading, EncodingPreset, InitialTier, preset_for, tier_for}` with
  the signatures they have today, minus `pub(crate)` (now `pub`).
- Produces: `screen_share_domain::status::status_meta(&str) -> (&'static str, &'static str)`.
- `apps/web`'s `session/quality/mod.rs` keeps `configure_encoding`,
  `apply_tier`, `read_reading` (→ builds a `domain::quality::RawReading`
  from `getStats`), `AutoPoll`, `start_auto_polling`, `set_quality_handler`.

### Task 3.1: Baseline
- [ ] `scripts/test-all.sh --no-mutants` green; record counts.

### Task 3.2: Move the quality state machine

**Files:**
- Create: `crates/domain/src/quality.rs`, `crates/domain/tests/quality.rs`
- Modify: `crates/domain/{Cargo.toml, src/lib.rs}`
- Modify: `apps/web/src/session/quality/mod.rs`, `apps/web/src/session/quality/tests.rs`

- [ ] **Step 1:** Add to `crates/domain/Cargo.toml`:

```toml
[dependencies]
screen-share-protocol = { path = "../protocol" }
```

Update `crates/domain/src/lib.rs` module doc: it now depends on
`protocol` (and thus `serde`) — no `web-sys`, no async, no Leptos.

- [ ] **Step 2:** `git mv` the pure items. Cut from
  `apps/web/src/session/quality/mod.rs` everything from `enum Tier`
  through `impl Default for AdaptiveQuality` plus `EncodingPreset`,
  `preset_for`, `tier_for`, `enum Signal`, `RawReading`, `InitialTier`.
  Paste into `crates/domain/src/quality.rs`. Change `pub(crate)` → `pub`.
  Replace `screen_share_protocol::QualityLevel` refs (now a direct dep).
- [ ] **Step 3:** `git mv apps/web/src/session/quality/tests.rs
  crates/domain/tests/quality.rs`; repoint `use` paths to
  `screen_share_domain::quality::*`. Keep every test.
- [ ] **Step 4:** In `apps/web/src/session/quality/mod.rs`, add
  `use screen_share_domain::quality::{...};` and keep the `web_sys` half
  (`configure_encoding`, `apply_tier`, `read_reading`, `AutoPoll`,
  `start_auto_polling`, `is_auto_polling`, `stop_*`, `set_quality_handler`).
- [ ] **Step 5:** `scripts/test-all.sh rust` — the moved tests now run
  under `-p screen-share-domain`; wasm count drops by the number of
  `quality/wasm_tests.rs` cases **only if** any were pure (they should
  stay — `apply_tier` etc. are still wasm). Native count rises by the
  moved `tests.rs` count. Record both; the *total* must not fall.
- [ ] **Step 6: Commit** `refactor(domain): move the adaptive-quality state machine out of the browser layer`.

### Task 3.3: Move `status_meta`

**Files:**
- Create: `crates/domain/src/status.rs`
- Modify: `crates/domain/src/lib.rs`, `apps/web/src/components/status.rs`
  (becomes a re-export or is deleted), every `use crate::components::status::status_meta`

- [ ] **Step 1:** `git mv apps/web/src/components/status.rs
  crates/domain/src/status.rs`. Add `pub mod status;` to
  `crates/domain/src/lib.rs`.
- [ ] **Step 2:** Its return strings are pt-BR UI copy matched against
  pt-BR status sentences — that is fine in `domain` (it is domain copy,
  not framework code) and matches CLAUDE.md's "English for code,
  Portuguese for user-facing strings". Keep it verbatim.
- [ ] **Step 3:** Repoint imports:

```bash
cd apps/web && grep -rl 'components::status::status_meta' src \
  | xargs sed -i 's#crate::components::status::status_meta#screen_share_domain::status::status_meta#g'
```

- [ ] **Step 4:** `scripts/test-all.sh --no-mutants`. Any `status`
  unit test moves to `crates/domain/tests/status.rs`.
- [ ] **Step 5: Commit** `refactor(domain): move status_meta classifier`.

### Task 3.4: `domain::ids` re-export

- [ ] **Step 1:** Add `crates/domain/src/ids.rs`:

```rust
//! Re-export of the wire identifier newtypes so callers that already
//! depend on `domain` (but not directly on `protocol`) can name them
//! without a second dependency line.
pub use screen_share_protocol::ids::{HexColor, IdError, Nick, PeerId, RoomCode};
```

Add `pub mod ids;` to `crates/domain/src/lib.rs`.
- [ ] **Step 2:** `scripts/test-all.sh rust`. **Commit**
  `chore(domain): re-export protocol::ids`.

---

# Phase 4 — `client/` umbrella + split `webrtc`

> **Expand into `2026-XX-XX-refactor-v3-phase-4-client-umbrella.md`
> before executing** — the `git mv` paths below assume Phases 1–3 have
> landed.

**Goal:** Every browser-only module sits under one
`#[cfg(feature = "hydrate")]` subtree, `client/`. The 564-line
`infra/webrtc/mod.rs` splits by responsibility. WebRTC errors that the UI
consumes become a concrete `enum`.

**Files:**
- Move: `apps/web/src/infra/` → `apps/web/src/client/` (module rename;
  update `lib.rs` `pub mod infra` → `pub mod client`, and every
  `crate::infra::` → `crate::client::`).
- Split: `client/webrtc/mod.rs` →
  - `client/webrtc/peer.rs` — `new_peer_connection`, `reserve_audio_mline`, connection teardown helpers.
  - `client/webrtc/connection.rs` — `create_offer`, `create_answer`, `accept_answer`, `add_ice_candidate`.
  - `client/webrtc/media.rs` — `play_stream_in`, `combine_video_and_audio`, `video_and_audio_tracks`, track wiring.
  - `client/webrtc/screen_share.rs` — `capture_display`, `display_media_constraints`, `is_display_media_supported`.
  - `client/webrtc/stats.rs` — the `getStats` reader; returns `screen_share_domain::quality::RawReading`.
  - `client/webrtc/error.rs` — `#[derive(Debug, thiserror::Error)] enum WebRtcError { CaptureFailed, PermissionDenied, NoDevice, Negotiation(String), … }`; the `pub` fns that the UI awaits return `Result<_, WebRtcError>` instead of `Result<_, JsValue>` (keep `JsValue` internal).
- Move: `infra/{signaling_transport,display_capture,peer_link}.rs` →
  `client/seam/{…}.rs` + `client/seam/mod.rs`.
- Move: the PCM-loopback + `notify_desktop_*` functions out of
  `webrtc/mod.rs` into `client/desktop_bridge.rs` (Phase 7 folds
  `quick_share.rs` in too; doing the `notify_*` move here is fine).
- Rename: `infra/session.rs` → `client/pending_session.rs`.

**Interfaces:**
- `client/webrtc/mod.rs` re-exports the split modules so existing
  `use crate::client::webrtc::{create_offer, …}` call sites need only the
  `infra`→`client` rename, not a per-item repath. (Do the split behind
  the facade; a follow-up commit can repoint call sites module-by-module
  if desired.)
- Produces: `client::webrtc::error::WebRtcError`.

**Acceptance gate:** `scripts/test-all.sh --no-mutants` green;
`infra/webrtc/wasm_tests.rs` cases split across the new modules'
`wasm_tests.rs`, total unchanged; browser sanity (share + watch once).

**Task outline (expand to bite-sized in the phase doc):**
1. Baseline.
2. `git mv infra client` + `sed` repath `crate::infra::` → `crate::client::`; gate.
3. Introduce `client/webrtc/error.rs` with `WebRtcError`; convert the
   ~6 `pub` fns the UI awaits; adjust their callers' error handling
   (they already `match`/`map_err` on failure — repoint to the named
   variants); gate.
4. Split `webrtc/mod.rs` into the six files behind the re-export facade;
   move each function group + its wasm tests together; gate after each
   file.
5. `git mv` the three seam files into `client/seam/`; gate.
6. Move `notify_desktop_*` + PCM loopback into `client/desktop_bridge.rs`;
   gate.
7. Maintainer gate + commit group.

---

# Phase 5 — `RoomState` + `provide_context`

> **Expand into `2026-XX-XX-refactor-v3-phase-5-roomstate.md` before
> executing.** Ordered **before** Phase 6 (maintainer decision): grouping
> the signals in place first means Phase 6's file moves carry a small
> stable struct instead of re-threading 16 loose signals.

**Goal:** Replace `RoomSignals` (16 fields) and `MemberCardSignals` (14
fields) with one `RoomState` composed of five `Copy` sub-stores,
delivered by `provide_context`.

**Files:**
- Create: `apps/web/src/session/state.rs` (moves to `room/state.rs` in
  Phase 6) with:

```rust
#[derive(Clone, Copy)]
pub(crate) struct RoomState {
    pub(crate) roster: RosterStore,        // members, my_peer_id
    pub(crate) watch: WatchStore,          // watching, watchers_by_sharer, expanded
    pub(crate) media: MediaStore,          // ShareUi's fields + audio_preset, video_mode, audio_muted
    pub(crate) diagnostics: DiagnosticsStore, // latency_by_peer, connection_errors, quality_by_peer, volume/muted_by_peer
    pub(crate) connection: ConnectionStore,   // status, authenticated, room_exists, room_name, requires_password, turn_credentials
}
```

  Each sub-store `#[derive(Clone, Copy)]`, fields are
  `ReadSignal`/`WriteSignal`/`RwSignal` handles. Provide a
  `RoomState::new() -> Self` that creates every signal (moves the ~32
  `signal(...)` / `RwSignal::new(...)` lines out of `RoomPage`).
- Modify: `pages`/`features/room` — `RoomPage` calls
  `let state = RoomState::new(); provide_context(state);` then passes
  `state` (one value) to `setup_room_connection`, `member_cards`,
  effects, actions.
- Modify: `session/handler`, `session/reconnect`, `session/share_effects`,
  `features/room/member_card` — take `RoomState` (or destructure the one
  sub-store they need) instead of `RoomSignals` / `MemberCardSignals`.
- Delete: `RoomSignals`, `MemberCardSignals`, `ShareUi`, `PeerMedia`
  (their fields are absorbed into `MediaStore` / `DiagnosticsStore`).

**Interfaces:**
- Produces: `RoomState` and its five sub-store types; `RoomState::new()`;
  `expect_context::<RoomState>()` is valid anywhere under `<RoomPage>`.
- Consumes: nothing new.

**Acceptance gate:** `scripts/test-all.sh --no-mutants` green; the
`clippy::too_many_lines` allow on `RoomPage` **shrinks** (fewer lines);
browser sanity — full create/join/share/watch/reconnect pass. No
`#[allow(clippy::too_many_arguments)]` added.

**Task outline:**
1. Baseline.
2. Create `state.rs` with the five sub-stores + `new()`, no callers yet;
   unit-test that `new()` wires distinct signals; gate.
3. Repoint `setup_room_connection` + `build_message_handler` from
   `RoomSignals` to `RoomState`; delete `RoomSignals`; gate.
4. Repoint `member_cards`/`MemberCard` from `MemberCardSignals` to
   `expect_context::<RoomState>()`; delete `MemberCardSignals`; gate.
5. Fold `ShareUi`/`PeerMedia` into `MediaStore`/`DiagnosticsStore`;
   delete them; gate.
6. Delete the now-dead `signal(...)` lines from `RoomPage`; gate;
   maintainer gate; commit group.

---

# Phase 6 — `pages/` + `room/` + `home/` feature slices

> **Expand into `2026-XX-XX-refactor-v3-phase-6-feature-slices.md` before
> executing.**

**Goal:** The React-style layout from "File Structure — end state".
`RoomPage`/`HomePage` become thin route components in `pages/`;
everything else moves into `room/` and `home/` slices with
`components/`, `actions/`, `effects/`, `state.rs`, `layout.rs`,
`messages.rs`, `connection.rs`.

**Files:** the full `(MOVED)`/`(NEW)` set in "File Structure — end
state" for `app/`, `pages/`, `room/`, `home/`, `components/`. Highlights:
- `app.rs` → `app/app.rs`; `app_routes()` → `app/router.rs`.
- `features/room/mod.rs::RoomPage` splits: route shell (params, auth
  gate, `provide_context`) → `pages/room.rs`; authenticated view →
  `room/components/stage.rs`; control bar markup →
  `room/components/toolbar.rs`.
- `session/handler/` → `room/messages.rs` (+ submodules if it stays >
  ~400 lines: `room/messages/{join,peer,media,watch}.rs` grouped by
  message family, `mod.rs` = the `match` + re-exports).
- `session/{media,audio,audio_health,latency,video_mode}` runtime →
  `room/actions/*` (handlers) and `room/effects/*` (Effect wirings), per
  the **sizing policy** section. Pure remnants already went to `domain`
  in Phase 3.
- `session/reconnect` → `room/connection.rs` (`BackoffPolicy` already in
  `domain::backoff`).
- `features/room/grid` → `room/components/participant_grid.rs` +
  `room/layout.rs` (`best_column_count`, `recompute_adaptive_grid`).
- `features/room/member_card/{mod,parts}.rs` →
  `room/components/participant.rs` (+ `participant/parts.rs` if > ~300
  lines).
- `features/home/*` → `home/actions/{create,join,recent}.rs` +
  `home/components/{create_panel,join_panel,recent_rooms}.rs`;
  `pages/home.rs` composes them.
- `components/` → `components/ui/` (primitives) + `components/layout/`
  (`header.rs`). `transmission_menu.rs` → `room/components/`.
- `features/not_found.rs` → `pages/not_found.rs`;
  `features/profile.rs` → `src/profile.rs` (shared).
- `session/` and `features/` directories deleted; `lib.rs` module tree
  rewritten.

**Interfaces:**
- `pages::{HomePage, RoomPage, NotFound}` — the only components
  `app/router.rs` names.
- `room::state::RoomState` (from Phase 5, moved here).
- `room::actions::*` handler constructors; `room::effects::install_all`.
- `room::messages::dispatch(ctx, ServerMessage)`.

**Acceptance gate:** `scripts/test-all.sh --no-mutants` green; **every
`#[allow(clippy::too_many_lines)]` in `apps/web` removed** (this is the
phase that earns it — `RoomPage` no longer exists as one function);
`grep -rn 'crate::features::\|crate::session::' apps/web/src` prints
nothing; browser sanity — full flow + the dev room preview
(`/dev/room-preview`) render.

**Task outline:** one task per top-level target directory
(`app/` → `pages/` → `room/components/` → `room/actions/` →
`room/effects/` → `room/{state,messages,connection,layout,participant}` →
`home/` → `components/` → cleanup), each `git mv` + `sed` repath + gate,
each ending with a commit. Maintainer gate before the `session/`+
`features/` directory deletion.

---

# Phase 7 — `desktop_bridge` + CSS `@layer`

> **Expand into `2026-XX-XX-refactor-v3-phase-7-bridge-and-css.md` before
> executing.**

**Goal:** One module owns everything that talks to the Electron shell.
The stylesheet cascade is explicit.

**Files:**
- Move: `apps/web/src/quick_share.rs` → merge into
  `apps/web/src/client/desktop_bridge.rs` (the `notify_desktop_*` and
  PCM-loopback functions arrived there in Phase 4).
  `desktop_bridge` exposes: `is_desktop_app()`, `requested()` (was
  `quick_share::requested`), `notify_share_ready(&str)`,
  `notify_member_joined(&str)`, `notify_sharing_changed(bool)`,
  `audio_loopback_active()`, `stop_audio_loopback()`. Update
  `room/effects/quick_share.rs` and the room components to
  `use crate::client::desktop_bridge`.
- Modify: `apps/web/public/styles/tokens.css` — first line:
  `@layer tokens, base, components, features, utilities;`
- Wrap each stylesheet's rules in its layer: `base.css` →
  `@layer base { … }`; `card.css`, `card-widgets.css`,
  `transmission-menu` → `@layer components { … }`; `home.css`,
  `room.css` → `@layer features { … }`; `tokens.css` body →
  `@layer tokens { … }`. `dev_preview.css` → `@layer utilities`.
  No selector or declaration changes — only the `@layer` wrapper.

**Acceptance gate:** `scripts/test-all.sh --no-mutants` green; browser
sanity — home + room render **pixel-identical** (compare against a
screenshot taken before the change); `grep -rn 'quick_share' apps/web/src`
resolves only to `client::desktop_bridge`.

---

# Phase 8 — Collapse the four peer maps into `HashMap<PeerId, PeerLink>`

> **Expand into `2026-XX-XX-refactor-v3-phase-8-peerlink.md` before
> executing. This phase requires the maintainer driving a two-tab manual
> checklist between sub-steps — there is no automation harness for
> WebRTC teardown races or ICE ordering (CLAUDE.md §Testing approach).**

**Goal:** `RoomSession` holds one `Rc<RefCell<HashMap<PeerId,
PeerLink>>>` instead of four parallel maps (`outgoing`, `incoming`,
`outgoing_callbacks`, `incoming_callbacks`). `PeerLink` bundles the
connection, its callbacks, and its direction, so "a `pc` without its
callbacks" and "a callback set without its `pc`" stop being
representable. `session`/`room` negotiation code becomes generic over the
Phase-4 seam traits, so it can be unit-tested with fakes.

**Architecture:**

```rust
// client/seam/peer_link.rs — PeerLink gains a concrete bundling struct
pub(crate) enum LinkDirection { Outgoing, Incoming }

pub(crate) struct PeerLink {
    pub(crate) pc: web_sys::RtcPeerConnection,
    pub(crate) callbacks: crate::room::messages::PeerCallbacks,
    pub(crate) direction: LinkDirection,
}
```

`RoomSession::links: Rc<RefCell<HashMap<PeerId, PeerLink>>>`. All
`conn.outgoing.borrow_mut().insert(id, pc)` +
`conn.outgoing_callbacks.borrow_mut().insert(id, cbs)` pairs become one
`conn.links.borrow_mut().insert(id, PeerLink { pc, callbacks, direction })`.
`teardown_outgoing`/`teardown_incoming` merge into
`teardown_link(&conn, &peer_id)`. `session::{media,quality}` read
`conn.links.borrow().get(&id).map(|l| &l.pc)` where they previously read
the concrete maps.

**Files:** `apps/web/src/client/seam/peer_link.rs` (add the struct +
`LinkDirection`), `apps/web/src/room/state.rs` (the `RoomSession`
struct), `apps/web/src/room/messages.rs` (negotiation fns —
`answer_offer`, `accept_answer_from`, `route_ice_candidate`,
`offer_to_watcher` — generic over `SignalingTransport` + a
`PeerConnectionApi` covering the senders/transceivers surface
`session::{media,quality}` need), `apps/web/src/session/media/mod.rs`,
`apps/web/src/session/quality/mod.rs` (now `client/webrtc/…`),
`apps/web/src/features/room/watch.rs` (now `room/actions/watch.rs`).

**Interfaces:**
- `PeerLink` struct, `LinkDirection`.
- `RoomSession::links` replaces `.outgoing`, `.incoming`,
  `.outgoing_callbacks`, `.incoming_callbacks`.
- `teardown_link(conn: &RoomSession, peer: &PeerId)`.
- Negotiation fns gain a generic `<T: SignalingTransport>` (and, where
  needed, `<P: PeerConnectionApi>`); their prod call sites pass the
  concrete `WsClient` / `RtcPeerConnection`, tests pass fakes.

**Acceptance gate:** `scripts/test-all.sh --no-mutants` green; **new
native tests** for the negotiation fns against `FakeTransport` +
`FakePeerConnection` (at minimum: an incoming offer produces an answer
sent back; an ICE candidate for an unknown peer is dropped, not
panicked); the `infra::webrtc::wasm_tests` two-local-peer roundtrip still
green; **maintainer two-tab checklist** from
`2026-08-28-refactor-phase-5-roomsession.md` §"5b" — share → watch → real
media → stop via the in-app button → stop via **Chrome's own** bar →
indicator clears with no stuck "sharing" state → source switch keeps the
share alive → one watcher leaving doesn't disturb another → reconnect
after a dropped socket replays share/watch intent.

**Task outline (bite-sized in the phase doc; each ends with a maintainer
two-tab check where it touches teardown/ICE):**
1. Baseline + record the manual-checklist starting state.
2. Add `PeerLink` struct + `LinkDirection` to `client/seam/peer_link.rs`;
   no callers; gate.
3. Add `RoomSession::links`; keep the old four maps in parallel, write to
   both; gate + two-tab check.
4. Repoint readers (`session::media`, `session::quality`,
   `room/actions/watch`) to `.links`; gate + two-tab check.
5. Repoint writers/teardown to `.links` only; delete the four old maps;
   `teardown_outgoing`+`teardown_incoming` → `teardown_link`; gate +
   **full two-tab checklist**.
6. Genericize the four negotiation fns over the seams; add the
   `FakeTransport`/`FakePeerConnection` native tests; gate.
7. Maintainer gate + commit group; update
   `2026-09-02-structure-refactor-progress.md` to mark step 8 /
   phase-5b done.

---

## Self-Review

**Spec coverage** — every item from the v3 report maps to a phase:
P1→Phase 3 (+ Phase 4 for the `web_sys` half); P2→Phase 2; P3→Phase 8;
P4→Phase 5; P5→Phase 6; P6 (dispatch) already done on the branch, Phase 6
only relocates it; P7→Phase 4 + Phase 6; P8→Phase 4; P9→Phase 4 + Phase
7; P10→Phase 6; P11→settled, no task; P12→Phase 7. The
back-end-separation concern→Phase 1. The `pages/`+`components/` concern→
Phase 6. The tests concern→settled (kept co-located); the SSR-test
relocation that *does* happen is Task 1.4 Step 8.

**Placeholder scan** — Phases 4–8 use a task *outline* rather than
bite-sized steps by explicit design (repo convention: per-phase docs
written against the tree the previous phase produced). Each still carries
concrete Files, Interfaces, and an Acceptance Gate. Phases 1–3 are fully
bite-sized with real commands and code. No "TBD"/"add error
handling"/"similar to Task N".

**Type consistency** — `PeerId`/`RoomCode`/`Nick`/`HexColor`/`IdError`
defined in Task 2.2, re-exported in Task 3.4, used in Phases 5/8.
`RoomState` + five sub-stores defined in Phase 5, moved (not
redefined) in Phase 6, referenced in Phase 8. `PeerLink` named as a
trait in the existing branch, extended to a struct in Phase 8 (§Phase 8
Architecture spells out both). `WebRtcError` defined in Phase 4, no
earlier use. `teardown_link` introduced once (Phase 8). `status_meta`
signature preserved verbatim across the Phase 3 move.

**Known follow-ups deliberately out of scope:** tightening
`status_meta`'s `(&str, &str)` return into an enum; the `RoomSession`
method-API rewrite beyond the map collapse (making `RoomSession.sharing`
reactive — see `2026-08-28-refactor-phase-5-roomsession.md` §5b.5); an
`xtask` crate for `scripts/`.
