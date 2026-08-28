import { app, Menu } from 'electron';

import { registerAudioIpcHandlers, stopAudioLoopbackNow } from '../features/audio-share/ipc.js';
import { registerDisplayMediaHandler } from '../features/screen-share/display-media.js';
import { markQuitting } from './lifecycle.js';
import { createMainWindow } from './window.js';
import { registerQuickShareIpcHandlers } from '../features/screen-share/quick-share.js';
import { createTray } from './tray.js';

app.on('before-quit', () => {
  // Synchronous and already platform-resolved (see
  // `stopAudioLoopbackNow`) — `before-quit` doesn't wait for promises a
  // listener returns or kicks off, so resolving the backend here via a
  // fresh `loadAudioBackend()` could lose the race against the process
  // actually exiting.
  stopAudioLoopbackNow();
  markQuitting();
});

app.whenReady().then(async () => {
  Menu.setApplicationMenu(null);
  createMainWindow();
  createTray();
  registerQuickShareIpcHandlers();
  await registerAudioIpcHandlers();
  await registerDisplayMediaHandler();
});
