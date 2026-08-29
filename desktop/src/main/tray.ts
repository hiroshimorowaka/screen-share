import * as path from 'node:path';
import { ipcMain, Menu, Tray } from 'electron';

import { requestQuit } from '#main/lifecycle.js';
import { showMainWindow, startQuickShare } from '#main/window.js';

let tray: Tray | null = null;

const iconPath = (name: string): string => path.join(__dirname, '..', '..', 'icons', name);

/** Idle = green dot, live = red dot — the tray icon is the app's on-air
 * indicator (see `scripts/gen-tray-icons.mjs`). Driven by the room page
 * through the `desktop-share:sharing-changed` IPC channel. */
function setTrayLive(live: boolean): void {
  tray?.setImage(iconPath(live ? 'tray-live.png' : 'tray-idle.png'));
  tray?.setToolTip(live ? 'Screen Share — transmitindo' : 'Screen Share');
}

export function createTray(): void {
  tray = new Tray(iconPath('tray-idle.png'));
  tray.setToolTip('Screen Share');

  const menu = Menu.buildFromTemplate([
    { label: 'Abrir', click: showMainWindow },
    { label: 'Compartilhar tela', click: startQuickShare },
    { label: 'Sair', click: requestQuit },
  ]);
  tray.setContextMenu(menu);
  tray.on('click', showMainWindow);

  ipcMain.on('desktop-share:sharing-changed', (_event, isSharing: boolean) => {
    setTrayLive(Boolean(isSharing));
  });
}
