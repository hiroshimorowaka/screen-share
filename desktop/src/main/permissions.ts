import type { MediaAccessPermissionRequest } from 'electron';
import { session } from 'electron';

/**
 * The renderer loads a **remote** origin (`APP_URL`). The web layer needs
 * no browser permissions from it *except* screen capture: `getUserMedia`
 * (mic/camera), geolocation, notifications, pointer lock, MIDI, etc. are
 * all denied so a compromised app origin can't prompt for — or in some
 * cases silently gain — them (follow-up audit finding 6). OS notifications
 * go through the native IPC bridge, not the web Notification permission.
 *
 * Screen capture is the exception: it must be allowed *through the
 * permission layer* for `setDisplayMediaRequestHandler` (registered in
 * `features/screen-share/display-media.ts`) to ever be reached. Chromium
 * runs a permission **check** and a permission **request**, both typed
 * `media`, before it routes a `getDisplayMedia` call to that handler — a
 * blanket deny here means the source picker never opens and the renderer
 * just gets `NotAllowedError`.
 *
 * `getDisplayMedia` and a camera/mic `getUserMedia` are told apart at
 * *request* time by {@link isDisplayCaptureRequest}: a `getUserMedia`
 * request always names the devices it wants in `mediaTypes`, a display
 * capture request never does. The real gate on *which* screen or window
 * is shared stays `setDisplayMediaRequestHandler` + our own picker window.
 */
export function lockDownPermissions(): void {
  session.defaultSession.setPermissionRequestHandler(
    (_webContents, permission, callback, details) => {
      callback(
        permission === 'media' && isDisplayCaptureRequest(details as MediaAccessPermissionRequest),
      );
    },
  );
  // The check handler can't see `mediaTypes`, so it can't tell a display
  // capture check from a camera/mic one — it only gates whether the flow
  // proceeds to the request handler above, which makes the real decision.
  session.defaultSession.setPermissionCheckHandler(
    (_webContents, permission) => permission === 'media',
  );
}

/** A `getDisplayMedia` permission request carries no explicit `mediaTypes`
 * (the sources are chosen later, in `setDisplayMediaRequestHandler`); a
 * `getUserMedia` request always lists `'video'` and/or `'audio'`. */
function isDisplayCaptureRequest(details: MediaAccessPermissionRequest): boolean {
  return !details.mediaTypes || details.mediaTypes.length === 0;
}
