import { BrowserWindow } from 'electron';
import * as path from 'path';

import { isQuitting } from './lifecycle.js';

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

let mainWindow: BrowserWindow | null = null;

export function createMainWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
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

/** Used as the picker window's `parent` so it stays above the main
 * window and closes if the main window does. */
export function getMainWindow(): BrowserWindow | null {
  return mainWindow;
}
