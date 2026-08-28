import { beforeEach, describe, expect, it, vi } from 'vitest';

const m = vi.hoisted(() => ({
  setToolTip: vi.fn(),
  setContextMenu: vi.fn(),
  trayOn: vi.fn(),
  buildFromTemplate: vi.fn((t: unknown) => ({ __menu: t })),
  showMainWindow: vi.fn(),
  startQuickShare: vi.fn(),
  requestQuit: vi.fn(),
}));

vi.mock('electron', () => ({
  Tray: class {
    setToolTip = m.setToolTip;
    setContextMenu = m.setContextMenu;
    on = m.trayOn;
  },
  Menu: { buildFromTemplate: m.buildFromTemplate },
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
});
