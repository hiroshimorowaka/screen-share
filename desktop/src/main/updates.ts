import { app } from 'electron';
import electronUpdater from 'electron-updater';

/** The tray app can stay open for days, so a single check at launch isn't
 * enough — re-check on this interval too. */
const RECHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

function checkNow(): void {
  const { autoUpdater } = electronUpdater;
  autoUpdater.checkForUpdatesAndNotify().catch((err: unknown) => {
    // Offline, GitHub down, no newer release — all non-fatal. The app
    // keeps running on the current version; the next check will retry.
    console.error('[updates] check failed:', err);
  });
}

/**
 * Wires up silent background auto-updates for the packaged Windows build.
 *
 * `electron-updater` downloads a newer GitHub release in the background,
 * shows a native "update ready" notification, and installs it the next
 * time the app quits.
 *
 * Update integrity (see ADR-0008, finding F04):
 * - `electron-updater` always verifies the downloaded installer's SHA512
 *   against `latest.yml`.
 * - `build.win.verifyUpdateCodeSignature` is set to `true` in
 *   `package.json` (and guarded by `packaging-security.test.ts`), so on
 *   Windows the NSIS updater additionally refuses any installer not
 *   Authenticode-signed by the same publisher as the running app. This
 *   means a signed release build is required for auto-update to apply —
 *   CI must provide `CSC_LINK` / `CSC_KEY_PASSWORD`. An unsigned build
 *   still runs; its update checks simply never apply an update, which is
 *   the safe failure mode.
 *
 * No-op unless:
 * - the app is packaged — a `pnpm start` dev run has no `app-update.yml`
 *   and must not phone GitHub; and
 * - the platform is Windows — only the NSIS installer can replace itself
 *   in place. The Windows *portable* `.exe` and the Linux AppImage/`.deb`
 *   can't, so there updating stays a manual "grab the new release"
 *   (`electron-updater` would just fail on them).
 */
export function setupAutoUpdates(): void {
  if (!app.isPackaged || process.platform !== 'win32') {
    return;
  }

  const { autoUpdater } = electronUpdater;
  autoUpdater.on('error', (err) => {
    console.error('[updates] auto-updater error:', err);
  });

  checkNow();
  setInterval(checkNow, RECHECK_INTERVAL_MS);
}
