import { contextBridge, ipcRenderer } from 'electron';

interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
  iconDataUrl: string | null;
}

interface PickerChoice {
  sourceId: string;
  shareAudio: boolean;
  excludedBinaries: string[];
}

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

interface AudioShareTarget {
  mode: 'window' | 'screen';
  binary?: string;
  excludedBinaries?: string[];
}

contextBridge.exposeInMainWorld('desktopAudio', {
  start: (target: AudioShareTarget) => ipcRenderer.invoke('start-audio-loopback', target),
  stop: () => ipcRenderer.invoke('stop-audio-loopback'),
});
