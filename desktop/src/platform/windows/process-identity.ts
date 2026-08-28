import type { AudioShareTarget, ShareChoice } from '../../ipc/types.js';
import { getExeNameForPid, getPidForWindow } from '../../../native/windows-audio/index.js';

export function parseWindowsWindowId(sourceId: string): number | null {
  const match = sourceId.match(/^window:(\d+):/);
  return match ? parseInt(match[1], 10) : null;
}

/** Re-exported (and named to match) the Linux `platform/linux/process-identity.ts`'s
 * `resolveWindowPid`/`resolveProcessBinary` pair — both are direct native
 * calls here rather than a subprocess spawn, so neither needs a
 * `Promise` wrapper. `getPidForWindow` takes an hwnd and resolves it to
 * the PID of the window's owning process — same direction as
 * `resolveWindowPid` on Linux, hence the same name (not
 * `resolveWindowHandle`, which reads as producing a handle rather than
 * consuming one). */
export const resolveWindowPid = getPidForWindow;
export const resolveExeNameForPid = getExeNameForPid;

/** Windows counterpart of the Linux `resolveAudioTarget`: for a window,
 * the owning process's executable name (matched on the Rust side against
 * the audio session's process list); for the whole screen, the exclusion
 * list straight through. `async` only to match the shared `AudioBackend`
 * signature — the native calls it makes are synchronous. */
export async function resolveAudioTarget(chosen: ShareChoice): Promise<AudioShareTarget | null> {
  if (chosen.source.id.startsWith('window:')) {
    const hwnd = parseWindowsWindowId(chosen.source.id);
    if (hwnd === null) return null;
    const pid = resolveWindowPid(hwnd);
    if (pid === null) return null;
    const binary = resolveExeNameForPid(pid);
    if (binary === null) return null;
    return { mode: 'window', binary };
  }
  return { mode: 'screen', excludedBinaries: chosen.excludedBinaries };
}
