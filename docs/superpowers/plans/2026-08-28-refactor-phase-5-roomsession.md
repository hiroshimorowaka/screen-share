# Refactor Phase 5 — Feature layout + RoomSession

> Part of `2026-08-28-architecture-refactor-roadmap.md`. Read its Global
> Constraints and Dependency invariants first.

**Goal:** (5a) Flatten `apps/web/src/ui/` into a feature-oriented layout
with no behavior change. (5b) Pull the room's signaling + WebRTC + local
media wiring out of the `features/room/*` helper modules into a named
`RoomSession` the component drives through a small API.

**Why split into 5a / 5b:** 5a is a pure path move — `cargo leptos build`
+ an HTTP smoke gate it, plus one "click around in a browser" check. 5b
touches the delicate Chrome screen-share teardown quirks (the stuck
"sharing" indicator workarounds in `share.rs`, ICE ordering in
`message_handler.rs`) that **cannot be verified without two real browser
tabs sharing screens** — there is no automation harness for that layer
(CLAUDE.md "Testing approach"). 5b therefore needs the maintainer
driving a manual checklist between sub-steps.

---

## 5a — Feature layout (mechanical, no behavior change)

### Target

```
apps/web/src/
├── lib.rs
├── main.rs
├── app.rs                     # was ui/app.rs
├── components/                # was ui/components/  (generic: color_picker, icons, palette, status, status_message)
├── infra/                     # was ui/client/      (rooms_api, session, socket, storage, webrtc)
├── features/
│   ├── home/                  # was ui/pages/home/
│   ├── room/                  # was ui/pages/room/
│   └── profile.rs             # was ui/profile.rs
└── quick_share.rs             # was ui/quick_share.rs
```

`ui/` disappears; the crate root *is* the UI.

### Steps

- [ ] **1. Move the trees with git**

```bash
cd apps/web/src
git mv ui/app.rs app.rs
git mv ui/components components
git mv ui/client infra
git mv ui/pages features
git mv ui/profile.rs features/profile.rs
git mv ui/quick_share.rs quick_share.rs
rmdir ui            # ui/mod.rs is deleted in step 2
git rm ui/mod.rs
```

- [ ] **2. Rewrite `lib.rs` module tree**

```rust
#![recursion_limit = "256"]

pub mod app;
pub mod components;
pub mod features;

#[cfg(feature = "hydrate")]
pub mod infra;
#[cfg(feature = "hydrate")]
pub mod quick_share;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
```

`profile` is now `features::profile` (add `pub mod profile;` to
`features/mod.rs`, which was `ui/pages/mod.rs`).

- [ ] **3. Repoint every path**

```bash
cd apps/web
grep -rl 'crate::ui::' src | xargs sed -i \
  -e 's/crate::ui::client::/crate::infra::/g' \
  -e 's/crate::ui::pages::/crate::features::/g' \
  -e 's/crate::ui::components::/crate::components::/g' \
  -e 's/crate::ui::profile/crate::features::profile/g' \
  -e 's/crate::ui::quick_share/crate::quick_share/g' \
  -e 's/crate::ui::app/crate::app/g'
grep -rn 'crate::ui::' src   # must print nothing
```

`main.rs`: `use screen_share::ui::app::{shell, App};` →
`use screen_share::app::{shell, App};`

- [ ] **4. `cargo fmt` then verify**

```bash
cargo fmt
cargo test --workspace --features ssr          # 80, unchanged
cargo clippy --workspace --all-targets --features ssr -- -D warnings
cargo clippy -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate -- -D warnings
cargo leptos build
```

- [ ] **5. HTTP smoke** (`cargo leptos watch`, then curl `/`,
  `/pkg/screen_share.wasm`, `/styles/home.css`, `/api/rooms/ZZZZZZ` — all
  200 / expected JSON, no hydration mismatch in the served HTML).

- [ ] **6. Commit** `refactor: flatten apps/web into feature-oriented layout`.

- [ ] **7. MAINTAINER GATE:** load `http://127.0.0.1:3000/` in a browser,
  create a room, open the room page, confirm styling + no console errors.

---

## 5b — RoomSession extraction (needs maintainer browser testing)

### Current shape (what 5b consolidates)

`features/room/` today already keeps the I/O out of the `RoomPage`
component itself — it lives in `pub(super) fn` helpers:

| module | owns |
|--------|------|
| `connection.rs` | `RoomConnection` struct (`ws`, `outgoing`/`incoming` `RtcPeerConnection` maps, `local_stream`, `expected_close`, `last_ping_sent_at`, `quality_auto_intervals`); `setup_room_connection`; `adopt_pending_session` |
| `message_handler.rs` | `build_message_handler` — the `ServerMessage` match arm that creates peer connections, does offer/answer/ICE, applies quality |
| `share.rs` | `start_sharing` / `stop_sharing` — `getDisplayMedia`, the `track.onended` hook, and the Chrome "sharing indicator" teardown sequence (detach senders → stop+removeTrack → clear `srcObject` → close PCs) |
| `watch.rs` | `watch_click_handler` / `stop_watching_click_handler` / `leave_room` / `leave_or_stop_watching_handler` |
| `quality.rs` | adaptive tier logic + per-viewer `setInterval` "Auto" polling |
| `latency.rs` | the ping loop |

`RoomConnection` is already the de-facto session object. `RoomPage`
declares ~20 `RwSignal`s inline and passes them around in `RoomSignals`.

### Target

