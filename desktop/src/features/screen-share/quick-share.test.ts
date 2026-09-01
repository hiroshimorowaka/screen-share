import { beforeEach, describe, expect, it, vi } from 'vitest';

import { APP_ORIGIN } from '#main/app-url.js';

const FROM_APP = { senderFrame: { url: `${APP_ORIGIN}/room` } };
const FROM_EVIL = { senderFrame: { url: 'https://evil.example' } };

const handlers = new Map<string, (event: unknown, ...args: unknown[]) => void>();
const mocks = vi.hoisted(() => ({
  writeText: vi.fn(),
  notificationShow: vi.fn(),
  notificationCtor: vi.fn(),
  isSupported: vi.fn(() => true),
}));

vi.mock('electron', () => ({
  clipboard: { writeText: mocks.writeText },
  ipcMain: {
    on: (channel: string, handler: (event: unknown, ...args: unknown[]) => void) => {
      handlers.set(channel, handler);
    },
  },
  Notification: Object.assign(
    class {
      constructor(opts: unknown) {
        mocks.notificationCtor(opts);
      }
      show = mocks.notificationShow;
    },
    { isSupported: mocks.isSupported },
  ),
}));

import { registerQuickShareIpcHandlers } from '#features/screen-share/quick-share.js';

beforeEach(() => {
  handlers.clear();
  for (const m of Object.values(mocks)) m.mockReset();
  mocks.isSupported.mockReturnValue(true);
  registerQuickShareIpcHandlers();
});

describe('registerQuickShareIpcHandlers', () => {
  it('copies the invite link the room page hands over', () => {
    handlers.get('desktop-share:link-ready')?.(FROM_APP, 'https://example.com/r/ABCD');
    expect(mocks.writeText).toHaveBeenCalledWith('https://example.com/r/ABCD');
  });

  it('raises an OS notification when the quick-share link is ready', () => {
    handlers.get('desktop-share:link-ready')?.(FROM_APP, 'https://example.com/r/ABCD');
    expect(mocks.notificationCtor).toHaveBeenCalledWith({
      title: 'Screen Share',
      body: 'Transmissão no ar — link da sala copiado!',
    });
    expect(mocks.notificationShow).toHaveBeenCalledOnce();
  });

  it('still copies the link when notifications are unsupported', () => {
    mocks.isSupported.mockReturnValue(false);
    handlers.get('desktop-share:link-ready')?.(FROM_APP, 'https://example.com/r/ABCD');
    expect(mocks.writeText).toHaveBeenCalledWith('https://example.com/r/ABCD');
    expect(mocks.notificationCtor).not.toHaveBeenCalled();
  });

  it('raises an OS notification when a member joins', () => {
    handlers.get('desktop-share:member-joined')?.(FROM_APP, 'Bia');
    expect(mocks.notificationCtor).toHaveBeenCalledWith({
      title: 'Screen Share',
      body: 'Bia entrou na sala.',
    });
    expect(mocks.notificationShow).toHaveBeenCalledOnce();
  });

  it('does nothing on member-joined when notifications are unsupported', () => {
    mocks.isSupported.mockReturnValue(false);
    handlers.get('desktop-share:member-joined')?.(FROM_APP, 'Bia');
    expect(mocks.notificationCtor).not.toHaveBeenCalled();
  });

  it('ignores IPC from a frame that is not the app origin (F11)', () => {
    handlers.get('desktop-share:link-ready')?.(FROM_EVIL, 'https://evil.example/steal');
    handlers.get('desktop-share:member-joined')?.(FROM_EVIL, 'spoofed');
    expect(mocks.writeText).not.toHaveBeenCalled();
    expect(mocks.notificationCtor).not.toHaveBeenCalled();
  });
});
