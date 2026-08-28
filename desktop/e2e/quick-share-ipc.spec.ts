import { _electron as electron, expect, test } from '@playwright/test';

const launchOptions = {
  args: ['dist/main/index.js'],
  env: { ...process.env, SCREEN_SHARE_URL: 'about:blank' },
};

test('desktop-share:link-ready copies the invite link to the clipboard', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  const copied = await app.evaluate(({ ipcMain, clipboard }) => {
    clipboard.writeText('stale');
    // The handler `registerQuickShareIpcHandlers` bound synchronously
    // calls `clipboard.writeText(link)`.
    ipcMain.emit('desktop-share:link-ready', {}, 'https://example.com/r/ABCD');
    return clipboard.readText();
  });

  expect(copied).toBe('https://example.com/r/ABCD');
  await app.close();
});

test('desktop-share:member-joined runs without throwing (notification path)', async () => {
  const app = await electron.launch(launchOptions);
  await app.firstWindow();

  const threw = await app.evaluate(({ ipcMain }) => {
    try {
      ipcMain.emit('desktop-share:member-joined', {}, 'Bia');
      return false;
    } catch {
      return true;
    }
  });

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
