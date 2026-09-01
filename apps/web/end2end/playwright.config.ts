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
  // The fake device's audio track is a 1 kHz tone; the room tests pipe it
  // through a real peer connection and the watcher's <video> plays it out
  // loud when the run is headed. No test asserts on audible audio.
  '--mute-audio',
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
  projects: [
    // The room / home flows, on a normal desktop viewport (the historical
    // default). Everything except the touch-specific suite.
    {
      name: 'desktop',
      testIgnore: /room-mobile\.spec\.ts/,
    },
    // A phone-sized, touch-input viewport for the mobile behaviour:
    // patch -> focus, tap toggles the chrome, the bottom-sheet menus,
    // finger-sized targets. Same fake-media Chrome args (inherited from
    // `use` above); `isMobile` is Chromium-only, which is all this suite
    // runs on.
    {
      name: 'mobile-web',
      testMatch: /room-mobile\.spec\.ts/,
      use: {
        viewport: { width: 390, height: 844 },
        hasTouch: true,
        isMobile: true,
        userAgent:
          'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) ' +
          'AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
      },
    },
  ],
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
