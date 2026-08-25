import type { ChildProcess } from 'child_process';

import type { AudioShareTarget } from '../shared-types.js';
import { linkNodeToMix, listAudioOutputStreams, spawnMixProcess, waitForMixSinkReady } from './pipewire.js';
import { OWN_BINARY_NAME } from './process-identity.js';

interface AudioLoopbackSession {
  mixProcess: ChildProcess;
  pollInterval: NodeJS.Timeout;
  shouldInclude: (binary: string | null) => boolean;
}

let audioSession: AudioLoopbackSession | null = null;

function shouldIncludeFor(target: AudioShareTarget): (binary: string | null) => boolean {
  const isOwnPlayback = (binary: string | null) => binary === OWN_BINARY_NAME;
  if (target.mode === 'window') {
    return (binary) => !isOwnPlayback(binary) && binary === target.binary;
  }
  return (binary) =>
    !isOwnPlayback(binary) && (!binary || !target.excludedBinaries.includes(binary));
}

/** A single logical app's audio can show up as more than one PipeWire
 * node sharing the same node.name — e.g. Spotify always splits into a
 * named client node (which carries application.process.binary but has
 * no ports of its own) and a separate adapter/follower node (which owns
 * the actual linkable ports but has no binary). Deciding inclusion per
 * individual stream entry meant the follower's missing binary always
 * fell through the "unknown app" fail-open case, so excluding an app by
 * binary silently kept linking its real ports anyway. Resolving one
 * binary per node.name — from whichever entry sharing that name
 * actually has it — makes the decision consistent across every node
 * backing the same app.
 *
 * Every included node is (re-)linked on every poll rather than once:
 * Chromium-based apps tear down and recreate their audio stream node
 * after a period of silence (confirmed live — a paused/idle tab's node
 * disappears and a fresh one appears on the next playback), so "link
 * once and remember the name" left later replacement nodes never linked
 * at all. Re-linking an already-connected pair just fails harmlessly
 * (the exit code isn't checked), so this is safe to redo every second. */
async function scanAndLink(): Promise<void> {
  if (!audioSession) return;
  const streams = await listAudioOutputStreams();

  const binaryByName = new Map<string, string | null>();
  for (const stream of streams) {
    if (!stream.nodeName) continue;
    if (stream.binary) {
      binaryByName.set(stream.nodeName, stream.binary);
    } else if (!binaryByName.has(stream.nodeName)) {
      binaryByName.set(stream.nodeName, null);
    }
  }

  for (const nodeName of binaryByName.keys()) {
    if (!audioSession.shouldInclude(binaryByName.get(nodeName) ?? null)) continue;
    await linkNodeToMix(nodeName);
  }
}

export async function startAudioLoopback(target: AudioShareTarget): Promise<void> {
  if (audioSession) return;
  // Computed before spawning anything: a malformed `target` must fail
  // here, not after the mix process is already running with nothing
  // left to kill it.
  const shouldInclude = shouldIncludeFor(target);
  const mixProcess = spawnMixProcess();
  try {
    await waitForMixSinkReady(3000);
  } catch (err) {
    mixProcess.kill();
    throw err;
  }

  const session: AudioLoopbackSession = {
    mixProcess,
    shouldInclude,
    pollInterval: setInterval(() => {
      void scanAndLink();
    }, 1000),
  };
  audioSession = session;
  mixProcess.on('exit', () => {
    if (audioSession === session) {
      clearInterval(session.pollInterval);
      audioSession = null;
    }
  });

  await scanAndLink();
}

export function stopAudioLoopback(): void {
  if (!audioSession) return;
  clearInterval(audioSession.pollInterval);
  audioSession.mixProcess.kill();
  audioSession = null;
}
