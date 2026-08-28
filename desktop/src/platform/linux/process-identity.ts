import * as fs from 'fs/promises';
import * as path from 'path';

import { runCollectingStdout } from '../run-command.js';

/** This app's own binary name, so its own audio playback (e.g. a member
 * watching someone's share, including their own, in this same app) can
 * never be swept into the mix — doing so would feed the mix's captured
 * audio back into itself once shared with a watcher on this machine. */
export const OWN_BINARY_NAME = path.basename(process.execPath);

export function parseX11WindowId(sourceId: string): number | null {
  const match = sourceId.match(/^window:(\d+):/);
  return match ? parseInt(match[1], 10) : null;
}

export function resolveWindowPid(x11WindowId: number): Promise<number | null> {
  return runCollectingStdout('xprop', ['-id', String(x11WindowId), '_NET_WM_PID']).then(
    (output) => {
      const match = output.match(/=\s*(\d+)/);
      return match ? parseInt(match[1], 10) : null;
    },
  );
}

/** Falls back to `/proc/<pid>/cmdline`'s first argument (argv[0], not
 * truncated) when `/proc/<pid>/exe` isn't readable. Reading the `exe`
 * symlink requires ptrace-level permission on the target process
 * (`ptrace_may_access`, gated by the Yama LSM's `ptrace_scope`), which
 * recent Ubuntu-based distros restrict by default to actual process
 * ancestors — denying it for an unrelated process like this app reading
 * another app's window owner, even as the same user. `cmdline` carries
 * no such restriction (only `hidepid`, rarely enabled on desktop
 * systems), and argv[0] is normally the same absolute path `exe` would
 * have resolved to anyway. */
function resolveProcessBinaryFromCmdline(pid: number): Promise<string | null> {
  return fs
    .readFile(`/proc/${pid}/cmdline`, 'utf8')
    .then((cmdline) => {
      const argv0 = cmdline.split('\0')[0];
      return argv0 ? path.basename(argv0) : null;
    })
    .catch(() => null);
}

/** A Chromium-based app (browsers, and this app itself) plays all of its
 * audio through one shared "Audio Service" subprocess, whose PID never
 * matches any of the app's own window PIDs — matching audio by exact PID
 * only ever works for single-process audio backends (e.g. Spotify).
 * Binary name is the only identifier stable across that process split. */
export function resolveProcessBinary(pid: number): Promise<string | null> {
  return fs
    .readlink(`/proc/${pid}/exe`)
    .then((target) => path.basename(target))
    .catch(() => resolveProcessBinaryFromCmdline(pid));
}
