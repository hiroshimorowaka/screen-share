import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

/**
 * Guards the auto-update / bundle-integrity posture in `package.json`
 * (ADR-0008, findings F04 and F18). These are one-line config flags with
 * no runtime surface, so a regression test on the manifest itself is the
 * only place to catch them being flipped back.
 */
describe('electron-builder packaging config', () => {
  const pkg = JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8')) as {
    build?: { asar?: unknown; win?: { verifyUpdateCodeSignature?: unknown } };
  };

  it('never disables the Windows auto-update signature check', () => {
    // `undefined` (default true) is fine; an explicit `false` is the F04 hole.
    expect(pkg.build?.win?.verifyUpdateCodeSignature).not.toBe(false);
  });

  it('ships the app inside an asar archive for bundle integrity', () => {
    expect(pkg.build?.asar).toBe(true);
  });
});
