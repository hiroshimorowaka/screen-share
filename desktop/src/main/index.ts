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

// One bootstrap step. Isolated so a synchronous throw in any one of them
// (a broken auto-updater, a tray-icon failure on an odd Windows build)
// can't abort the rest of startup — most importantly not the
// screen-share handler, whose absence makes `getDisplayMedia` silently do
// nothing. The label goes to stderr so a failure is diagnosable in a
// packaged build (`--enable-logging`).
function step(label: string, run: () => void): void {
  try {
    run();
  } catch (err) {
    console.error(`[bootstrap] ${label} failed:`, err);
  }
}

app
  .whenReady()
  .then(async () => {
    step('menu', () => Menu.setApplicationMenu(null));
    step('permissions', lockDownPermissions);
    // First and on its own: nothing else in bootstrap may keep
    // `getDisplayMedia` from finding a registered handler.
    step('display-media', registerDisplayMediaHandler);
    step('main-window', createMainWindow);
    step('tray', createTray);
    step('auto-updates', setupAutoUpdates);
    step('quick-share-ipc', registerQuickShareIpcHandlers);
    await registerAudioIpcHandlers().catch((err) => {
      console.error('[bootstrap] audio-ipc failed:', err);
    });
  })
  .catch((err) => {
    console.error('[bootstrap] fatal:', err);
  });
