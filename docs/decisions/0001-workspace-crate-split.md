# ADR-0001: Cargo workspace with core / protocol / signaling / apps-web seams

Date: 2026-08-28
Status: accepted

## Context

The project grew as a single crate `screen_share` compiled twice (`ssr`
binary, `hydrate` WASM lib). By ~7.5k lines it had started to braid three
concerns: the screenshare domain and wire protocol, the Leptos + WebRTC
web app, and the Electron desktop shell. Concrete symptoms:

- `signaling/protocol.rs` pulls the whole `signaling/` module tree into
  the `wasm32` build graph, though the browser only needs the wire enums.
- WebRTC calls (`new_peer_connection`, `create_offer`, `create_answer`)
  sit inline in a `ServerMessage` match arm in a Leptos component.
- Test modules inflate their source files 2x.
- No stated dependency-direction rule, so an agent will eventually wire a
  component straight to the registry "because it works".

An external review proposed a full DDD layering
(`domain`/`application`/`infrastructure`) plus `apps/` + `crates/`. That
was judged directionally right but roughly twice the structure this size
of project warrants.

## Decision

Adopt a Cargo workspace with four Rust seams and one desktop seam:

```
core        →  serde only            — pure domain types
protocol    →  core                  — ClientMessage / ServerMessage / info
signaling   →  core, protocol, axum… — in-memory registry + WS relay
apps/web    →  core, protocol, signaling, leptos, web-sys…
desktop     →  (talks to apps/web over the wire only)
```

Dependencies point downward only, enforced by the dependency graph and
stated in `CLAUDE.md` §"Dependency invariants". Do **not** add
`application`/`infrastructure` layers inside each crate; do **not** make
the Electron desktop a Rust workspace member.

`core` is allowed to end up small (just id/nick/color newtypes). If it
would hold nothing but a handful of newtypes, fold them into `protocol`
and drop `core` — decided during Phase 3, with a serde round-trip test as
the arbiter for any wire-representation change.

The web package keeps the name `screen_share` even though its directory
becomes `apps/web`, and `LEPTOS_OUTPUT_NAME` stays `screen_share`, so the
Dockerfile and every `use screen_share::…` path are untouched by the
move.

## Consequences

- The `wasm32` build stops compiling Axum/Tokio via a protocol path.
- `core` and `protocol` become unit-testable with no async or browser in
  the loop.
- Six mechanical, independently-green phases instead of one big-bang
  (see the roadmap in `docs/superpowers/plans/`).
- `cargo-leptos` is now driven from the repo root via
  `[[workspace.metadata.leptos]]`; contributors must run it from there.
- Slightly more `Cargo.toml` bookkeeping and longer `cargo test`
  invocations (`-p <crate>`).
