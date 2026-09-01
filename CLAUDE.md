# CLAUDE.md

Guidance for AI agents and humans working in this repository.

---

## 1. Language policy

**English:** code, identifiers, comments, doc comments, commit messages, branch
names, issue titles and bodies, PR, log strings, generated docs.

**Portuguese (pt-BR):** conversation with the maintainer.

No mixing. A Portuguese identifier or commit message is a defect.

Be concise and prioritize brevity over completeness. Respond with only the information necessary to answer the request or complete the task.

* Avoid unnecessary explanations, introductions, conclusions, and repetition.
* Keep responses as short as possible while remaining accurate.
* Prefer bullet points over long paragraphs when appropriate.
* Do not explain obvious concepts unless explicitly asked.
* Focus on actionable information instead of background context.
* Do not restate the user's request or summarize your own response.
* Assume the reader is technically proficient unless stated otherwise.

Default to minimal responses. Expand only when the user explicitly requests more detail.

---

## What this project is

A persistent room where a small group can share their screens with each
other — any number of them, at any time, not just one person presenting to
the rest. Someone creates a room (it gets a short code and a name), choosing
either a password or to make it explicitly public — anyone with the link can
join a public room, no password needed. They share the link (and password,
for a closed room) with whoever should join, and from then on anyone in the
room can start or stop sharing their own screen independently. Sharing and watching are decoupled, Discord-style:
starting a share never pushes video to anyone automatically — it just lights
up a "watch" button on that member's card for everyone else. Watching
someone is an explicit, per-person choice, made and revoked independently by
each viewer, and doesn't affect anyone else watching the same sharer. Each
member picks a nick and a color when they join; a small round avatar (the
nick's first letter over that color) stands in for their card until they're
sharing and someone is watching them. A screen share can carry sound: in a
plain browser that's the tab audio Chrome's own picker offers to include
(a shared tab only, never a window or the whole system); the desktop app
captures real system audio through a platform backend. Nobody installs
anything — everyone just uses a browser, on Windows or Linux. Video goes
directly from each sharer's browser to each viewer's
browser (WebRTC, peer-to-peer); the server's only job is introducing peers
to each other so those direct connections can be established.

## Tech stack

- **Rust** with **Leptos** (a full-stack, isomorphic web framework) as the
  single language and framework for both server and browser code.
- **Axum** as the HTTP/WebSocket server, wired up via `leptos_axum`.
- **Tokio** as the async runtime.
- WASM in the browser: the same Rust crate compiles to a `wasm32-unknown-unknown`
  target and runs client-side via `wasm-bindgen`, calling browser APIs
  (`web-sys`) directly — `getDisplayMedia`, `RTCPeerConnection`, `WebSocket`,
  `Clipboard`, etc.
- `serde`/`serde_json` for the signaling wire protocol.
- Styling is plain, hand-authored CSS on a design-token system (see
  `docs/decisions/0006-visual-redesign.md`). Web fonts (Google Fonts
  `<link>`) and established third-party libraries are allowed — prefer a
  proven dependency over reimplementing what it already does.
- `cargo-leptos` orchestrates building both the server binary and the WASM
  bundle from one `cargo` invocation.

## Commands

Prerequisites (once per machine):

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-leptos
```

Run the dev server (hot-reloading):

```bash
cargo leptos watch
```

Open `http://127.0.0.1:3000/`. **Always run it this way** (or via
`cargo leptos serve`) — running the compiled binary directly does not
produce a working page, because the server and the WASM bundle have to
agree on an output-name that's only threaded through correctly by
`cargo-leptos`'s own build/run flow (see `.cargo/config.toml`).

Run the automated test suite (from the repo root):

```bash
cargo test -p screen_share --features ssr
```

Run a single test:

```bash
cargo test -p screen_share --features ssr <test_name>
# e.g.
cargo test -p screen_share --features ssr create_room_registers_host_and_returns_code
```

The `-p screen_share` scope is explicit so the command keeps working as
the crate is split into a Cargo workspace (see the architecture-refactor
roadmap in `docs/superpowers/plans/`). Once extra crates exist, run their
tests with `cargo test -p <crate>`; `cargo test --workspace` runs all.

