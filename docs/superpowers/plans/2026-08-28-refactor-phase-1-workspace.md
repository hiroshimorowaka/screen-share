# Refactor Phase 1 — Cargo Workspace + `apps/web` Move

> **For agentic workers:** REQUIRED SUB-SKILL: use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking. This is Phase 1 of the
> refactor described in
> `docs/superpowers/plans/2026-08-28-architecture-refactor-roadmap.md` —
> read that roadmap's "Global Constraints" and "Dependency invariants"
> first; they apply here in full.

**Goal:** Convert the single `screen_share` crate into a Cargo workspace
whose one member is `apps/web/`, with **zero** behavior change and zero
change to the deployed artifact.

**Architecture:** A virtual workspace manifest at the repo root
(`[workspace]` + `[[workspace.metadata.leptos]]` + `[profile.*]` only).
The entire current crate moves verbatim into `apps/web/` and keeps its
package name `screen_share`, so `cargo-leptos`, the `Dockerfile`, and
every `use screen_share::…` path keep working untouched. `public/` moves
under `apps/web/`. The two root integration-test files move into
`apps/web/tests/`.

**Tech stack:** Rust 2021, Cargo workspace (`resolver = "2"`),
`cargo-leptos` 0.3.x, Leptos 0.8.

## Global Constraints (from the roadmap — repeated for the implementer)

- No behavior change. Pure mechanical move.
- Package name stays `screen_share`. Directory becomes `apps/web`.
- `LEPTOS_OUTPUT_NAME` stays `screen_share`. Do not touch
  `.cargo/config.toml`, `Dockerfile`, `docker-entrypoint.sh`, `fly.toml`.
- Build/validate the web app only via `cargo leptos build` /
  `cargo leptos watch`, never the bare binary.
- Lint gate before "done": `cargo clippy --all-targets -- -D warnings`
  and `cargo fmt --check` clean.
- English everywhere in code/commits/docs. Commit trailer:
  `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>`.
- Work on a branch, not `main`.

---

## File Structure after this phase

```
Cargo.toml                     # NEW content: [workspace] + [[workspace.metadata.leptos]] + [profile.wasm-release]
Cargo.lock                     # regenerated in place (paths change, versions don't)
.cargo/config.toml             # UNCHANGED
Dockerfile                     # UNCHANGED
apps/
└── web/
    ├── Cargo.toml             # NEW: the old [package]/[lib]/[dependencies]/[features]/[dev-dependencies]
    ├── public/                # moved from ./public/
    ├── src/                   # moved from ./src/  (lib.rs, main.rs, signaling/, ui/)
    └── tests/                 # moved from ./tests/ (rooms_status.rs, signaling_ws.rs)
```

Nothing under `src/` changes internally in this phase — no file contents,
no `use` paths, no module tree. Only the manifest split and the
directory move.

---

## Task 1: Capture the baseline

**Files:** none (measurement only).

- [ ] **Step 1: Create the working branch**

```bash
git checkout -b refactor/phase-1-workspace
```

- [ ] **Step 2: Record the passing test suite and its count**

```bash
cargo test --features ssr 2>&1 | tee /tmp/phase1-baseline-tests.txt
cargo test --features ssr -- --list 2>&1 | grep -c ': test$' | tee /tmp/phase1-baseline-count.txt
```

Expected: all tests pass. Note the number in
`/tmp/phase1-baseline-count.txt` — every later step must reproduce it.

- [ ] **Step 3: Record a successful production-style build**

```bash
cargo leptos build 2>&1 | tail -20
ls target/site/pkg/
```

Expected: build succeeds; `target/site/pkg/` contains
`screen_share.js`, `screen_share.wasm` (or `screen_share_bg.wasm`), and
the bundled CSS.

- [ ] **Step 4: Record the clean lint state**

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

Expected: no output from `fmt --check`; clippy finishes with no warnings.

---

## Task 2: Move the crate into `apps/web/`

**Files:**
- Move: `src/` → `apps/web/src/`
- Move: `tests/` → `apps/web/tests/`
- Move: `public/` → `apps/web/public/`

**Interfaces:** unchanged — package remains `screen_share`, all modules
and paths identical.

- [ ] **Step 1: Create the directory and move the three trees with git**

```bash
mkdir -p apps/web
git mv src apps/web/src
git mv tests apps/web/tests
git mv public apps/web/public
```

