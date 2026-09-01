# Quality Gate Roadmap

> **For agentic workers:** this is a phased roadmap. Execute phases in
> order; do not start phase N+1 until phase N's acceptance gate is green
> and the maintainer has approved the commit(s). Each phase is one branch
> and one PR. Steps use checkbox (`- [ ]`) syntax for tracking. Read
> `CLAUDE.md` and `RUST_GUIDELINES.md` in full before touching code; the
> language policy (English everywhere in code/CI/docs, pt-BR only in
> conversation) and the maintainer-gated commit rule apply here without
> exception.

**Goal:** A quality gate that lets the project keep adding features and
changing internals without silently breaking what worked — automated
tests across the Rust back-end, the Leptos/WASM front-end and the
Electron desktop app, mutation testing to prove the tests bite, and CI
enforcement on every pull request. Replace as much of the manual
front-end / Electron testing as is feasible.

**Design rationale and the decisions behind it:**
`docs/decisions/0005-quality-gate.md` (ADR-0005). Read it first.

## Open: confirm on a real PR / CI run

Everything below was verified locally. These items can only be confirmed
once the work is pushed and a PR opens — check them off on the first PR
that carries these workflow changes:

Confirmed on PR #16's CI (run 2, commit `55813f5`):

- [x] **Phase 0** — jobs run on the PR; `deploy-web` /
      `publish-desktop-release` are `skipping`.
- [x] **Phase 2** — `test-web-wasm` green (`browser-actions/setup-chrome`
      + generated `webdriver.json` wiring works on the runner).
- [x] **Phase 4** — `test-desktop` green; `cargo test` in
      `desktop/native/windows-audio` green on the Windows job.
- [x] **Phase 5** — `e2e-desktop` green under `xvfb`.
- [x] **Phase 6** — `mutants-web` (blocking) green on the PR diff.

Still open:

- [ ] **Phase 0** — a `desktop/**`-only PR starts no web jobs (and vice
      versa); a deliberate `cargo fmt` violation fails `build-web`.
- [ ] **Phase 0** — a **push to `main`** (a merge) runs only
      `build-web`/`test-web` → `deploy-web` and
      `test-desktop`/`build-desktop-*` → `publish-desktop-release`; the
      slow layers (`test-web-wasm`, `e2e-web`, `e2e-desktop`) and the
      mutation jobs are **skipped** on push.
- [ ] **Phase 0** — branch protection lists the required checks **with
      `strict=true`** (maintainer runs the `gh api` call in Task 0.3).
- [ ] **Phase 1** — `quality-scheduled.yml` `mutants-full` +
      `mutants-full-app` + `stryker-desktop` green on a manual
      `workflow_dispatch`.
- [ ] **Phase 2** — `cargo leptos watch` browser smoke unchanged (no
      product code changed; low risk).
- [ ] **Phase 3** — `e2e-web` green under `xvfb` on the runner (fixed:
      `e2e-web` now installs the `wasm32-unknown-unknown` target that
      `cargo leptos serve` needs).
- [ ] **Phase 6** — `mutants-web` (now blocking) passes on this PR;
      `mutants-web-app` / the incremental Stryker step run report-only;
      `mutants-full`, `mutants-full-app`, `stryker-desktop` run on a
      manual `workflow_dispatch`; a Codecov comment appears (or the
      `coverage-rust` / `coverage-desktop` artifacts are present) and does
      not fail the PR. Add `mutants-web` to branch protection.
- [ ] **Phase 6** — if `mutants-full` is red on the first scheduled run,
      restore `continue-on-error: true` on `mutants-web` until it is
      clean two runs running (the comment in the job says so).

## Guiding decisions (fixed — do not relitigate)

- `wasm-bindgen-test` browser runner: **Chrome**, headless.
- Web + desktop E2E: **Playwright, headful, under `xvfb`** in CI.
- Mutation gate: **report-only first**; becomes blocking per crate once
  that crate's scheduled full `cargo-mutants` run is at zero survivors.
  Order: `crates/protocol`, then `crates/signaling`, then `apps/web`.
- StrykerJS (desktop mutation): **only after** Phase 4's Vitest suite
  exists.
- Coverage: **Codecov**, used only while free for this repo; lcov
  artifact is the fallback. Report-only, never a hard gate in this
  roadmap.

## Global constraints

- **No product behaviour change.** This roadmap adds tests, config and CI
  only. Source-tree changes are limited to testability seams with the
  same default behaviour — extracting pure logic into a testable function,
  or an env override that is unset in every shipped build (e.g.
  `SCREEN_SHARE_URL` in Phase 5) — each covered by a new test and noted
  in the PR.
- **`cargo-leptos` is the build authority for `apps/web`.** Never
  validate the web app by running the bare binary.
- **Lint gate stays green** at every phase:
  `cargo clippy --workspace --all-targets --features ssr -- -D warnings`,
  `cargo clippy -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate -- -D warnings`,
  `cargo fmt --check`, and `pnpm --dir desktop run check`.
- **Test count must not silently drop.** Removing a test is deliberate
  and explained in the PR.
- **Commits are maintainer-gated.** Present the finished, verified phase;
  wait for explicit approval before `git commit` / `git push`.
- **Every new magic value gets a named `const`/constant with a one-line
  reason** (timeouts, thresholds, shard counts) — `CLAUDE.md` §Constants
  applies to test and CI code too, within reason.
- **Interaction with the architecture refactor.** If refactor Phase 4
  (tests move into `tests/` dirs) has not landed when Phase 2 here
  starts, put new `apps/web` WASM tests in `apps/web/tests/` from the
  start so the two efforts do not collide.

---

## File / artefact map when the roadmap is complete

