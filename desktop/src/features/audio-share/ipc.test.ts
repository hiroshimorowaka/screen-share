import { beforeEach, describe, expect, it, vi } from 'vitest';

const handle = vi.hoisted(() => vi.fn());
const backend = vi.hoisted(() => ({
  startAudioLoopback: vi.fn(),
  stopAudioLoopback: vi.fn(),
  listDistinctAudioApps: vi.fn(),
  resolveAudioTarget: vi.fn(),
}));
const loadAudioBackend = vi.hoisted(() => vi.fn(async () => backend));

vi.mock('electron', () => ({ ipcMain: { handle } }));
vi.mock('#features/audio-share/backend.js', () => ({ loadAudioBackend }));

// `stopActiveAudioLoopback` is module-level; reset the module per test.
async function freshIpc() {
  vi.resetModules();
  return import('#features/audio-share/ipc.js');
}

beforeEach(() => {
  handle.mockReset();
  for (const fn of Object.values(backend)) fn.mockReset();
});

describe('registerAudioIpcHandlers', () => {
  it('binds the start / stop / list channels to the platform backend', async () => {
    const { registerAudioIpcHandlers } = await freshIpc();
    await registerAudioIpcHandlers();

    const channels = handle.mock.calls.map(([channel]) => channel);
    expect(channels).toEqual(
      expect.arrayContaining(['start-audio-loopback', 'stop-audio-loopback', 'list-audio-apps']),
    );

    const startHandler = handle.mock.calls.find(([c]) => c === 'start-audio-loopback')?.[1];
    startHandler?.({}, { mode: 'screen', excludedBinaries: [] });
    expect(backend.startAudioLoopback).toHaveBeenCalledWith({
      mode: 'screen',
      excludedBinaries: [],
    });
  });
});

describe('stopAudioLoopbackNow', () => {
  it('is a safe no-op before the backend has been registered', async () => {
    const { stopAudioLoopbackNow } = await freshIpc();
    expect(() => stopAudioLoopbackNow()).not.toThrow();
    expect(backend.stopAudioLoopback).not.toHaveBeenCalled();
  });

  it('calls the backend stop synchronously once handlers are registered', async () => {
    const { registerAudioIpcHandlers, stopAudioLoopbackNow } = await freshIpc();
    await registerAudioIpcHandlers();

    stopAudioLoopbackNow();
    expect(backend.stopAudioLoopback).toHaveBeenCalledOnce();
  });
});
