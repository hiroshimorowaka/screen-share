import type { MediaAccessPermissionRequest } from 'electron';
import { session } from 'electron';

/**
 * The renderer loads a **remote** origin (`APP_URL`). The web layer needs
 * no browser permissions from it *except* screen capture and a write-only
 * clipboard: `getUserMedia` (mic/camera), geolocation, notifications,
 * pointer lock, MIDI, clipboard **read**, etc. are all denied so a
 * compromised app origin can't prompt for — or in some cases silently
 * gain — them (follow-up audit finding 6). OS notifications go through the
 * native IPC bridge, not the web Notification permission.
 *
 * Screen capture is the first exception: it must be allowed *through the
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
 *
 * The Linux system-audio path is the second exception. There the picker's
 * "compartilhar áudio" tick starts a PipeWire loopback that exposes a
 * "Screen Share Mix" input device, and the renderer then grabs it with
 * `getUserMedia({ audio: { deviceId } })` — an audio-only `media` request.
 * A blanket deny here (audit finding 6: "covert system-audio capture")
 * leaves the share silently video-only. Rather than allow every
 * audio-only request, {@link armAudioCaptureGrant} opens a single,
 * time-boxed window right after a user-confirmed loopback starts; the one
 * capture that follows is let through and nothing else.
 *
 * `clipboard-sanitized-write` is the third exception: the invite button
 * (and the "copy the link when a share of ours goes live" effect) copy
 * the room link with `navigator.clipboard.writeText`, which Chromium
 * gates behind this permission. It is the write-only, payload-sanitized
 * variant — it grants no clipboard **read**, and a `writeText` still needs
 * a user gesture — so allowing it doesn't let the origin snoop on or
 * silently overwrite the clipboard.
 */
const ALLOWED_NON_MEDIA_PERMISSIONS = new Set(['clipboard-sanitized-write']);

/** How long a {@link armAudioCaptureGrant} window stays open. The renderer
 * calls `getUserMedia` for the mix device immediately after `capture_display`
 * confirms the loopback is live, so a few seconds is ample; keeping it
 * short bounds the blast radius if that capture never happens. */
const AUDIO_CAPTURE_GRANT_TTL_MS = 10_000;

/** Unix-ms deadline for the currently-armed system-audio capture grant, or
 * `0` when none is armed. Consumed (reset to `0`) by the first audio-only
 * `media` request that arrives before it. */
let audioCaptureGrantExpiresAt = 0;

/**
 * Open a one-shot, time-boxed window in which a single audio-only
 * `getUserMedia` is permitted. Called by the display-media handler once a
 * user-confirmed PipeWire loopback is actually running, so the renderer's
 * follow-up capture of the "Screen Share Mix" device gets through while
 * an out-of-band `getUserMedia({ audio })` stays denied.
 */
export function armAudioCaptureGrant(): void {
  audioCaptureGrantExpiresAt = Date.now() + AUDIO_CAPTURE_GRANT_TTL_MS;
}

export function lockDownPermissions(): void {
  session.defaultSession.setPermissionRequestHandler(
    (_webContents, permission, callback, details) => {
      callback(isPermissionAllowed(permission, details as MediaAccessPermissionRequest));
    },
  );
  // The check handler can't see `mediaTypes`, so it can't tell a display
  // capture check from a camera/mic one — it only gates whether the flow
  // proceeds to the request handler above, which makes the real decision.
  session.defaultSession.setPermissionCheckHandler(
    (_webContents, permission) =>
      permission === 'media' || ALLOWED_NON_MEDIA_PERMISSIONS.has(permission),
  );
}

function isPermissionAllowed(permission: string, details: MediaAccessPermissionRequest): boolean {
  if (ALLOWED_NON_MEDIA_PERMISSIONS.has(permission)) return true;
  if (permission !== 'media') return false;
  if (isDisplayCaptureRequest(details)) return true;
  return isAudioOnlyRequest(details) && consumeAudioCaptureGrant();
}

/** A `getDisplayMedia` permission request carries no explicit `mediaTypes`
 * (the sources are chosen later, in `setDisplayMediaRequestHandler`); a
 * `getUserMedia` request always lists `'video'` and/or `'audio'`. */
function isDisplayCaptureRequest(details: MediaAccessPermissionRequest): boolean {
  return !details.mediaTypes || details.mediaTypes.length === 0;
}

/** `getUserMedia({ audio: … })` with no video — the shape of the mix-device
 * capture. A camera+mic call lists both and stays denied. */
function isAudioOnlyRequest(details: MediaAccessPermissionRequest): boolean {
  return details.mediaTypes?.length === 1 && details.mediaTypes[0] === 'audio';
}

/** `true` at most once per {@link armAudioCaptureGrant}, and only within
 * {@link AUDIO_CAPTURE_GRANT_TTL_MS} of it. */
function consumeAudioCaptureGrant(): boolean {
  const armed = audioCaptureGrantExpiresAt > Date.now();
  audioCaptureGrantExpiresAt = 0;
  return armed;
}
