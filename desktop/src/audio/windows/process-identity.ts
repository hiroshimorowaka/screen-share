import { getExeNameForPid, getPidForWindow } from '../../../native/windows-audio/index.js';

export function parseWindowsWindowId(sourceId: string): number | null {
  const match = sourceId.match(/^window:(\d+):/);
  return match ? parseInt(match[1], 10) : null;
}

/** Re-exported (and named to match) the Linux `process-identity.ts`'s
 * `resolveWindowPid`/`resolveProcessBinary` pair — both are direct native
 * calls here rather than a subprocess spawn, so neither needs a
 * `Promise` wrapper. `getPidForWindow` takes an hwnd and resolves it to
 * the PID of the window's owning process — same direction as
 * `resolveWindowPid` on Linux, hence the same name (not
 * `resolveWindowHandle`, which reads as producing a handle rather than
 * consuming one). */
export const resolveWindowPid = getPidForWindow;
export const resolveExeNameForPid = getExeNameForPid;
