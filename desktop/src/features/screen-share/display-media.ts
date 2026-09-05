import { session } from 'electron';

import { loadAudioBackend } from '#features/audio-share/backend.js';
import { showSourcePicker } from '#features/screen-share/picker.js';
import type { ShareChoice } from '#ipc/types.js';
import { armAudioCaptureGrant } from '#main/permissions.js';

export function registerDisplayMediaHandler(): void {
  session.defaultSession.setDisplayMediaRequestHandler(async (_request, callback) => {
    const chosen = await showSourcePicker();
    if (!chosen) {
      callback({});
      return;
    }
    if (chosen.shareAudio) await startShareAudioLoopback(chosen);
    callback({ video: chosen.source });
  });
}

// Screen video must not depend on the platform audio backend loading. A
// broken or missing audio module (e.g. the Windows native `.node` failing
// to load) downgrades this share to video-only, rather than propagating
// out of handler registration and leaving `getDisplayMedia` with no
// handler at all — which is silent, and Windows-only.
async function startShareAudioLoopback(chosen: ShareChoice): Promise<void> {
  try {
    const { startAudioLoopback, resolveAudioTarget } = await loadAudioBackend();
    const target = await resolveAudioTarget(chosen);
    if (!target) return;
    await startAudioLoopback(target);
    // The loopback is live: let the renderer's one follow-up `getUserMedia`
    // for the "Screen Share Mix" device through the permission lockdown
    // (see `main/permissions.ts`).
    armAudioCaptureGrant();
  } catch (err) {
    // A silent failure here (e.g. EACCES reading another process's
    // /proc/<pid>/exe under a restrictive ptrace_scope, or the Windows
    // addon missing) is near-impossible to diagnose from the outside.
    console.error('Audio loopback unavailable, sharing video only:', err);
  }
}
