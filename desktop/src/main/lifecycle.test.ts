import { beforeEach, describe, expect, it, vi } from 'vitest';

const quit = vi.hoisted(() => vi.fn());
vi.mock('electron', () => ({ app: { quit } }));

// `quitting` is module-level state, so each test gets a fresh module.
async function freshLifecycle() {
  vi.resetModules();
  return import('#main/lifecycle.js');
}

beforeEach(() => {
  quit.mockReset();
});

describe('lifecycle', () => {
  it('starts not-quitting', async () => {
    const { isQuitting } = await freshLifecycle();
    expect(isQuitting()).toBe(false);
  });

  it('markQuitting flips the flag without quitting the app', async () => {
    const { isQuitting, markQuitting } = await freshLifecycle();
    markQuitting();
    expect(isQuitting()).toBe(true);
    expect(quit).not.toHaveBeenCalled();
  });

  it('requestQuit marks intent first, then quits the app', async () => {
    const { isQuitting, requestQuit } = await freshLifecycle();
    requestQuit();
    expect(isQuitting()).toBe(true);
    expect(quit).toHaveBeenCalledOnce();
  });
});
