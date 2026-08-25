import { BrowserWindow, desktopCapturer, ipcMain } from 'electron';
import * as path from 'path';

import { getMainWindow } from './main-window.js';
import type { PickerChoice, PickerSource, ShareChoice } from './shared-types.js';

export function showSourcePicker(): Promise<ShareChoice | null> {
  return new Promise((resolve) => {
    void (async () => {
      const sources = await desktopCapturer.getSources({
        types: ['screen', 'window'],
        thumbnailSize: { width: 300, height: 200 },
        fetchWindowIcons: true,
      });

      const pickerSources: PickerSource[] = sources.map((s) => ({
        id: s.id,
        name: s.name,
        thumbnailDataUrl: s.thumbnail.toDataURL(),
        iconDataUrl: s.appIcon && !s.appIcon.isEmpty() ? s.appIcon.toDataURL() : null,
      }));

      const pickerWindow = new BrowserWindow({
        width: 1000,
        height: 720,
        parent: getMainWindow() ?? undefined,
        frame: false,
        transparent: true,
        resizable: true,
        minWidth: 640,
        minHeight: 480,
        skipTaskbar: true,
        webPreferences: {
          preload: path.join(__dirname, 'preload.js'),
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

      await pickerWindow.loadFile(
        path.join(__dirname, '..', 'static', 'picker.html'),
      );
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}
