import { beforeEach, describe, expect, it, vi } from 'vitest';

const runCollectingStdout = vi.hoisted(() => vi.fn());
const fs = vi.hoisted(() => ({ readlink: vi.fn(), readFile: vi.fn() }));
vi.mock('#platform/run-command.js', () => ({ runCollectingStdout }));
vi.mock('node:fs/promises', () => fs);

import type { ShareChoice } from '#ipc/types.js';
import {
  parseX11WindowId,
  resolveAudioTarget,
  resolveWindowPid,
} from '#platform/linux/process-identity.js';

function choice(id: string, excludedBinaries: string[] = []): ShareChoice {
  return {
    source: { id } as Electron.DesktopCapturerSource,
    shareAudio: true,
    excludedBinaries,
  };
}

beforeEach(() => {
  runCollectingStdout.mockReset();
  fs.readlink.mockReset();
  fs.readFile.mockReset();
});

describe('parseX11WindowId', () => {
  it('extracts the numeric id from a window source id', () => {
    expect(parseX11WindowId('window:12345:0')).toBe(12345);
  });

  it('returns null for a screen source id', () => {
    expect(parseX11WindowId('screen:0:0')).toBeNull();
  });
});

describe('resolveWindowPid', () => {
  it('parses the pid out of xprop _NET_WM_PID output', async () => {
    runCollectingStdout.mockResolvedValue('_NET_WM_PID(CARDINAL) = 4242\n');
    expect(await resolveWindowPid(99)).toBe(4242);
    expect(runCollectingStdout).toHaveBeenCalledWith('xprop', ['-id', '99', '_NET_WM_PID']);
  });

  it('returns null when xprop prints no pid', async () => {
    runCollectingStdout.mockResolvedValue('');
    expect(await resolveWindowPid(99)).toBeNull();
  });
});

describe('resolveAudioTarget', () => {
  it('passes the exclusion list straight through for a whole-screen share', async () => {
    const target = await resolveAudioTarget(choice('screen:0:0', ['discord', 'spotify']));
    expect(target).toEqual({ mode: 'screen', excludedBinaries: ['discord', 'spotify'] });
  });

  it('resolves a window share to its owning process binary', async () => {
    runCollectingStdout.mockResolvedValue('_NET_WM_PID(CARDINAL) = 777\n');
    fs.readlink.mockResolvedValue('/usr/lib/chromium/chromium');

    expect(await resolveAudioTarget(choice('window:55:0'))).toEqual({
      mode: 'window',
      binary: 'chromium',
    });
  });

  it('falls back to /proc/<pid>/cmdline argv[0] when the exe symlink is unreadable', async () => {
    runCollectingStdout.mockResolvedValue('_NET_WM_PID(CARDINAL) = 777\n');
    fs.readlink.mockRejectedValue(new Error('EACCES'));
    fs.readFile.mockResolvedValue('/opt/spotify/spotify\0--flag\0');

    expect(await resolveAudioTarget(choice('window:55:0'))).toEqual({
      mode: 'window',
      binary: 'spotify',
    });
  });

  it('returns null (share proceeds video-only) when the window owner cannot be identified', async () => {
    runCollectingStdout.mockResolvedValue(''); // no pid from xprop
    expect(await resolveAudioTarget(choice('window:55:0'))).toBeNull();
  });
});
