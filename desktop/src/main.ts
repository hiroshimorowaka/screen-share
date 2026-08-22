import { ChildProcess, spawn } from 'child_process';
import { app, BrowserWindow, desktopCapturer, ipcMain, Menu, session, Tray } from 'electron';
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
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
    },
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

let audioLoopback: ChildProcess | null = null;

function stopAudioLoopback(): void {
  if (audioLoopback) {
    audioLoopback.kill();
    audioLoopback = null;
  }
}

function isLoopbackDevicePresent(): Promise<boolean> {
  return new Promise((resolve) => {
    const dump = spawn('pw-dump');
    let output = '';
    dump.stdout.on('data', (chunk: Buffer) => {
      output += chunk.toString();
    });
    dump.on('close', () => {
      resolve(output.includes('"node.name": "screen_share_audio"'));
    });
    dump.on('error', () => resolve(false));
  });
}

async function waitForLoopbackDevice(timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await isLoopbackDevicePresent()) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('Timed out waiting for the audio loopback device to appear');
}

ipcMain.handle('start-audio-loopback', async () => {
  if (audioLoopback) return;
  // `media.class=Audio/Source` must be on the *playback* side, not the
  // capture side — that's the node other apps actually see and select
  // from. Getting this backwards (as an earlier version of this code
  // did) still creates a selectable, correctly-named device, but one
  // that carries pure silence: confirmed by recording it directly with
  // pw-record while audio played, bypassing this app entirely.
  audioLoopback = spawn('pw-loopback', [
    '-C', '@DEFAULT_SINK@',
    '--capture-props', 'stream.capture.sink=true node.passive=true',
    '--playback-props', 'media.class=Audio/Source node.name=screen_share_audio node.description="Screen Share Audio"',
  ]);
  audioLoopback.on('exit', () => {
    audioLoopback = null;
  });
  try {
    await waitForLoopbackDevice(3000);
  } catch (err) {
    stopAudioLoopback();
    throw err;
  }
});

ipcMain.handle('stop-audio-loopback', () => {
  stopAudioLoopback();
});

app.on('before-quit', () => {
  stopAudioLoopback();
  isQuitting = true;
});

interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
  iconDataUrl: string | null;
}

function showSourcePicker(): Promise<Electron.DesktopCapturerSource | null> {
  return new Promise((resolve) => {
    void (async () => {
      const sources = await desktopCapturer.getSources({
        types: ['screen', 'window'],
        thumbnailSize: { width: 300, height: 200 },
        fetchWindowIcons: true,
      });

      const pickerSources: PickerSource[] = sources.map((s) => ({
        id: s.id,
        name: s.name,
        thumbnailDataUrl: s.thumbnail.toDataURL(),
        iconDataUrl: s.appIcon && !s.appIcon.isEmpty() ? s.appIcon.toDataURL() : null,
      }));

      const pickerWindow = new BrowserWindow({
        width: 1000,
        height: 720,
        parent: mainWindow ?? undefined,
        frame: false,
        transparent: true,
        resizable: true,
        minWidth: 640,
        minHeight: 480,
        skipTaskbar: true,
        webPreferences: {
          preload: path.join(__dirname, 'preload.js'),
        },
      });

      let settled = false;
      const settle = (id: string | null) => {
        if (settled) return;
        settled = true;
        resolve(id ? sources.find((s) => s.id === id) ?? null : null);
        if (!pickerWindow.isDestroyed()) pickerWindow.close();
      };

      ipcMain.once('picker:selected', (_event, id: string) => settle(id));
      pickerWindow.on('closed', () => settle(null));

      // Delay arming "click outside closes it" slightly so the window
      // manager focusing this new window doesn't itself trigger a blur.
      setTimeout(() => {
        pickerWindow.on('blur', () => settle(null));
      }, 300);

      await pickerWindow.loadFile(
        path.join(__dirname, '..', 'static', 'picker.html'),
      );
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}

app.whenReady().then(() => {
  Menu.setApplicationMenu(null);
  createMainWindow();
  createTray();

  session.defaultSession.setDisplayMediaRequestHandler(
    async (_request, callback) => {
      const chosen = await showSourcePicker();
      callback(chosen ? { video: chosen } : {});
    },
  );
});
