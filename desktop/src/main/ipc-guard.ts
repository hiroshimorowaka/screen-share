import type { IpcMainEvent, IpcMainInvokeEvent } from 'electron';
import { PICKER_FILE_URL } from '#features/screen-share/picker-page.js';
import { APP_ORIGIN } from '#main/app-url.js';

/**
 * Whether an IPC message came from a frame we trust to drive the
 * privileged bridges (system-audio capture, clipboard, OS notifications,
 * the source picker).
 *
 * Trusted: a frame on the app's own origin, or the source-picker window's
 * exact `file://` URL. Everything else — a remote page that hijacked or
 * XSS'd its way into the renderer and then reached for `ipcRenderer`, or
 * any other local `file://` frame — is rejected (finding F11; the exact
 * URL match rather than a blanket `file://` prefix is follow-up audit
 * finding 13). Without this check any script running in the main window
 * could start a covert system-audio capture, enumerate running apps, or
 * hijack the clipboard.
 *
 * `senderFrame` can be `null` if the frame was disposed between sending
 * and handling; treat that as untrusted.
 */
export function isTrustedFrame(event: IpcMainEvent | IpcMainInvokeEvent): boolean {
  const url = event.senderFrame?.url;
  if (!url) return false;
  if (url === PICKER_FILE_URL) return true;
  try {
    return new URL(url).origin === APP_ORIGIN;
  } catch {
    return false;
  }
}
