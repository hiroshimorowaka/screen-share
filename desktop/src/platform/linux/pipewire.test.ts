import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const runCollectingStdout = vi.hoisted(() => vi.fn());
const spawn = vi.hoisted(() => vi.fn(() => ({ on: vi.fn(), stdout: { on: vi.fn() } })));
vi.mock('#platform/run-command.js', () => ({ runCollectingStdout }));
vi.mock('node:child_process', () => ({ spawn }));

import {
  linkNodeToMix,
  listAudioOutputStreams,
  listDistinctAudioApps,
  spawnMixProcess,
  waitForMixSinkReady,
} from '#platform/linux/pipewire.js';

/** One `pw-dump` object with an output-audio stream's props. */
function stream(id: number, props: Record<string, unknown>) {
  return { id, info: { props: { 'media.class': 'Stream/Output/Audio', ...props } } };
}

beforeEach(() => {
  runCollectingStdout.mockReset();
  spawn.mockClear();
});

describe('listAudioOutputStreams', () => {
  it('keeps only Stream/Output/Audio objects and pulls out id/name/pid/binary', async () => {
    runCollectingStdout.mockResolvedValue(
      JSON.stringify([
        stream(41, {
          'node.name': 'Chromium',
          'application.process.id': '1234',
          'application.process.binary': 'chromium',
        }),
        { id: 9, info: { props: { 'media.class': 'Audio/Sink' } } }, // not an output stream
        stream(42, {}), // no optional props
      ]),
    );

    const streams = await listAudioOutputStreams();

    expect(streams).toEqual([
      { id: 41, nodeName: 'Chromium', pid: 1234, binary: 'chromium' },
      { id: 42, nodeName: null, pid: null, binary: null },
    ]);
  });

  it('returns an empty list when pw-dump output is not valid JSON', async () => {
    runCollectingStdout.mockResolvedValue('pw-dump: command not found');
    expect(await listAudioOutputStreams()).toEqual([]);
  });

  it('returns an empty list when pw-dump output is not an array', async () => {
    runCollectingStdout.mockResolvedValue('{"unexpected": true}');
    expect(await listAudioOutputStreams()).toEqual([]);
  });
});

describe('listDistinctAudioApps', () => {
  it('deduplicates by binary and drops streams with no binary', async () => {
    runCollectingStdout.mockResolvedValue(
      JSON.stringify([
        stream(1, { 'application.process.binary': 'chromium' }),
        stream(2, { 'application.process.binary': 'chromium' }),
        stream(3, { 'application.process.binary': 'spotify' }),
        stream(4, {}),
      ]),
    );

    expect(await listDistinctAudioApps()).toEqual([
      { binary: 'chromium', label: 'chromium' },
      { binary: 'spotify', label: 'spotify' },
    ]);
  });
});

describe('waitForMixSinkReady', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('resolves as soon as the mix sink node shows up in pw-dump', async () => {
    // `pw-dump` is pretty-printed, so the check looks for `"node.name": "…"`
    // with the space after the colon.
    runCollectingStdout.mockResolvedValue(
      '[ { "info": { "props": { "node.name": "screen_share_mix" } } } ]',
    );
    await expect(waitForMixSinkReady(1000)).resolves.toBeUndefined();
  });

  it('rejects once the timeout elapses without the node appearing', async () => {
    vi.useFakeTimers();
    runCollectingStdout.mockResolvedValue('[]');

    const pending = waitForMixSinkReady(250);
    const assertion = expect(pending).rejects.toThrow(/Timed out waiting for node/);
    await vi.advanceTimersByTimeAsync(400);
    await assertion;
  });
});

describe('linkNodeToMix / spawnMixProcess', () => {
  it('links a node into the mix sink by numeric id', () => {
    linkNodeToMix(42);
    expect(spawn).toHaveBeenCalledWith('pw-link', ['42', 'screen_share_mix']);
  });

  it('spawns pw-loopback with the mix sink + source node classes', () => {
    spawnMixProcess();
    const [cmd, args] = spawn.mock.calls[0] as [string, string[]];
    expect(cmd).toBe('pw-loopback');
    expect(args.join(' ')).toContain('media.class=Audio/Sink node.name=screen_share_mix');
    expect(args.join(' ')).toContain('media.class=Audio/Source node.name=screen_share_mix_out');
  });
});
