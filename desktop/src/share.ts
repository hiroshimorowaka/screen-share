import { clipboard, ipcMain } from 'electron';

/** Copies the invite link the room page hands over once the tray's quick
 * share flow (see `main-window.ts`'s `startQuickShare`) has a stream
 * live — the room page's own window stays hidden throughout, so its
 * Clipboard API (which requires document focus) can't be relied on here.
 *
 * The channel name must match `preload.ts`'s `desktopShare.linkReady`
 * exactly — see the comment there for why it's a literal instead of a
 * shared import. */
export function registerQuickShareIpcHandlers(): void {
  ipcMain.on('desktop-share:link-ready', (_event, link: string) => {
    clipboard.writeText(link);
  });
}
