import { type BrowserContext, type Page, expect, test } from '@playwright/test';

import {
  SHARE_BUTTON,
  createPublicRoom,
  joinRoom,
  memberCard,
  startSharing,
  watchSharer,
} from './helpers';

// Automates the two "still needs a real-browser smoke test" gates from
// ADR-0008: F12 / A-02 (the per-request-nonce CSP must not break the
// stack) and F01 (leaving a room by the browser back button must not
// leave a zombie reconnect loop rejoining every idle reap).

/** Records CSP violations the browser reports, from both channels: the
 * `securitypolicyviolation` DOM event and the console message Chromium
 * logs alongside it. Installed before any page script runs. */
async function collectCspViolations(context: BrowserContext): Promise<string[]> {
  const violations: string[] = [];
  await context.addInitScript(() => {
    window.addEventListener('securitypolicyviolation', (e) => {
      // Stash on the document so the test can read it back per page.
      const bag = ((window as unknown as { __csp?: string[] }).__csp ??= []);
      bag.push(`${e.violatedDirective} blocked ${e.blockedURI || '(inline)'}`);
    });
  });
  context.on('page', (page: Page) => {
    page.on('console', (msg) => {
      const text = msg.text();
      if (/content security policy/i.test(text)) violations.push(`console: ${text}`);
    });
  });
  return violations;
}

async function domCspViolations(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __csp?: string[] }).__csp ?? []);
}

test('the nonce CSP does not block create, join, share or watch', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const anaViolations = await collectCspViolations(anaCtx);
  const bobViolations = await collectCspViolations(bobCtx);

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala CSP');
  const bob = await joinRoom(bobCtx, url, 'Bob');
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();

  await startSharing(ana);
  await watchSharer(bob, 'Ana');

  // The page hydrated (the share/watch flow is all client-side), fonts and
  // the wasm module loaded, and the signaling socket connected — every one
  // of those is a CSP directive. Nothing should have been refused.
  expect([...anaViolations, ...(await domCspViolations(ana))]).toEqual([]);
  expect([...bobViolations, ...(await domCspViolations(bob))]).toEqual([]);

  await anaCtx.close();
  await bobCtx.close();
});

test('leaving via the browser back button drops the member with no zombie rejoin', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala saída');

  // Give Bob real history (home -> room) so `goBack()` lands on home, the
  // way a user backing out of a room does. The nick gate keeps the same
  // URL, so back from the room goes straight to "/".
  const bob = await bobCtx.newPage();
  const bobErrors: string[] = [];
  bob.on('pageerror', (err) => bobErrors.push(String(err)));
  await bob.goto('/');
  await expect(bob.getByRole('heading', { name: 'Criar sala' })).toBeVisible();
  await bob.goto(url);
  await bob.getByLabel('Nick').fill('Bob');
  await bob.getByRole('button', { name: 'Entrar' }).click();
  await expect(bob.locator('#member-grid')).toBeVisible();
  await expect(memberCard(ana, 'Bob')).toHaveCount(1);

  await bob.goBack();
  await expect(bob).toHaveURL(/\/$/);
  await expect(bob.getByRole('heading', { name: 'Criar sala' })).toBeVisible();

  // The server saw a clean leave immediately, not after the 90 s idle reap.
  await expect(memberCard(ana, 'Bob')).toHaveCount(0);

  // And Bob does not silently rejoin: the roster stays put and the left
  // page raised no errors from a disposed reactive owner / dead socket.
  // (The grid always renders MAX_MEMBERS slots; the empty ones carry
  // `.hidden`, so count the visible cards.)
  await bob.waitForTimeout(5_000);
  await expect(memberCard(ana, 'Bob')).toHaveCount(0);
  await expect(ana.locator('.card:not(.hidden)')).toHaveCount(1);
  expect(bobErrors).toEqual([]);

  await anaCtx.close();
  await bobCtx.close();
});

test('a member who shared then backs out stops appearing as a sharer', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala saída share');

  const bob = await bobCtx.newPage();
  await bob.goto('/');
  await expect(bob.getByRole('heading', { name: 'Criar sala' })).toBeVisible();
  await bob.goto(url);
  await bob.getByLabel('Nick').fill('Bob');
  await bob.getByRole('button', { name: 'Entrar' }).click();
  await expect(bob.locator('#member-grid')).toBeVisible();
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();

  await startSharing(bob);
  await expect(memberCard(ana, 'Bob').locator('.card__watch-pill')).toBeVisible();

  await bob.goBack();
  await expect(bob).toHaveURL(/\/$/);

  // Bob's share is torn down on the way out along with his membership.
  await expect(memberCard(ana, 'Bob')).toHaveCount(0);
  await bob.waitForTimeout(3_000);
  await expect(ana.getByRole('button', { name: SHARE_BUTTON })).toBeVisible();
  await expect(ana.locator('.card:not(.hidden)')).toHaveCount(1);

  await anaCtx.close();
  await bobCtx.close();
});
