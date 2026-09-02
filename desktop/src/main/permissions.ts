import { session } from 'electron';

/**
 * The renderer loads a **remote** origin (`APP_URL`). The web layer needs
 * no browser permissions from it: screen capture goes through the
 * dedicated `setDisplayMediaRequestHandler`, and OS notifications go
 * through the native IPC bridge. Without a handler, a compromised app
 * origin could prompt for — or in some cases silently gain —
 * `getUserMedia` (mic/camera), geolocation, notifications, pointer lock,
 * MIDI, etc. Deny everything (follow-up audit finding 6).
 *
 * `setDisplayMediaRequestHandler` is a separate mechanism and is
 * unaffected, so `getDisplayMedia` keeps working.
 */
export function lockDownPermissions(): void {
  session.defaultSession.setPermissionRequestHandler((_webContents, _permission, callback) => {
    callback(false);
  });
  session.defaultSession.setPermissionCheckHandler(() => false);
}
