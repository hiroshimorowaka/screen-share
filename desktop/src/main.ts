import { app, Menu } from 'electron';

import { registerAudioIpcHandlers } from './audio/ipc-handlers.js';
import { stopAudioLoopback } from './audio/loopback-session.js';
import { registerDisplayMediaHandler } from './display-media-handler.js';
import { markQuitting } from './lifecycle.js';
import { createMainWindow } from './main-window.js';
import { createTray } from './tray.js';

app.on('before-quit', () => {
  stopAudioLoopback();
  markQuitting();
});

app.whenReady().then(() => {
  Menu.setApplicationMenu(null);
  createMainWindow();
  createTray();
  registerAudioIpcHandlers();
  registerDisplayMediaHandler();
});
