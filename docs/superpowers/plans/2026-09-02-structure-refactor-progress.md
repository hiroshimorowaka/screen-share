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
| 5 | `ac80131` | The three pre-auth panels + `manual_join` → `<RoomGate>` in `features/room/gate.rs` | **partial** |
| 8 | — | Seam traits (`SignalingTransport` / `PeerLink` / `DisplayCapture`) | **deferred** |

## Remaining work

### Step 5 — the rest

`RoomPage` is still ~450 lines: it owns ~25 signals, the desktop
quick-share auto-flow effect, the audio-self-test / mute / invite-copy
effect block, and the stage-header + control-bar markup. Not yet done,
each needs the manual two-tab gate:

- **Bundle the signals** into small state structs (`ShareUi`, `WatchUi`,
  `PeerMedia`) so consumers depend only on what they read.
- **Lift the two `#[cfg(feature = "hydrate")]` effect blocks** into named
  `fn`s under `session/` (paired `#[cfg(not(hydrate))]` no-ops), so
  RoomPage stops holding browser-effect wiring inline.
- **`SharingState` enum** (`Idle` / `Sharing { stream }`) replacing the
  `is_sharing: bool` signal + `RoomSession.local_stream: Option<…>` held
  in parallel — the "impossible states" tightening. Invasive: touches
  `session/{mod,media}`, every `is_sharing` reader, and the wasm tests.

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
(54), and every desktop check — all green.

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
