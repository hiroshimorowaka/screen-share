import { describe, expect, it } from 'vitest';

import { APP_ORIGIN } from '#main/app-url.js';
import { isTrustedFrame } from '#main/ipc-guard.js';

type FakeEvent = Parameters<typeof isTrustedFrame>[0];
const eventFrom = (url: string | undefined): FakeEvent =>
  ({ senderFrame: url === undefined ? null : { url } }) as unknown as FakeEvent;

describe('isTrustedFrame', () => {
  it('accepts a frame on the app origin', () => {
    expect(isTrustedFrame(eventFrom(`${APP_ORIGIN}/`))).toBe(true);
    expect(isTrustedFrame(eventFrom(`${APP_ORIGIN}/room/ABCD?x=1`))).toBe(true);
  });

  it('accepts a local file frame (the source picker window)', () => {
    expect(isTrustedFrame(eventFrom('file:///opt/app/static/picker.html'))).toBe(true);
  });

  it('rejects another origin, a subdomain, and a non-URL', () => {
    expect(isTrustedFrame(eventFrom('https://evil.example/'))).toBe(false);
    expect(isTrustedFrame(eventFrom(`https://evil.${new URL(APP_ORIGIN).host}/`))).toBe(false);
    expect(isTrustedFrame(eventFrom('not a url'))).toBe(false);
  });

  it('rejects a disposed frame (null senderFrame)', () => {
    expect(isTrustedFrame(eventFrom(undefined))).toBe(false);
  });
});