- [ ] **Step 2: Verify the move**

```bash
git status
ls apps/web/src apps/web/tests apps/web/public
```

Expected: `git status` shows the three trees as renamed (`R`), nothing
deleted-and-re-added, no stray files left at the old paths.

- [ ] **Step 3: Do NOT build yet** — the manifests are still wrong. Proceed
  to Task 3.

---

## Task 3: Split the root manifest into workspace + `apps/web/Cargo.toml`

**Files:**
- Create: `apps/web/Cargo.toml`
- Modify: `Cargo.toml` (root) — replace its entire contents

**Interfaces produced:** a virtual workspace with one member,
`apps/web` (package `screen_share`); `cargo-leptos` driven from the root
via `[[workspace.metadata.leptos]]`.

- [ ] **Step 1: Write `apps/web/Cargo.toml`**

Copy every `[dependencies]`, `[features]`, and `[dev-dependencies]` entry
**verbatim** from the current root `Cargo.toml` (do not re-version or
re-order). The `web-sys` feature list is long — copy it whole.

```toml
[package]
name = "screen_share"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
leptos = { version = "0.8.0" }
leptos_router = { version = "0.8.0" }
leptos_meta = { version = "0.8.0" }
axum = { version = "0.8.0", optional = true }
console_error_panic_hook = { version = "0.1", optional = true }
leptos_axum = { version = "0.8.0", optional = true }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"], optional = true }
wasm-bindgen = { version = "0.2.106", optional = true }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
uuid = { version = "1.24.1", features = ["v4"], optional = true }
rand = { version = "0.10.2", optional = true }
futures-util = { version = "0.3.34", optional = true }
wasm-bindgen-futures = { version = "0.4.77", optional = true }
js-sys = { version = "0.3.104", optional = true }
web-sys = { version = "0.3.104", optional = true, features = [
    "WebSocket", "MessageEvent", "MediaDevices", "MediaStream",
    "MediaStreamTrack", "DisplayMediaStreamConstraints", "MediaDeviceInfo",
    "MediaDeviceKind", "MediaTrackConstraints", "MediaStreamConstraints",
    "ConstrainDomStringParameters", "Navigator", "Window", "Location",
    "RtcPeerConnection", "RtcConfiguration", "RtcIceServer", "RtcSdpType",
    "RtcSessionDescriptionInit", "RtcIceCandidateInit", "RtcIceCandidate",
    "RtcPeerConnectionIceEvent", "RtcTrackEvent", "RtcIceConnectionState",
    "RtcRtpSender", "RtcRtpParameters", "RtcRtpEncodingParameters",
    "HtmlVideoElement", "HtmlMediaElement", "Element", "NodeList",
    "PictureInPictureWindow", "MediaStreamTrackGenerator",
    "MediaStreamTrackGeneratorInit", "AudioData", "AudioDataInit",
    "AudioSampleFormat", "WritableStream", "WritableStreamDefaultWriter",
    "Clipboard", "Storage", "Response", "Crypto", "Performance", "console",
] }
argon2 = { version = "0.5.3", optional = true, features = ["std"] }
hmac = { version = "0.12", optional = true }
sha1 = { version = "0.10", optional = true }
base64 = { version = "0.22", optional = true }

[features]
hydrate = [
    "leptos/hydrate",
    "dep:console_error_panic_hook",
    "dep:wasm-bindgen",
    "dep:wasm-bindgen-futures",
    "dep:js-sys",
    "dep:web-sys",
]
ssr = [
    "dep:axum",
    "dep:tokio",
    "dep:leptos_axum",
    "dep:uuid",
    "dep:rand",
    "dep:futures-util",
    "dep:argon2",
    "dep:hmac",
    "dep:sha1",
    "dep:base64",
    "leptos/ssr",
    "leptos_meta/ssr",
    "leptos_router/ssr",
]
argon2 = ["dep:argon2"]

[dev-dependencies]
tokio-tungstenite = "0.30.0"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["test-util"] }
```

- [ ] **Step 2: Replace the root `Cargo.toml` with a workspace manifest**

`[profile.*]` tables are only honored at the workspace root, so
`wasm-release` stays here. `[[workspace.metadata.leptos]]` carries every
key from the old `[package.metadata.leptos]`, plus `bin-package` /
`lib-package` pointing at the member.

