# ADR-0005: Quality gate — layered automated tests, mutation testing, CI enforcement

Date: 2026-08-28
Status: accepted

## Context

The test story is uneven and the CI does not gate anything:

- `crates/protocol` and `crates/signaling` have real unit/integration
  tests (wire (de)serialization, registry behaviour, auth + per-IP
  rate-limit, a live WebSocket round-trip, rooms-status HTTP, TURN
  credentials). Good, but nothing measures whether those tests would
  actually catch a regression.
- `apps/web` has five `*_tests.rs` files, all pure-logic and all compiled
  **only under `ssr`** (native). Not one line of the `hydrate` (WASM)
  code path is executed by any test: `infra/socket`, `infra/webrtc`,
  `infra/storage`, `session/*`, and every Leptos component are untested.
- The browser flows — creating/joining a room, screen share, the
  decoupled per-viewer watch, clipboard, `getDisplayMedia`, the P2P
  two-tab scenario — are verified **100% by hand** against the checklist
  in `CLAUDE.md` §"Definition of done". Slow, tedious, easy to skip.
- `desktop/` (Electron/TypeScript) has **no tests at all** — only
  `biome check` and `tsc`. The main process, IPC surface, the
  platform Linux/Windows branches, the source picker and quick-share are
  entirely uncovered.
- `.github/workflows/ci-cd.yml` triggers on **`push` to `main`** and
  `workflow_dispatch` only. There is no `pull_request` trigger, so no
  check blocks a merge. `cargo leptos build` — the web app's real build
  authority — is not run in CI either.
- No mutation testing, no coverage measurement.

The maintainer wants a quality gate: automated tests across back-end,
front-end and the Electron app; mutation tests to prove the tests bite;
and as much of the front-end / Electron manual testing automated as
possible.

## Decision

Adopt a layered testing pyramid with one tool per layer, and make CI the
gate. Roll it out in phases (detailed, task-by-task, in
`docs/superpowers/plans/2026-08-28-quality-gate.md`), ordered so the
cheap high-value checks land first. The maintainer opted to land all six
phases on a single branch rather than one PR per phase.

### Tooling per layer

| Layer | Tool | Notes |
|---|---|---|
| Rust native logic (`crates/*`, `apps/web` under `ssr`) | `cargo test` | keep; expand edge cases (sharer teardown paths, member cap, last-to-leave, code collision, reconnect) |
| Rust WASM (`apps/web` under `hydrate`) | `wasm-bindgen-test`, headless **Chrome** | first automated coverage of `infra/`, `session/`, and component DOM interaction (`mount_to`) |
| Leptos component render | `leptos::ssr::render_to_string` snapshots | runs in the existing native `cargo test`; cheapest component layer |
| E2E web | Playwright, **headful Chromium under `xvfb`** in CI | fake media (`--use-fake-device-for-media-stream`, `--auto-select-desktop-capture-source`); two browser contexts drive the P2P two-tab checklist; assertions via `RTCPeerConnection.getStats()` and `<video>` state |
| Desktop main process | Vitest, `electron` module mocked | `platform/*`, `run-command`, `process-identity`, `pipewire`/`loopback` parsing, `features/*` selection logic, `lifecycle`, `tray`, `preload` bridge shape |
| Desktop E2E | Playwright `_electron.launch`, headful under `xvfb` | window boot, tray, quick-share → `desktopShare.linkReady`, member-joined notification, `before-quit` loopback stop; native dialogs replaced via `electronApp.evaluate` |
| `desktop/native/windows-audio` (napi) | `cargo test` in the existing Windows CI job | pure logic in `process_identity.rs`, parsing in `capture.rs` |
| Mutation — Rust | `cargo-mutants` | `--in-diff` on PRs, full run scheduled + sharded |
| Mutation — desktop | StrykerJS | **deferred** until the Vitest base exists |
| Coverage | `cargo-llvm-cov` (native) + `vitest --coverage` (v8) → Codecov | report/PR-comment only, not a hard gate initially |

