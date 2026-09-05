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
let constructorError: Error | null = null;

vi.mock('electron', () => ({
  app: { isPackaged: false },
  desktopCapturer: { getSources: state.getSources },
  ipcMain: { once: vi.fn(), removeListener: vi.fn() },
  BrowserWindow: class {
    constructor() {
      if (constructorError) throw constructorError;
    }
    webContents = { setWindowOpenHandler: vi.fn(), send: sendMock };
    on(event: string, handler: Handler) {
      // Real Electron throws "Object has been destroyed" here.
      if (destroyed) throw new Error('Object has been destroyed');
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

vi.mock('#main/window.js', () => ({
  getMainWindow: () => undefined,
  lockNavigation: vi.fn(),
}));
vi.mock('#main/ipc-guard.js', () => ({ isTrustedFrame: () => true }));

import { ipcMain } from 'electron';

import { showSourcePicker } from '#features/screen-share/picker.js';

beforeEach(() => {
  vi.useFakeTimers();
  windowHandlers.clear();
  sendMock.mockReset();
  closeMock.mockReset();
  state.getSources.mockClear();
  state.loadFileDeferred = null;
  destroyed = false;
  constructorError = null;
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

  it('does not arm the blur handler if the window is already gone when the timer fires', async () => {
    const pending = showSourcePicker();
    await vi.advanceTimersByTimeAsync(0); // window up, blur-arm timer still pending

    destroyed = true; // window torn down before the 300ms timer
    state.loadFileDeferred?.reject(new Error('ERR_ABORTED'));
    await expect(pending).resolves.toBeNull();

    // Firing the timer must not throw "Object has been destroyed".
    await expect(vi.advanceTimersByTimeAsync(300)).resolves.not.toThrow();
    expect(windowHandlers.has('blur')).toBe(false);
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

  it('resolves null and logs when the picker window cannot be created', async () => {
    constructorError = new Error('Failed to create window');
    const pending = showSourcePicker();
    await vi.advanceTimersByTimeAsync(0);

    await expect(pending).resolves.toBeNull();
    expect(console.error).toHaveBeenCalledOnce();
    expect(sendMock).not.toHaveBeenCalled();
  });

  it('removes the picker:selected listener when the picker is dismissed (finding 8d)', async () => {
    const pending = showSourcePicker();
    await vi.advanceTimersByTimeAsync(300);

    const registered = (ipcMain.once as ReturnType<typeof vi.fn>).mock.calls.at(-1);
    expect(registered?.[0]).toBe('picker:selected');
    const handler = registered?.[1];

    windowHandlers.get('blur')?.(); // dismiss without ever sending picker:selected
    state.loadFileDeferred?.reject(new Error('ERR_ABORTED'));
    await expect(pending).resolves.toBeNull();

    expect(ipcMain.removeListener).toHaveBeenCalledWith('picker:selected', handler);
  });
});
