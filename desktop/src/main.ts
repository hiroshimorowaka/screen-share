import { app, Menu } from 'electron';

import { registerAudioIpcHandlers, stopAudioLoopbackNow } from './audio/ipc-handlers.js';
import { registerDisplayMediaHandler } from './display-media-handler.js';
import { markQuitting } from './lifecycle.js';
import { createMainWindow } from './main-window.js';
import { createTray } from './tray.js';

app.on('before-quit', () => {
  // Synchronous and already platform-resolved (see
  // `stopAudioLoopbackNow`) — `before-quit` doesn't wait for promises a
  // listener returns or kicks off, so resolving the backend here via a
  // fresh `await import(...)` could lose the race against the process
  // actually exiting.
  stopAudioLoopbackNow();
  markQuitting();
});

app.whenReady().then(async () => {
  Menu.setApplicationMenu(null);
  createMainWindow();
  createTray();
  await registerAudioIpcHandlers();
  await registerDisplayMediaHandler();
});
