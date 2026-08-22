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

A persistent, password-protected room where a small group can share their
screens with each other — any number of them, at any time, not just one
person presenting to the rest. Someone creates a room (it gets a short code,
a name, and a password), shares the link and password with whoever should
join, and from then on anyone in the room can start or stop sharing their
own screen independently. Sharing and watching are decoupled, Discord-style:
starting a share never pushes video to anyone automatically — it just lights
up a "watch" button on that member's card for everyone else. Watching
someone is an explicit, per-person choice, made and revoked independently by
each viewer, and doesn't affect anyone else watching the same sharer. Each
member picks a nick and a color when they join; a small round avatar (the
nick's first letter over that color) stands in for their card until they're
sharing and someone is watching them. There's no audio yet (still out of
scope). Nobody installs anything — everyone just uses a browser, on Windows
or Linux. Video goes directly from each sharer's browser to each viewer's
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
- Plain CSS (no framework, no external fonts/assets) for styling.
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

Run the automated test suite:

```bash
cargo test --features ssr
```

Run a single test:

```bash
cargo test --features ssr <test_name>
# e.g.
cargo test --features ssr create_room_registers_host_and_returns_code
```

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
UI code. The crate is compiled twice, under two mutually exclusive Cargo
features:

- **`ssr`** — a native binary (`src/main.rs`) that renders pages
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
a short code and a name, and protected by a password (hashed with `argon2`,
verified server-side); anyone with the code and password can join under
whatever nick and color they choose, up to 10 members per room. Any member
can start or stop sharing their own screen at any time, independently of
what anyone else is doing, but a share alone opens no connections — it only
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
should keep having them as it grows. Anything that only exists inside a
real browser (screen capture, the actual media flowing over a WebRTC
connection, clipboard access) is exercised by hand in a real browser
instead; there is no browser automation harness in this repo for that
layer, so changes touching `client/` or the pages should be sanity-checked
in an actual browser before being considered done.

## Rust and Leptos coding practices

General practices to keep this codebase clean, testable, and cheap to
extend as it grows past the pilot feature set.

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

- Run `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
  before considering a change done; fix warnings rather than silencing
  them, and if a lint must be allowed, do it at the item level with a
  short reason, not at the crate level.
