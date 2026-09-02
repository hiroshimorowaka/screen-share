import { contextBridge, ipcRenderer } from 'electron';

import type { AudioShareTarget, PickerChoice, PickerSource } from '#ipc/types.js';

// Must match `SHARE_LINK_READY_CHANNEL` in `features/screen-share/quick-share.ts` exactly — kept as a
// literal, not a shared import, because the sandboxed preload script can't
// `require()` local project files (only `import type`, erased at compile
// time, survives here).
contextBridge.exposeInMainWorld('desktopShare', {
  linkReady: (link: string) => ipcRenderer.send('desktop-share:link-ready', link),
  memberJoined: (nick: string) => ipcRenderer.send('desktop-share:member-joined', nick),
  // Drives the tray icon's idle (green) / live (red) state — channel name
  // must match `main/tray.ts` exactly (see the linkReady comment above for
  // why these are literals).
  sharingChanged: (isSharing: boolean) =>
    ipcRenderer.send('desktop-share:sharing-changed', isSharing),
});

contextBridge.exposeInMainWorld('picker', {
  onSources: (callback: (sources: PickerSource[]) => void) => {
    // `picker:sources` is sent exactly once per picker window, so `once`
    // both matches the protocol and can't pile up an `ipcRenderer`
    // listener if the page calls `onSources` more than once (finding 8c).
    ipcRenderer.once('picker:sources', (_event, sources: PickerSource[]) => {
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
  // Resolves to whether a loopback is currently running — `webrtc.rs`
  // (`desktop_audio_loopback_active`) checks this after the picker closes
  // so an audio-less share doesn't probe for (and log about) a device
  // that was never started.
  active: () => ipcRenderer.invoke('audio-loopback-active'),
  // `webrtc.rs`'s `has_pcm_bridge()` uses this property's mere *existence*
  // as its signal for which track-construction path to run — it must
  // only be present on Windows, not merely inert elsewhere, or Linux
  // would wrongly take the PCM-bridge path (and get a track that's real
  // but silent, since nothing ever sends `desktop-audio-pcm-chunk` there)
  // instead of its own `getUserMedia` device-label path.
  ...(process.platform === 'win32'
    ? {
        onPcmChunk: (callback: (chunk: ArrayBuffer) => void) => {
          // Called once per Windows share. This runs in the persistent
          // main-window preload, so without clearing first a permanent
          // `desktop-audio-pcm-chunk` listener would accumulate per share
          // (Node warns past 10), each calling into a since-dropped
          // generator (finding 8c).
          ipcRenderer.removeAllListeners('desktop-audio-pcm-chunk');
          ipcRenderer.on('desktop-audio-pcm-chunk', (_event, chunk: ArrayBuffer) =>
            callback(chunk),
          );
        },
        offPcmChunk: () => ipcRenderer.removeAllListeners('desktop-audio-pcm-chunk'),
      }
    : {}),
});
