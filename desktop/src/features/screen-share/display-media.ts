import { session } from 'electron';

import { loadAudioBackend } from '../audio-share/backend.js';
import { showSourcePicker } from './picker.js';

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
          // but this shouldn't be silent — a failure here previously had
          // no visible signal at all, which made a real bug (EACCES
          // reading another process's /proc/<pid>/exe under this
          // machine's ptrace_scope) far harder to track down than it
          // needed to be.
          console.error('Failed to start audio loopback:', err);
        }
      }
    }
    callback({ video: chosen.source });
  });
}
