import { ipcMain } from 'electron';

import type { AudioShareTarget } from '../../ipc/types.js';

// Cached once `registerAudioIpcHandlers` resolves its platform's backend,
// so `stopAudioLoopbackNow` can call it synchronously — `before-quit`
// fires synchronously and Electron doesn't wait for anything a listener
// returns or kicks off, so a fresh `await import(...)` at quit time could
// easily lose the race against the process actually exiting.
let stopActiveAudioLoopback: (() => void) | null = null;

export async function registerAudioIpcHandlers(): Promise<void> {
  const { startAudioLoopback, stopAudioLoopback, listDistinctAudioApps } =
    process.platform === 'win32'
      ? await import('../../platform/windows/audio.js')
      : { ...(await import('../../platform/linux/loopback.js')), ...(await import('../../platform/linux/pipewire.js')) };

  stopActiveAudioLoopback = stopAudioLoopback;

  ipcMain.handle('start-audio-loopback', (_event, target: AudioShareTarget) =>
    startAudioLoopback(target),
  );

  ipcMain.handle('stop-audio-loopback', () => {
    stopAudioLoopback();
  });

  ipcMain.handle('list-audio-apps', () => listDistinctAudioApps());
}

/** Safe to call even before `registerAudioIpcHandlers` has resolved (a
 * no-op then, which only matters if `app.quit()` somehow fires before
 * `app.whenReady()` ever does — not a real scenario, but not worth a
 * crash either). Exists for `main/index.ts`'s `before-quit` handler, which
 * can't itself await the dynamic import `registerAudioIpcHandlers`
 * needs. */
export function stopAudioLoopbackNow(): void {
  stopActiveAudioLoopback?.();
}
