# ADR-0004: Electron for the desktop shell; Rust/napi for Windows loopback audio

Date: 2026-08-28
Status: accepted

## Context

**Shell.** The desktop shell was first built in Tauri (Rust, system
WebView). After unblocking the screen-capture permission prompt, screen
sharing rendered a solid black rectangle on Linux. Root cause, confirmed
with `GST_DEBUG` instrumentation: WebKitGTK disables caps re-negotiation
for display-capture sources
(`GStreamerVideoCapturer.cpp:320`), so when PipeWire offers frames as
DMA-BUF and the modifier negotiation fails, there is no shared-memory
fallback and the capture dies before the first frame. Chromium and
Firefox implement that fallback correctly, which is why sharing always
worked in a normal browser. Full write-up:
`docs/superpowers/specs/2026-08-21-tauri-screen-share-black-video-investigation.md`.

**Audio.** System-audio sharing needs per-application loopback capture
with the ability to exclude the app's own output (to avoid an echo
loop). On Windows that means WASAPI process-loopback, which Electron/Node
does not expose.

## Decision

**Shell:** Electron. It bundles a real Chromium instead of depending on
the system WebView, eliminating the black-video root cause at the source.
Accepted costs: a heavier app, and the shell is no longer Rust. The
Tauri `src-tauri/` tree was removed (history kept in git). `desktop/` is
an Electron + TypeScript project managed with `pnpm`, wrapping the
existing web front end — nothing is reimplemented.

**Windows audio:** a standalone Rust napi module,
`desktop/native/windows-audio`, calling WASAPI directly. It keeps its own
`Cargo.lock` and is built by `electron-builder`; it is deliberately
**not** a member of the Rust workspace (Windows-only, and workspace
feature unification with the `wasm`/`ssr` app would be a headache). Linux
uses PipeWire from TypeScript. Both sit behind one `AudioBackend`
interface selected once at startup.

## Consequences

- Screen sharing works in the desktop app on Linux (and Windows).
- Larger install size and memory footprint than a WebView shell.
- Two audio backends to maintain, but `process.platform` branching is
  confined to one backend-selection point.
- The native module needs a Windows toolchain in CI to produce the
  `.node` binary.
