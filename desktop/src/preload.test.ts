import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const exposed = new Map<string, Record<string, unknown>>();
const ipc = vi.hoisted(() => ({
  send: vi.fn(),
  invoke: vi.fn(),
  on: vi.fn(),
  once: vi.fn(),
  removeAllListeners: vi.fn(),
}));

vi.mock('electron', () => ({
  contextBridge: {
    exposeInMainWorld: (key: string, api: Record<string, unknown>) => {
      exposed.set(key, api);
    },
  },
  ipcRenderer: ipc,
}));

async function loadPreloadOn(platform: NodeJS.Platform) {
  const original = process.platform;
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
  vi.resetModules();
  exposed.clear();
  await import('#preload.js');
  Object.defineProperty(process, 'platform', { value: original, configurable: true });
}

beforeEach(() => {
  for (const fn of Object.values(ipc)) fn.mockReset();
});

afterEach(() => {
  exposed.clear();
});

describe('preload bridges', () => {
  it('exposes desktopShare / picker / desktopAudio with the expected shape', async () => {
    await loadPreloadOn('linux');

    expect([...exposed.keys()]).toEqual(
      expect.arrayContaining(['desktopShare', 'picker', 'desktopAudio']),
    );
    expect(Object.keys(exposed.get('desktopShare') ?? {})).toEqual([
      'linkReady',
      'memberJoined',
      'sharingChanged',
    ]);
    expect(Object.keys(exposed.get('picker') ?? {})).toEqual(
      expect.arrayContaining(['onSources', 'select', 'listAudioApps']),
    );
    expect(Object.keys(exposed.get('desktopAudio') ?? {})).toEqual(
      expect.arrayContaining(['start', 'stop']),
    );
  });

  it('forwards each bridge call to its matching ipc channel', async () => {
    await loadPreloadOn('linux');
    const share = exposed.get('desktopShare') as Record<string, (arg: unknown) => void>;

    share.linkReady('https://x/r/AB');
    expect(ipc.send).toHaveBeenCalledWith('desktop-share:link-ready', 'https://x/r/AB');

    share.memberJoined('Bia');
    expect(ipc.send).toHaveBeenCalledWith('desktop-share:member-joined', 'Bia');

    share.sharingChanged(true);
    expect(ipc.send).toHaveBeenCalledWith('desktop-share:sharing-changed', true);

    (exposed.get('picker') as Record<string, (arg: unknown) => void>).select({ sourceId: 's' });
    expect(ipc.send).toHaveBeenCalledWith('picker:selected', { sourceId: 's' });

    (exposed.get('desktopAudio') as Record<string, (arg: unknown) => void>).stop();
    expect(ipc.invoke).toHaveBeenCalledWith('stop-audio-loopback');
  });

  it('only exposes onPcmChunk on win32', async () => {
    await loadPreloadOn('linux');
    expect(exposed.get('desktopAudio')).not.toHaveProperty('onPcmChunk');

    await loadPreloadOn('win32');
    expect(exposed.get('desktopAudio')).toHaveProperty('onPcmChunk');
  });

  it('onSources registers a one-shot listener (finding 8c)', async () => {
    await loadPreloadOn('linux');
    const picker = exposed.get('picker') as Record<string, (cb: () => void) => void>;

    picker.onSources(() => {});
    picker.onSources(() => {});

    expect(ipc.once).toHaveBeenCalledTimes(2);
    expect(ipc.once).toHaveBeenCalledWith('picker:sources', expect.any(Function));
    expect(ipc.on).not.toHaveBeenCalledWith('picker:sources', expect.anything());
  });

  it('onPcmChunk clears the previous listener before re-adding, and offPcmChunk removes it (finding 8c)', async () => {
    await loadPreloadOn('win32');
    const audio = exposed.get('desktopAudio') as Record<string, (cb?: () => void) => void>;

    audio.onPcmChunk?.(() => {});
    audio.onPcmChunk?.(() => {});
    expect(ipc.removeAllListeners).toHaveBeenCalledWith('desktop-audio-pcm-chunk');
    expect(ipc.removeAllListeners).toHaveBeenCalledTimes(2);
    expect(ipc.on).toHaveBeenCalledTimes(2);

    audio.offPcmChunk?.();
    expect(ipc.removeAllListeners).toHaveBeenCalledTimes(3);
  });
});
