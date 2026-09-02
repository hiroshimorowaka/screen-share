import { describe, expect, it } from 'vitest';

import { PICKER_FILE_URL } from '#features/screen-share/picker-page.js';
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

  it('accepts the source-picker window at its exact file URL', () => {
    expect(isTrustedFrame(eventFrom(PICKER_FILE_URL))).toBe(true);
  });

  it('rejects any other file:// frame (finding 13)', () => {
    expect(isTrustedFrame(eventFrom('file:///etc/passwd'))).toBe(false);
    expect(isTrustedFrame(eventFrom('file:///opt/app/static/picker.html'))).toBe(false);
    expect(isTrustedFrame(eventFrom(`${PICKER_FILE_URL}/../evil.html`))).toBe(false);
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
