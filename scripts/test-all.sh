#!/usr/bin/env bash
# Test runner for the project — runs a chosen group of checks (the same
# set CI runs, plus a few that only run in the scheduled workflow),
# collects failures, prints a summary, exits non-zero if anything failed.
# Missing optional tools are SKIPped, not failed.
#
#   scripts/test-all.sh [target] [options]
#
# Targets (pick one; default `all`):
#   all            every group below, minus whatever --no-* flags remove
#   lint           cargo fmt --check + clippy (ssr + wasm/hydrate) + cargo-machete
#   build          cargo leptos build
#   rust           cargo test (workspace, ssr) + wasm-bindgen (hydrate)
#   wasm           wasm-bindgen (hydrate) suite only
#   mutants        cargo-mutants (protocol + signaling); + apps/web with --web-mutants
#   desktop        desktop: biome + knip + tsc + vitest + StrykerJS
#   e2e            both Playwright suites (web + desktop)
#   e2e-web        Playwright web suite only
#   e2e-desktop    Playwright _electron suite only
#
# Options:
#   --no-lint          (target all) skip fmt + clippy
#   --no-mutants       (target all) skip cargo-mutants (protocol + signaling, ~4 min)
#   --no-e2e           (target all) skip the Playwright suites
#   --no-xvfb          run the Playwright/Electron windows on the real
#                      $DISPLAY (visible) instead of hidden under xvfb-run
#   --web-mutants      also run cargo-mutants on apps/web (VERY slow)
#   --coverage         also produce coverage (cargo-llvm-cov + vitest)
#   --quick            = all --no-lint --no-mutants --no-e2e
#
# E2E needs a display. By default the suites run hidden under `xvfb-run`
# (in-memory X server — no window on screen, no focus stealing) whenever
# it is installed. `--no-xvfb` runs them on the real $DISPLAY instead.
# With neither `xvfb-run` nor a $DISPLAY available, the two suites SKIP.

set -o pipefail
cd "$(dirname "$0")/.."

target=all target_set=0
run_lint=1 run_mutants=1 run_e2e=1 run_web_mutants=0 run_coverage=0 use_xvfb=1
for arg in "$@"; do
  case "$arg" in
    --no-lint) run_lint=0 ;;
    --no-mutants) run_mutants=0 ;;
    --no-e2e) run_e2e=0 ;;
    --no-xvfb) use_xvfb=0 ;;
    --web-mutants) run_web_mutants=1 ;;
    --coverage) run_coverage=1 ;;
    --quick) run_lint=0 run_mutants=0 run_e2e=0 ;;
    -h | --help) sed -n '2,${/^#/!q;s/^# \{0,1\}//p;}' "$0"; exit 0 ;;
    -*) echo "unknown option: $arg" >&2; exit 2 ;;
    all | lint | build | rust | wasm | mutants | desktop | e2e | e2e-web | e2e-desktop)
      if [ "$target_set" = 1 ]; then
        echo "more than one target given: $target, $arg" >&2; exit 2
      fi
      target="$arg" target_set=1 ;;
    *) echo "unknown target: $arg (see --help)" >&2; exit 2 ;;
  esac
done

# True when the chosen target selects the named check group.
want() {
  local key="$1"
  case "$target" in
    all)     return 0 ;;
    rust)    [ "$key" = rust-test ] || [ "$key" = wasm ] ;;
    e2e)     [ "$key" = e2e-web ] || [ "$key" = e2e-desktop ] ;;
    mutants) [ "$key" = mutants ] || [ "$key" = web-mutants ] ;;
    *)       [ "$key" = "$target" ] ;;
  esac
}

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
if want desktop || want e2e-web || want e2e-desktop; then
  if have pnpm && [ ! -d desktop/node_modules ]; then
    run "desktop: pnpm install" in_dir desktop pnpm install --frozen-lockfile
  fi
  if have npm && [ ! -d apps/web/end2end/node_modules ]; then
    run "web e2e: npm ci" in_dir apps/web/end2end npm ci
  fi
fi

