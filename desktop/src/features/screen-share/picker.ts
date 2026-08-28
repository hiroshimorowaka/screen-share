import * as path from 'node:path';
import { BrowserWindow, desktopCapturer, ipcMain } from 'electron';
import type { PickerChoice, PickerSource, ShareChoice } from '#ipc/types.js';
import { getMainWindow } from '#main/window.js';

export function showSourcePicker(): Promise<ShareChoice | null> {
  return new Promise((resolve) => {
    void (async () => {
      let sources: Electron.DesktopCapturerSource[];
      try {
        sources = await desktopCapturer.getSources({
          types: ['screen', 'window'],
          thumbnailSize: { width: 300, height: 200 },
          fetchWindowIcons: true,
        });
      } catch (err) {
        // Previously swallowed silently — the picker would just open
        // empty with no indication anything had gone wrong.
        console.error('desktopCapturer.getSources failed:', err);
        sources = [];
      }
      const pickerSources: PickerSource[] = sources.map((s) => ({
        id: s.id,
        name: s.name,
        thumbnailDataUrl: s.thumbnail.toDataURL(),
        iconDataUrl: s.appIcon && !s.appIcon.isEmpty() ? s.appIcon.toDataURL() : null,
      }));

      // Skip anchoring to a hidden main window (the tray's quick share
      // flow) — several window managers won't surface a child window
      // whose parent isn't visible, and the picker must always show up.
      const owner = getMainWindow();
      const parent = owner?.isVisible() ? owner : undefined;

      const pickerWindow = new BrowserWindow({
        width: 1000,
        height: 720,
        parent,
        frame: false,
        transparent: true,
        resizable: true,
        minWidth: 640,
        minHeight: 480,
        skipTaskbar: true,
        webPreferences: {
          preload: path.join(__dirname, '..', '..', 'preload.js'),
        },
      });

      let settled = false;
      const settle = (choice: PickerChoice | null) => {
        if (settled) return;
        settled = true;
        if (!choice) {
          resolve(null);
        } else {
          const source = sources.find((s) => s.id === choice.sourceId) ?? null;
          resolve(
            source
              ? {
                  source,
                  shareAudio: choice.shareAudio,
                  excludedBinaries: choice.excludedBinaries,
                }
              : null,
          );
        }
        if (!pickerWindow.isDestroyed()) pickerWindow.close();
      };

      ipcMain.once('picker:selected', (_event, choice: PickerChoice) => settle(choice));
      pickerWindow.on('closed', () => settle(null));

      // Delay arming "click outside closes it" slightly so the window
      // manager focusing this new window doesn't itself trigger a blur.
      setTimeout(() => {
        pickerWindow.on('blur', () => settle(null));
      }, 300);

      await pickerWindow.loadFile(path.join(__dirname, '..', '..', '..', 'static', 'picker.html'));
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}
