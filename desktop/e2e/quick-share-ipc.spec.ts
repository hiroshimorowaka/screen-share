import { _electron as electron, expect, test } from '@playwright/test';

// A real https origin (never actually loaded — the E2E only drives the
// main-process IPC handlers) so `link-ready`'s invite-link check has a
// valid `/r/` URL to accept (P3 follow-up).
const APP_URL = 'https://screenshare.test/';
const APP_ORIGIN = 'https://screenshare.test';

const launchOptions = {
  args: ['dist/main/index.js'],
  env: { ...process.env, SCREEN_SHARE_URL: APP_URL },
};

// The IPC handlers reject a message unless it comes from a frame on the
// app's origin (finding F11).
const TRUSTED_FRAME = { senderFrame: { url: `${APP_ORIGIN}/room` } };
const INVITE_LINK = `${APP_ORIGIN}/r/ABCD`;

test('desktop-share:link-ready copies the invite link to the clipboard', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  const copied = await app.evaluate(
    ({ ipcMain, clipboard }, { frame, link }) => {
      clipboard.writeText('stale');
      // The handler `registerQuickShareIpcHandlers` bound synchronously
      // calls `clipboard.writeText(link)`.
      ipcMain.emit('desktop-share:link-ready', frame, link);
      return clipboard.readText();
    },
    { frame: TRUSTED_FRAME, link: INVITE_LINK },
  );

  expect(copied).toBe(INVITE_LINK);
  await app.close();
});

test('desktop-share:link-ready ignores a link that is not a room invite on this origin (P3)', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  const copied = await app.evaluate(({ ipcMain, clipboard }, frame) => {
    clipboard.writeText('stale');
    ipcMain.emit('desktop-share:link-ready', frame, 'https://evil.example/r/ABCD');
    return clipboard.readText();
  }, TRUSTED_FRAME);

  expect(copied).toBe('stale');
  await app.close();
});

test('desktop-share:link-ready ignores a message from an untrusted frame (F11)', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  const copied = await app.evaluate(({ ipcMain, clipboard }) => {
    clipboard.writeText('stale');
    ipcMain.emit(
      'desktop-share:link-ready',
      { senderFrame: { url: 'https://evil.example/steal' } },
      'https://evil.example/r/PWNED',
    );
    return clipboard.readText();
  });

  expect(copied).toBe('stale');
  await app.close();
});

test('desktop-share:member-joined runs without throwing (notification path)', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  const threw = await app.evaluate(({ ipcMain }, frame) => {
    try {
      ipcMain.emit('desktop-share:member-joined', frame, 'Bia');
      return false;
    } catch {
      return true;
    }
  }, TRUSTED_FRAME);

  expect(threw).toBe(false);
  await app.close();
});

test('before-quit marks the app as quitting so the window really closes', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  // `main/index.ts`'s before-quit handler calls markQuitting(); after that
  // the window's own close handler must not veto the close.
  await app.evaluate(({ app: electronApp }) => electronApp.emit('before-quit'));

  const vetoed = await app.evaluate(({ BrowserWindow }) => {
    const win = BrowserWindow.getAllWindows()[0];
    if (!win) return false;
    let prevented = false;
    win.emit('close', {
      preventDefault: () => {
        prevented = true;
      },
    });
    return prevented;
  });

  expect(vetoed).toBe(false);
  await app.close();
});
