import type { AudioShareTarget } from '#ipc/types.js';
import { getMainWindow } from '#main/window.js';
import { listActiveAudioProcesses, WindowsAudioSession } from '#native/windows-audio/index.js';

let activeSession: WindowsAudioSession | null = null;

/** Converts one mixed PCM chunk (a Node `Buffer` — a view over a possibly
 * shared, possibly larger underlying pool buffer) into a standalone
 * `ArrayBuffer` before it crosses the IPC bridge — the plain, transferable
 * type `contextBridge` can actually clone, unlike a `Buffer`/`Uint8Array`
 * view whose backing storage the renderer has no business touching. */
function toArrayBuffer(chunk: Buffer): ArrayBuffer {
  // `Buffer.buffer` is typed `ArrayBufferLike` (it could in principle be
  // backed by a `SharedArrayBuffer`) but a `Buffer` handed to us straight
  // from a napi callback is never one — safe to narrow.
  return chunk.buffer.slice(chunk.byteOffset, chunk.byteOffset + chunk.byteLength) as ArrayBuffer;
}

export function startAudioLoopback(target: AudioShareTarget): Promise<void> {
  if (activeSession) return Promise.resolve();

  const targetName = target.mode === 'window' ? target.binary : null;
  const excludedNames = target.mode === 'screen' ? target.excludedBinaries : [];

  const session = new WindowsAudioSession();
  session.start(target.mode, targetName, excludedNames, (err, chunk) => {
    if (err) return;
    getMainWindow()?.webContents.send('desktop-audio-pcm-chunk', toArrayBuffer(chunk));
  });
  activeSession = session;
  return Promise.resolve();
}

export function stopAudioLoopback(): void {
  if (!activeSession) return;
  activeSession.stop();
  activeSession = null;
}

export function isAudioLoopbackActive(): boolean {
  return activeSession !== null;
}

/** Every currently playing app, one entry per distinct resolved
 * executable name (already deduplicated on the Rust side) — what the
 * picker shows in its exclusion list. Mirrors Linux's
 * `listDistinctAudioApps` in `platform/linux/pipewire.ts`. */
export function listDistinctAudioApps(): Promise<{ binary: string; label: string }[]> {
  const processes = listActiveAudioProcesses();
  return Promise.resolve(
    processes.map((process) => ({ binary: process.exeName, label: process.exeName })),
  );
}
