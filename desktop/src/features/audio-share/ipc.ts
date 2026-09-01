import { ipcMain } from 'electron';
import { loadAudioBackend } from '#features/audio-share/backend.js';
import type { AudioShareTarget } from '#ipc/types.js';
import { isTrustedFrame } from '#main/ipc-guard.js';

// Cached once `registerAudioIpcHandlers` resolves the platform backend,
// so `stopAudioLoopbackNow` can call it synchronously — `before-quit`
// fires synchronously and Electron doesn't wait for anything a listener
// returns or kicks off, so a fresh `loadAudioBackend()` at quit time
// could easily lose the race against the process actually exiting.
let stopActiveAudioLoopback: (() => void) | null = null;

export async function registerAudioIpcHandlers(): Promise<void> {
  const { startAudioLoopback, stopAudioLoopback, listDistinctAudioApps } = await loadAudioBackend();

  stopActiveAudioLoopback = stopAudioLoopback;

  // System-audio capture and process enumeration — only the app's own
  // frames may drive these, never a hijacked/XSS'd remote page that got
  // into the renderer (finding F11).
  ipcMain.handle('start-audio-loopback', (event, target: AudioShareTarget) => {
    if (!isTrustedFrame(event)) throw new Error('start-audio-loopback: untrusted sender');
    return startAudioLoopback(target);
  });

  ipcMain.handle('stop-audio-loopback', (event) => {
    if (!isTrustedFrame(event)) throw new Error('stop-audio-loopback: untrusted sender');
    stopAudioLoopback();
  });

  ipcMain.handle('list-audio-apps', (event) => {
    if (!isTrustedFrame(event)) throw new Error('list-audio-apps: untrusted sender');
    return listDistinctAudioApps();
  });
}

/** Safe to call even before `registerAudioIpcHandlers` has resolved (a
 * no-op then, which only matters if `app.quit()` somehow fires before
 * `app.whenReady()` ever does — not a real scenario, but not worth a
 * crash either). Exists for `main/index.ts`'s `before-quit` handler,
 * which can't itself await the dynamic backend load
 * `registerAudioIpcHandlers` needs. */
export function stopAudioLoopbackNow(): void {
  stopActiveAudioLoopback?.();
}
