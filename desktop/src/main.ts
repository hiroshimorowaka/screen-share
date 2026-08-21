import { app, BrowserWindow, Tray, Menu } from 'electron';
import * as path from 'path';

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let isQuitting = false;

function createMainWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
  });
  mainWindow.loadURL(PROD_URL);

  mainWindow.on('close', (event) => {
    if (!isQuitting) {
      event.preventDefault();
      mainWindow?.hide();
    }
  });
}

function showMainWindow(): void {
  mainWindow?.show();
  mainWindow?.focus();
}

function createTray(): void {
  const iconPath = path.join(__dirname, '..', 'icons', 'tray-icon.png');
  tray = new Tray(iconPath);
  tray.setToolTip('Screen Share');

  const menu = Menu.buildFromTemplate([
    { label: 'Abrir', click: showMainWindow },
    {
      label: 'Sair',
      click: () => {
        isQuitting = true;
        app.quit();
      },
    },
  ]);
  tray.setContextMenu(menu);
  tray.on('click', showMainWindow);
}

app.on('before-quit', () => {
  isQuitting = true;
});

app.whenReady().then(() => {
  createMainWindow();
  createTray();
});
