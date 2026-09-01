import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Handler = (...args: unknown[]) => void;

const state = vi.hoisted(() => ({
  getSources: vi.fn(async () => [] as unknown[]),
  loadFileDeferred: null as null | { resolve: () => void; reject: (err: unknown) => void },
}));

const windowHandlers = new Map<string, Handler>();
const sendMock = vi.fn();
const closeMock = vi.fn();
let destroyed = false;

vi.mock('electron', () => ({
  desktopCapturer: { getSources: state.getSources },
  ipcMain: { once: vi.fn() },
  BrowserWindow: class {
    webContents = { setWindowOpenHandler: vi.fn(), send: sendMock };
    on(event: string, handler: Handler) {
      windowHandlers.set(event, handler);
    }
    isDestroyed() {
      return destroyed;
    }
    close = closeMock;
    loadFile() {
      return new Promise<void>((resolve, reject) => {
        state.loadFileDeferred = { resolve, reject };
      });
    }
  },
}));

vi.mock('#main/window.js', () => ({ getMainWindow: () => undefined }));
vi.mock('#main/ipc-guard.js', () => ({ isTrustedFrame: () => true }));

import { showSourcePicker } from '#features/screen-share/picker.js';

beforeEach(() => {
  vi.useFakeTimers();
  windowHandlers.clear();
  sendMock.mockReset();
  closeMock.mockReset();
  state.getSources.mockClear();
  state.loadFileDeferred = null;
  destroyed = false;
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('showSourcePicker', () => {
  it('resolves null and logs once when the picker window fails to load', async () => {
    const pending = showSourcePicker();
    await vi.advanceTimersByTimeAsync(0); // let getSources resolve and the window come up

    state.loadFileDeferred?.reject(new Error('ERR_ABORTED (-3) loading picker.html'));

    await expect(pending).resolves.toBeNull();
    expect(console.error).toHaveBeenCalledOnce();
    expect(sendMock).not.toHaveBeenCalled();
  });

  it('stays quiet when the load is aborted because the user already dismissed it', async () => {
    const pending = showSourcePicker();
    await vi.advanceTimersByTimeAsync(300); // also arms the "click outside closes it" blur handler

    windowHandlers.get('blur')?.(); // click outside -> settle(null) -> window.close()
    state.loadFileDeferred?.reject(new Error('ERR_ABORTED (-3) loading picker.html'));

    await expect(pending).resolves.toBeNull();
    await vi.advanceTimersByTimeAsync(0);
    expect(console.error).not.toHaveBeenCalled();
    expect(sendMock).not.toHaveBeenCalled();
  });

  it('pushes the sources to the picker once it has loaded', async () => {
    const pending = showSourcePicker();
    await vi.advanceTimersByTimeAsync(0);

    state.loadFileDeferred?.resolve();
    await vi.advanceTimersByTimeAsync(0);
    windowHandlers.get('closed')?.(); // window eventually closes -> settle(null)

    await expect(pending).resolves.toBeNull();
    expect(sendMock).toHaveBeenCalledWith('picker:sources', []);
  });
});
