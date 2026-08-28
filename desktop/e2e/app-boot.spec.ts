import { _electron as electron, expect, test } from '@playwright/test';

// A blank page keeps the boot deterministic — the real app loads a
// production URL (overridable via SCREEN_SHARE_URL, see main/window.ts).
const launchOptions = {
  args: ['dist/main/index.js'],
  env: { ...process.env, SCREEN_SHARE_URL: 'about:blank' },
};

test('the app boots, creates its (hidden) main window, and quits cleanly', async () => {
  const app = await electron.launch(launchOptions);

  await app.firstWindow();

  const { title, visible } = await app.evaluate(({ BrowserWindow }) => {
    const win = BrowserWindow.getAllWindows()[0];
    return { title: win?.getTitle(), visible: win?.isVisible() };
  });
  // The OS-level window title comes from the `title` option until the
  // loaded page overrides it (about:blank never does).
  expect(title).toBe('Screen Share');
  // The window starts hidden (tray app) — it only appears on "Abrir".
  expect(visible).toBe(false);

  // No hard failure surfaced on the main process's stderr during boot.
  // (System-tray / GPU warnings under xvfb are expected and ignored.)
  let stderr = '';
  app.process().stderr?.on('data', (d) => {
    stderr += d.toString();
  });

  await app.close();
  expect(stderr).not.toMatch(/Uncaught|TypeError|ReferenceError|Cannot find module/);
});

test('the main process registered the audio-loopback IPC handlers', async () => {
  const app = await electron.launch(launchOptions);
  const window = await app.firstWindow();

  // The preload bridge forwards to `ipcRenderer.invoke('stop-audio-loopback')`.
  // If the handler weren't registered, invoke() rejects with
  // "No handler registered for 'stop-audio-loopback'".
  await expect(
    window.evaluate(() =>
      (window as unknown as { desktopAudio: { stop: () => Promise<unknown> } }).desktopAudio.stop(),
    ),
  ).resolves.toBeUndefined();

  await app.close();
});
