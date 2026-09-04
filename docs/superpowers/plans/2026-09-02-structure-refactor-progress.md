# Structure & code-design refactor — progress

## v3 plan phase status (`2026-09-03-structure-refactor-v3.md`)

| Phase | What | Status |
|-------|------|--------|
| 1 | Extract `apps/server` | done (`d6ade1f`) |
| 2 | Typed ids in `protocol::ids` | done (`8a7a347`, `543b33f`) |
| 3 | Finish `crates/domain` (quality machine, status) | done (`f7ef111`, `60aa7af`) |
| 4 | `client/` umbrella + split `webrtc/` | done (`07fae72`, `85044d3`) |
| 5 | `RoomState` + `provide_context` (flat struct, deviation noted) | done (`35a0e37`) |
| 6 | `pages/` + `room/` + `home/` feature slices | done |
| 7 | `desktop_bridge` fold-in + CSS `@layer` | done |
| 8 | Collapse the 4 peer maps into two `HashMap<String, PeerLink>` | done (see notes) |

### Phase 6 notes

`features/` and `session/` are gone. New tree: `app/{mod,router}`,
`room/` (the room slice — `page` is the `/r/:code` route shell, plus
`state`, `session`, `connection`, `messages`, `reconnect`,
`share_effects`, the per-capability handlers, and `components/` with
`stage` + `stage_header` + `sharing_controls` + `view_controls` +
`room_controls` + `gate` + `participant/{mod,badges,action_bar,watch_widgets,parts}`
+ `participant_grid` + `transmission_menu`), `home/` (`page` is the `/`
route, plus `state` + `create`/`join`/`recent` +
`components/{create_panel,join_panel}`), top-level `profile.rs` +
`not_found.rs`, `components/{ui,palette}`.

The v3 plan's separate `pages/` layer was dropped (maintainer decision,
post-Phase-6): it was a three-file indirection whose "thin route
component" meaning didn't survive `pages/room.rs` being a shell over a
whole `room/` slice. Each route entry now lives in its feature folder as
`page.rs` (`room::RoomPage`, `home::HomePage`); `app::router` names them
directly.

`RoomPage` / `HomePage` split into a route shell plus components, which
retired four of the five `#[allow(clippy::too_many_lines)]`. The fifth,
on `room::dev_preview::DevRoomPreviewPage`, is kept: it is
`#[cfg(debug_assertions)]` fixture scaffolding that never ships, and
splitting a 350-line hand-built `view!` there is churn without payoff —
a conscious carve-out, like Phase 5's flat `RoomState`.

Deviations from the v3 file map: `RoomSession` (not `Send`) is threaded
as a value / component prop, never `provide_context`'d (a
`components/layout/header.rs` `<Wordmark>` component was tried and
reverted — it broke hydration); the `room/` internals stay a flat set of
modules rather than the full `actions/` + `effects/` folder split (same
"consolidation is the goal" call as Phase 5). `components/` keeps `ui/`
but not `layout/`.

Gate: `scripts/test-all.sh` targets `lint`, `build`, `rust` (workspace
ssr + wasm 62), and `e2e-web` (36) — all green. `grep -rn
'crate::features::\|crate::session::' apps/web/src` is empty. Desktop
suite / `e2e-desktop` not re-run (untouched by this phase).

### Phase 7 notes

`apps/web/src/quick_share.rs` is folded into
`client/desktop_bridge.rs` (its "Desktop tray quick-share flow"
section) — one hydrate-only module owns everything that reads the
`quick_share=1` param or calls the Electron `window.desktopShare.*` /
`window.desktopAudio.*` bridges. `crate::quick_share::*` →
`crate::client::desktop_bridge::*` everywhere; `pub mod quick_share`
dropped from `lib.rs`. The `notify_desktop_*` names are kept as-is (the
v3 plan's rename to `notify_*` is cosmetic and left for later).

CSS `@layer`: `tokens.css` opens with
`@layer tokens, base, components, features, utilities;`, then each
stylesheet's whole body is wrapped in its layer — `tokens`→tokens,
`base`→base, `card`/`card-widgets`/`room-transmission-menu`→components,
`home`/`room`→features, `dev_preview`→utilities. No selector or
declaration edits. This does reorder the cascade (card rules now lose
ties to room rules, per the plan's layer assignment); `e2e-web` (render
+ CSP + the two-tab flow) stays green, but a by-hand pixel check is
still worth doing.

Gate: `lint`, `build`, `rust` (workspace ssr + wasm 62), `e2e-web` (36)
all green. Desktop suites untouched, not re-run.

### Phase 8 notes

`RoomSession`'s four parallel maps (`outgoing`, `incoming`,
`outgoing_callbacks`, `incoming_callbacks`) collapse into a `PeerLink`
struct — `{ pc, callbacks }` — held in **two** maps, `links_out` and
`links_in` (`room::session`). A `pc` without its callbacks (or vice
versa) is no longer representable.