```
.github/
  workflows/
    ci-cd.yml                    # MODIFIED: pull_request trigger; deploy jobs gated to push@main;
                                 #           new jobs build-web, test-web-wasm, e2e-web,
                                 #           mutants-web(-app), test-desktop, e2e-desktop
    quality-scheduled.yml        # NEW: weekly sharded cargo-mutants + StrykerJS
  actions/
    setup-rust/action.yml        # NEW: composite — toolchain + cache + optional cargo tool
    setup-desktop/action.yml     # NEW: composite — pnpm + Node + `pnpm install`
.cargo/config.toml               # MODIFIED: runner for [target.wasm32-unknown-unknown]
.cargo/mutants.toml              # NEW: cargo-mutants config
codecov.yml                      # NEW: informational (no failure), flags per layer
scripts/test-all.sh              # NEW: test runner — `[target] [flags]`, default target `all`,
                                 #      per-group targets (e2e, e2e-web, lint, rust, …); pass/fail/skip summary
scripts/test-wasm.sh             # NEW: wasm-bindgen browser suite (auto-fetches chromedriver)

apps/web/
  Cargo.toml                     # MODIFIED: [dev-dependencies] wasm-bindgen-test
  src/**/*_wasm_tests.rs         # NEW: #[cfg(all(test, target_arch="wasm32", feature="hydrate"))] suites
  tests/
    ssr_render.rs                # NEW: RenderHtml::to_html component snapshots (native)
  end2end/                       # NEW: Playwright project for the web app
    package.json
    playwright.config.ts
    tests/*.spec.ts

desktop/
  package.json                   # MODIFIED: vitest, @playwright/test, stryker, test scripts
  vitest.config.mts              # NEW (.mts — the package is CJS)
  stryker.config.mjs             # NEW: StrykerJS (report-only, inPlace, vitest runner)
  tsconfig.json                  # MODIFIED: exclude src/**/*.test.ts from tsc emit
  biome.json                     # MODIFIED: also lint e2e/ + config files
  src/**/*.test.ts               # NEW: Vitest unit tests
  e2e/                           # NEW: Playwright _electron suites
    playwright.config.ts
    *.spec.ts
  native/windows-audio/
    Cargo.toml                   # MODIFIED: crate-type += "rlib" (for cargo test)
    src/capture.rs               # MODIFIED: #[cfg(test)] mod for should_include

CLAUDE.md, RUST_GUIDELINES.md     # MODIFIED: as phases land (Definition of done, Testing approach)
```

---

## Phase 0 — CI gates pull requests

**Branch:** `quality/phase-0-ci-gate`
**Goal:** Every PR runs the checks that already exist plus
`cargo leptos build`, and cannot merge if they fail. Zero new tests.

### Task 0.1 — Trigger the pipeline on PRs, keep deploys on `main`

- [ ] Add to `.github/workflows/ci-cd.yml` `on:`:

  ```yaml
  on:
    push:
      branches: [main]
    pull_request:
      branches: [main]
    workflow_dispatch: {}
  ```