That native suite does **not** cover the `hydrate` (WASM) code path. Run
those in a headless browser with:

```bash
scripts/test-wasm.sh          # extra args pass through to `cargo test`
```

`.cargo/config.toml` already sets `runner = "wasm-bindgen-test-runner"`
for the `wasm32` target; the runner needs a WebDriver binary. The script
uses `chromedriver` if it is on `PATH`, otherwise downloads a
version-matched headless Chrome + chromedriver via `@puppeteer/browsers`
into `.wasm-browser/` (git-ignored). It also `cargo install`s
`wasm-bindgen-cli` at the `Cargo.lock` version if the runner is missing.

`scripts/test-all.sh` is the test runner. It takes an optional target
(default `all`) plus flags:

```bash
scripts/test-all.sh --no-mutants   # all checks except mutation (the usual pre-change run)
scripts/test-all.sh                 # everything, incl. mutation + coverage, like scheduled CI
scripts/test-all.sh e2e             # just the Playwright suites (web + desktop)
scripts/test-all.sh e2e-web         # just one suite: e2e-web | e2e-desktop | lint | build | rust | wasm | mutants | desktop
```

`--help` lists every target and flag. It collects failures, prints a
pass/fail/skip summary, and exits non-zero if anything failed; missing
optional tools are skipped, not failed. The Playwright suites run hidden
under `xvfb-run` when it is installed (`--no-xvfb` to show the windows).
The full matrix with prerequisites is in
`docs/superpowers/plans/2026-08-28-quality-gate.md`.

Production build:

```bash
cargo leptos build --release
```

Build and run the Docker image locally (mirrors how it runs in production):

```bash
docker build -t screen-share .
docker run -p 8080:8080 screen-share
```

Deploy (Fly.io):

```bash
fly deploy
```

## Architecture

### One crate, two compiled targets

This is the single most important thing to understand before touching any
UI code. The web crate lives at `apps/web/` as the sole member of a Cargo
workspace (`cargo-leptos` is driven from the repo root via
`[[workspace.metadata.leptos]]`). It is compiled twice, under two
mutually exclusive Cargo features:

- **`ssr`** — a native binary (`apps/web/src/main.rs`) that renders pages
  server-side and serves them over HTTP/WebSocket via Axum.
- **`hydrate`** — a `wasm32-unknown-unknown` library that runs in the
  browser, takes over the server-rendered HTML, and makes it interactive.

Every Leptos component (`#[component] fn ...`) is written once and runs
under *both* targets — the same function produces the initial HTML on the
server and re-renders/reacts to state in the browser. Anything that touches
a browser-only API (`web-sys`, `wasm-bindgen`, `RTCPeerConnection`,
`getDisplayMedia`, `WebSocket`, clipboard, timers, etc.) cannot exist in the
code path compiled for `ssr`, or the server binary won't build. The
established pattern for this is a pair of functions with the same
signature, one gated `#[cfg(feature = "hydrate")]` containing the real
logic, and one gated `#[cfg(not(feature = "hydrate"))]` that's a harmless
no-op stub — the component body calls the function without needing to know
which target it's running under. Follow this pattern for any new feature
that needs to reach into the browser.

### Signaling: a thin, protocol-driven relay

Two browsers never exchange video through the server. They exchange a
handful of small JSON messages (session descriptions and ICE candidates)
over a WebSocket, just enough for each side to open a direct WebRTC
connection to the other. The message shapes are a pair of Rust enums
(client → server and server → client) shared verbatim between the browser
code and the server code — there's exactly one definition of the protocol,
used by both sides, so it can't drift.

The server side of signaling is intentionally dumb: an in-memory registry
maps a room code to the set of connected peers and relays a message from
one named peer to another. It has no opinion about what an "offer" or an
"ice candidate" *means* — that interpretation lives entirely in the
browser-side code that constructs and reacts to these messages. Keep new
signaling-related logic split the same way: wire format and routing on the
server, meaning and behavior on the client.

### Room lifecycle