Deviations from the v3 Phase 8 design:

- **Two maps, not one.** A single `HashMap<peer, PeerLink>` collides
  under mutual watching: A can have both an outgoing link to B (B watches
  A) and an incoming link from B (A watches B), keyed identically. Kept
  the split; `LinkDirection` (an enum) is what `teardown_link` takes to
  pick a map, and there is no redundant `direction` field on `PeerLink`.
- **Keys stay `String`.** The web side never adopted the typed `PeerId`
  for these maps; converting ~40 sites was out of scope here.
- **Trait rename.** The step-8c seam trait `PeerLink` (offer / answer /
  ICE on one `RtcPeerConnection`) is renamed `Negotiate` to free the
  name for the bundle struct.
- **`teardown_outgoing` + `teardown_incoming` → `teardown_link(conn,
  peer, direction)`** — the one close-and-forget path the plan asked for.
- **`offer_to_watcher` reordered** so the closures + the `links_out`
  insert all run before the first `.await` (the encoding setup), so a
  relayed ICE candidate arriving mid-setup still finds the link — the
  early bare-`pc` insert the four-map version relied on is no longer
  needed.
- **Deferred (as before):** genericizing the negotiation fns over the
  seam traits + `FakeTransport` / `FakePeerConnection` native tests —
  still bundled with the `RoomSession` method-API rewrite
  (`2026-08-28-refactor-phase-5-roomsession.md`).

Gate: `lint` (clippy ssr + hydrate), `build`, `rust` (workspace ssr +
wasm 62), `e2e-web` (36 — incl. `room-p2p` real-media share/watch/
teardown, source switch, stop-watching, watcher reload + reconnect) all
green. **Still owed:** the maintainer's two-tab manual checklist
(`2026-08-28-refactor-phase-5-roomsession.md` §5b) — Chrome's own "stop
sharing" bar, mutual watch with 3+ members, ICE ordering — has no
automation harness.

---

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
| 8 | `7060a9d`, `bfbe16a`, — | Seam traits (`SignalingTransport` / `DisplayCapture` / `PeerLink`) | **done** |

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

### Step 8 — done

Introduced a capability trait per browser boundary — `SignalingTransport`
(typed send over the socket), `DisplayCapture` (`getDisplayMedia`),
`PeerLink` (one `RtcPeerConnection`'s offer/answer/ICE) — with the
`web-sys` impls in `infra/`. Two of the three ship with fakes and new
tests exercising a real coverage gap each; the third (`PeerLink`) is a
named domain contract without a fake, for reasons specific to it — see
below. All three commits (`7060a9d`, `bfbe16a`, and `PeerLink`'s) each
shipped behind a full `--no-mutants` + `e2e-web` gate.

#### 8a — `SignalingTransport` (`7060a9d`)

`infra::signaling_transport::SignalingTransport` (`send` / `close`) —
`RoomSession.ws` is now `Rc<RefCell<Option<Box<dyn SignalingTransport>>>>`
instead of `Rc<RefCell<Option<WsClient>>>`; `WsClient` implements it.
Deliberately minimal: `on_open` / `on_close` / `set_on_message` stay
inherent `WsClient` methods, never abstracted, because they're only ever
called on a freshly connected, still-concrete `WsClient` *before* it's
boxed into `conn.ws` — abstracting them would add trait surface no
caller through the trait object actually uses.

A `FakeTransport` (records sent messages, in
`session/reconnect/wasm_tests.rs`) backs two tests for
`replay_intent_after_rejoin` — resends the share and only the still-
present watch after a reconnect; a no-op when no reconnect was in
flight — with no live socket and no signaling server.

#### 8b — `DisplayCapture` (`bfbe16a`)

`infra::display_capture::DisplayCapture` (`async fn capture`) —
`session::media::{start_sharing, switch_source_handler}` take a generic
`C: DisplayCapture` instead of calling `infra::webrtc::capture_display`
directly. Static dispatch (a generic parameter, not `dyn`): `capture` is
`async` in the trait and the two call sites are all that exist, so no
dyn-safety cost to pay. `BrowserDisplayCapture` (the real impl) is a
zero-sized marker defined in `session::media` — not `infra`, which is
entirely `hydrate`-gated — so the `ssr` build's inert
`switch_source_handler` stub can still name the type at the call site it
shares with the `hydrate` one.

This closed a real gap, not a style one: headless Chrome has no display
to capture, so `capture_display()` always rejects there —
`start_sharing`'s happy path (the stream gets stored, the native-stop
listener attaches, `StartShare` gets sent) had no unit-level coverage
before this, only the cancelled-picker branch. Two new wasm tests in
`session/media/wasm_tests.rs` cover both paths with a
`FakeDisplayCapture` / `RejectingDisplayCapture`; needed `any_spawner` as
a dev-dependency to init `leptos`'s executor in tests (the real app gets
this for free from `leptos::mount::hydrate_body`, which the test harness
never calls). Also caught, in review, a bug in the *test* itself (not
production code): `web_sys::MediaStream` has an inherent `clone()` bound
to the DOM's `MediaStream.clone()` — an actual new stream, new id —
which shadows `Clone::clone` on direct method-call syntax; fixed with
`Clone::clone(&self.stream)` (UFCS) to force the trait's cheap reference
clone.