- [ ] In the `changes` job, the `dorny/paths-filter` step is currently
      `if: github.event_name == 'push'`. Change it to
      `if: github.event_name != 'workflow_dispatch'` so PRs also get
      path-filtered (a PR touching only `desktop/**` must not run the web
      jobs, matching today's push behaviour).
- [ ] Gate the deploy/publish jobs so they never run on a PR. On
      `deploy-web` and `publish-desktop-release`, extend the `if:`:

  ```yaml
  if: needs.changes.outputs.web == 'true' && github.event_name != 'pull_request'
  ```

  (and the desktop equivalent). `build-desktop-*` still run on PRs — only
  the GitHub release publish is suppressed.

### Task 0.2 — Add `fmt` + `cargo leptos build` to the web job

- [ ] Rename `test-web` responsibilities: split into `build-web` (fast,
      required) and keep `test-web`. In `build-web`:

  ```yaml
  build-web:
    name: Build + lint web
    needs: changes
    if: needs.changes.outputs.web == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2       # binary download of cargo-leptos
        with:
          tool: cargo-leptos
      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-targets --features ssr -- -D warnings
      - run: cargo clippy -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate -- -D warnings
      - run: cargo leptos build
  ```

  `dtolnay/rust-toolchain@stable` already ships `rustfmt` + `clippy`, so
  no `components:` line is needed (matches the existing jobs). `cargo
  leptos` is unpinned to track the Dockerfile's `cargo install
  cargo-leptos --locked`; pin `tool: cargo-leptos@<ver>` if drift bites.
- [ ] `test-web` keeps only `cargo test --workspace --features ssr`.

### Task 0.3 — Branch protection (manual, record the command)

- [ ] Not a committed file. After the workflow PR merges, apply
      (`strict=true` = "require branches up to date before merging", which
      is what lets `push`-to-`main` skip the slow layers safely):

  ```bash
  gh api -X PUT repos/hiroshimorowaka/screen-share/branches/main/protection \
    -f 'required_status_checks[strict]=true' \
    -F 'required_status_checks[contexts][]=Build + lint web' \
    -F 'required_status_checks[contexts][]=Test web server' \
    -F 'required_status_checks[contexts][]=Test web (WASM, headless Chrome)' \
    -F 'required_status_checks[contexts][]=E2E web (Playwright, xvfb)' \
    -F 'required_status_checks[contexts][]=Mutation test — protocol + signaling (changed lines)' \
    -F 'required_status_checks[contexts][]=Test desktop (Vitest)' \
    -F 'required_status_checks[contexts][]=E2E desktop (Playwright _electron, xvfb)' \
    -F 'required_status_checks[contexts][]=Build desktop app (Linux)' \
    -F 'required_status_checks[contexts][]=Build desktop app (Windows)' \
    -f 'enforce_admins=false' \
    -f 'required_pull_request_reviews=null' \
    -f 'restrictions=null'
  ```

  A path-filtered job that is *skipped* on a PR (e.g. the web checks on a
  `desktop/**`-only PR) reports a `skipped` conclusion, which current
  GitHub branch protection treats as passing — so the cross-domain
  required checks above do not block a single-domain PR. Record in the PR
  description that this was run, and by whom.

### Acceptance gate — Phase 0

- [x] `on:` has `pull_request: [main]`; deploy/publish jobs carry
      `github.event_name != 'pull_request'`; `paths-filter` runs for
      `pull_request` too. (Workflow YAML validated locally.)
- [x] `build-web` runs `cargo fmt --check`, both `clippy` invocations,
      and `cargo leptos build` — all pass locally on this branch.
- [ ] Needs a real PR to confirm in CI: checks appear under **Checks**;
      a deliberate fmt violation fails `build-web`; desktop-only vs
      web-only PRs start only their side's jobs; deploy jobs absent on PR.
- [ ] Branch protection lists the required checks (maintainer runs the
      `gh api` call above after the workflow PR merges).

---

## Phase 1 — Mutation testing for `crates/protocol` and `crates/signaling` (report-only)

**Branch:** `quality/phase-1-mutants`
**Goal:** `cargo-mutants` runs on PRs over the changed lines and on a
weekly full sweep, surfacing surviving mutants without blocking. Reinforce
tests until the two crates' full sweeps reach zero survivors.

### Task 1.1 — `.cargo/mutants.toml`

cargo-mutants reads config from `.cargo/mutants.toml` (source-tree root,
next to the existing `.cargo/config.toml`) — **not** a repo-root
`mutants.toml`. `deny_unknown_fields` is on, so a typo'd key fails the
run. Do **not** set `features` / `additional_cargo_args` here: this phase
scopes to `screen-share-protocol` and `screen-share-signaling` with `-p`
on the command line, those crates have no Cargo features, and cargo-mutants
runs `cargo test -p <mutated-crate>` per mutant. `apps/web` (feature
`ssr`) is added in Phase 6 as a **separate** invocation that passes
`--features ssr` on the command line.

- [ ] Create `.cargo/mutants.toml`:

  ```toml
  # exclude_globs: inert until apps/web is in scope (Phase 6); listed now
  # so the exclusion of framework glue is not forgotten then.
  exclude_globs = [
      "apps/web/src/main.rs",
      "apps/web/src/lib.rs",
      "apps/web/src/app.rs",
  ]
  # Kill an infinite-loop mutant instead of waiting out the global
  # timeout: 5x the measured baseline `cargo test` time, floored at 60s.
  timeout_multiplier = 5.0
  minimum_test_timeout = 60
  ```

### Task 1.2 — PR job: `mutants-web` (in-diff, non-blocking)

Matches the cargo-mutants project's own recommended PR workflow
(`--in-place`, two-dot `git diff origin/<base>..`, default `mutants.out/`
output dir). Shards are **0-indexed** (`k/n` with `k < n`).

- [ ] Add to `ci-cd.yml`, before `deploy-web`:

  ```yaml
  mutants-web:
    name: Mutation test (changed lines)
    needs: changes
    if: needs.changes.outputs.web == 'true' && github.event_name == 'pull_request'
    runs-on: ubuntu-latest
    continue-on-error: true            # report-only until each crate's full sweep is clean
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-mutants
      - name: Diff against the PR base
        run: git diff origin/${{ github.base_ref }}.. | tee mutants.diff
      - name: cargo-mutants (in-diff, protocol + signaling)
        run: |
          cargo mutants --no-shuffle -vV --in-place --in-diff mutants.diff \
            -p screen-share-protocol -p screen-share-signaling
      - name: Job summary
        if: always()
        run: |
          {
            echo '## cargo-mutants — mutants in the changed lines'
            if [ -s mutants.out/missed.txt ]; then
              echo '### Missed (no test failed when the code was broken):'
              echo '```'; cat mutants.out/missed.txt; echo '```'
            else
              echo 'No missed mutants in the diff.'
            fi
            if [ -s mutants.out/timeout.txt ]; then
              echo '### Timed out:'; echo '```'; cat mutants.out/timeout.txt; echo '```'
            fi
          } >> "$GITHUB_STEP_SUMMARY"
      - if: always()
        uses: actions/upload-artifact@v4
        with:
          name: mutants-incremental
          path: mutants.out/
  ```

  `deploy-web` does **not** gain `mutants-web` as a dependency — the job
  is PR-only, so on push it is skipped anyway.

### Task 1.3 — Weekly full sweep: `.github/workflows/quality-scheduled.yml`

- [ ] New file (implemented form):

  ```yaml
  name: Quality (scheduled)
  on:
    schedule:
      - cron: '17 4 * * 1'   # Mondays 04:17 UTC — off-peak, once a week
    workflow_dispatch: {}
  permissions:
    contents: read
  env:
    CARGO_TERM_COLOR: always
  jobs:
    mutants-full:
      name: Full mutation sweep (shard ${{ matrix.shard }})
      runs-on: ubuntu-latest
      strategy:
        fail-fast: false
        matrix:
          shard: [0, 1, 2, 3]     # keep length == the /N below
      steps:
        - uses: actions/checkout@v4
        - uses: dtolnay/rust-toolchain@stable
        - uses: Swatinem/rust-cache@v2
        - uses: taiki-e/install-action@v2
          with:
            tool: cargo-mutants
        - run: |
            cargo mutants --no-shuffle -vV --in-place \
              -p screen-share-protocol -p screen-share-signaling \
              --shard ${{ matrix.shard }}/4
        - if: always()
          uses: actions/upload-artifact@v4
          with:
            name: mutants-full-${{ matrix.shard }}
            path: mutants.out/
  ```

### Task 1.4 — Close the survivor list for the two crates

`crates/protocol` generates **0 mutants** (pure `#[derive]` data types) —
trivially "0 missed". All work was in `crates/signaling`. First full
sweep: **1 missed + 11 timeouts** out of 71 mutants. Resolved as follows
(all committed with the workflow files in this phase):

- **11 timeouts** — every one was a `registry.rs` function that
  broadcasts/relays a `ServerMessage`, mutated to a no-op. The
  integration tests awaited the (now-missing) message with an unbounded
  `ws.next().await` / `rx.recv().await`, so the test **hung** instead of
  failing and cargo-mutants scored it `timeout`, not `caught`.
  Fix: a bounded receive helper in each test file —
  `recv_json` in `tests/signaling_ws.rs` and a new `recv()` in
  `tests/registry.rs`, both wrapping the await in
  `tokio::time::timeout(RECV_TIMEOUT, …)` (named const, 5s). All 11
  became `caught`.
- **`turn.rs:16` (×3)** — `CREDENTIAL_TTL = 6 * 60 * 60` arithmetic. The
  TTL test asserted against `CREDENTIAL_TTL.as_secs()` itself, so it
  moved with the mutation. Fix: assert against the literal `21_600`
  (with a few seconds' slack for the clock read).
- **`turn.rs:32/34/35` (`from_env` `-> None`, `delete !`)** — untestable
  without process-global env mutation. Fix: extracted the parsing into a
  pure `TurnConfig::from_vars(Option<String>, Option<String>)`
  (thoroughly unit-tested — both-present, either-empty, split/trim), and
  `exclude_re = ["TurnConfig::from_env"]` in `.cargo/mutants.toml` with a
  reason, since `from_env` is now a 2-line `std::env::var` read.
- **`ws.rs:19` `client_key` (×2)** — no unit test for the private helper.
  Fix: `src/ws_tests.rs` (same `#[cfg(test)] #[path] mod tests;` pattern
  as `turn_tests.rs`) — header present / absent / non-UTF-8.
- **`state.rs:22` `Option<TurnConfig>::from_ref -> None`** — no test
  checked TURN config reaching a handler. Fix: `spawn_test_server` in
  `tests/signaling_ws.rs` now takes an `Option<TurnConfig>`; a new test
  asserts `Joined { turn: Some(creds), .. }` carries the configured URLs
  and a non-empty username/password.

Net: signaling test count 65 → 72 (workspace 80 → 87).

If a future diff surfaces a new survivor, resolve it the same way:
strengthen a test first; `exclude_re` (with a reason) only for a mutant
that is genuinely equivalent or only observable by mutating global state.

### Acceptance gate — Phase 1

- [x] `.cargo/mutants.toml` present; local `cargo mutants -p
      screen-share-protocol -p screen-share-signaling` → **71 tested,
      0 missed, 0 timeout** (49 caught, 22 unviable).
- [x] `mutants-web` added to `ci-cd.yml` (PR-only, `--in-diff`,
      `continue-on-error: true`). Needs a real PR to see it post a
      summary.
- [x] `quality-scheduled.yml` added (weekly + `workflow_dispatch`,
      4 shards). Needs a `workflow_dispatch` run to confirm green in CI.
- [x] The one `exclude_re` entry (`TurnConfig::from_env`) has a reason
      comment.
- [x] `cargo test --workspace --features ssr` = **87** (was 80).

---

## Phase 2 — WASM unit tests + component render tests for `apps/web`

**Branch:** `quality/phase-2-wasm-tests`
**Goal:** First automated execution of the `hydrate` code path.
`wasm-bindgen-test` (headless Chrome) covers `infra/` and `session/`
helpers and a few component DOM interactions; native
`render_to_string` snapshots cover component output cheaply.

### Task 2.1 — Wire the wasm test runner  ✅

- [x] `apps/web/Cargo.toml` gains a `[dev-dependencies]` section:

  ```toml
  # Released in lockstep with the `wasm-bindgen` crate; Cargo enforces the
  # pairing via `wasm-bindgen-test`'s `=` requirement on `wasm-bindgen`.
  # wasm-bindgen 0.2.127 (Cargo.lock) ↔ wasm-bindgen-test 0.3.77.
  wasm-bindgen-test = "=0.3.77"
  ```

  Note: `wasm-bindgen-test` versions on the **0.3.x** line, *not* the
  `wasm-bindgen` 0.2.x line — check the resolved pair with
  `grep -A1 '^name = "wasm-bindgen' Cargo.lock`.
- [x] `.cargo/config.toml` — `runner = "wasm-bindgen-test-runner"` added
      to the **existing** `[target.wasm32-unknown-unknown]` table, with a
      comment that it only affects `cargo test` for that target.
- [x] Verified: `cargo leptos build` still produces
      `target/site/pkg/screen_share.{js,wasm,css}` with the runner set —
      the plain-`[target.*]` form works, no `cfg(test)` scoping needed.

### Task 2.2 — WASM unit tests  ✅

In-crate modules gated
`#[cfg(all(test, target_arch = "wasm32", feature = "hydrate"))]` so they
compile only for `cargo test --target wasm32 --features hydrate` and never
touch the native `ssr` run (same `#[path]` include pattern as
`turn_tests.rs`). Integration-style `tests/wasm/` was rejected because the
useful targets (`round_trip_ms_since`, private storage consts) are
`pub(crate)`/private.

- [x] `src/infra/storage_wasm_tests.rs` (8 tests): recent-rooms
      round-trip / dedup-to-front / truncate-to-`MAX_RECENT_ROOMS` /
      per-code removal / empty-on-corrupt-JSON; `RoomSession` round-trip +
      clear through `sessionStorage` (carries the password);
      `Profile` round-trip + default-on-corrupt-JSON; `ensure_device_id`
      generate-once-then-stable.
- [x] `src/infra/webrtc_wasm_tests.rs` (8 tests): `is_desktop_app`
      false/true by injecting `window.desktopAudio`;
      `notify_desktop_share_ready` no-op without the bridge and calls
      `desktopShare.linkReady` with the link when present (recorded via a
      real JS `Function`); `is_display_media_supported`;
      `new_peer_connection` with/without TURN creds; `create_offer`
      yields a `v=0` SDP; full offer→answer→`accept_answer` roundtrip
      between two local `RtcPeerConnection`s.
- [x] `src/session/latency.rs` inline module (2 tests):
      `round_trip_ms_since` clamps a future timestamp to `Some(0)` and
      measures elapsed ms for a past one.
- [x] `src/session/handler_wasm_tests.rs` (4 tests): `apply_joined_snapshot`
      under an `Owner::new().with(...)` + a full `RoomSignals` fixture —
      derives each member's `sharing` flag from `active_sharers`; maps
      `watcher_info` / `latencies` / `turn` into their signals; sets
      `authenticated` + peer id + room name + `"Conectado."` status;
      persists the room to the recent-rooms list.
- [ ] Deferred (follow-up, not blocking): `mount_to` component-DOM
      interaction.

### Task 2.3 — Native component render snapshots  ✅

- [x] `apps/web/tests/ssr_render.rs` (`#![cfg(feature = "ssr")]`, runs in
      `cargo test --workspace --features ssr`, 5 tests). Leptos 0.8 has no
      `leptos::ssr::render_to_string`; the SSR string comes from
      `RenderHtml::to_html()` under a fresh `Owner` — a small `render()`
      helper wraps that. Covers `StatusMessage` (error-modifier class
      present / absent, text passed through) and `status_meta`
      classification (previously untested pure logic).
- [ ] Deferred: `MemberCard` / grid / `ColorPicker` snapshots —
      follow-up.

### Task 2.4 — CI job `test-web-wasm`  ✅

- [x] Added to `ci-cd.yml` (implemented form):

  ```yaml
  test-web-wasm:
    name: Test web (WASM, headless Chrome)
    needs: changes
    if: needs.changes.outputs.web == 'true'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: wasm32-unknown-unknown }
      - uses: Swatinem/rust-cache@v2
      - uses: browser-actions/setup-chrome@v1
        id: chrome
        with: { install-chromedriver: true }
      - uses: taiki-e/install-action@v2
        with: { tool: wasm-bindgen-cli@0.2.127 }
      - name: Pin the browser for wasm-bindgen-test
        run: |
          printf '{ "goog:chromeOptions": { "binary": "%s", "args": ["--headless=new","--no-sandbox","--disable-gpu","--disable-dev-shm-usage"] } }\n' \
            "${{ steps.chrome.outputs.chrome-path }}" > "$RUNNER_TEMP/webdriver.json"
      - run: cargo test -p screen_share --target wasm32-unknown-unknown --no-default-features --features hydrate --lib
        env:
          WASM_BINDGEN_TEST_WEBDRIVER_JSON: ${{ runner.temp }}/webdriver.json
          WASM_BINDGEN_TEST_TIMEOUT: "120"
  ```

  `WASM_BINDGEN_TEST_WEBDRIVER_JSON` points wasm-bindgen-test-runner at a
  capabilities file naming the exact Chrome binary — more deterministic
  than relying on `chromedriver`'s PATH auto-discovery.
- [x] `build-web`'s wasm `clippy` gained `--all-targets` so the new
      `wasm_tests` modules are linted.

### Local dev note

`wasm-bindgen-test` needs `wasm-bindgen-test-runner` (from `cargo install
wasm-bindgen-cli --version <Cargo.lock version>`) plus a WebDriver binary
and a browser. **`scripts/test-wasm.sh`** handles all of that: uses a
system `chromedriver` if present, else downloads a version-matched
headless Chrome + chromedriver via `@puppeteer/browsers` into
`.wasm-browser/` and writes a `webdriver.json` (both git-ignored). CI
uses `browser-actions/setup-chrome` instead and generates its own
`webdriver.json` in `$RUNNER_TEMP`.

`.cargo/config.toml`'s `runner = "wasm-bindgen-test-runner"` is the
standard wasm-bindgen wiring and is **correct as-is** — a "failed to find
a suitable WebDriver binary" error means the browser/driver is missing,
not the config.

### Acceptance gate — Phase 2

- [x] `cargo test -p screen_share --target wasm32-unknown-unknown
      --no-default-features --features hydrate --lib` — **22 passed** in
      headless Chrome locally.
- [x] `cargo test --workspace --features ssr` — **92** (was 87), includes
      `ssr_render.rs`.
- [x] `cargo leptos build` still produces `target/site/pkg/`.
- [x] `cargo fmt --check`, `cargo clippy --workspace --all-targets
      --features ssr`, and `cargo clippy … wasm32 --all-targets … hydrate`
      all clean.
- [ ] `test-web-wasm` green in CI — needs a real PR to confirm
      `browser-actions/setup-chrome` + the webdriver.json wiring work on
      the runner.
- [ ] `cargo leptos watch` browser smoke unchanged — recommended before
      the phase PR merges (no product code changed, so low risk).

---

## Phase 3 — Playwright E2E for the web app

**Branch:** `quality/phase-3-e2e-web`
**Goal:** The manual room checklist runs as an automated job — headful
Chromium under `xvfb`, fake media, two browser contexts for the P2P
scenario.

### Task 3.1 — Playwright project  ✅

- [x] `apps/web/end2end/` — private `package.json` (npm, not pnpm; own
      `package-lock.json`), `@playwright/test` pinned `1.62.1`,
      `playwright.config.ts`:
  - `headless: false` (getDisplayMedia needs a headed browser) with
    Chromium flags `--use-fake-device-for-media-stream`,
    `--use-fake-ui-for-media-stream`,
    `--auto-select-desktop-capture-source=Entire screen`,
    `--auto-accept-this-tab-capture`.
  - `workers: 1`, `fullyParallel: false` — one shared `cargo leptos
    serve` and one in-memory signaling registry.
  - `webServer` runs `cargo leptos serve` from the repo root
    (`reuseExistingServer` off in CI), 240s cold-build timeout.
- [x] Root `Cargo.toml` `[[workspace.metadata.leptos]]` gains
      `end2end-cmd = "npx playwright test"` /
      `end2end-dir = "apps/web/end2end"`.
- [x] `.gitignore`: `node_modules/`, `test-results/`,
      `playwright-report/`, `blob-report/`, `.last-run.json`.

### Task 3.2 — Deterministic flows  ✅

`apps/web/end2end/tests/home.spec.ts` (5 tests, pass headed locally):
- [x] blank password + no "sala pública" checkbox → validation error, no
      navigation.
- [x] password room created → browser lands in `/r/<CODE>`, member grid +
      own card visible.
- [x] "sala pública" checked → password field hidden, room created with
      no password.
- [x] unknown room code → the "Sala não encontrada" screen, not a nick
      form.
- [x] a full invite link is accepted and its code upper-cased.
- [ ] Deferred (follow-up): the 10-member cap, per-IP wrong-password
      rate-limit, and recent-rooms `localStorage` behaviour as E2E specs
      (all already covered by signaling integration + wasm-bindgen tests;
      lower marginal value).

### Task 3.3 — P2P two-tab scenario  ✅

`apps/web/end2end/tests/room-p2p.spec.ts` (3 tests, pass headed locally
with **real WebRTC media**):
- [x] two `browser.newContext()` = two members in one room; Ana creates a
      public room, Bob joins by URL + nick.
- [x] Ana starts sharing → Bob's card for Ana shows the "Assistir
      transmissão" pill and **no** video yet (`readyState === 0`).
- [x] Bob clicks Ana's card → the peer `<video>` reaches
      `readyState >= 2` with `videoWidth > 0` (real decoded frames from
      the fake screen capture over a real `RTCPeerConnection`), and the
      avatar is hidden. `MEDIA_SETTLE_MS` names the poll bound.
- [x] Ana stops sharing via the in-app button → Bob's card falls back to
      the avatar.
- [x] a watcher reload mid-session silently rejoins (tab-scoped
      `RoomSession`) and keeps the roster — no nick gate.
- [x] **3rd context**: Bob and Caio both watch Ana; Bob stops watching
      (the per-card "Parar de assistir" button) → Bob's tile returns to
      the avatar while Caio's independent connection keeps decoding.
- [ ] Deferred (follow-up): stop-sharing via the browser's own control
      (`track.stop()` — the app owns the stream, no `window` hook to
      reach it; would need a debug-only test hook in the app).
- Note: asserting on the `<video>` element's `readyState` / `videoWidth`
  rather than `pc.getStats()` — the app doesn't expose the
  `RTCPeerConnection` to `window`, and a painted frame is already proof
  the media arrived and decoded.

### Task 3.4 — CI job `e2e-web`  ✅

- [x] Added to `ci-cd.yml`: `needs: [changes, build-web]`,
      `setup-node@24` + `npm ci` + `npx playwright install --with-deps
      chromium`, then `xvfb-run -a npm test` in `apps/web/end2end`.
      Uploads `playwright-report/` on failure (7-day retention).
- [x] No `paths-filter` change needed — `apps/web/**` already covers
      `apps/web/end2end/**`.

### Task 3.5 — Manual checklist updated  ✅

- [x] `CLAUDE.md` §"Testing approach" and §"Definition of done" →
      "Browser layer", and `RUST_GUIDELINES.md` §Testing: the two-tab
      checklist is now the `e2e-web` job; what stays manual is narrowed
      to the browser's own stop-sharing control, real window/screen
      capture, audio, and bitrate adaptation.

### Acceptance gate — Phase 3

- [x] `npm --prefix apps/web/end2end test` — **8 passed** headed locally
      (Playwright's `webServer` builds + serves).
- [x] The two-tab spec asserts real media flow (`<video>.readyState` /
      `videoWidth`), not just DOM presence.
- [x] `cargo leptos build`, `cargo test --workspace --features ssr` (92),
      `cargo fmt --check`, `cargo clippy` (ssr + wasm) — all still clean.
- [x] `CLAUDE.md` + `RUST_GUIDELINES.md` updated.
- [ ] `e2e-web` green in CI — needs a real PR to confirm `xvfb-run` +
      `playwright install --with-deps` + the headed getDisplayMedia flags
      work on the GitHub runner.

---

## Phase 4 — Vitest unit tests for the desktop main process + `windows-audio`

**Branch:** `quality/phase-4-desktop-unit`
**Goal:** The Electron main process gets real unit coverage with
`electron` and `node:child_process` mocked; the napi crate gets `#[test]`s.

### Task 4.1 — Vitest setup  ✅

- [x] `desktop/package.json`: `vitest` + `@vitest/coverage-v8` (both
      `4.1.11`) in `devDependencies`; scripts `test` / `test:watch` /
      `test:cov`.
- [x] `desktop/vitest.config.mts` (`.mts`, not `.ts` — the package is
      CJS): `environment: 'node'`, `include: ['src/**/*.test.ts']`,
      `clearMocks` + `restoreMocks`, coverage provider `v8` (text +
      lcov). `resolve.alias` rewrites `#native/*.js` → `native/*.js` and
      `#*.js` → `src/*.ts` so tests run against source, not stale `dist/`.
- [x] `desktop/tsconfig.json`: `exclude` `src/**/*.test.ts` +
      `src/test-helpers` so `tsc` never emits test files into `dist/`
      (which electron-builder would then package).
- [x] Biome already lints `src/**/*.ts` (test files included); no config
      change needed. `biome check --write` reformatted the new files.
- No shared `electron-mock` module: `vi.mock` hoisting makes a per-file
  inline factory (with `vi.hoisted` spies) simpler and self-contained
  than a shared one — a little repetition over the wrong abstraction.

### Task 4.2 — Unit tests  ✅

`desktop/src/**/*.test.ts` — 11 files, 41 tests, all pass locally:
- [x] `platform/run-command.test.ts` — resolves with collected stdout;
      `''` on spawn `error`; keeps partial stdout on non-zero exit
      (`node:child_process` mocked).
- [x] `platform/linux/pipewire.test.ts` — `listAudioOutputStreams` keeps
      only `Stream/Output/Audio`, extracts id/name/pid/binary; non-JSON /
      non-array → `[]`; `listDistinctAudioApps` dedups by binary.
- [x] `platform/linux/process-identity.test.ts` — `parseX11WindowId`;
      `resolveWindowPid` parses `xprop` output; `resolveAudioTarget`:
      screen passthrough, window→binary via exe symlink, cmdline
      fallback, `null` when the owner can't be identified.
- [x] `platform/windows/process-identity.test.ts` — `parseWindowsWindowId`;
      `resolveAudioTarget` window/screen paths (`#native/windows-audio`
      mocked).
- [x] `platform/windows/audio.test.ts` — `listDistinctAudioApps` mapping;
      `startAudioLoopback` starts one native session and ignores a second;
      `stopAudioLoopback` stops once then no-ops.
- [x] `features/audio-share/backend.test.ts` — `loadAudioBackend`
      assembles the right `AudioBackend` shape on `linux` and `win32`,
      and memoizes (same promise on repeat calls).
- [x] `features/audio-share/ipc.test.ts` — `registerAudioIpcHandlers`
      binds `start/stop/list-audio-*`; `stopAudioLoopbackNow` is a safe
      no-op before registration and calls the backend stop after.
- [x] `features/screen-share/quick-share.test.ts` — `desktop-share:link-ready`
      → `clipboard.writeText`; `desktop-share:member-joined` →
      `Notification` (and nothing when `Notification.isSupported()` is
      false).
- [x] `main/lifecycle.test.ts` — `isQuitting` starts false; `markQuitting`
      flips it without quitting; `requestQuit` marks then `app.quit()`.
- [x] `main/tray.test.ts` — the Abrir / Compartilhar tela / Sair menu is
      built and each `click` wired to the right function; tray click
      opens the window.
- [x] `main/window.test.ts` — `getMainWindow` null before create;
      `startQuickShare` no-ops without a window, else reloads the URL with
      `quick_share=1`; the `close` handler hides unless `isQuitting()`.
- [ ] Deferred (follow-up): `features/screen-share/picker.ts` (heavy
      `BrowserWindow` + `desktopCapturer` + `ipcMain.once` + timers
      choreography; low logic-per-mock ratio) and `main/index.ts`
      (bootstrap wiring only).

### Task 4.3 — `windows-audio` napi `#[test]`s  ✅ (unverified locally)

- [x] `src/capture.rs` `#[cfg(test)] mod tests` for `should_include`
      (self-exclusion always wins; window mode → only the target binary;
      screen mode → all but the excluded list). `process_identity.rs` has
      no pure logic worth testing — it is all Win32 / WASAPI FFI.
- [x] `Cargo.toml` `[lib] crate-type` gains `"rlib"` alongside `"cdylib"`
      so `cargo test` links a normal test binary (the napi `cdylib` alone
      has unresolved `napi_*` symbols at test-link time). `napi build` /
      electron-builder only consume the `cdylib`.
- [x] `build-desktop-windows` job gets a `cargo test` step (after
      `npm run build`). **Cannot run on Linux** — `wasapi` / `windows-rs`
      don't compile off Windows — so it is verified only on the Windows CI
      job.

### Task 4.4 — CI job `test-desktop`  ✅

- [x] New `test-desktop` job (implemented form): `pnpm install` →
      `pnpm run check` (Biome) → `pnpm run build` (tsc typecheck) →
      `pnpm run test` (Vitest). Desktop-path-filtered.
- [x] `pnpm run check` removed from `build-desktop-linux` — that job now
      only packages; lint/type/unit live in `test-desktop`.
- [x] `publish-desktop-release` `needs` now includes `test-desktop`, so a
      failing unit test blocks the rolling release.

### Acceptance gate — Phase 4

- [x] `pnpm --dir desktop run test` — **44 passed** (12 files) locally;
      `pnpm run check` and `pnpm run build` clean.
- [ ] `cargo test` in `desktop/native/windows-audio` — cannot build on
      this Linux box (`windows-future` fails to compile); confirm on the
      Windows CI job.
- [x] No `electron` import escapes a mock — every test file that touches
      `electron` declares `vi.mock('electron', …)`; an unmocked import
      would resolve to a path string and fail the assertions loudly.
- [ ] `test-desktop` green in CI — needs a real PR to confirm.

---

## Phase 5 — Playwright `_electron` E2E for the desktop app

**Branch:** `quality/phase-5-e2e-desktop`
**Goal:** The desktop app boots and its main flows are exercised
end-to-end, headful under `xvfb`, native dialogs stubbed via
`electronApp.evaluate`.

### Task 5.1 — Playwright Electron project  ✅

- [x] `@playwright/test` `1.62.1` added to `desktop` devDependencies.
- [x] `desktop/e2e/playwright.config.ts` — `testDir: '.'`,
      `testMatch: '*.spec.ts'`, `workers: 1`, no `webServer`, 60s timeout,
      `trace: retain-on-failure`.
- [x] `desktop/package.json` script
      `"test:e2e": "tsc && playwright test --config e2e/playwright.config.ts"`
      (the `tsc` builds `dist/` that `_electron` launches).
- [x] **Product change (testability seam):** `main/window.ts` now reads
      `SCREEN_SHARE_URL` with `?? PROD_URL` — unset in every shipped
      build, so behaviour is unchanged; lets a dev point the shell at a
      local `cargo leptos serve` and lets the E2E load `about:blank`
      instead of hitting production. Covered by the existing
      `window.test.ts` (still asserts the default is the fly.dev URL).

### Task 5.2 — Flows  ✅

`desktop/e2e/*.spec.ts` (5 tests, pass headed locally in ~2s):
- [x] `app-boot.spec.ts` — the app boots; `BrowserWindow` #0 has the OS
      title `"Screen Share"` and starts hidden (tray app); `app.close()`
      exits with no `Uncaught` / `TypeError` / `Cannot find module` on the
      main process's stderr.
- [x] `app-boot.spec.ts` — the audio-loopback IPC handlers are live: the
      preload bridge's `window.desktopAudio.stop()`
      (`ipcRenderer.invoke('stop-audio-loopback')`) resolves rather than
      rejecting with "No handler registered".
- [x] `quick-share-ipc.spec.ts` — emitting `desktop-share:link-ready`
      into `ipcMain` copies the link to the clipboard
      (`clipboard.readText()`); `desktop-share:member-joined` runs without
      throwing.
- [x] `quick-share-ipc.spec.ts` — after `app.emit('before-quit')` the
      window's own `close` handler no longer vetoes the close
      (`markQuitting()` took effect).
- [ ] Deferred (follow-up): asserting the tray was created — no exported
      handle and no `Tray` registry to inspect without a test hook; the
      boot spec covers "`createTray()` didn't throw" indirectly.

### Task 5.3 — CI job `e2e-desktop`  ✅

- [x] `e2e-desktop` job: `needs: [changes, test-desktop]`, `pnpm install`
      → `npx playwright install-deps chromium` (shared libs only —
      `_electron` uses the app's own bundled Electron, no browser
      download) → `xvfb-run -a pnpm run test:e2e`. Uploads
      `playwright-report/` on failure.
- [x] `publish-desktop-release` `needs` now includes `e2e-desktop`.

### Task 5.4 — Manual checklist updated  ✅

- [x] `CLAUDE.md` §"Definition of done" → "Desktop": `pnpm run test` and
      `pnpm run test:e2e` added; still-manual list narrowed to the source
      picker window, real capture, system-audio loopback, and
      Windows-only paths.

### Acceptance gate — Phase 5

- [x] `pnpm --dir desktop run test:e2e` — **5 passed** headed locally
      (builds `dist/`, launches real Electron per spec).
- [x] No test depends on a real OS dialog or real screen capture —
      everything goes through `ipcMain`/`BrowserWindow` from
      `app.evaluate`, page content is `about:blank`.
- [x] `CLAUDE.md` updated.
- [ ] `e2e-desktop` green in CI under `xvfb` — needs a real PR to confirm
      `playwright install-deps` covers Electron's shared libs on the
      runner.

---

## Phase 6 — Tighten the mutation gate; add StrykerJS; wire coverage

**Branch:** `quality/phase-6-tighten`
**Goal:** Turn the proven-clean mutation sweeps into blocking gates,
extend mutation to `apps/web` and the desktop unit suite, and publish
coverage.

### Task 6.1 — `mutants-web` blocking for protocol + signaling  ✅

- [x] `continue-on-error` dropped from `mutants-web` — an uncaught mutant
      in changed lines of `crates/protocol` / `crates/signaling` now
      **fails the PR**. Both crates' full sweeps are at zero survivors
      (verified locally across repeated runs; Phase 1). A job comment and
      the "Open: confirm on a real PR" checklist note the rollback: if the
      scheduled `mutants-full` regresses, restore `continue-on-error`
      until it is clean again.
- [ ] Add `mutants-web` to branch protection (`gh api`, Phase 0 Task 0.3)
      — maintainer, after merge.

### Task 6.2 — `cargo-mutants` extended to `apps/web` (report-only)  ✅

- [x] New `mutants-web-app` PR job + `mutants-full-app` in
      `quality-scheduled.yml`, both `continue-on-error: true` (report-only).
- [x] **Scoped to the `apps/web` files that compile under `--features
      ssr`** — `-f` flags for `components/{palette,status,status_message}`,
      `features/room/grid`, `features/home/join_room`, `session/quality`
      (~138 mutants). The rest of `apps/web` is `#[cfg(feature =
      "hydrate")]` browser code; cargo-mutants can't evaluate that `cfg`
      and would report every mutant there — and every mutant in a
      `#[cfg(…test…)] mod wasm_tests` — as a false "survivor" (a mutated
      line that then compiles out and passes all tests). The `-f` list is
      duplicated between the two jobs with a keep-in-sync note.
- [x] `.cargo/mutants.toml` `exclude_globs`: `app.rs`, `dev_preview.rs`,
      `**/*_tests.rs`, `**/*_wasm_tests.rs`. All wasm test modules are now
      separate `*_wasm_tests.rs` files (extracted `latency.rs`'s inline
      one) so the glob covers them.

### Task 6.3 — StrykerJS for the desktop unit suite  ✅

- [x] `desktop`: `@stryker-mutator/core` + `@stryker-mutator/vitest-runner`
      (`10.0.0`); script `"test:mutation": "stryker run"`.
- [x] `desktop/stryker.config.mjs` — `testRunner: 'vitest'`, explicit
      `plugins: ['@stryker-mutator/vitest-runner']` (pnpm's layout
      defeats Stryker's glob auto-discovery), `inPlace: true` (skips
      Stryker's tsconfig rewriter, which calls
      `ts.parseConfigFileTextToJson` — absent from the TypeScript 7
      native build this project pins). `mutate` excludes `*.test.ts`,
      `test-helpers`, `main/index.ts`, and the three still-untested
      modules (`picker` / `display-media` / `linux/loopback`).
- [x] Local run: **mutation score 69.90 %** (83.60 % of covered code),
      209 killed / 41 survived / 49 no-coverage / 0 timeouts, in ~13s.
- [x] `stryker-desktop` job in `quality-scheduled.yml`
      (`continue-on-error: true`); an incremental `stryker run --since
      origin/<base>` step in `test-desktop` on PRs (`continue-on-error`).
- [x] `.gitignore`: `desktop/.stryker-tmp/`, `desktop/reports/`.

### Task 6.4 — Coverage → Codecov  ✅

- [x] `codecov.yml` at the repo root — `informational: true` on project
      and patch (never fails a PR), `rust` / `desktop` flags.
- [x] `test-web`: `cargo test` replaced by `cargo llvm-cov --workspace
      --features ssr --lcov` (same gate, emits coverage; native `ssr`
      only — the wasm paths are `test-web-wasm`'s).
      `--ignore-filename-regex` drops `main.rs` / `app.rs` (entry-point
      wiring), `dev_preview.rs` (`#[cfg(debug_assertions)]` bench, never
      shipped) and `icons.rs` (SVG constants) — kept in sync between the
      job and `scripts/test-all.sh`. **Verified locally: 43.6 % regions /
      46.4 % lines** (signaling ~95 %+; the remaining 0 % rows are Leptos
      view macros — covered by Playwright — and `hydrate`-gated glue —
      covered by wasm-bindgen).
- [x] `test-desktop`: `pnpm run test` → `pnpm run test:cov` (v8 lcov).
      **Verified locally: 69.5 % statements / 71.5 % lines** (after adding
      `platform/linux/loopback.ts` + `pipewire.ts` tests, that dir went
      43 % → 90 %). The 0 % holdouts are `picker.ts` / `display-media.ts`
      / `main/index.ts` — deferred (low logic-per-mock).
- [x] Both jobs `upload-artifact` the lcov **and** push to Codecov via
      `codecov/codecov-action@v5` with `continue-on-error: true` +
      `fail_ci_if_error: false` + `token: ${{ secrets.CODECOV_TOKEN }}`
      (optional for a public repo). The artifact is the guaranteed
      fallback if Codecov is unavailable or the repo is private and over
      the free plan.

### Acceptance gate — Phase 6

- [x] `mutants-web` no longer `continue-on-error` — it can fail a PR.
      (Behaviour on a real PR: pending — see the checklist.)
- [x] `mutants-web-app` / `mutants-full-app` run report-only; the
      `.cargo/mutants.toml` scoping is verified with `--list`.
- [x] `pnpm --dir desktop run test:mutation` — Stryker runs green
      (report-only), score 69.90 %.
- [x] `cargo llvm-cov --workspace --features ssr` and `pnpm run test:cov`
      both produce lcov locally.
- [x] `CLAUDE.md` §"Definition of done" + `RUST_GUIDELINES.md` §Testing
      updated for the mutation gate.
- [ ] On a real PR: Codecov comment appears (or lcov artifacts are there)
      and cannot fail the PR; the scheduled workflow's `mutants-full`,
      `mutants-full-app`, `stryker-desktop` all run on `workflow_dispatch`.

---

## Master acceptance — quality gate complete

- [ ] A PR runs, at minimum: `build-web`, `test-web`, `test-web-wasm`,
      `e2e-web`, `mutants-web` (web changes); `test-desktop`,
      `e2e-desktop` (desktop changes) — path-filtered.
- [ ] The required-status-checks set on `main` includes every
      deterministic blocking job above.
- [ ] `deploy-web` and `publish-desktop-release` run only on `push` to
      `main`, never on a PR.
- [ ] The scheduled workflow (weekly) runs the full mutation sweep and
      StrykerJS and publishes coverage.
- [ ] `CLAUDE.md` manual checklist is trimmed to only what genuinely
      cannot be automated here, with each item's reason stated.
- [ ] `docs/decisions/0005-quality-gate.md` Status stays `accepted`;
      update its Consequences if the rollout diverged.

## Self-review

- **Scope coverage:** the request — tests across back-end / front-end /
  Electron, mutation testing, and automating the manual front-end +
  Electron testing — maps to Phases 1+2 (back-end + WASM), 2+3
  (front-end unit + E2E), 4+5 (Electron unit + E2E), 1+6 (mutation), with
  Phase 0 making any of it a real gate.
- **Ordering:** cheapest, highest-certainty checks first (Phase 0 costs
  almost nothing and immediately stops un-checked merges); the two
  slowest/most-flake-prone layers (E2E web, E2E desktop) come after their
  unit layers so failures are easier to localise; blocking mutation last,
  after the survivor lists are provably empty.
- **Genuine unknowns, handled inline not deferred:** exact Chromium
  screen-capture flag names (Task 3.1 — verify against the pinned build,
  alternatives listed); whether `runner` in `.cargo/config.toml`
  perturbs `cargo leptos build` (Task 2.1 — verification step + fallback
  form); Codecov free-tier eligibility (Task 6.4 — artifact fallback).
- **Consistency:** job names here match the names used in the branch
  protection command and the master acceptance list; `paths-filter`
  updates are called out wherever a new path (`apps/web/end2end/**`)
  needs to count as a change.
