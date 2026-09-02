import { beforeEach, describe, expect, it, vi } from 'vitest';

const win = vi.hoisted(() => ({
  loadURL: vi.fn(),
  on: vi.fn(),
  show: vi.fn(),
  focus: vi.fn(),
  hide: vi.fn(),
  wcOn: vi.fn(),
  setWindowOpenHandler: vi.fn(),
  toggleDevTools: vi.fn(),
  lastWebPreferences: undefined as Record<string, unknown> | undefined,
}));
const electronApp = vi.hoisted(() => ({ isPackaged: true }));
const isQuitting = vi.hoisted(() => vi.fn(() => false));
const openExternal = vi.hoisted(() => vi.fn());

vi.mock('electron', () => ({
  app: electronApp,
  BrowserWindow: class {
    constructor(opts?: { webPreferences?: Record<string, unknown> }) {
      win.lastWebPreferences = opts?.webPreferences;
    }
    loadURL = win.loadURL;
    on = win.on;
    show = win.show;
    focus = win.focus;
    hide = win.hide;
    webContents = {
      on: win.wcOn,
      setWindowOpenHandler: win.setWindowOpenHandler,
      toggleDevTools: win.toggleDevTools,
    };
  },
  shell: { openExternal },
}));
vi.mock('#main/lifecycle.js', () => ({ isQuitting }));

const stopAudioLoopbackNow = vi.hoisted(() => vi.fn());
vi.mock('#features/audio-share/ipc.js', () => ({ stopAudioLoopbackNow }));

async function freshWindow() {
  vi.resetModules();
  return import('#main/window.js');
}

beforeEach(() => {
  for (const fn of Object.values(win)) if (typeof fn === 'function') fn.mockReset();
  win.lastWebPreferences = undefined;
  electronApp.isPackaged = true;
  isQuitting.mockReset().mockReturnValue(false);
  openExternal.mockReset();
  stopAudioLoopbackNow.mockReset();
});

describe('window', () => {
  it('getMainWindow is null before the window is created', async () => {
    const { getMainWindow } = await freshWindow();
    expect(getMainWindow()).toBeNull();
  });

  it('startQuickShare is a no-op with no window', async () => {
    const { startQuickShare } = await freshWindow();
    startQuickShare();
    expect(win.loadURL).not.toHaveBeenCalled();
  });

  it('createMainWindow loads the prod URL and startQuickShare reloads it with the quick_share flag', async () => {
    const { createMainWindow, startQuickShare } = await freshWindow();
    createMainWindow();
    expect(win.loadURL).toHaveBeenCalledWith(expect.stringMatching(/^https:\/\/.*fly\.dev\/$/));

    startQuickShare();
    expect(win.loadURL).toHaveBeenLastCalledWith(expect.stringContaining('quick_share=1'));
  });

  it('blocks cross-origin navigation and window.open, allows the app origin (F10)', async () => {
    const { createMainWindow } = await freshWindow();
    createMainWindow();

    const appUrl = win.loadURL.mock.calls[0]?.[0] as string;
    const appOrigin = new URL(appUrl).origin;

    const willNavigate = win.wcOn.mock.calls.find(([evt]) => evt === 'will-navigate')?.[1] as (
      e: { preventDefault: () => void },
      url: string,
    ) => void;

    const blocked = { preventDefault: vi.fn() };
    willNavigate(blocked, 'https://evil.example/phish');
    expect(blocked.preventDefault).toHaveBeenCalledOnce();

    const allowed = { preventDefault: vi.fn() };
    willNavigate(allowed, `${appOrigin}/room/ABCD`);
    expect(allowed.preventDefault).not.toHaveBeenCalled();

    const openHandler = win.setWindowOpenHandler.mock.calls[0]?.[0] as (a: { url: string }) => {
      action: string;
    };
    expect(openHandler({ url: 'https://example.com' })).toEqual({ action: 'deny' });
    expect(openExternal).toHaveBeenCalledWith('https://example.com');
  });

  it('a packaged build disables DevTools and binds no toggle shortcut', async () => {
    electronApp.isPackaged = true;
    const { createMainWindow } = await freshWindow();
    createMainWindow();

    expect(win.lastWebPreferences?.devTools).toBe(false);
    expect(win.wcOn.mock.calls.some(([evt]) => evt === 'before-input-event')).toBe(false);
  });

  it('a dev build enables DevTools and toggles them on F12 / Ctrl+Shift+I', async () => {
    electronApp.isPackaged = false;
    const { createMainWindow } = await freshWindow();
    createMainWindow();

    expect(win.lastWebPreferences?.devTools).toBe(true);
    const onInput = win.wcOn.mock.calls.find(([evt]) => evt === 'before-input-event')?.[1] as (
      e: unknown,
      input: Record<string, unknown>,
    ) => void;

    onInput({}, { type: 'keyUp', key: 'F12' });
    expect(win.toggleDevTools).not.toHaveBeenCalled();

    onInput({}, { type: 'keyDown', key: 'F12' });
    onInput({}, { type: 'keyDown', key: 'I', control: true, shift: true });
    expect(win.toggleDevTools).toHaveBeenCalledTimes(2);

    onInput({}, { type: 'keyDown', key: 'I', control: true, shift: false });
    expect(win.toggleDevTools).toHaveBeenCalledTimes(2);
  });

  it('stops the audio loopback when the main frame navigates away, is destroyed, or crashes (finding 7)', async () => {
    const { createMainWindow } = await freshWindow();
    createMainWindow();

    const nav = win.wcOn.mock.calls.find(([evt]) => evt === 'did-start-navigation')?.[1] as (d: {
      isMainFrame: boolean;
      isSameDocument: boolean;
    }) => void;
    nav({ isMainFrame: true, isSameDocument: true }); // SPA route change: ignored
    expect(stopAudioLoopbackNow).not.toHaveBeenCalled();
    nav({ isMainFrame: false, isSameDocument: false }); // subframe: ignored
    expect(stopAudioLoopbackNow).not.toHaveBeenCalled();
    nav({ isMainFrame: true, isSameDocument: false }); // real reload / quick-share loadURL
    expect(stopAudioLoopbackNow).toHaveBeenCalledTimes(1);

    win.wcOn.mock.calls.find(([evt]) => evt === 'destroyed')?.[1]();
    win.wcOn.mock.calls.find(([evt]) => evt === 'render-process-gone')?.[1]();
    expect(stopAudioLoopbackNow).toHaveBeenCalledTimes(3);
  });

  it('the close handler hides the window unless the app is really quitting', async () => {
    const { createMainWindow } = await freshWindow();
    createMainWindow();
    const closeHandler = win.on.mock.calls.find(([evt]) => evt === 'close')?.[1] as (e: {
      preventDefault: () => void;
    }) => void;

    const notQuitting = { preventDefault: vi.fn() };
    closeHandler(notQuitting);
    expect(notQuitting.preventDefault).toHaveBeenCalledOnce();
    expect(win.hide).toHaveBeenCalledOnce();

    isQuitting.mockReturnValue(true);
    const quitting = { preventDefault: vi.fn() };
    closeHandler(quitting);
    expect(quitting.preventDefault).not.toHaveBeenCalled();
    expect(win.hide).toHaveBeenCalledOnce(); // unchanged
  });
});