#### 8c — `PeerLink` (no fake — see below)

`infra::peer_link::PeerLink` (`offer` / `answer` / `accept_answer` /
`add_ice_candidate`), implemented for `RtcPeerConnection`.
`session::handler`'s four negotiation call sites (`answer_offer`,
`accept_answer_from`, `route_ice_candidate`, `offer_to_watcher`) now call
`pc.offer()` / `pc.answer(&sdp)` / `pc.accept_answer(&sdp)` /
`pc.add_ice_candidate(...)` instead of importing
`infra::webrtc::{create_offer, create_answer, accept_answer,
add_ice_candidate}` directly — the negotiation contract reads as one
named interface at its call sites (CLAUDE.md's "design around domain
concepts rather than leaking implementation details"), even though it
isn't wired up for a fake.

**Why no fake, unlike 8a/8b:** `session::handler`'s negotiation functions
don't just call offer/answer/close — they also wire `RtcPeerConnection`
event listeners (`ontrack`, `onicecandidate`,
`oniceconnectionstatechange`) and store the connection itself in
`RoomSession::{outgoing,incoming}` (`HashMap<String, RtcPeerConnection>`,
a concrete type `session::{media,quality}` also read for operations
`PeerLink` doesn't cover — senders, transceivers). Genericizing these
functions over `PeerLink` alone wouldn't compile without *also*
abstracting that other surface, and abstracting all of it is the
`RoomSession` method-API rewrite already scoped and deferred in
`docs/superpowers/plans/2026-08-28-refactor-phase-5-roomsession.md` —
not something to fold into this pass. Concretely: even a "fake"
`PeerLink` is only useful in a test if the surrounding function can run
against it too, and that function needs an event-emitting object a fake
can't provide without becoming a large chunk of `RtcPeerConnection`
itself — see `infra::webrtc::wasm_tests::offer_answer_roundtrip_
completes_between_two_local_peers`, which already tests real
offer/answer/ICE end-to-end with two local (no-network) connections,
without needing any trait.

Verified with the full gate anyway, since it touches
`session::handler`'s negotiation path directly: full `--no-mutants`, plus
`e2e-web`'s `room-p2p.spec.ts` (real two-tab share/watch, real media
flowing) — the strongest signal available for this exact change without
a live two-tab manual session.

## Verification

`scripts/test-all.sh --no-mutants` on the branch tip: fmt, clippy
(ssr + hydrate, incl. `--tests`), `cargo leptos build`, workspace tests
(209), wasm suite (62 — the original 54, plus 4 `SharingState` tests, 2
`FakeTransport`-backed `replay_intent_after_rejoin` tests, and 2
`FakeDisplayCapture`/`RejectingDisplayCapture`-backed `start_sharing`
tests), and every desktop check (incl. `e2e-desktop`) — all green.

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