There is no host — every member of a room is equal. A room is identified by
a short code and a name, and is either password-protected (hashed with
`argon2`, verified server-side) or explicitly public — the creator picks one
of the two, never an accidental default: on the create-room form, the "sala
pública" checkbox and the password field are mutually exclusive, and leaving
the password blank without checking the box is a validation error, not a
silent public room. Anyone with the code (and password, for a closed room)
can join under whatever nick and color they choose, up to 10 members per
room. Wrong-password attempts are rate-limited per client (keyed by IP, via
Fly's `Fly-Client-IP` header — not the client-supplied `device_id`, which a
client controls and can't be trusted for this) rather than per room, so one
attacker guessing passwords can't lock out everyone else trying to join the
same room. Any member can start or stop sharing their own screen at any
time, independently of what anyone else is doing, but a share alone opens
no connections — it only
flips a flag every member sees on that person's card. A peer-to-peer
connection between a sharer and a viewer only exists while that specific
viewer has chosen to watch that specific sharer; a room with several active
sharers and several people watching them ends up with as many independent
connections as there are (sharer, viewer) pairs currently watching, not one
mesh per sharer. A room is only removed from the registry when its last
member leaves; the person who created it leaving early doesn't affect
anyone else still there. The same per-connection teardown path handles
every way a member's sharing session can end — stopping deliberately, using
the browser's own screen-share controls, or the connection simply
dropping — so there is one place that owns "what happens when this sharer
stops," not several divergent ones.

### Descoberta de salas

Each browser remembers, purely client-side (`localStorage`, never sent to
or stored by the server), the rooms that browser has created or joined —
code and name, deliberately never the password — and lists them as "salas
recentes" on the home page; an entry disappears once its room no longer
exists. Opening a room link checks a plain HTTP endpoint
(`GET /api/rooms/:code`, outside the WebSocket signaling protocol) for
whether that room still exists before showing the nick/password form, so a
dead link fails immediately instead of after the user has already typed in
a nick.

### Client-side building blocks

- A thin WebSocket wrapper handles connecting, sending, and receiving the
  signaling protocol as typed messages instead of raw JSON strings.
- A WebRTC helper module wraps the browser calls needed to capture the
  screen and drive a peer connection through its lifecycle (create, offer,
  answer, exchange ICE candidates, tear down).
- The home page only creates a room; the room page is where everyone
  actually is — there's no separate "share" page or "watch" page, since
  every member can do both. The room page owns its own state (connection
  status, nick/auth, the roster and who's currently sharing, the set of
  active peer connections) and wires the WebSocket and WebRTC helpers
  together to implement the room lifecycle described above.

### Status-driven UI

Each page tracks connection state as one human-readable status sentence
(a single reactive value), and a small pure function classifies that
sentence into a visual state (idle / busy / live / error) used to drive a
status indicator and its color. New states should extend that
classification rather than introducing a second, parallel piece of state to
keep in sync with the status text.

### Configuration

The server's runtime configuration (bind address, output name, asset paths)
is read entirely from environment variables at process start — there's no
config file bundled with the deployed artifact. This is what makes the
same build artifact portable across a local run, a container, and a
hosting platform: only the environment differs.

## Testing approach

Anything that's plain Rust logic without a browser in the loop — the
signaling protocol's (de)serialization, the room registry's behavior, the
WebSocket endpoint's wiring — has automated unit and integration tests and
should keep having them as it grows. The `hydrate` (WASM) code path has
`wasm-bindgen-test` suites that run in headless Chrome (`infra/`,
`session/` helpers; see the `test-web-wasm` job). Full browser flows —
create/join a room, the two-tab share/watch scenario with real WebRTC
media — are covered by Playwright in `apps/web/end2end/` (`e2e-web` job,
headed under `xvfb`). What still isn't automated here: the browser's own
"stop sharing" control, real window/screen capture, audio, and bitrate
adaptation — sanity-check those by hand in a real browser before
considering a change to that layer done (see §"Definition of done" →
"Browser layer").

### Tests are mandatory

- Every new feature ships with tests covering its behavior, at the right
  layer (native Rust, `wasm-bindgen-test`, or Playwright).
- Every bug fix ships with a test that fails before the fix and passes
  after it. This is required unless a test is genuinely impossible to
  write (physical devices, real screen/window capture, system audio, the
  browser's own share controls) — in that case, state why in the change.

### Running tests

- Run `scripts/test-all.sh --no-mutants` for every change. It runs
  `cargo fmt`, `cargo clippy`, `cargo leptos build`, the Rust suite, the
  WASM suite, and the Playwright suites.
- While iterating, narrow to one group with a target — e.g.
  `scripts/test-all.sh e2e-web`, `scripts/test-all.sh lint` (`--help`
  lists them) — but the full `--no-mutants` run is what gates the change.
- Do not run individual `cargo test` / `cargo clippy` / `playwright test`
  commands by hand.
- Mutation tests are not run locally — CI runs them.

## Rust and Leptos coding practices

General practices to keep this codebase clean, testable, and cheap to
extend as it grows past the pilot feature set.

**Before editing any Rust file, read `RUST_GUIDELINES.md` in full.** It
holds the mechanical, checklist-style rules (naming, error handling,
imports, the pre-commit checklist); this section holds the
project-specific judgment calls. Where the two overlap, this section
wins. Read `RUST_GUIDELINES.md` at the start of every task that will
touch `.rs` code — do not rely on remembering it from a previous task.

### Dependency invariants

The codebase is being split into a Cargo workspace (see the
architecture-refactor roadmap in `docs/superpowers/plans/`). These rules
hold now and are enforced by the dependency graph once the crates exist:

```
protocol    →  (serde only)
signaling   →  protocol
apps/web    →  protocol, signaling
```

(A `crates/core` for shared domain types was considered and deferred —
see `docs/decisions/0001-workspace-crate-split.md`. Add it only if
genuinely shared, browser-agnostic domain logic appears.)

- **Dependency direction.** Dependencies point toward lower-level
  abstractions only. `crates/protocol` depends on nothing but `serde` and
  must never depend on Axum, Tokio, `web-sys`, `wasm-bindgen`, Leptos,
  Electron, or any OS API. `crates/signaling` may depend on `protocol`,
  never the reverse.
- **UI components never do I/O.** A Leptos `#[component]` must not open a
  `WebSocket`, construct an `RtcPeerConnection`, call `getDisplayMedia`,
  touch `localStorage`, or reach into `crates/signaling`. It calls a
  method on a `RoomSession` (or a plain helper) and renders signals.
  Networking, signaling, and WebRTC lifecycle live in
  `apps/web/src/infra` and `apps/web/src/session`.
- **Platform code is isolated.** `process.platform` / `#[cfg(target_os)]`
  branching lives only under a `platform/` module that exposes one
  interface; the rest of the code depends on the interface, not the
  branch.

### Feature ownership

Code belongs to the feature that owns it, not to the technology it
happens to use. `features/room/member_card.rs`, not
`components/room_member_card.rs`. `components/` holds only genuinely
generic pieces (button, modal, status indicator, color picker).

### Engineering philosophy

Every abstraction must earn its existence — a trait, a generic, or a new
layer is justified only if it improves testability, isolates the
browser/WebRTC boundary, reduces coupling, or expresses a domain concept
(a room, a member, a share). If it doesn't do one of those, it's making
the code harder to follow for no return.

- Prefer simple code over generic code.
- Prefer explicit code over reusable code until there's a second real call
  site — not a hypothetical one.
- Optimize for correctness first, maintainability second, performance
  third.
- Write for the next person reading this without you around to explain
  it — future maintainability outweighs saving a few lines.

### Code quality

- Single responsibility per function, module, and component — one reason
  for each to change.
- Small functions and small components with one clear purpose; if a
  component's body doesn't fit on screen, split it.
- High cohesion, low coupling. Keep browser-only code behind the
  `hydrate`/`ssr` split described above rather than sprinkling `#[cfg(...)]`
  through shared logic.
- Remove duplication, but never at the cost of readability — a little
  repetition beats the wrong abstraction.
- Explicitness over cleverness; readability over premature optimization.
- Push logic that doesn't need a browser or a live connection (parsing,
  validation, the status-classification function, room-code generation,
  etc.) into plain functions that can be unit tested, rather than burying
  it inside a component or a `web-sys` callback.

### Complexity and control flow

- Prefer early returns over deep nesting.
- Split complex logic into smaller pure functions rather than growing one
  function to cover every case.
- If a function can't be understood without scrolling, it's doing too
  much.

### State and mutability

- Prefer immutable data; reach for `mut` (or a `RwSignal`/`WriteSignal`)
  only when the value genuinely changes.
- Keep mutable state as local as possible — don't thread a signal or a
  `&mut` further than it needs to go.
- Model state transitions explicitly (a matched enum, as with the
  status-driven UI's idle/busy/live/error classification) rather than a
  handful of booleans that can drift out of sync with each other.
- Minimize what's shared across components; prefer passing signals down
  explicitly over reaching for global state.

### Constants

Avoid magic numbers and literals (timeouts, retry counts, the 10-member
room cap, reconnect delays). Give each a named `const` with a short
comment explaining why that value, not just what it is.

### Function design

- Prefer early returns to reduce nesting.
- Prefer borrowing (`&str`, `&[T]`) over taking ownership when the
  function doesn't need to keep the value.
- Limit public function parameters to five or fewer; past that, introduce
  a small config/context struct instead of a long parameter list.
- Avoid boolean flag parameters — prefer an enum that names the behavior,
  or split into two functions.
- Prefer iterator combinators over manual loops when they read more
  clearly; don't force them where a plain loop is clearer.

### Type design

- Prefer domain-specific newtypes over bare primitives where a value has
  meaning beyond its representation (a room code, a nick, a hex color)
  rather than passing raw `String`s everywhere.
- Derive only the traits actually used — an unused `Clone` or `Default` is
  noise and a maintenance liability.
- Keep struct fields private unless external mutation is genuinely
  required; expose behavior through methods.
- Represent impossible states as impossible types where practical — an
  enum variant that can't coexist with a field should replace that field,
  not add a runtime check for it (e.g. a `SharingState` enum instead of an
  `is_sharing: bool` plus a separately-tracked stream handle that may or
  may not be `None` in sync with it).

### API design

Public functions, server functions, and module boundaries should be:

- **predictable** — the same shape of input produces the same shape of
  output, no hidden side effects;
- **minimal** — expose what the caller needs, nothing else;
- **orthogonal** — independent capabilities are independent
  functions/types, not one function with a mode switch.

Design around domain concepts (`Room`, `Member`, `SignalMessage`) rather
than leaking implementation details (internal registry data structures,
raw WebSocket frames) across a module boundary.

### Comments

Comments explain **why**, never **what** the code already says. If code
needs a comment to explain what it does, rewrite it until the intent is
obvious instead of narrating around it. A comment earns its place by
capturing a constraint or a piece of context that isn't visible in the
code itself (e.g. why a particular WebRTC/browser quirk is being worked
around).

### Error handling

- Use concrete error types (an enum implementing `std::error::Error`, or
  Leptos's `ServerFnError`) rather than stringly-typed errors passed
  around as `String`.
- Avoid `.unwrap()`/`.expect()`/`panic!` outside tests and outside cases
  that are genuinely infallible (and say so in a comment when non-obvious
  at the call site); a panic on the server takes the whole process down,
  a panic in `hydrate` code takes down the tab.
- Avoid opaque catch-all error variants — each variant should make it
  possible to tell what failed and what the caller (or the UI) should do
  about it (retry, show a message, redirect home).

### Dependencies and lints

- The full pre-done checklist (exact commands) is in §"Definition of
  done" below. Fix warnings rather than silencing them; if a lint must be
  allowed, do it at the item level with a short reason, not at the crate
  level.

## Definition of done

A task is not done because the code compiles. It is done when every check
below has been run **and passed**, by you, before you report back. Do not
hand work over for the maintainer to discover a failing check.

### Every change

- It does what was actually asked — verified against the request, not
  assumed from "it builds".
- No debug leftovers introduced by the change: no `dbg!`, stray
  `println!`/`console.log`, commented-out code, or `TODO`s.
- Any comment added explains **why** (§Comments); any new timeout, limit,
  retry count, or other magic value has a named `const` with a one-line
  reason (§Constants).
- Docs updated when the change alters architecture, commands, an
  invariant, or a decision: the relevant `docs/architecture/*`, a new
  `docs/decisions/NNNN-*.md` ADR, and this file.

### Rust — `crates/*` and `apps/web` (run from the repo root)

Run `scripts/test-all.sh --no-mutants` — it covers every check below.
The list is what must pass:

- `cargo test --workspace --features ssr` — all green. The test count
  must not silently drop; removing a test is a deliberate, explained
  change.
- `cargo clippy --workspace --all-targets --features ssr -- -D warnings`
  — clean.
- `cargo clippy -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate -- -D warnings`
  — clean (the browser/WASM build compiles different code).
- `cargo fmt --check` — clean.
- `cargo leptos build` — succeeds. This is the web app's real build
  authority; a plain `cargo build` passing is not enough.
- `cargo test -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate --lib`
  — the `hydrate` (WASM) suite, green in headless Chrome (`test-web-wasm`
  runs it in CI).
- **Mutation:** a PR that changes `crates/protocol` or `crates/signaling`
  must not add an uncaught mutant — `cargo mutants --in-diff … -p
  screen-share-protocol -p screen-share-signaling` is a **blocking** CI
  check. `apps/web` mutation (`mutants-web-app`) and the weekly full
  sweeps are report-only. When touching those crates, run cargo-mutants
  locally on the changed area before pushing.
- If the change touches the `Dockerfile`, the deployment, or anything the
  container build depends on: `docker build .` succeeds.

### Browser layer — `apps/web` UI and everything under `apps/web/src/session/`

Automated: `apps/web/end2end/` (Playwright, headed under `xvfb` in CI —
job `e2e-web`). It covers the home create/join flows and the two-tab
room scenario end to end: two members in one room, share, watch, **real
WebRTC media flowing** (asserted via the peer `<video>`'s `readyState` /
`videoWidth`), stop sharing via the in-app button, and a watcher reload
mid-session. Run it locally with `npm --prefix apps/web/end2end test`
(needs a display) or `cargo leptos end-to-end`.

Still hand-verified in a real browser (`cargo leptos watch`) for a
UI-touching change:

- the changed screen or flow renders and behaves; no console errors; no
  hydration mismatch.
- Not yet automated — check by hand when the change touches these: stop
  sharing via the **browser's own** "stop sharing" control (not the
  in-app button); per-viewer watch independence with 3+ members; screen
  capture of a real window; audio; bitrate adaptation under a throttled
  network.

### Desktop — `desktop/` (run with `pnpm --dir desktop …` or from `desktop/`)

- `pnpm run check` — Biome (lint + format + import order) clean.
- `pnpm build` — `tsc` clean. Note that `tsc` does **not** check
  `__dirname`-relative runtime paths or the `#…` import map — those only
  fail at launch.
- `pnpm run test` — Vitest unit suite (`electron` mocked) green.
- `pnpm run test:e2e` — Playwright `_electron` suite: the app boots, the
  audio IPC handlers are registered, `desktop-share:link-ready` copies
  the link, `before-quit` lets the window close. Needs a display
  (`xvfb-run` in CI). Point the shell at a local server with
  `SCREEN_SHARE_URL=…` (the E2E uses `about:blank`).
- Still by hand: the source picker window, real screen/window capture,
  system-audio loopback (PipeWire / WASAPI), and anything Windows-only —
  the `windows-audio` napi tests run only on the Windows CI job. State
  explicitly which platform paths could not be tested.

### Commits, pushes, and branches — maintainer-gated

`git commit`, `git push`, `git merge`, and deleting branches happen
**only with the maintainer's explicit approval for that specific
action**. "The task is done" is not approval to commit. Present the
finished, verified change — say what you ran and what passed — and wait
for the go-ahead. Approval for one commit is not standing approval for
the next.
