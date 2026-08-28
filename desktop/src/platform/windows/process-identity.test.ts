import { beforeEach, describe, expect, it, vi } from 'vitest';

// The native addon throws at load time off win32/x64, so it must be
// mocked before `#platform/windows/process-identity.js` pulls it in.
const getPidForWindow = vi.hoisted(() => vi.fn());
const getExeNameForPid = vi.hoisted(() => vi.fn());
vi.mock('#native/windows-audio/index.js', () => ({
  getPidForWindow,
  getExeNameForPid,
  listActiveAudioProcesses: vi.fn(),
  WindowsAudioSession: class {},
}));

import type { ShareChoice } from '#ipc/types.js';
import { parseWindowsWindowId, resolveAudioTarget } from '#platform/windows/process-identity.js';

function choice(id: string, excludedBinaries: string[] = []): ShareChoice {
  return {
    source: { id } as Electron.DesktopCapturerSource,
    shareAudio: true,
    excludedBinaries,
  };
}

beforeEach(() => {
  getPidForWindow.mockReset();
  getExeNameForPid.mockReset();
});

describe('parseWindowsWindowId', () => {
  it('extracts the numeric hwnd from a window source id', () => {
    expect(parseWindowsWindowId('window:987654:0')).toBe(987654);
  });

  it('returns null for a screen source id', () => {
    expect(parseWindowsWindowId('screen:0:0')).toBeNull();
  });
});

describe('resolveAudioTarget (windows)', () => {
  it('passes the exclusion list through for a whole-screen share', async () => {
    expect(await resolveAudioTarget(choice('screen:0:0', ['Discord.exe']))).toEqual({
      mode: 'screen',
      excludedBinaries: ['Discord.exe'],
    });
  });

  it('resolves a window share to its owning executable name', async () => {
    getPidForWindow.mockReturnValue(4242);
    getExeNameForPid.mockReturnValue('chrome.exe');

    expect(await resolveAudioTarget(choice('window:12:0'))).toEqual({
      mode: 'window',
      binary: 'chrome.exe',
    });
    expect(getPidForWindow).toHaveBeenCalledWith(12);
    expect(getExeNameForPid).toHaveBeenCalledWith(4242);
  });

  it('returns null when the window pid or exe name is unavailable', async () => {
    getPidForWindow.mockReturnValue(null);
    expect(await resolveAudioTarget(choice('window:12:0'))).toBeNull();

    getPidForWindow.mockReturnValue(4242);
    getExeNameForPid.mockReturnValue(null);
    expect(await resolveAudioTarget(choice('window:12:0'))).toBeNull();
  });
});