```toml
[workspace]
members = ["apps/web"]
resolver = "2"

# Size-optimized profile for the WASM bundle in release mode. Profiles are
# only read from the workspace root, so this must live here, not in apps/web.
[profile.wasm-release]
inherits = "release"
opt-level = 'z'
lto = true
codegen-units = 1
panic = "abort"

[[workspace.metadata.leptos]]
name = "screen_share"
bin-package = "screen_share"
lib-package = "screen_share"

# The name used by wasm-bindgen/cargo-leptos for the JS/WASM bundle.
# MUST stay in sync with LEPTOS_OUTPUT_NAME in .cargo/config.toml.
output-name = "screen_share"

# cargo-leptos resolves these paths relative to this workspace-root directory.
site-root = "target/site"
site-pkg-dir = "pkg"
assets-dir = "apps/web/public"

site-addr = "127.0.0.1:3000"
reload-port = 3001
browserquery = "defaults"

bin-features = ["ssr"]
bin-default-features = false
lib-features = ["hydrate"]
lib-default-features = false
lib-profile-release = "wasm-release"
```

- [ ] **Step 3: Regenerate the lockfile**

```bash
cargo update --workspace --dry-run   # sanity: no version churn expected
cargo metadata --format-version 1 >/dev/null   # forces Cargo.lock rewrite with new paths
git diff Cargo.lock | head -40
```

Expected: `Cargo.lock` diff is limited to the `screen_share` package's
`source`/path bookkeeping — **no dependency version changes**. If any
third-party crate version moves, stop and investigate before continuing.

---

## Task 4: Verify the workspace builds and tests exactly as the baseline

**Files:** none (verification).

- [ ] **Step 1: Compile both feature sets**

```bash
cargo build -p screen_share --features ssr
cargo build -p screen_share --features hydrate --target wasm32-unknown-unknown
```

Expected: both succeed. (Building `hydrate` for the host target will fail
on `web-sys` — that is expected; only the `wasm32` target is valid for
`hydrate`, matching how `cargo-leptos` builds it.)

- [ ] **Step 2: Run the full test suite and compare the count**

```bash
cargo test -p screen_share --features ssr 2>&1 | tee /tmp/phase1-after-tests.txt
cargo test -p screen_share --features ssr -- --list 2>&1 | grep -c ': test$'
```

Expected: all tests pass; the count equals
`/tmp/phase1-baseline-count.txt` from Task 1 Step 2. The two moved
integration files (`apps/web/tests/rooms_status.rs`,
`apps/web/tests/signaling_ws.rs`) must still run — confirm their test
names appear in the `--list` output.

- [ ] **Step 3: Production-style build via cargo-leptos from the repo root**

```bash
cargo leptos build 2>&1 | tail -30
ls -la target/site/pkg/
ls -la target/site/   # assets from apps/web/public must be here
```

Expected: build succeeds; `target/site/pkg/` has `screen_share.js` +
`screen_share*.wasm` + bundled CSS; `target/site/` contains the files
that were under `public/` (e.g. `styles/card.css`, favicons). If assets
are missing, `assets-dir` is resolving wrong — try `assets-dir = "public"`
(and move `public/` back to the repo root) as the fallback, re-run, and
record which form worked in this plan.

- [ ] **Step 4: Manual browser smoke test**

```bash
cargo leptos watch
```

Open `http://127.0.0.1:3000/`. Verify: home page renders with styling;
create a room; the room page loads; browser devtools Network tab shows
`screen_share.wasm` fetched with 200 (not 404) and no hydration
mismatch errors in the console. Stop `watch` with Ctrl-C.

- [ ] **Step 5: Lint gate**

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: both clean, matching Task 1 Step 4.

- [ ] **Step 6: Docker build parity (optional but recommended before merge)**

```bash
docker build -t screen-share-phase1 .
```

Expected: succeeds unchanged. The `Dockerfile`'s `COPY . .`,
`cargo leptos build --release`, `cp target/release/screen_share …`, and
`cp -r target/site …` all still resolve because the package name and
`target/` location are unchanged.

---

## Task 5: Update in-repo references to the old layout

**Files:**
- Modify: `CLAUDE.md` — §"Commands" and §"Architecture"
- Modify: `README.md` — any build/layout references
- Modify: `.claude/skills/verify/SKILL.md` — only if it hard-codes `src/`
  or the single-crate test command (check first)

