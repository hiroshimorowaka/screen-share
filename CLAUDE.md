# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

A way to share your screen with several people at once, quickly and simply:
open the site, click a button, send a link. Nobody installs anything — the
person sharing and everyone watching just use a browser, on Windows or
Linux. The video goes directly from the sharer's browser to each viewer's
browser (WebRTC, peer-to-peer); the server's only job is introducing peers
to each other so that direct connection can be established.

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

One peer starts a room and becomes its host; that action produces a short
room code embedded in a shareable link. Anyone opening that link joins the
same room as a viewer. The host is the one who fans out a direct
peer-to-peer connection to each viewer that joins — from the host's
perspective this is one connection per viewer; viewers don't connect to
each other. When the host's connection to the signaling server ends, the
room and everyone's peer connections are torn down and viewers are told the
session ended. The same teardown path handles every way a sharing session
can end — the person deliberately stopping, the browser's own screen-share
controls being used to stop, or the connection simply dropping — so there
is one place that owns "what happens when sharing stops," not several
divergent ones.

### Client-side building blocks

- A thin WebSocket wrapper handles connecting, sending, and receiving the
  signaling protocol as typed messages instead of raw JSON strings.
- A WebRTC helper module wraps the browser calls needed to capture the
  screen and drive a peer connection through its lifecycle (create, offer,
  answer, exchange ICE candidates, tear down).
- Each page (the "share" page and the "watch" page) owns its own state —
  connection status, the set of active peer connections, whatever's needed
  for its side of the exchange — and wires the WebSocket and WebRTC helpers
  together to implement its half of the room lifecycle described above.

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
