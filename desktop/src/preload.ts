import { contextBridge, ipcRenderer } from 'electron';

import type { AudioShareTarget, PickerChoice, PickerSource } from './shared-types.js';

contextBridge.exposeInMainWorld('picker', {
  onSources: (callback: (sources: PickerSource[]) => void) => {
    ipcRenderer.on('picker:sources', (_event, sources: PickerSource[]) => {
      callback(sources);
    });
  },
  select: (choice: PickerChoice) => {
    ipcRenderer.send('picker:selected', choice);
  },
  listAudioApps: () => ipcRenderer.invoke('list-audio-apps'),
});

contextBridge.exposeInMainWorld('desktopAudio', {
  start: (target: AudioShareTarget) => ipcRenderer.invoke('start-audio-loopback', target),
  stop: () => ipcRenderer.invoke('stop-audio-loopback'),
});
