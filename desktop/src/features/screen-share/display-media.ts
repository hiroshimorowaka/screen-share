import { session } from 'electron';

import { loadAudioBackend } from '#features/audio-share/backend.js';
import { showSourcePicker } from '#features/screen-share/picker.js';
import { armAudioCaptureGrant } from '#main/permissions.js';

export async function registerDisplayMediaHandler(): Promise<void> {
  const { startAudioLoopback, resolveAudioTarget } = await loadAudioBackend();

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
          // The loopback is live: let the renderer's one follow-up
          // `getUserMedia` for the "Screen Share Mix" device through the
          // permission lockdown (see `main/permissions.ts`).
          armAudioCaptureGrant();
        } catch (err) {
          // Proceed with video-only rather than failing the whole share,
          // but log it: a silent failure here (e.g. EACCES reading another
          // process's /proc/<pid>/exe under a restrictive ptrace_scope) is
          // near-impossible to diagnose from the outside.
          console.error('Failed to start audio loopback:', err);
        }
      }
    }
    callback({ video: chosen.source });
  });
}
