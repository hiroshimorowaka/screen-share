import { beforeEach, describe, expect, it, vi } from 'vitest';

const win = vi.hoisted(() => ({
  loadURL: vi.fn(),
  on: vi.fn(),
  show: vi.fn(),
  focus: vi.fn(),
  hide: vi.fn(),
}));
const isQuitting = vi.hoisted(() => vi.fn(() => false));

vi.mock('electron', () => ({
  BrowserWindow: class {
    loadURL = win.loadURL;
    on = win.on;
    show = win.show;
    focus = win.focus;
    hide = win.hide;
  },
}));
vi.mock('#main/lifecycle.js', () => ({ isQuitting }));

async function freshWindow() {
  vi.resetModules();
  return import('#main/window.js');
}

beforeEach(() => {
  for (const fn of Object.values(win)) fn.mockReset();
  isQuitting.mockReset().mockReturnValue(false);
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
