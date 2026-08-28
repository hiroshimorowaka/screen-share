import { beforeEach, describe, expect, it, vi } from 'vitest';

const native = vi.hoisted(() => ({
  listActiveAudioProcesses: vi.fn(),
  start: vi.fn(),
  stop: vi.fn(),
}));
vi.mock('#native/windows-audio/index.js', () => ({
  listActiveAudioProcesses: native.listActiveAudioProcesses,
  getPidForWindow: vi.fn(),
  getExeNameForPid: vi.fn(),
  WindowsAudioSession: class {
    start = native.start;
    stop = native.stop;
  },
}));
vi.mock('#main/window.js', () => ({ getMainWindow: () => null }));

async function freshAudio() {
  vi.resetModules();
  return import('#platform/windows/audio.js');
}

beforeEach(() => {
  for (const fn of Object.values(native)) fn.mockReset();
});

describe('windows audio', () => {
  it('listDistinctAudioApps maps each active process to a {binary,label}', async () => {
    native.listActiveAudioProcesses.mockReturnValue([
      { exeName: 'chrome.exe' },
      { exeName: 'Spotify.exe' },
    ]);
    const { listDistinctAudioApps } = await freshAudio();

    expect(await listDistinctAudioApps()).toEqual([
      { binary: 'chrome.exe', label: 'chrome.exe' },
      { binary: 'Spotify.exe', label: 'Spotify.exe' },
    ]);
  });

  it('startAudioLoopback starts one native session and ignores a second start', async () => {
    const { startAudioLoopback } = await freshAudio();
    await startAudioLoopback({ mode: 'window', binary: 'chrome.exe' });
    await startAudioLoopback({ mode: 'window', binary: 'chrome.exe' });
    expect(native.start).toHaveBeenCalledOnce();
    expect(native.start).toHaveBeenCalledWith('window', 'chrome.exe', [], expect.any(Function));
  });

  it('stopAudioLoopback stops the active session and is a no-op afterwards', async () => {
    const { startAudioLoopback, stopAudioLoopback } = await freshAudio();
    await startAudioLoopback({ mode: 'screen', excludedBinaries: ['Discord.exe'] });

    stopAudioLoopback();
    stopAudioLoopback();
    expect(native.stop).toHaveBeenCalledOnce();
  });
});
