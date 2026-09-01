import { clipboard, ipcMain, Notification } from 'electron';

import { isTrustedFrame } from '#main/ipc-guard.js';

/** Copies the invite link the room page hands over once the tray's quick
 * share flow (see `main/window.ts`'s `startQuickShare`) has a stream
 * live — the room page's own window stays hidden throughout, so its
 * Clipboard API (which requires document focus) can't be relied on here.
 *
 * The channel name must match `preload.ts`'s `desktopShare.linkReady`
 * exactly — see the comment there for why it's a literal instead of a
 * shared import. */
export function registerQuickShareIpcHandlers(): void {
  ipcMain.on('desktop-share:link-ready', (event, link: string) => {
    // Clipboard hijack / notification spoofing guard (finding F11).
    if (!isTrustedFrame(event)) return;
    clipboard.writeText(link);
    // The room window is hidden throughout the tray's quick-share flow, so
    // an OS notification is the only way to tell the user the share is live
    // and the link is already on their clipboard, ready to paste.
    if (!Notification.isSupported()) return;
    new Notification({
      title: 'Screen Share',
      body: 'Transmissão no ar — link da sala copiado!',
    }).show();
  });

  // Same rationale as above: the room page's window is hidden/backgrounded
  // for most of a desktop session, so an OS-level notification is the only
  // reliable way to surface this. Channel name matches `preload.ts`'s
  // `desktopShare.memberJoined` exactly.
  ipcMain.on('desktop-share:member-joined', (event, nick: string) => {
    if (!isTrustedFrame(event)) return;
    if (!Notification.isSupported()) return;
    new Notification({ title: 'Screen Share', body: `${nick} entrou na sala.` }).show();
  });
}
