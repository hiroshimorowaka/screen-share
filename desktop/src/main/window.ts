import * as path from 'node:path';
import { app, BrowserWindow, shell } from 'electron';

import { stopAudioLoopbackNow } from '#features/audio-share/ipc.js';
import { APP_ORIGIN, APP_URL } from '#main/app-url.js';
import { isQuitting } from '#main/lifecycle.js';

let mainWindow: BrowserWindow | null = null;

/** Keeps the privileged renderer pinned to the app's own origin. A hijack
 * or open-redirect on that origin could otherwise navigate it to
 * attacker content that then reaches the IPC bridges (finding F10). SPA
 * routing uses `history.pushState` and `loadURL` from the main process
 * (see `startQuickShare`) — neither fires `will-navigate` — so this only
 * ever blocks a real cross-origin navigation. */
export function lockNavigation(window: BrowserWindow): void {
  const staysOnAppOrigin = (target: string): boolean => {
    try {
      return new URL(target).origin === APP_ORIGIN;
    } catch {
      return false;
    }
  };

  window.webContents.on('will-navigate', (event, url) => {
    if (!staysOnAppOrigin(url)) event.preventDefault();
  });
  window.webContents.on('will-redirect', (event, url) => {
    if (!staysOnAppOrigin(url)) event.preventDefault();
  });
  window.webContents.setWindowOpenHandler(({ url }) => {
    // Never spawn a second renderer; hand real links to the OS browser.
    if (/^https?:\/\//.test(url)) void shell.openExternal(url);
    return { action: 'deny' };
  });
}

/** Dev-only. The app ships with no application menu
 * (`Menu.setApplicationMenu(null)` in `index.ts`), which also strips the
 * default DevTools accelerators, so a development build has no way to open
 * them. Re-bind F12 and Ctrl+Shift+I by hand. A packaged build never gets
 * here and also has `devTools` turned off in `webPreferences`. */
function enableDevToolsShortcuts(window: BrowserWindow): void {
  window.webContents.on('before-input-event', (_event, input) => {
    if (input.type !== 'keyDown') return;
    const toggle =
      input.key === 'F12' || (input.control && input.shift && input.key.toLowerCase() === 'i');
    if (toggle) window.webContents.toggleDevTools();
  });
}

/** The renderer's own teardown (`stop_desktop_audio_loopback` in
 * `webrtc.rs`) is the normal path, but a quick-share `loadURL`, a manual
 * reload, or a renderer crash never runs it — leaving `pw-loopback` / the
 * WASAPI capture, its 1 s poll interval and its `pw-link`s orphaned, and
 * on Windows `desktop-audio-pcm-chunk` firing ~50x/s at a dead frame
 * (finding 7). Stop the loopback whenever the main frame goes away.
 * `stopAudioLoopbackNow` is a safe no-op when nothing is running. */
function stopLoopbackOnRendererGone(window: BrowserWindow): void {
  const { webContents } = window;
  webContents.on('did-start-navigation', (details) => {
    if (details.isMainFrame && !details.isSameDocument) stopAudioLoopbackNow();
  });
  webContents.on('destroyed', () => stopAudioLoopbackNow());
  webContents.on('render-process-gone', () => stopAudioLoopbackNow());
}

export function createMainWindow(): void {
  const devMode = !app.isPackaged;

  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
    // The app starts tucked away in the tray — nothing shows until the
    // user picks "Abrir" or triggers a share from the tray menu.
    show: false,
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload.js'),
      // Pinned explicitly rather than relying on the Electron defaults —
      // this window loads a remote origin, so the isolation boundary must
      // not silently change under a future Electron upgrade (finding F10).
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      webSecurity: true,
      // Off in production as a hard backstop; a dev run re-enables it here
      // and rebinds the toggle shortcuts below.
      devTools: devMode,
    },
  });
  lockNavigation(mainWindow);
  stopLoopbackOnRendererGone(mainWindow);
  if (devMode) enableDevToolsShortcuts(mainWindow);
  mainWindow.loadURL(APP_URL);

  mainWindow.on('close', (event) => {
    if (!isQuitting()) {
      event.preventDefault();
      mainWindow?.hide();
    }
  });
}

export function showMainWindow(): void {
  mainWindow?.show();
  mainWindow?.focus();
}

/** The tray's "Compartilhar tela" action: reuses the (possibly hidden)
 * main window to create a room with a random name, join it, and start
 * sharing with no dialog ever shown other than the display picker itself
 * — see `quick_share.rs` on the web app side for the flow this URL flag
 * kicks off. The window is left exactly as visible/hidden as it already
 * was; it only becomes visible if the user separately opens it. */
export function startQuickShare(): void {
  if (!mainWindow) return;
  const url = new URL(APP_URL);
  url.searchParams.set('quick_share', '1');
  mainWindow.loadURL(url.toString());
}

/** Used as the picker window's `parent` so it stays above the main
 * window and closes if the main window does. */
export function getMainWindow(): BrowserWindow | null {
  return mainWindow;
}
