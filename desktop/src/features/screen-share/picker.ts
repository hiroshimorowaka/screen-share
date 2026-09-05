import * as path from 'node:path';
import type { IpcMainEvent } from 'electron';
import { app, BrowserWindow, desktopCapturer, ipcMain } from 'electron';
import { PICKER_HTML_PATH } from '#features/screen-share/picker-page.js';
import type { PickerChoice, PickerSource, ShareChoice } from '#ipc/types.js';
import { isTrustedFrame } from '#main/ipc-guard.js';
import { getMainWindow, lockNavigation } from '#main/window.js';

async function enumerateSources(): Promise<{
  sources: Electron.DesktopCapturerSource[];
  pickerSources: PickerSource[];
}> {
  let sources: Electron.DesktopCapturerSource[];
  try {
    sources = await desktopCapturer.getSources({
      types: ['screen', 'window'],
      thumbnailSize: { width: 300, height: 200 },
      fetchWindowIcons: true,
    });
  } catch (err) {
    // Log rather than swallow: otherwise the picker just opens empty
    // with no indication anything went wrong.
    console.error('desktopCapturer.getSources failed:', err);
    sources = [];
  }
  const pickerSources: PickerSource[] = sources.map((s) => ({
    id: s.id,
    name: s.name,
    thumbnailDataUrl: s.thumbnail.toDataURL(),
    iconDataUrl: s.appIcon && !s.appIcon.isEmpty() ? s.appIcon.toDataURL() : null,
  }));
  return { sources, pickerSources };
}

// Skip anchoring to a hidden main window (the tray's quick share flow) —
// several window managers won't surface a child window whose parent isn't
// visible, and the picker must always show up.
function pickParentWindow(): BrowserWindow | undefined {
  const owner = getMainWindow();
  return owner?.isVisible() ? owner : undefined;
}

function createPickerWindow(parent: BrowserWindow | undefined): BrowserWindow {
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
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      // Match the main window: no DevTools in a packaged build.
      devTools: !app.isPackaged,
    },
  });
  // Same navigation lock the main window gets — `will-navigate` /
  // `will-redirect` / `window.open` off the picker page all blocked.
  // `loadFile` below does not fire `will-navigate`.
  lockNavigation(pickerWindow);
  return pickerWindow;
}

// The chosen source id only names a `PickerSource` (the picker page's own
// DTO); this resolves it back to the real `DesktopCapturerSource` the rest
// of the app needs to actually start the capture.
function toShareChoice(
  sources: Electron.DesktopCapturerSource[],
  choice: PickerChoice | null,
): ShareChoice | null {
  if (!choice) return null;
  const source = sources.find((s) => s.id === choice.sourceId) ?? null;
  return source
    ? { source, shareAudio: choice.shareAudio, excludedBinaries: choice.excludedBinaries }
    : null;
}

export function showSourcePicker(): Promise<ShareChoice | null> {
  return new Promise((resolve) => {
    void (async () => {
      const { sources, pickerSources } = await enumerateSources();
      const pickerWindow = createPickerWindow(pickParentWindow());

      let settled = false;
      const settle = (choice: PickerChoice | null) => {
        if (settled) return;
        settled = true;
        // If the picker was dismissed (blur -> close) the `once` listener
        // was never consumed — one leaked `ipcMain` listener per
        // cancelled picker otherwise. Safe if already spent.
        ipcMain.removeListener('picker:selected', onSelected);
        resolve(toShareChoice(sources, choice));
        if (!pickerWindow.isDestroyed()) pickerWindow.close();
      };

      const onSelected = (event: IpcMainEvent, choice: PickerChoice) => {
        if (!isTrustedFrame(event)) return;
        settle(choice);
      };

      ipcMain.once('picker:selected', onSelected);
      pickerWindow.on('closed', () => settle(null));

      // Delay arming "click outside closes it" slightly so the window
      // manager focusing this new window doesn't itself trigger a blur.
      // The window can already be gone by the time this fires (a fast
      // dismiss, or the `loadFile` failure path below closing it) —
      // `.on()` on a destroyed BrowserWindow throws.
      setTimeout(() => {
        if (pickerWindow.isDestroyed()) return;
        pickerWindow.on('blur', () => settle(null));
      }, 300);

      try {
        await pickerWindow.loadFile(PICKER_HTML_PATH);
      } catch (err) {
        // Dismissing the picker (a click outside → `blur` → `settle` →
        // `pickerWindow.close()`) aborts an in-flight load, rejecting
        // `loadFile` with ERR_ABORTED. Harmless once we've already settled;
        // anything else is a genuine failure to show the picker. Either way
        // the `void`-invoked task must not leak an unhandled rejection.
        if (!settled) {
          console.error('Failed to load the source picker:', err);
          settle(null);
        }
        return;
      }
      if (settled || pickerWindow.isDestroyed()) return;
      pickerWindow.webContents.send('picker:sources', pickerSources);
    })();
  });
}
