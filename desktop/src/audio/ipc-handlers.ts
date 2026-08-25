import { ipcMain } from 'electron';

import type { AudioShareTarget } from '../shared-types.js';
import { startAudioLoopback, stopAudioLoopback } from './loopback-session.js';
import { listDistinctAudioApps } from './pipewire.js';

export function registerAudioIpcHandlers(): void {
  ipcMain.handle('start-audio-loopback', (_event, target: AudioShareTarget) =>
    startAudioLoopback(target),
  );

  ipcMain.handle('stop-audio-loopback', () => {
    stopAudioLoopback();
  });

  ipcMain.handle('list-audio-apps', () => listDistinctAudioApps());
}
