import { session } from 'electron';

import { startAudioLoopback } from './audio/loopback-session.js';
import { parseX11WindowId, resolveProcessBinary, resolveWindowPid } from './audio/process-identity.js';
import { showSourcePicker } from './picker.js';
import type { AudioShareTarget, ShareChoice } from './shared-types.js';

async function resolveAudioTarget(chosen: ShareChoice): Promise<AudioShareTarget | null> {
  if (chosen.source.id.startsWith('window:')) {
    const x11Id = parseX11WindowId(chosen.source.id);
    if (x11Id === null) return null;
    const pid = await resolveWindowPid(x11Id);
    if (pid === null) return null;
    const binary = await resolveProcessBinary(pid);
    if (binary === null) return null;
    return { mode: 'window', binary };
  }
  return { mode: 'screen', excludedBinaries: chosen.excludedBinaries };
}

export function registerDisplayMediaHandler(): void {
  session.defaultSession.setDisplayMediaRequestHandler(async (_request, callback) => {
    const chosen = await showSourcePicker();
    if (!chosen) {
      callback({});
      return;
    }
    if (chosen.shareAudio) {
      const target = await resolveAudioTarget(chosen);
      if (target) {
        try {
          await startAudioLoopback(target);
        } catch {
          // Proceed with video-only rather than failing the whole share.
        }
      }
    }
    callback({ video: chosen.source });
  });
}
