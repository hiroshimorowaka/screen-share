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
  // `webrtc.rs`'s `has_pcm_bridge()` uses this property's mere *existence*
  // as its signal for which track-construction path to run — it must
  // only be present on Windows, not merely inert elsewhere, or Linux
  // would wrongly take the PCM-bridge path (and get a track that's real
  // but silent, since nothing ever sends `desktop-audio-pcm-chunk` there)
  // instead of its own `getUserMedia` device-label path.
  ...(process.platform === 'win32'
    ? {
        onPcmChunk: (callback: (chunk: ArrayBuffer) => void) => {
          ipcRenderer.on('desktop-audio-pcm-chunk', (_event, chunk: ArrayBuffer) => callback(chunk));
        },
      }
    : {}),
});