# --- lint / format ---
if want lint && [ "$run_lint" = 1 ]; then
  run "cargo fmt --check" cargo fmt --check
  run "clippy (ssr)" cargo clippy --workspace --all-targets --features ssr -- -D warnings
  run "clippy (wasm/hydrate)" cargo clippy -p screen_share --target wasm32-unknown-unknown \
    --all-targets --no-default-features --features hydrate -- -D warnings
  if have cargo-machete; then
    run "cargo-machete (unused deps)" cargo machete
  else
    skip "cargo-machete (not installed: cargo install cargo-machete)"
  fi
fi

# --- build gate ---
if want build; then
  if have cargo-leptos; then
    run "cargo leptos build" cargo leptos build
  else
    skip "cargo leptos build (cargo-leptos not installed)"
  fi
fi

# --- Rust tests ---
if want rust-test; then
  run "cargo test (workspace, ssr)" cargo test --workspace --features ssr
fi
if want wasm; then
  run "wasm-bindgen tests (hydrate)" bash scripts/test-wasm.sh
fi

# --- Rust mutation ---
if want mutants && [ "$run_mutants" = 1 ]; then
  if have cargo-mutants; then
    run "cargo-mutants (protocol + signaling)" \
      cargo mutants -p screen-share-protocol -p screen-share-signaling
  else
    skip "cargo-mutants (not installed: cargo install cargo-mutants)"
  fi
fi
if want web-mutants && [ "$run_web_mutants" = 1 ]; then
  if have cargo-mutants; then
    run "cargo-mutants (apps/web, slow)" cargo mutants --features ssr -p screen_share
  else
    skip "cargo-mutants apps/web (not installed)"
  fi
fi

# --- Rust coverage ---
if want rust-test && [ "$run_coverage" = 1 ]; then
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
if want desktop; then
  if have pnpm; then
    run "desktop: biome check" in_dir desktop pnpm run check
    run "desktop: knip (unused files/exports/deps)" in_dir desktop pnpm run check:unused
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
fi

# --- windows-audio napi (Windows only) ---
if want windows-audio; then
  case "$(uname -s 2>/dev/null || echo unknown)" in
    *NT* | MINGW* | MSYS* | CYGWIN*)
      run "windows-audio: cargo test" in_dir desktop/native/windows-audio cargo test ;;
    *)
      skip "windows-audio cargo test (Windows-only crate)" ;;
  esac
fi

# --- E2E (Playwright) ---
# Default: hide the browser/Electron windows on an in-memory X display so
# they never appear on screen or steal focus. `--no-xvfb` (or no xvfb-run
# installed) runs them on the real $DISPLAY instead; with neither, SKIP.
if { want e2e-web || want e2e-desktop; } && [ "$run_e2e" = 1 ]; then
  xvfb=()
  if { [ "$use_xvfb" = 1 ] || [ -z "${DISPLAY:-}" ]; } && have xvfb-run; then
    xvfb=(xvfb-run -a)
  fi
  if [ -n "${DISPLAY:-}" ] || [ "${#xvfb[@]}" -gt 0 ]; then
    if want e2e-web; then
      if have npm; then
        run "e2e-web (Playwright)" in_dir apps/web/end2end "${xvfb[@]}" npm test
      else
        skip "e2e-web (npm not installed)"
      fi
    fi
    if want e2e-desktop; then
      if have pnpm; then
        run "e2e-desktop (Playwright _electron)" in_dir desktop "${xvfb[@]}" pnpm run test:e2e
      else
        skip "e2e-desktop (pnpm not installed)"
      fi
    fi
  else
    want e2e-web && skip "e2e-web (no \$DISPLAY and no xvfb-run)"
    want e2e-desktop && skip "e2e-desktop (no \$DISPLAY and no xvfb-run)"
  fi
fi

# --- summary ---
printf '\n%s══════ summary ══════%s\n' "$bold" "$off"
if [ "${#results[@]}" -eq 0 ]; then
  printf '  (nothing ran for target %q)\n' "$target"
  exit 0
fi
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