### Maintainer choices baked in

- **`wasm-bindgen-test` browser: Chrome** (headless; a real display is
  not needed for this layer).
- **E2E runs headful under `xvfb`** in CI. The ~1–2 min overhead is
  accepted rather than betting on `--headless=new` supporting
  `getDisplayMedia`.
- **Mutation gate starts report-only.** `cargo mutants --in-diff` runs on
  every PR with `continue-on-error`, posting survivors to the job
  summary. It becomes **blocking per crate** once that crate's scheduled
  full run reaches zero survivors — `crates/protocol` and
  `crates/signaling` first, `apps/web` later.
- **StrykerJS is added only after** the desktop Vitest suite has real
  mass.
- **Codecov is used only while it is free** for this repo (public-repo
  free tier). If the repo goes private and exceeds the free plan, fall
  back to an lcov artifact uploaded by CI.

### CI becomes the gate

- Add a `pull_request` trigger to the pipeline. Keep `deploy-web` and
  `publish-desktop-release` gated to `push` on `main` (`if:
  github.event_name != 'pull_request'`).
- New jobs: `build-web` (adds `cargo fmt --check` + `cargo leptos
  build`), `test-web-wasm`, `e2e-web`, `mutants-web` (PR, `--in-diff`),
  `test-desktop` (Vitest), `e2e-desktop`; a scheduled `mutants-full` +
  coverage job.
- Mark the fast deterministic jobs as **required status checks** on
  `main` via branch protection (a repo setting, applied with `gh api`,
  not a committed file).

## Consequences

- The browser and Electron layers get their first real safety net. The
  manual two-tab checklist becomes an automated `e2e-web` job; it stays
  in `CLAUDE.md` only as the fallback for what genuinely cannot be
  automated here: real system-audio capture (PipeWire / WASAPI, no
  deterministic audio hardware in CI), WebRTC bitrate adaptation under
  degraded networks, and OS-native window/screen pickers outside Electron.
- CI wall-clock grows: a WASM job, an E2E job under `xvfb`, and a
  mutation job on PRs (bounded by `--in-diff`). Mitigated with the
  existing `paths-filter`, `Swatinem/rust-cache`, a Playwright-browser
  cache, and `cargo-mutants` sharding on the scheduled full run.
- New toolchain and dev-dependencies: `wasm-bindgen-test` (the `0.3.x`
  line — `=0.3.77`, released in lockstep with the `wasm-bindgen` crate in
  `Cargo.lock`, currently `0.2.127`) plus the matching
  `wasm-bindgen-test-runner` binary; `cargo-mutants`, `cargo-llvm-cov`,
  Playwright + browsers, Vitest. Node/pnpm is now needed to test
  `apps/web` (E2E), not just `desktop/`.
- Newly-required checks can block merges on **pre-existing** gaps, not
  just regressions — the reason the mutation gate and any coverage
  threshold start report-only and tighten per crate.
- A soft dependency on Codecov remaining free at this repo's visibility;
  the artifact-only fallback is documented in the plan.
- `.cargo/config.toml` gains a `runner` key under
  `[target.wasm32-unknown-unknown]`. This affects `cargo test` for the
  wasm target only; `cargo leptos build` (which runs `cargo build`) is
  unaffected. Verified as a step in the plan.
- Docs to update as phases land: `CLAUDE.md` §"Definition of done" and
  §"Testing approach", `RUST_GUIDELINES.md` §"Testing", and the
  architecture docs if the test layout moves.
- Interaction with the architecture-refactor roadmap
  (`docs/superpowers/plans/2026-08-28-architecture-refactor-roadmap.md`):
  both touch `apps/web` test layout and CI. Quality-gate Phases 0–1 are
  independent and can land first; Phase 2 (WASM tests) is coordinated
  with refactor Phase 4 (tests move into `tests/` directories) so the two
  do not fight over the same files.
