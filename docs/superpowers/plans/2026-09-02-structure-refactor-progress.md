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
| 5 | `ac80131`, `a85ce85`, — | The three pre-auth panels + `manual_join` → `<RoomGate>`; own-share/audio signals bundled into `ShareUi`/`PeerMedia` (`session::share_ui`); the quick-share and audio-effects blocks lifted into `session::share_effects`; `RoomSession`'s `local_stream: Option<MediaStream>` replaced by a `SharingState` enum (`session::sharing_state`) | **done** |
| 8 | — | Seam traits (`SignalingTransport` / `PeerLink` / `DisplayCapture`) | **deferred** |

## Remaining work

### `SharingState` — what shipped and what stayed out of scope

`RoomSession.local_stream: Rc<RefCell<Option<MediaStream>>>` is now
`RoomSession.sharing: Rc<RefCell<SharingState>>`
(`SharingState::Idle` / `Sharing { stream }`, in `session::sharing_state`,
with `is_sharing()` / `stream()` / `take()`). This replaces the ~10
scattered `.borrow().is_some()` / `.clone()` / `.take()` /
`*x.borrow_mut() = Some(...)` call sites across `session/{media,handler,
audio,video_mode,reconnect}` and `features/room/watch.rs` with one type
that can't represent "has a stream but isn't sharing" or vice versa.
Unit-tested in `session/sharing_state/wasm_tests.rs` (4 tests: default
is `Idle`, `Sharing` exposes its stream, `take` empties it back to
`Idle`, `take` on `Idle` is a no-op).

The UI-facing `is_sharing: ReadSignal<bool>` (in `ShareUi`) is
**unchanged and deliberately not merged into this enum** — it's a
reactive Leptos signal the view reads, while `RoomSession.sharing` is
plain `Rc<RefCell<...>>` kept non-reactive on purpose, so it stays
readable/writable from JS callbacks and from a bare `wasm-bindgen-test`
with no reactive runtime (see `teardown_local_share`'s doc comment and
`session/media/wasm_tests.rs`, both of which still touch `RoomSession`
outside any component). That pairing is a deliberate reactive/imperative
boundary, kept in sync at the same points as before (`start_sharing`,
`switch_source_handler`), not a bug this enum was meant to fix. Merging
the two — the literal reading of the original plan bullet — was
evaluated and rejected: it would require `RoomSession.sharing` to become
reactive, which breaks that "callable outside any component" invariant
several existing tests rely on. See
`docs/superpowers/plans/2026-08-28-refactor-phase-5-roomsession.md`
(§"5b.5 — DEFERRED"), which already scoped the full reactive-signal
merge as its own, separately-gated piece of work, bundled with the
`RoomSession` method-API refactor.

**Verification note:** this touches the exact code path
`docs/superpowers/plans/2026-08-28-refactor-phase-5-roomsession.md`
flags as needing a two-tab manual GATE (share → stop via Chrome's own
"stop sharing" bar → indicator clears with no stack). The automated
suite exercises the in-app stop-sharing button, a source switch, and
leaving mid-share (`room-controls.spec.ts` / `room-p2p.spec.ts`), which
all still pass, but the browser's-own-control path itself remains the
pre-existing hand-verification gap noted below — do that check before
relying on this in production.

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
(58 — the pre-existing 54 plus 4 new `SharingState` tests — including
`session::media::wasm_tests` and `session::audio::wasm_tests`, both of
which touch `RoomSession.sharing` directly and still pass), and every
desktop check — all green.

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
