import { clipboard, ipcMain, Notification } from 'electron';

/** Copies the invite link the room page hands over once the tray's quick
 * share flow (see `main/window.ts`'s `startQuickShare`) has a stream
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

  // Same rationale as above: the room page's window is hidden/backgrounded
  // for most of a desktop session, so an OS-level notification is the only
  // reliable way to surface this. Channel name matches `preload.ts`'s
  // `desktopShare.memberJoined` exactly.
  ipcMain.on('desktop-share:member-joined', (_event, nick: string) => {
    if (!Notification.isSupported()) return;
    new Notification({ title: 'Screen Share', body: `${nick} entrou na sala.` }).show();
  });
}
