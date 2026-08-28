import { ChildProcess, spawn } from 'child_process';

import { runCollectingStdout } from '../run-command.js';

/** Name of the virtual sink apps get pw-link'd into, and of its paired
 * Audio/Source — see `spawnMixProcess` for why these are two separate
 * PipeWire nodes rather than one. */
const MIX_SINK_NAME = 'screen_share_mix';
const MIX_SOURCE_NAME = 'screen_share_mix_out';
const MIX_PLAYBACK_PORTS = [
  `${MIX_SINK_NAME}:playback_FL`,
  `${MIX_SINK_NAME}:playback_FR`,
];

export interface AudioStreamInfo {
  id: number;
  nodeName: string | null;
  pid: number | null;
  binary: string | null;
}

export async function listAudioOutputStreams(): Promise<AudioStreamInfo[]> {
  const output = await runCollectingStdout('pw-dump', []);
  let data: unknown;
  try {
    data = JSON.parse(output);
  } catch {
    return [];
  }
  if (!Array.isArray(data)) return [];

  const streams: AudioStreamInfo[] = [];
  for (const obj of data) {
    const props = (obj as { info?: { props?: Record<string, unknown> } })?.info?.props;
    if (!props || props['media.class'] !== 'Stream/Output/Audio') continue;
    streams.push({
      id: (obj as { id: number }).id,
      nodeName: typeof props['node.name'] === 'string' ? (props['node.name'] as string) : null,
      pid:
        props['application.process.id'] !== undefined
          ? Number(props['application.process.id'])
          : null,
      binary:
        typeof props['application.process.binary'] === 'string'
          ? (props['application.process.binary'] as string)
          : null,
    });
  }
  return streams;
}

/** Every currently playing app, deduplicated by binary — what the picker
 * shows in its exclusion list. */
export async function listDistinctAudioApps(): Promise<{ binary: string; label: string }[]> {
  const streams = await listAudioOutputStreams();
  const seen = new Set<string>();
  const apps: { binary: string; label: string }[] = [];
  for (const stream of streams) {
    if (!stream.binary || seen.has(stream.binary)) continue;
    seen.add(stream.binary);
    apps.push({ binary: stream.binary, label: stream.binary });
  }
  return apps;
}

async function isNodeNamePresent(nodeName: string): Promise<boolean> {
  const output = await runCollectingStdout('pw-dump', []);
  return output.includes(`"node.name": "${nodeName}"`);
}

export async function waitForMixSinkReady(timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await isNodeNamePresent(MIX_SINK_NAME)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for node "${MIX_SINK_NAME}" to appear`);
}

async function listOutputPorts(nodeName: string): Promise<string[]> {
  const output = await runCollectingStdout('pw-link', ['-o']);
  const prefix = `${nodeName}:`;
  return output
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith(prefix));
}

export async function linkNodeToMix(nodeName: string): Promise<void> {
  const outputs = await listOutputPorts(nodeName);
  const count = Math.min(outputs.length, MIX_PLAYBACK_PORTS.length);
  for (let i = 0; i < count; i++) {
    spawn('pw-link', [outputs[i], MIX_PLAYBACK_PORTS[i]]);
  }
}

/** Creates the virtual mix: a Sink (`screen_share_mix`) that other apps'
 * audio gets pw-link'd into, paired with an explicit Audio/Source
 * (`screen_share_mix_out`) that the browser actually captures from.
 *
 * The playback side must be a real Audio/Source (not left to rely on the
 * sink's implicit monitor): on stock PipeWire/WirePlumber configs the
 * monitor of an ad-hoc sink like this one is never exposed as a
 * capturable input device to browser clients, only as an audiooutput —
 * confirmed by enumerating devices in a real Chromium renderer.
 * `node.autoconnect=false` keeps WirePlumber from wiring this playback
 * stream into the real default sink, which otherwise doubled every
 * linked app's audio onto real speakers the moment the mix was created
 * (reproduced live: the stream appeared connected to the hardware sink
 * before any app was even linked in). */
export function spawnMixProcess(): ChildProcess {
  return spawn('pw-loopback', [
    '--capture-props',
    `media.class=Audio/Sink node.name=${MIX_SINK_NAME} node.description="Screen Share Mix"`,
    '--playback-props',
    `media.class=Audio/Source node.name=${MIX_SOURCE_NAME} node.description="Screen Share Mix" node.passive=true node.autoconnect=false`,
  ]);
}
