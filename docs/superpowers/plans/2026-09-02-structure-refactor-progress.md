# Structure & code-design refactor — progress

Branch: `worktree-refactor+structure-and-code-design`. Executes the
sequence from the independent review (decoupling / directory layout /
code design + cyclomatic complexity). Every step below is its own commit;
each left `cargo test --workspace --features ssr` (209), the wasm suite
(54), `clippy` (ssr + hydrate), `fmt` and `cargo leptos build` green.

| # | Commit | Step | Status |
|---|--------|------|--------|
| 1 | `64869d0` | Fold modules with test siblings into `foo/{mod,tests,wasm_tests}.rs` — `session/` 24 files → `mod.rs` + 9 folders; same for `infra/`, `components/`, `features/{home,room}` | done |
| 2 | `af2999d` | `#![warn(clippy::too_many_lines / too_many_arguments)]` on the web crate; item-level `#[allow]` + note on the 5 current offenders as the backlog | done |
| 3 | `65feee8` | `build_message_handler` (432 ln) → flat dispatch + one `fn` per message; `teardown_outgoing/incoming`, `drop_focus_if_showing`, `attach_local_tracks`, `fixed_status_text` remove 5 duplicated `pc.close()` sites | done |
| 6 | `fd75e11` | Split `card.css` (691) → +`card-widgets.css`; `room.css` (590) → +`room-transmission-menu.css`. Verified the selector + declaration set is byte-identical | done |
| 7 | `2a5923a` | New dependency-free `crates/domain`: `sdp` (whole module) + `backoff::BackoffPolicy`, now native-tested. Dependency ladder + CLAUDE.md updated | done |
| 4 | `83c9415` | `member_cards` (333-ln free fn) → 6-line loop over `<MemberCard>`; `QualityMenu` + `VolumeControl` become components in `member_card/parts.rs` | done |
| 5 | `ac80131`, — | The three pre-auth panels + `manual_join` → `<RoomGate>`; own-share/audio signals bundled into `ShareUi`/`PeerMedia` (`session::share_ui`); the quick-share and audio-effects blocks lifted into `session::share_effects` | **done** (`SharingState` enum split out — see below) |
| 8 | — | Seam traits (`SignalingTransport` / `PeerLink` / `DisplayCapture`) | **deferred** |

## Remaining work

### `SharingState` enum — deferred, own session (not part of step 5 anymore)

Replacing the `is_sharing: bool` signal + `RoomSession.local_stream:
Option<MediaStream>` held separately with one `SharingState` enum
(`Idle` / `Sharing { stream }`) was step 5's last item. Re-examining it
against `docs/superpowers/plans/2026-08-28-refactor-phase-5-roomsession.md`
(§"5b.5 — DEFERRED"), this exact change was already scoped and
deliberately deferred there, bundled with the `RoomSession` method-API
work, for the same reason: "each of its own sub-steps needs a full
2-tab browser GATE" and it should be "scheduled as a dedicated
session." That reasoning still holds and a background session has no
way to run that gate — this stays out of scope here rather than being
forced through unverified. Splitting it out of step 5 (which is now
otherwise done) instead of leaving step 5 "partial" indefinitely.

**Why it doesn't reduce to a pure refactor:** `is_sharing` is a reactive
Leptos signal the view reads; `local_stream` is a plain
`Rc<RefCell<Option<MediaStream>>>` deliberately *non*-reactive so it stays
readable/writable from JS callbacks and from a bare `wasm-bindgen-test`
with no reactive runtime (see `teardown_local_share`'s doc comment and
`session/media/wasm_tests.rs`). Folding both into one signal would need
`local_stream` to become reactive too, which breaks that "callable
outside any component" invariant several existing tests rely on. A
narrower, still-real slice — replacing `RoomSession`'s
`Option<MediaStream>` with an `Idle`/`Sharing{stream}` enum on its own,
independent of the UI-facing `is_sharing` signal — is worth doing, but
still needs the two-tab GATE (share → the browser's own "stop sharing"
control → no stuck indicator) that this session can't run.

### Step 8 — deferred, own session

Introduce a capability trait per browser boundary — `SignalingTransport`
(typed send over the socket), `PeerLink` (one `RtcPeerConnection`'s
lifecycle: offer/answer/ICE/close), `DisplayCapture` (`getDisplayMedia`)
— with the `web-sys` impls in `infra/` and fakes for native tests, so
`session/handler`'s dispatch logic can be unit-tested without the
headless-browser harness.

**Why deferred:** the blast radius is `infra/webrtc/mod.rs` (564 ln),
`session/{handler,media,quality,watch,reconnect}` and the `RoomSession`
struct itself (`Rc<RefCell<HashMap<String, web_sys::RtcPeerConnection>>>`
→ trait objects / generics). That is the exact code path with no
automation — its teardown races and ICE ordering can only be checked
with two real browsers sharing screens. It is a multi-step change that
needs a maintainer gate between sub-steps, like the previously-deferred
`RoomSession` method-API work. Schedule it on its own.

## Verification

`scripts/test-all.sh --no-mutants` on the branch tip: fmt, clippy
(ssr + hydrate), `cargo leptos build`, workspace tests (209), wasm suite
(54, including `session::media::wasm_tests` and
`session::audio::wasm_tests`, both of which touch `local_stream`
directly and still pass unchanged), and every desktop check — all green.

`scripts/test-all.sh e2e-web` (Playwright, 36 tests, headed under xvfb) —
all green. This covers the two-tab path the refactor most affects:
share → watch → real WebRTC media flows → teardown; per-viewer watch
independence (one watcher stopping doesn't disturb another); the
stop-watching button; source switch keeping the share alive; the per-card
quality menu (hover open / leave close) and the transmission menu; a
watcher reload and a dropped-connection reconnect; the mobile quality
bottom-sheet and 44px touch targets; CSP not blocking create/join/share/
watch.

Still only checkable by hand (no automation harness — pre-existing gap,
CLAUDE.md "Testing approach"): stop sharing via **Chrome's own** "stop
sharing" bar, real screen/window capture, system audio, bitrate
adaptation under a throttled link, and the desktop tray quick-share
auto-flow.
