import { Menu, Tray } from 'electron';
import * as path from 'path';

import { requestQuit } from './lifecycle.js';
import { showMainWindow, startQuickShare } from './main-window.js';

let tray: Tray | null = null;

export function createTray(): void {
  const iconPath = path.join(__dirname, '..', 'icons', 'tray-icon.png');
  tray = new Tray(iconPath);
  tray.setToolTip('Screen Share');

  const menu = Menu.buildFromTemplate([
    { label: 'Abrir', click: showMainWindow },
    { label: 'Compartilhar tela', click: startQuickShare },
    { label: 'Sair', click: requestQuit },
  ]);
  tray.setContextMenu(menu);
  tray.on('click', showMainWindow);
}