`apps/web/src/session/` (new, `#[cfg(feature = "hydrate")]` with an inert
`ssr` stub mirroring the existing `RoomConnection` pattern):

- `mod.rs` — `RoomSession` struct, owns `SignalingClient` +
  `PeerConnectionManager` + `LocalMedia` + the reactive room state it
  exposes read-only. Public API (binding — the component uses exactly
  these):
  - `RoomSession::new() -> Self` (inert-stub-able)
  - `fn connect(&self, room: &str, nick: String, color: String, password: Option<String>)`
  - `fn adopt_pending(&self, room: &str) -> bool`
  - `fn start_sharing(&self, on_cancelled: impl Fn() + 'static)`
  - `fn stop_sharing(&self)`
  - `fn watch(&self, sharer_id: &str)` / `fn unwatch(&self, sharer_id: &str)`
  - `fn leave(&self, room: &str)`
  - `fn set_quality(&self, sharer_id: &str, level: QualityLevel)`
  - read accessors returning the existing signals
    (`members`, `sharing`, `watching`, `watchers_by_sharer`,
    `latency_by_peer`, `quality_by_peer`, `connection_errors`,
    `expanded`, `status`, …)
- `peers.rs` — `PeerConnectionManager`: the `outgoing`/`incoming` maps,
  offer/answer/ICE, and **the single teardown path** (doc-comment it as
  the one owner of "what happens when a sharer stops", per CLAUDE.md).
  Absorbs the WebRTC calls currently inline in `message_handler.rs` and
  the sender/track teardown loop in `share.rs::stop_sharing`.
- `media.rs` — `LocalMedia`: `getDisplayMedia`, the `track.onended` hook,
  and a `SharingState` enum (`Idle` / `Sharing { stream, .. }`)
  **replacing** `is_sharing: bool` + `local_stream: Option<MediaStream>`
  held separately.
- `handler.rs` — the `ServerMessage` dispatch (was `message_handler.rs`),
  now calling `PeerConnectionManager` / `LocalMedia` methods, not raw
  `web-sys`.

`features/room/*` components: drop the `RoomSignals` bundle and the
`setup_*`/`*_handler` free functions; take a `RoomSession` (via Leptos
context) and call its methods / render its accessors. `RoomPage`
constructs the `RoomSession` once.

Keep every browser call behind the `#[cfg(hydrate)]` /
`#[cfg(not(hydrate))]` paired-function pattern.

### Sub-steps — each ends with the MAINTAINER GATE below

1. **`SignalingClient`**: rename `infra/socket.rs`'s `WsClient` usage into
   a `session::SignalingClient` that owns connect + typed send + the
   `on_open`/`on_close` wiring currently in `connection.rs`. Move
   `connection.rs::setup_room_connection` / `adopt_pending_session` into
   `session/mod.rs`. `message_handler` untouched this step.
   → GATE: join a room in two tabs, roster appears both sides.
2. **`LocalMedia` + `SharingState`**: move `share.rs::start_sharing` /
   `stop_sharing` into `session/media.rs` behind
   `RoomSession::start_sharing` / `stop_sharing`; replace the
   `is_sharing` signal + `local_stream` cell with a `SharingState`
   signal; keep the Chrome-indicator teardown sequence **byte-for-byte**
   (it is load-bearing — see the comments in `stop_sharing`).
   → GATE: share from tab A; the card lights up in tab B; stop via the
   in-app button; stop via Chrome's own "Stop sharing" bar; the red
   tab indicator clears both ways; share again — no stacked indicator.
3. **`PeerConnectionManager`**: move the `outgoing`/`incoming` maps and
   the offer/answer/ICE handling out of `message_handler.rs` into
   `session/peers.rs`; `handler.rs` calls it. One `teardown(peer_id)`
   method used by stop-sharing, unwatch, leave, and `PeerLeft`.
   → GATE: A shares, B watches — video flows. B stops watching — B's
   connection closes, A keeps sharing for any other watcher. A stops —
   B's card goes back to the avatar. Reload B mid-watch — clean rejoin.
4. **`quality.rs` + `latency.rs`** move their `conn`-held interval state
   (`quality_auto_intervals`, `last_ping_sent_at`) onto `RoomSession`.
   → GATE: change a stream's quality (Auto + a fixed tier); ping shows
   and updates on every card.
5. **Component cleanup**: `RoomPage` provides `RoomSession` via context;
   delete `RoomSignals`, the `setup_*`/`*_handler` shims, and
   `connection.rs`. `features/room/` components read context.
   → GATE: full run of gates 1–4 again, plus: two sharers + two watchers
   simultaneously; leave room; the desktop quick-share auto-flow
   (`quick_share`) still starts a share and copies the link.

### MAINTAINER GATE (run at the end of every sub-step)

Two Chrome tabs (or two machines), same room:
- roster shows both; nick + color correct
- A shares → B sees the "watch" button on A's card (no auto-video)
- B watches A → video flows; A's preview shows locally
- B stops watching (button + fullscreen exit) → only B's PC closes
- A stops sharing (in-app button) → B's card returns to avatar; Chrome
  red indicator on A clears
- A stops sharing (Chrome's own bar) → same result
- reload B while watching → silent rejoin, no gate
- `cargo test --workspace --features ssr` still 80, clippy + fmt clean

Behaviour must match `main` (well, `refactor/workspace-split` pre-5b) exactly.

### Acceptance

Full suite green; clippy (ssr + wasm) and fmt clean; `cargo leptos
build` + `docker build` succeed; the MAINTAINER GATE checklist passes and
is recorded here.
