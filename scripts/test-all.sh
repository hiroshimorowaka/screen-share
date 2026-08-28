#!/usr/bin/env bash
# Run every automated check in the project — the same set CI runs, plus a
# few that only run in the scheduled workflow.
#
# Runs everything, collects failures, prints a summary, exits non-zero if
# anything failed. Missing optional tools are SKIPped, not failed.
#
#   scripts/test-all.sh [options]
#     --no-lint          skip fmt + clippy
#     --no-mutants       skip cargo-mutants (protocol + signaling, ~4 min)
#     --no-e2e           skip the Playwright suites
#     --web-mutants      also run cargo-mutants on apps/web (VERY slow)
#     --coverage         also produce coverage (cargo-llvm-cov + vitest)
#     --quick            = --no-lint --no-mutants --no-e2e
#
# E2E needs a display; without $DISPLAY the script uses `xvfb-run` if it is
# installed, otherwise it SKIPs those two suites.

set -o pipefail
cd "$(dirname "$0")/.."

run_lint=1 run_mutants=1 run_e2e=1 run_web_mutants=0 run_coverage=0
for arg in "$@"; do
  case "$arg" in
    --no-lint) run_lint=0 ;;
    --no-mutants) run_mutants=0 ;;
    --no-e2e) run_e2e=0 ;;
    --web-mutants) run_web_mutants=1 ;;
    --coverage) run_coverage=1 ;;
    --quick) run_lint=0 run_mutants=0 run_e2e=0 ;;
    -h | --help) sed -n '2,${/^#/!q;s/^# \{0,1\}//p;}' "$0"; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

bold=$'\033[1m'; blue=$'\033[1;34m'; yellow=$'\033[1;33m'; green=$'\033[1;32m'; red=$'\033[1;31m'; off=$'\033[0m'
results=()
have() { command -v "$1" >/dev/null 2>&1; }
in_dir() { local d="$1"; shift; (cd "$d" && "$@"); }

run() {
  local name="$1"; shift
  printf '\n%s━━━ %s ━━━%s\n' "$blue" "$name" "$off"
  if "$@"; then
    results+=("${green}PASS${off}  $name")
  else
    results+=("${red}FAIL${off}  $name")
  fi
}
skip() {
  printf '\n%s━━━ SKIP: %s ━━━%s\n' "$yellow" "$1" "$off"
  results+=("${yellow}SKIP${off}  $1")
}

# --- make sure node deps are present (cheap no-op if already installed) ---
if have pnpm && [ ! -d desktop/node_modules ]; then
  run "desktop: pnpm install" in_dir desktop pnpm install --frozen-lockfile
fi
if have npm && [ ! -d apps/web/end2end/node_modules ]; then
  run "web e2e: npm ci" in_dir apps/web/end2end npm ci
fi

# --- lint / format / build gates ---
if [ "$run_lint" = 1 ]; then
  run "cargo fmt --check" cargo fmt --check
  run "clippy (ssr)" cargo clippy --workspace --all-targets --features ssr -- -D warnings
  run "clippy (wasm/hydrate)" cargo clippy -p screen_share --target wasm32-unknown-unknown \
    --all-targets --no-default-features --features hydrate -- -D warnings
fi
if have cargo-leptos; then
  run "cargo leptos build" cargo leptos build
else
  skip "cargo leptos build (cargo-leptos not installed)"
fi

# --- Rust tests ---
run "cargo test (workspace, ssr)" cargo test --workspace --features ssr
run "wasm-bindgen tests (hydrate)" bash scripts/test-wasm.sh

# --- Rust mutation ---
if [ "$run_mutants" = 1 ]; then
  if have cargo-mutants; then
    run "cargo-mutants (protocol + signaling)" \
      cargo mutants -p screen-share-protocol -p screen-share-signaling
  else
    skip "cargo-mutants (not installed: cargo install cargo-mutants)"
  fi
fi
if [ "$run_web_mutants" = 1 ]; then
  if have cargo-mutants; then
    run "cargo-mutants (apps/web, slow)" cargo mutants --features ssr -p screen_share
  else
    skip "cargo-mutants apps/web (not installed)"
  fi
fi

# --- Rust coverage ---
if [ "$run_coverage" = 1 ]; then
  if have cargo-llvm-cov; then
    # Drop files that can't move the needle (kept in sync with the
    # `test-web` job in .github/workflows/ci-cd.yml): debug-only bench,
    # SVG constants, entry-point wiring.
    cov_ignore='apps/web/src/(main|app)\.rs$|features/room/dev_preview\.rs$|components/icons\.rs$'
    run "cargo-llvm-cov (workspace, ssr)" \
      cargo llvm-cov --workspace --features ssr --lcov --output-path lcov-rust.info \
      --ignore-filename-regex "$cov_ignore"
  else
    skip "cargo-llvm-cov (not installed: cargo install cargo-llvm-cov)"
  fi
fi

# --- desktop (Electron) ---
if have pnpm; then
  run "desktop: biome check" in_dir desktop pnpm run check
  run "desktop: tsc build" in_dir desktop pnpm run build
  if [ "$run_coverage" = 1 ]; then
    run "desktop: vitest + coverage" in_dir desktop pnpm run test:cov
  else
    run "desktop: vitest" in_dir desktop pnpm run test
  fi
  run "desktop: StrykerJS mutation" in_dir desktop pnpm run test:mutation
else
  skip "desktop suite (pnpm not installed)"
fi

# --- windows-audio napi (Windows only) ---
case "$(uname -s 2>/dev/null || echo unknown)" in
  *NT* | MINGW* | MSYS* | CYGWIN*)
    run "windows-audio: cargo test" in_dir desktop/native/windows-audio cargo test ;;
  *)
    skip "windows-audio cargo test (Windows-only crate)" ;;
esac

# --- E2E (Playwright) ---
if [ "$run_e2e" = 1 ]; then
  xvfb=()
  if [ -z "${DISPLAY:-}" ]; then
    if have xvfb-run; then xvfb=(xvfb-run -a); else
      skip "e2e-web (no \$DISPLAY and no xvfb-run)"
      skip "e2e-desktop (no \$DISPLAY and no xvfb-run)"
    fi
  fi
  if [ -n "${DISPLAY:-}" ] || [ "${#xvfb[@]}" -gt 0 ]; then
    if have npm; then
      run "e2e-web (Playwright)" in_dir apps/web/end2end "${xvfb[@]}" npm test
    else
      skip "e2e-web (npm not installed)"
    fi
    if have pnpm; then
      run "e2e-desktop (Playwright _electron)" in_dir desktop "${xvfb[@]}" pnpm run test:e2e
    else
      skip "e2e-desktop (pnpm not installed)"
    fi
  fi
fi

# --- summary ---
printf '\n%s══════ summary ══════%s\n' "$bold" "$off"
failed=0
for r in "${results[@]}"; do
  printf '  %b\n' "$r"
  [[ "$r" == *FAIL* ]] && failed=1
done
if [ "$failed" = 1 ]; then
  printf '\n%sSome checks FAILED.%s\n' "$red" "$off"
  exit 1
fi
printf '\n%sAll checks passed.%s\n' "$green" "$off"
