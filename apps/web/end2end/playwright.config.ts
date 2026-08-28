import { defineConfig } from '@playwright/test';

// The room flows need WebRTC with predictable media. `getDisplayMedia`
// only works in a headed browser, so the whole suite runs headed — under
// `xvfb-run` in CI (see the `e2e-web` job). The fake-device flags hand
// `getUserMedia`/`getDisplayMedia` a synthetic stream with no permission
// prompt and no OS picker; `--auto-select-desktop-capture-source` picks a
// capture target by title. Flag names verified against the Chromium
// build Playwright 1.62 bundles.
const chromeArgs = [
  '--use-fake-device-for-media-stream',
  '--use-fake-ui-for-media-stream',
  '--auto-select-desktop-capture-source=Entire screen',
  '--auto-accept-this-tab-capture',
];

const PORT = 3000;

export default defineConfig({
  testDir: './tests',
  fullyParallel: false, // one shared leptos server + one signaling registry
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 10_000 },
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : [['list']],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    headless: false,
    launchOptions: { args: chromeArgs },
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
  },
  webServer: {
    // `cargo leptos serve` builds (if needed) then serves the SSR binary
    // + wasm bundle the same way production does.
    command: 'cargo leptos serve',
    cwd: '../../..',
    url: `http://127.0.0.1:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 240_000, // cold `cargo leptos` build
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
