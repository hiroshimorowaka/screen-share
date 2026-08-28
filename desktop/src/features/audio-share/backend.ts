import type { AudioShareTarget, ShareChoice } from '../../ipc/types.js';

/** The platform's audio-loopback implementation: PipeWire on Linux,
 * WASAPI (via `native/windows-audio`) on Windows. `resolveAudioTarget`
 * turns a picked source into the target the loopback needs (which window
 * or which exclusions), using that platform's process-identity helpers.
 *
 * This interface is the single point where `process.platform` is
 * consulted for audio — every other module depends on `loadAudioBackend`,
 * not on a `win32` check of its own. */
export interface AudioBackend {
  startAudioLoopback(target: AudioShareTarget): Promise<void>;
  stopAudioLoopback(): void;
  listDistinctAudioApps(): Promise<{ binary: string; label: string }[]>;
  resolveAudioTarget(chosen: ShareChoice): Promise<AudioShareTarget | null>;
}

let cached: Promise<AudioBackend> | null = null;

/** Loads the platform backend once (memoized) and lazily: the Windows
 * modules pull in `native/windows-audio/index.js`, which throws at load
 * time on anything but win32/x64, so a Linux process must never evaluate
 * them — hence dynamic `import()`, never a static one. */
export function loadAudioBackend(): Promise<AudioBackend> {
  cached ??=
    process.platform === 'win32'
      ? Promise.all([
          import('../../platform/windows/audio.js'),
          import('../../platform/windows/process-identity.js'),
        ]).then(([audio, identity]): AudioBackend => ({ ...audio, ...identity }))
      : Promise.all([
          import('../../platform/linux/loopback.js'),
          import('../../platform/linux/pipewire.js'),
          import('../../platform/linux/process-identity.js'),
        ]).then(
          ([loopback, pipewire, identity]): AudioBackend => ({
            ...loopback,
            ...pipewire,
            ...identity,
          }),
        );
  return cached;
}
