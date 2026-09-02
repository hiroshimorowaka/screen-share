import { describe, expect, it, vi } from 'vitest';

const setPermissionRequestHandler = vi.hoisted(() => vi.fn());
const setPermissionCheckHandler = vi.hoisted(() => vi.fn());

vi.mock('electron', () => ({
  session: { defaultSession: { setPermissionRequestHandler, setPermissionCheckHandler } },
}));

import { lockDownPermissions } from '#main/permissions.js';

describe('lockDownPermissions', () => {
  it('denies every permission request and every permission check (finding 6)', () => {
    lockDownPermissions();

    expect(setPermissionRequestHandler).toHaveBeenCalledOnce();
    const requestHandler = setPermissionRequestHandler.mock.calls[0]?.[0] as (
      wc: unknown,
      permission: string,
      cb: (granted: boolean) => void,
    ) => void;
    const callback = vi.fn();
    requestHandler({}, 'media', callback);
    expect(callback).toHaveBeenCalledWith(false);

    expect(setPermissionCheckHandler).toHaveBeenCalledOnce();
    const checkHandler = setPermissionCheckHandler.mock.calls[0]?.[0] as () => boolean;
    expect(checkHandler()).toBe(false);
  });
});
