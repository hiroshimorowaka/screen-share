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
| 8 | — | Seam traits (`SignalingTransport` / `PeerLink` / `DisplayCapture`) | **in progress** (8a `SignalingTransport` done; `PeerLink` / `DisplayCapture` remain) |

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

### Step 8 — in progress

Introduce a capability trait per browser boundary — `SignalingTransport`
(typed send over the socket), `PeerLink` (one `RtcPeerConnection`'s
lifecycle: offer/answer/ICE/close), `DisplayCapture` (`getDisplayMedia`)
— with the `web-sys` impls in `infra/` and fakes for tests, so parts of
`session/`'s dispatch logic can be tested against a fake instead of a
live signaling server / real WebRTC negotiation.

#### 8a — `SignalingTransport` (done)

`infra::signaling_transport::SignalingTransport` (`send` / `close`) —
`RoomSession.ws` is now `Rc<RefCell<Option<Box<dyn SignalingTransport>>>>`
instead of `Rc<RefCell<Option<WsClient>>>`; `WsClient` implements it.
Deliberately minimal: `on_open` / `on_close` / `set_on_message` stay
inherent `WsClient` methods, never abstracted, because they're only ever
called on a freshly connected, still-concrete `WsClient` *before* it's
boxed into `conn.ws` — abstracting them would add trait surface no
caller through the trait object actually uses.

New: a `FakeTransport` (records sent messages, in
`session/reconnect/wasm_tests.rs`) backs two new tests for
`replay_intent_after_rejoin` — resends the share and only the still-
present watch after a reconnect; a no-op when no reconnect was in
flight — with no live socket and no signaling server. This is the shape
of the capability step 8 exists for: `session::handler`'s three
`ws.send(...)` sites (`Answer`, `Offer`, `IceCandidate`) and
`session::{media,quality,latency,watch}`'s could get the same treatment
directly with today's trait — no further plumbing needed, just tests.

**What this does *not* touch:** `PeerLink` / `DisplayCapture` (below) —
those abstract `web_sys::RtcPeerConnection` and `getDisplayMedia`
directly, a far bigger surface across `infra/webrtc.rs` (564 ln) and
`session/{handler,media,quality,watch,reconnect}`, and unlike
`SignalingTransport`'s `send`/`close`, a fake standing in for a real
`RtcPeerConnection` only has any value if it can still negotiate a real
SDP offer/answer — meaning even a "fake" `PeerLink` used in a
meaningful test still needs a real `web_sys::RtcPeerConnection`
underneath (see `infra::webrtc::wasm_tests::offer_answer_roundtrip_
completes_between_two_local_peers`, which already does this without any
trait). The actual gap `PeerLink` would close is narrower than it looks:
mostly letting `session::handler`'s branch logic (which connection maps
get touched, which messages get sent, in what order) be asserted without
also standing up two full local `RtcPeerConnection`s per test — real,
but modest next to the risk of touching every call site that holds
`outgoing`/`incoming: Rc<RefCell<HashMap<String, RtcPeerConnection>>>`
directly.

**Why `PeerLink` / `DisplayCapture` stay a separate sub-step:** the
blast radius is `infra/webrtc/mod.rs`, `session/{handler,media,quality,
watch,reconnect}`, and the `RoomSession` struct's connection maps
themselves. This is the exact code path with no automation for its
hardest failure modes — teardown races and ICE ordering can only be
checked with two real browsers sharing screens — so per
`docs/superpowers/plans/2026-08-28-refactor-phase-5-roomsession.md`'s
own precedent, each sub-step needs a maintainer gate (share → watch →
stop via the in-app button *and* the browser's own control → reload →
reconnect) before the next one starts. 8a shipped inside one such
gate (full `--no-mutants` + `e2e-web`, both green — see Verification);
`PeerLink` / `DisplayCapture` are large enough to warrant their own.

## Verification

`scripts/test-all.sh --no-mutants` on the branch tip: fmt, clippy
(ssr + hydrate, incl. `--tests`), `cargo leptos build`, workspace tests
(209), wasm suite (60 — the original 54, plus 4 `SharingState` tests,
plus 2 new `FakeTransport`-backed `replay_intent_after_rejoin` tests),
and every desktop check — all green.

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
