import { beforeEach, describe, expect, it, vi } from 'vitest';

import { APP_ORIGIN } from '#main/app-url.js';

const FROM_APP = { senderFrame: { url: `${APP_ORIGIN}/room` } };

const m = vi.hoisted(() => ({
  setToolTip: vi.fn(),
  setContextMenu: vi.fn(),
  setImage: vi.fn(),
  trayOn: vi.fn(),
  ipcOn: vi.fn(),
  buildFromTemplate: vi.fn((t: unknown) => ({ __menu: t })),
  showMainWindow: vi.fn(),
  startQuickShare: vi.fn(),
  requestQuit: vi.fn(),
}));

vi.mock('electron', () => ({
  Tray: class {
    setToolTip = m.setToolTip;
    setContextMenu = m.setContextMenu;
    setImage = m.setImage;
    on = m.trayOn;
  },
  Menu: { buildFromTemplate: m.buildFromTemplate },
  ipcMain: { on: m.ipcOn },
}));
vi.mock('#main/window.js', () => ({
  showMainWindow: m.showMainWindow,
  startQuickShare: m.startQuickShare,
}));
vi.mock('#main/lifecycle.js', () => ({ requestQuit: m.requestQuit }));

import { createTray } from '#main/tray.js';

beforeEach(() => {
  for (const fn of Object.values(m)) fn.mockReset();
  m.buildFromTemplate.mockImplementation((t: unknown) => ({ __menu: t }));
});

describe('createTray', () => {
  it('builds the Abrir / Compartilhar tela / Sair menu and wires each action', () => {
    createTray();

    const template = m.buildFromTemplate.mock.calls[0]?.[0] as {
      label: string;
      click: () => void;
    }[];
    expect(template.map((i) => i.label)).toEqual(['Abrir', 'Compartilhar tela', 'Sair']);

    template[0].click();
    template[1].click();
    template[2].click();
    expect(m.showMainWindow).toHaveBeenCalledOnce();
    expect(m.startQuickShare).toHaveBeenCalledOnce();
    expect(m.requestQuit).toHaveBeenCalledOnce();
  });

  it('opens the main window on a tray click', () => {
    createTray();
    const clickHandler = m.trayOn.mock.calls.find(([evt]) => evt === 'click')?.[1] as () => void;
    clickHandler();
    expect(m.showMainWindow).toHaveBeenCalledOnce();
  });

  it('swaps the tray icon to the live (red) dot while sharing and back to idle after', () => {
    createTray();
    m.setImage.mockClear();

    const onSharingChanged = m.ipcOn.mock.calls.find(
      ([channel]) => channel === 'desktop-share:sharing-changed',
    )?.[1] as (event: unknown, isSharing: boolean) => void;

    onSharingChanged(FROM_APP, true);
    expect(m.setImage).toHaveBeenLastCalledWith(expect.stringContaining('tray-live.png'));

    onSharingChanged(FROM_APP, false);
    expect(m.setImage).toHaveBeenLastCalledWith(expect.stringContaining('tray-idle.png'));
  });
});
