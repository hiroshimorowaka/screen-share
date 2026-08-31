import { session } from 'electron';

import { loadAudioBackend } from '#features/audio-share/backend.js';
import { showSourcePicker } from '#features/screen-share/picker.js';

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
        } catch (err) {
          // Proceed with video-only rather than failing the whole share,
          // but log the reason: an audio-loopback failure here (e.g.
          // EACCES reading another process's /proc/<pid>/exe under a
          // restrictive ptrace_scope) is otherwise invisible and hard to
          // diagnose.
          console.error('Failed to start audio loopback:', err);
        }
      }
    }
    callback({ video: chosen.source });
  });
}
