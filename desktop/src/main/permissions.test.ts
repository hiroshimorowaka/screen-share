import { describe, expect, it, vi } from 'vitest';

const setPermissionRequestHandler = vi.hoisted(() => vi.fn());
const setPermissionCheckHandler = vi.hoisted(() => vi.fn());

vi.mock('electron', () => ({
  session: { defaultSession: { setPermissionRequestHandler, setPermissionCheckHandler } },
}));

import { lockDownPermissions } from '#main/permissions.js';

type RequestHandler = (
  wc: unknown,
  permission: string,
  cb: (granted: boolean) => void,
  details: { mediaTypes?: Array<'video' | 'audio'> },
) => void;
type CheckHandler = (wc: unknown, permission: string) => boolean;

function handlers() {
  lockDownPermissions();
  return {
    request: setPermissionRequestHandler.mock.calls[0]?.[0] as RequestHandler,
    check: setPermissionCheckHandler.mock.calls[0]?.[0] as CheckHandler,
  };
}

function grants(request: RequestHandler, permission: string, details = {}): boolean {
  const cb = vi.fn();
  request({}, permission, cb, details);
  return cb.mock.calls[0]?.[0] as boolean;
}

describe('lockDownPermissions', () => {
  it('denies camera/mic, geolocation, notifications, clipboard-read and other requests (finding 6)', () => {
    const { request } = handlers();

    expect(grants(request, 'media', { mediaTypes: ['video'] })).toBe(false);
    expect(grants(request, 'media', { mediaTypes: ['audio'] })).toBe(false);
    expect(grants(request, 'geolocation')).toBe(false);
    expect(grants(request, 'notifications')).toBe(false);
    expect(grants(request, 'midi')).toBe(false);
    expect(grants(request, 'clipboard-read')).toBe(false);
  });

  it('allows clipboard-sanitized-write so the invite button can copy the room link', () => {
    const { request, check } = handlers();

    expect(grants(request, 'clipboard-sanitized-write')).toBe(true);
    expect(check({}, 'clipboard-sanitized-write')).toBe(true);
    expect(check({}, 'clipboard-read')).toBe(false);
  });

  it('allows a display-capture request so getDisplayMedia reaches setDisplayMediaRequestHandler', () => {
    const { request } = handlers();

    // getDisplayMedia carries no explicit mediaTypes.
    expect(grants(request, 'media', {})).toBe(true);
    expect(grants(request, 'media', { mediaTypes: [] })).toBe(true);
  });

  it('lets a media check through (both check and request are typed "media" for getDisplayMedia)', () => {
    const { check } = handlers();

    expect(check({}, 'media')).toBe(true);
    expect(check({}, 'geolocation')).toBe(false);
    expect(check({}, 'notifications')).toBe(false);
  });
});
