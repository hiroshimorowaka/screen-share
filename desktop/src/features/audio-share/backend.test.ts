import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('#platform/linux/loopback.js', () => ({
  startAudioLoopback: vi.fn(),
  stopAudioLoopback: vi.fn(),
}));
vi.mock('#platform/linux/pipewire.js', () => ({ listDistinctAudioApps: vi.fn() }));
vi.mock('#platform/linux/process-identity.js', () => ({ resolveAudioTarget: vi.fn() }));
vi.mock('#platform/windows/audio.js', () => ({
  startAudioLoopback: vi.fn(),
  stopAudioLoopback: vi.fn(),
  listDistinctAudioApps: vi.fn(),
}));
vi.mock('#platform/windows/process-identity.js', () => ({ resolveAudioTarget: vi.fn() }));

async function loadBackendOn(platform: NodeJS.Platform) {
  const original = process.platform;
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
  vi.resetModules();
  const mod = await import('#features/audio-share/backend.js');
  Object.defineProperty(process, 'platform', { value: original, configurable: true });
  return mod;
}

const AUDIO_BACKEND_KEYS = [
  'startAudioLoopback',
  'stopAudioLoopback',
  'listDistinctAudioApps',
  'resolveAudioTarget',
];

beforeEach(() => {
  vi.clearAllMocks();
});

describe('loadAudioBackend', () => {
  it('assembles the Linux backend from the pipewire modules', async () => {
    const { loadAudioBackend } = await loadBackendOn('linux');
    const backend = await loadAudioBackend();
    expect(Object.keys(backend)).toEqual(expect.arrayContaining(AUDIO_BACKEND_KEYS));
  });

  it('assembles the Windows backend from the WASAPI modules', async () => {
    const { loadAudioBackend } = await loadBackendOn('win32');
    const backend = await loadAudioBackend();
    expect(Object.keys(backend)).toEqual(expect.arrayContaining(AUDIO_BACKEND_KEYS));
  });

  it('memoizes: repeated calls return the same promise', async () => {
    const { loadAudioBackend } = await loadBackendOn('linux');
    expect(loadAudioBackend()).toBe(loadAudioBackend());
  });
});
