# Rust Code Quality Guidelines

Concrete Rust rules for this repository. This is the mechanical companion
to `CLAUDE.md` §"Rust and Leptos coding practices" — where the two overlap,
`CLAUDE.md` wins (it is tuned to this project); where this file is more
specific, follow it. Linked from `CLAUDE.md`.

Scope note: this project is a Leptos/WASM + Axum app. It has no CLI, no
TUI, no Python bindings, and no dataframe work — guidance for those is out
of scope and deliberately absent.

## Core principle

Optimize for correctness first, maintainability second, performance third
(`CLAUDE.md`). Do not reach for SIMD, `rayon`, or micro-optimizations
without a measured reason. "Fully optimized" here means: no dead code, no
needless allocation on hot paths, and the simplest design that is correct.

## Code style and formatting

- Use meaningful, descriptive names. `snake_case` for
  functions/variables/modules, `PascalCase` for types/traits,
  `SCREAMING_SNAKE_CASE` for constants.
- Follow the Rust API Guidelines and idiomatic Rust.
- 4-space indentation, never tabs. Line length 100 (rustfmt default).
- No emoji or emoji-like unicode (✓, ✗) in code or output. Exception:
  tests deliberately exercising multibyte input.
- Comments explain **why**, not **what** (`CLAUDE.md` §"Comments"). If code
  needs a comment to explain what it does, rewrite it.

## Documentation

- Doc comments on all public functions, structs, enums, and methods.
- Document parameters, return values, and error conditions (`# Errors`).
- Add `# Examples` for non-obvious public APIs.
- Keep doc comments in sync with the code.

## Type system

- Lean on the type system to make illegal states unrepresentable
  (`CLAUDE.md` §"Type design" — e.g. a `SharingState` enum, not
  `is_sharing: bool` plus a separate stream handle).
- Newtypes for values with meaning beyond their representation (room code,
  nick, hex color) rather than bare `String`.
- Prefer `Option<T>` over sentinel values.
- Derive only the traits actually used — an unused `Clone`/`Default` is a
  liability.
- Private fields by default; expose behavior through methods.

## Error handling

- Concrete error types: an `enum` implementing `std::error::Error` (via
  `thiserror` is fine), or Leptos's `ServerFnError`. Not stringly-typed
  errors.
- Each error variant must tell the caller what failed and what to do
  (retry / show message / redirect). No opaque catch-all variants.
- `anyhow` is discouraged in library-style crates (`crates/*`) — it erases
  the variant information the UI needs. It is acceptable only at the
  binary's top level (`main.rs`).
- Propagate with `?`. Add context at the boundary where it becomes
  actionable, not everywhere.
- No `.unwrap()` / `.expect()` / `panic!` outside tests and genuinely
  infallible cases — a panic on the server kills the process, a panic in
  `hydrate` kills the tab. When a call is truly infallible, say why in a
  comment.

## Function design

- One responsibility per function. If it can't be understood without
  scrolling, split it.
- Prefer borrowing (`&str`, `&[T]`) over taking ownership when the
  function doesn't keep the value.
- Five parameters max; past that, take a config/context struct.
- No boolean flag parameters — use an enum that names the behavior, or two
  functions.
- Early returns over deep nesting.
- Iterator combinators when they read more clearly; a plain loop when it
  doesn't.

## Struct and enum design

- One responsibility per type.
- Derive `Debug` where it aids diagnostics; derive `Clone`/`PartialEq`
  only where used.
- `#[derive(Default)]` only when a sensible default genuinely exists.
- Builder pattern only for construction that is genuinely complex.

## Constants

No magic numbers or literals (timeouts, retry counts, the 10-member cap,
reconnect delays). Each gets a named `const` with a short comment
explaining **why that value**.

## Testing

- Unit-test plain logic (parsing, validation, the status classifier,
  room-code generation, the signaling protocol's (de)serialization, the
  registry's behavior).
- Browser-only behavior (screen capture, live WebRTC media, clipboard) is
  exercised by hand in a real browser — there is no automation harness for
  it in this repo.
- Arrange-Act-Assert. No commented-out tests.
- After Phase 4 of the architecture refactor, tests live in each crate's
  `tests/` directory, not in `#[cfg(test)]` modules inside source files
  (see the refactor roadmap).

## Imports and dependencies

- No wildcard imports except preludes and `use super::*` in test modules.
- Order: std, external crates, local modules.
- Pin dependency versions in `Cargo.toml`.
- `rustfmt` owns import formatting.

## Rust best practices

- No `unsafe` unless genuinely necessary; document the safety invariants
  at the call site when used.
- Call `.clone()` explicitly; avoid hidden clones in closures/iterators.
- Match exhaustively; avoid catch-all `_` when the variants are known and
  finite.
- `format!` for string building; `enumerate()` over manual counters;
  `if let` / `while let` for single-pattern matches.

## Memory and performance

- Prefer `&str` over `String`, `&[T]` over `Vec<T>` in signatures.
- `Cow<'_, str>` when ownership is only conditionally needed.
- `Vec::with_capacity` when the size is known.
- `Arc` / `Rc` judiciously; prefer borrowing.

## Concurrency

- `tokio` is the async runtime (server side only).
- Correct `Send` / `Sync` bounds.
- Prefer `RwLock` or lock-free structures over `Mutex` when reads
  dominate; channels (`mpsc`) for message passing — the signaling relay
  already does this.

## Security

- Never store secrets, API keys, or passwords in code. Read them from the
  environment (`std::env`) — this project reads all runtime config from
  env vars at process start, so no `.env` loader is bundled in the
  deployed artifact.
- Never log passwords, tokens, or PII.

## Version control

- Clear, descriptive commit messages in English (`CLAUDE.md` §1).
- Never commit commented-out code, `dbg!`, stray `println!`, or
  credentials.

## Tools

- `rustfmt` for formatting; `clippy` for linting.
- Code must compile with no warnings; CI uses `-D warnings` (do not put
  `#![deny(warnings)]` in source). If a lint must be allowed, do it at the
  item level with a one-line reason.
- Build and run the web app only via `cargo leptos build` /
  `cargo leptos watch` — never the bare binary (`CLAUDE.md` §"Commands").

## Before committing

- [ ] Tests pass — `cargo test -p screen_share --features ssr` (or the
      relevant `-p <crate>` after the workspace split).
- [ ] No compiler warnings.
- [ ] `cargo clippy --all-targets -- -D warnings` clean.
- [ ] `cargo fmt --check` clean.
- [ ] If Rust that feeds the WASM bundle changed, rebuild via
      `cargo leptos build`.
- [ ] Public items have doc comments.
- [ ] No commented-out code, debug statements, or hardcoded credentials.

---

**Remember:** clarity and maintainability over cleverness.
