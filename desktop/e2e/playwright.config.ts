import { defineConfig } from '@playwright/test';

// `_electron` launches the built app (`dist/main/index.js`) — no
// `webServer`, no browser download. Headed by nature; CI runs the whole
// job under `xvfb-run`. One worker: each spec launches its own Electron
// process and they must not fight over the single system tray / audio.
export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.ts',
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : [['list']],
  use: {
    trace: 'retain-on-failure',
  },
});
