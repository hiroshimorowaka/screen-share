---
name: verify
description: Runs this repo's build/lint/test/format/deny verification suite, scoped to what actually changed, and reports one compact pass/fail summary instead of pasting raw cargo output into the conversation. Use after implementing or editing anything in a crates/* crate, before reporting a task as done or asking the maintainer to bench-test.
---

# Verify

Goal: give a trustworthy "is this green" answer while keeping raw `cargo`
output out of the conversation. Never paste full command output unless a
step actually failed — for a pass, one line per step is enough.

## 1. Scope — figure out what changed

Run `git status --short` (and `git diff --stat` if it's ambiguous). Map
touched paths to crates:

- `crates/tracker-core/**` → `tracker-core`
- `crates/tracker-device/**` → `tracker-device`
- `crates/tracker-modbus/**` → `tracker-modbus`
- `crates/tracker-hal/**`, `crates/tracker-drivers/**`, `crates/tracker-sim/**` → same pattern
- `crates/tracker-fw/**` → `tracker-fw` (needs the two-feature check below, not a plain `cargo test`)

If nothing is a clean subset (e.g. several unrelated crates touched, or
you're not sure), just run everything — `cargo test --workspace --exclude
tracker-fw` plus both `tracker-fw` feature checks. When in doubt, wider is
cheaper than a false "pass".

## 2. Commands, in this order

Run every command with a **short, descriptive Bash `description`**, and
capture output with something like `2>&1 | tail -40` — enough to see the
`test result:` / `warning:` / `error:` lines, not the whole build log.

1. `cargo fmt --all --check` — whole workspace, always, first (cheapest).
2. For each touched host crate: `cargo test -p <crate>`.
3. For each touched host crate: `cargo clippy -p <crate> --all-targets -- -D warnings`.
4. If `tracker-fw` touched, **from inside `crates/tracker-fw`** (not the
   repo root — the xtensa target config lives there):
   - `cargo clippy --features board-mock -- -D warnings`
   - `cargo +esp clippy --features board-esp32s3 --target xtensa-esp32s3-none-elf -- -D warnings`
   - Note: `tracker-fw` is `#![no_std]` unconditionally and has no host
     test harness — don't try `cargo test -p tracker-fw` or add
     `--all-targets` to its clippy calls, both fail for reasons unrelated
     to your change (see `crates/tracker-fw/tests/` doc comments if this
     needs re-deriving).
5. `cargo deny check` — whole workspace, once.
6. **Only if `tracker-core/src/state.rs` or `tracker-core/src/safety.rs`
   changed**: `cargo mutants -p tracker-core --timeout 60`. This can take
   several minutes — run it with `run_in_background: true` on the Bash
   call and continue with other work instead of blocking on it; report
   the result once the completion notification arrives. Required by
   `CLAUDE.md`'s Definition of Done whenever those two files change.

## 3. Known pre-existing noise — do not re-diagnose these

`cargo deny check` currently prints, on a clean tree with no relevant
change:
- Two `license-not-encountered` warnings (`MPL-2.0`, `CC0-1.0` in
  `deny.toml`'s allow-list with nothing currently using them)
- One `yanked` warning for `proc-macro-error3` (pulled in transitively by
  `embedded-test-macros`, a dev-dependency)

These are pre-existing and unrelated to any change in this project so
far. Treat them as "known, not a regression" — don't spend a turn
investigating them again unless the exact warning text changes or a real
`error` (not `warning`) appears.

## 4. Report format

One line per step, ✅/❌, plus a one-line total. Example:

```
✅ fmt --check
✅ tracker-device: 47 tests, clippy clean
✅ tracker-modbus: 100 tests (1 ignored), clippy clean
✅ tracker-fw board-mock: clippy clean
✅ tracker-fw board-esp32s3 (xtensa): clippy clean
✅ cargo deny: clean (2 known license-not-encountered + 1 known yanked warning, pre-existing)
⏭ mutants: skipped (state.rs/safety.rs not touched)
```

If something fails: show only the relevant error text (the actual
`error[...]`/panic/assertion block), not the whole build log, then stop
and fix it before re-running this skill from step 1.
