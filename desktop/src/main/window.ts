import { BrowserWindow } from 'electron';
import * as path from 'path';

import { isQuitting } from '#main/lifecycle.js';

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

let mainWindow: BrowserWindow | null = null;

export function createMainWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
    // The app starts tucked away in the tray — nothing shows until the
    // user picks "Abrir" or triggers a share from the tray menu.
    show: false,
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload.js'),
    },
  });
  mainWindow.loadURL(PROD_URL);

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
  const url = new URL(PROD_URL);
  url.searchParams.set('quick_share', '1');
  mainWindow.loadURL(url.toString());
}

/** Used as the picker window's `parent` so it stays above the main
 * window and closes if the main window does. */
export function getMainWindow(): BrowserWindow | null {
  return mainWindow;
}
