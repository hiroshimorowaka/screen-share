import { app, Menu } from 'electron';

import { registerAudioIpcHandlers, stopAudioLoopbackNow } from '#features/audio-share/ipc.js';
import { registerDisplayMediaHandler } from '#features/screen-share/display-media.js';
import { registerQuickShareIpcHandlers } from '#features/screen-share/quick-share.js';
import { markQuitting } from '#main/lifecycle.js';
import { lockDownPermissions } from '#main/permissions.js';
import { createTray } from '#main/tray.js';
import { setupAutoUpdates } from '#main/updates.js';
import { createMainWindow } from '#main/window.js';

app.on('before-quit', () => {
  // Synchronous and already platform-resolved (see
  // `stopAudioLoopbackNow`) — `before-quit` doesn't wait for promises a
  // listener returns or kicks off, so resolving the backend here via a
  // fresh `loadAudioBackend()` could lose the race against the process
  // actually exiting.
  stopAudioLoopbackNow();
  markQuitting();
});

app
  .whenReady()
  .then(async () => {
    Menu.setApplicationMenu(null);
    lockDownPermissions();
    createMainWindow();
    createTray();
    setupAutoUpdates();
    registerQuickShareIpcHandlers();
    // Register the screen-share picker first and unconditionally: it must
    // not be gated on the audio backend, which pulls in a native addon
    // that can fail to load on a user's machine.
    registerDisplayMediaHandler();
    await registerAudioIpcHandlers().catch((err) => {
      console.error('Audio IPC handlers unavailable:', err);
    });
  })
  .catch((err) => {
    console.error('Desktop bootstrap failed:', err);
  });
