import { session } from 'electron';

import { parseX11WindowId, resolveProcessBinary, resolveWindowPid } from '../../platform/linux/process-identity.js';
import { showSourcePicker } from './picker.js';
import type { AudioShareTarget, ShareChoice } from '../../ipc/types.js';

async function resolveLinuxAudioTarget(chosen: ShareChoice): Promise<AudioShareTarget | null> {
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

export async function registerDisplayMediaHandler(): Promise<void> {
  const isWindows = process.platform === 'win32';

  // Both dynamic — never statically imported — so a Linux process never
  // even evaluates `native/windows-audio/index.js`, which throws at
  // load time on any platform other than win32/x64.
  const { startAudioLoopback } = isWindows
    ? await import('../../platform/windows/audio.js')
    : await import('../../platform/linux/loopback.js');
  const windowsIdentity = isWindows ? await import('../../platform/windows/process-identity.js') : null;

  function resolveWindowsAudioTarget(chosen: ShareChoice): AudioShareTarget | null {
    if (!windowsIdentity) return null;
    if (chosen.source.id.startsWith('window:')) {
      const hwnd = windowsIdentity.parseWindowsWindowId(chosen.source.id);
      if (hwnd === null) return null;
      const pid = windowsIdentity.resolveWindowPid(hwnd);
      if (pid === null) return null;
      const binary = windowsIdentity.resolveExeNameForPid(pid);
      if (binary === null) return null;
      return { mode: 'window', binary };
    }
    return { mode: 'screen', excludedBinaries: chosen.excludedBinaries };
  }

  const resolveAudioTarget = isWindows
    ? (chosen: ShareChoice) => Promise.resolve(resolveWindowsAudioTarget(chosen))
    : resolveLinuxAudioTarget;

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
