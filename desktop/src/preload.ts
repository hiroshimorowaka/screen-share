import { contextBridge, ipcRenderer } from 'electron';

interface PickerSource {
  id: string;
  name: string;
  thumbnailDataUrl: string;
  iconDataUrl: string | null;
}

contextBridge.exposeInMainWorld('picker', {
  onSources: (callback: (sources: PickerSource[]) => void) => {
    ipcRenderer.on('picker:sources', (_event, sources: PickerSource[]) => {
      callback(sources);
    });
  },
  select: (id: string) => {
    ipcRenderer.send('picker:selected', id);
  },
});