- [ ] **Step 1: Update `CLAUDE.md` §"Commands"**

Change the test commands to the workspace forms:

```bash
# Run the automated test suite:
cargo test -p screen_share --features ssr

# Run a single test:
cargo test -p screen_share --features ssr <test_name>
```

Add a one-line note under §"Architecture" → "One crate, two compiled
targets": *The crate lives at `apps/web/` as the sole member of a Cargo
workspace; `cargo-leptos` is driven from the repo root via
`[[workspace.metadata.leptos]]`.* Do not rewrite the rest of the
architecture section — later phases update it as crates are extracted.

- [ ] **Step 2: Update `README.md`**

Grep for `src/`, `cargo test --features ssr`, and `./public` references;
update paths that are now wrong. Keep changes minimal and factual.

- [ ] **Step 3: Check the `verify` skill**

```bash
grep -n 'src/\|--features ssr\|cargo test' .claude/skills/verify/SKILL.md
```

If it references the single-package command or `src/` paths, update to
`cargo test -p screen_share --features ssr` and `apps/web/src/`.
If it already scopes with `-p` or reads `cargo metadata`, leave it.

- [ ] **Step 4: Re-run the lint gate** (docs-only changes, but confirm)

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

---

## Task 6: Commit

- [ ] **Step 1: Stage and review**

```bash
git add -A
git status
git diff --cached --stat
```

Expected in the diff: renames of `src/**`, `tests/**`, `public/**` under
`apps/web/`; new `apps/web/Cargo.toml`; rewritten root `Cargo.toml`;
`Cargo.lock` path bookkeeping; small `CLAUDE.md` / `README.md` edits. No
changes to any `.rs` file contents. No changes to `.cargo/config.toml`,
`Dockerfile`, `docker-entrypoint.sh`, `fly.toml`.

- [ ] **Step 2: Commit**

```bash
git commit -m "refactor: convert to Cargo workspace with apps/web member

Move the single screen_share crate verbatim into apps/web/ and introduce
a virtual workspace manifest at the repo root. Package name, module tree,
LEPTOS_OUTPUT_NAME, and the deployed artifact are all unchanged; this is
a pure directory-and-manifest move. cargo-leptos is now driven from the
repo root via [[workspace.metadata.leptos]].

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

- [ ] **Step 3: Open the PR** (if the workflow uses PRs)

```bash
gh pr create --fill --base main
```

PR body must state: pure refactor, no behavior change, baseline vs.
post test counts equal (cite the numbers), `cargo leptos build` +
browser smoke + `docker build` all verified.

---

## Acceptance gate (all must be true before Phase 2)

- [ ] `cargo test -p screen_share --features ssr` — all green, count ==
      baseline from Task 1.
- [ ] `cargo leptos build` from repo root — succeeds; `target/site/pkg/`
      and `target/site/` assets present.
- [ ] `cargo leptos watch` — home + room pages render, wasm 200, no
      hydration mismatch in console.
- [ ] `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
      — clean.
- [ ] `docker build .` — succeeds (recommended).
- [ ] No `.rs` file contents changed; no changes to `.cargo/config.toml`,
      `Dockerfile`, `docker-entrypoint.sh`, `fly.toml`.
- [ ] Committed on `refactor/phase-1-workspace`.

---

## Self-review

- **Spec coverage:** the roadmap's Phase 1 row asks for "Cargo workspace
  + move Leptos crate to `apps/web/` (package name unchanged), move
  `public/`, wire `[[workspace.metadata.leptos]]`" — Tasks 2–4 cover all
  four; Task 5 handles the doc fallout; Tasks 1 and 4 are the
  no-behavior-change gate.
- **Placeholder scan:** the one genuine unknown — whether cargo-leptos
  resolves `assets-dir` workspace-root-relative or package-relative — is
  handled inline in Task 4 Step 3 with a concrete fallback and a note to
  record the outcome, not left as "TBD".
- **Consistency:** package name `screen_share`, bundle/output name
  `screen_share`, `LEPTOS_OUTPUT_NAME=screen_share`, binary
  `target/release/screen_share`, test command
  `cargo test -p screen_share --features ssr` — all consistent across
  every task and with the roadmap.
