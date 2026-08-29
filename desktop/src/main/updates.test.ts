import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const m = vi.hoisted(() => ({
  isPackaged: true,
  checkForUpdatesAndNotify: vi.fn(() => Promise.resolve(null)),
  updaterOn: vi.fn(),
}));

vi.mock('electron', () => ({
  app: {
    get isPackaged() {
      return m.isPackaged;
    },
  },
}));

vi.mock('electron-updater', () => ({
  default: {
    autoUpdater: {
      on: m.updaterOn,
      checkForUpdatesAndNotify: m.checkForUpdatesAndNotify,
    },
  },
}));

import { setupAutoUpdates } from '#main/updates.js';

const realPlatform = process.platform;

function setPlatform(platform: NodeJS.Platform): void {
  Object.defineProperty(process, 'platform', { value: platform, configurable: true });
}

beforeEach(() => {
  vi.useFakeTimers();
  m.isPackaged = true;
  setPlatform('win32');
});

afterEach(() => {
  vi.useRealTimers();
  setPlatform(realPlatform);
});

describe('setupAutoUpdates', () => {
  it('checks for updates immediately and then on a recurring timer, on a packaged Windows build', () => {
    setupAutoUpdates();

    expect(m.checkForUpdatesAndNotify).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(6 * 60 * 60 * 1000);
    expect(m.checkForUpdatesAndNotify).toHaveBeenCalledTimes(2);
  });

  it('does nothing in an unpackaged dev run', () => {
    m.isPackaged = false;
    setupAutoUpdates();

    vi.advanceTimersByTime(24 * 60 * 60 * 1000);
    expect(m.checkForUpdatesAndNotify).not.toHaveBeenCalled();
  });

  it('does nothing on platforms whose installer cannot self-replace', () => {
    setPlatform('linux');
    setupAutoUpdates();

    setPlatform('darwin');
    setupAutoUpdates();

    vi.advanceTimersByTime(24 * 60 * 60 * 1000);
    expect(m.checkForUpdatesAndNotify).not.toHaveBeenCalled();
  });

  it('swallows a rejected update check instead of crashing the main process', async () => {
    m.checkForUpdatesAndNotify.mockRejectedValueOnce(new Error('offline'));
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});

    setupAutoUpdates();
    // Let the rejected promise's `.catch` handler run.
    await Promise.resolve();
    await Promise.resolve();

    expect(consoleError).toHaveBeenCalledWith('[updates] check failed:', expect.any(Error));
    consoleError.mockRestore();
  });
});
