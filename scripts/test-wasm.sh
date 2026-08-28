#!/usr/bin/env bash
# Run the apps/web WASM (`hydrate`) test suite in a headless browser.
#
# `wasm-bindgen-test-runner` (wired up in .cargo/config.toml) needs a
# WebDriver binary plus a browser. If `chromedriver` is already on PATH
# this just uses it; otherwise it downloads a version-matched
# chrome-headless-shell + chromedriver via `@puppeteer/browsers` into
# ./.wasm-browser/ (git-ignored) and points the runner at them.
#
# Extra args are forwarded to `cargo test` (e.g. a test-name filter).
set -euo pipefail
cd "$(dirname "$0")/.."

wbg_version="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"/\1/p;}' Cargo.lock)"
if ! command -v wasm-bindgen-test-runner >/dev/null 2>&1; then
  echo "==> installing wasm-bindgen-cli ${wbg_version} (provides wasm-bindgen-test-runner)"
  cargo install wasm-bindgen-cli --version "${wbg_version}" --locked
fi

test_cmd=(cargo test -p screen_share --target wasm32-unknown-unknown
  --no-default-features --features hydrate --lib "$@")

if command -v chromedriver >/dev/null 2>&1; then
  echo "==> using system chromedriver: $(command -v chromedriver)"
  exec "${test_cmd[@]}"
fi

cache="${PWD}/.wasm-browser"
echo "==> no system chromedriver — fetching a pinned headless Chrome into ${cache}/"
chrome_bin="$(npx --yes @puppeteer/browsers install 'chrome-headless-shell@stable' --path "${cache}" 2>/dev/null | tail -1)"
driver_bin="$(npx --yes @puppeteer/browsers install 'chromedriver@stable' --path "${cache}" 2>/dev/null | tail -1)"
chrome_bin="${chrome_bin##* }"
driver_bin="${driver_bin##* }"

cat > webdriver.json <<EOF
{ "goog:chromeOptions": { "binary": "${chrome_bin}",
  "args": ["--headless=new", "--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage"] } }
EOF

export CHROMEDRIVER="${driver_bin}"
export WASM_BINDGEN_TEST_WEBDRIVER_JSON="${PWD}/webdriver.json"
export WASM_BINDGEN_TEST_TIMEOUT="${WASM_BINDGEN_TEST_TIMEOUT:-120}"
exec "${test_cmd[@]}"
