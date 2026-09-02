import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const pw = vi.hoisted(() => ({
  linkNodeToMix: vi.fn(),
  listAudioOutputStreams: vi.fn(),
  spawnMixProcess: vi.fn(),
  waitForMixSinkReady: vi.fn(),
}));
vi.mock('#platform/linux/pipewire.js', () => pw);
vi.mock('#platform/linux/process-identity.js', () => ({ OWN_BINARY_NAME: 'screen-share' }));

async function load() {
  vi.resetModules();
  return import('#platform/linux/loopback.js');
}

function fakeMixProcess() {
  return { kill: vi.fn(), on: vi.fn() };
}

beforeEach(() => {
  for (const fn of Object.values(pw)) fn.mockReset();
  pw.spawnMixProcess.mockReturnValue(fakeMixProcess());
  pw.waitForMixSinkReady.mockResolvedValue(undefined);
  pw.listAudioOutputStreams.mockResolvedValue([]);
});

describe('shouldIncludeFor', () => {
  it('never includes this app’s own playback, in either mode', async () => {
    const { shouldIncludeFor } = await load();
    expect(shouldIncludeFor({ mode: 'window', binary: 'screen-share' })('screen-share')).toBe(
      false,
    );
    expect(shouldIncludeFor({ mode: 'screen', excludedBinaries: [] })('screen-share')).toBe(false);
  });

  it('window mode includes only the target binary', async () => {
    const { shouldIncludeFor } = await load();
    const predicate = shouldIncludeFor({ mode: 'window', binary: 'chromium' });
    expect(predicate('chromium')).toBe(true);
    expect(predicate('spotify')).toBe(false);
    expect(predicate(null)).toBe(false);
  });

  it('screen mode includes everything except the excluded list, and fails open for unknown binaries', async () => {
    const { shouldIncludeFor } = await load();
    const predicate = shouldIncludeFor({
      mode: 'screen',
      excludedBinaries: ['discord', 'spotify'],
    });
    expect(predicate('chromium')).toBe(true);
    expect(predicate('discord')).toBe(false);
    expect(predicate(null)).toBe(true); // a node with no resolvable binary is still linked
  });
});

describe('startAudioLoopback / stopAudioLoopback', () => {
  afterEach(async () => {
    // Clear the module-level session + poll interval between tests.
    const { stopAudioLoopback } = await import('#platform/linux/loopback.js');
    stopAudioLoopback();
  });

  it('links every stream whose resolved binary passes the target predicate', async () => {
    pw.listAudioOutputStreams.mockResolvedValue([
      { id: 10, nodeName: 'Chromium', pid: 1, binary: 'chromium' },
      { id: 11, nodeName: 'Chromium', pid: null, binary: null }, // follower node, same name
      { id: 20, nodeName: 'Discord', pid: 2, binary: 'discord' },
      { id: 30, nodeName: 'own', pid: 3, binary: 'screen-share' },
    ]);
    const { startAudioLoopback } = await load();

    await startAudioLoopback({ mode: 'screen', excludedBinaries: ['discord'] });

    const linkedIds = pw.linkNodeToMix.mock.calls.map(([id]) => id).sort((a, b) => a - b);
    // Both Chromium nodes (the follower's binary resolves via its named
    // sibling), not Discord (excluded), not our own playback.
    expect(linkedIds).toEqual([10, 11]);
    expect(pw.spawnMixProcess).toHaveBeenCalledOnce();
    expect(pw.waitForMixSinkReady).toHaveBeenCalledOnce();
  });

  it('is a no-op when a session is already running', async () => {
    const { startAudioLoopback } = await load();
    await startAudioLoopback({ mode: 'screen', excludedBinaries: [] });
    await startAudioLoopback({ mode: 'screen', excludedBinaries: [] });
    expect(pw.spawnMixProcess).toHaveBeenCalledOnce();
  });

  it('kills the mix process and propagates the error if the sink never appears', async () => {
    const mix = fakeMixProcess();
    pw.spawnMixProcess.mockReturnValue(mix);
    pw.waitForMixSinkReady.mockRejectedValue(new Error('Timed out'));
    const { startAudioLoopback } = await load();

    await expect(startAudioLoopback({ mode: 'screen', excludedBinaries: [] })).rejects.toThrow(
      'Timed out',
    );
    expect(mix.kill).toHaveBeenCalledOnce();
  });

  it('stopAudioLoopback kills the mix process once, then no-ops', async () => {
    const mix = fakeMixProcess();
    pw.spawnMixProcess.mockReturnValue(mix);
    const { startAudioLoopback, stopAudioLoopback } = await load();
    await startAudioLoopback({ mode: 'screen', excludedBinaries: [] });

    stopAudioLoopback();
    stopAudioLoopback();
    expect(mix.kill).toHaveBeenCalledOnce();
  });

  it('isAudioLoopbackActive tracks the session', async () => {
    const mix = fakeMixProcess();
    pw.spawnMixProcess.mockReturnValue(mix);
    const { startAudioLoopback, stopAudioLoopback, isAudioLoopbackActive } = await load();

    expect(isAudioLoopbackActive()).toBe(false);
    await startAudioLoopback({ mode: 'screen', excludedBinaries: [] });
    expect(isAudioLoopbackActive()).toBe(true);
    stopAudioLoopback();
    expect(isAudioLoopbackActive()).toBe(false);
  });
});
