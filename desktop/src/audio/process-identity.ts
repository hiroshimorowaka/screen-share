import * as fs from 'fs/promises';
import * as path from 'path';

import { runCollectingStdout } from '../process-utils.js';

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

/** A Chromium-based app (browsers, and this app itself) plays all of its
 * audio through one shared "Audio Service" subprocess, whose PID never
 * matches any of the app's own window PIDs — matching audio by exact PID
 * only ever works for single-process audio backends (e.g. Spotify).
 * Binary name is the only identifier stable across that process split. */
export function resolveProcessBinary(pid: number): Promise<string | null> {
  return fs
    .readlink(`/proc/${pid}/exe`)
    .then((target) => path.basename(target))
    .catch(() => null);
}
