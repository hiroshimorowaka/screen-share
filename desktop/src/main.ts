import { app, BrowserWindow } from 'electron';

const PROD_URL = 'https://screen-share-h0rb5w.fly.dev/';

function createMainWindow(): void {
  const mainWindow = new BrowserWindow({
    width: 1100,
    height: 750,
    title: 'Screen Share',
  });
  mainWindow.loadURL(PROD_URL);
}

app.whenReady().then(() => {
  createMainWindow();
});
