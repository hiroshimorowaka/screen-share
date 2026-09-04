import { expect, test } from '@playwright/test';

import {
  MEDIA_SETTLE_MS,
  SHARE_BUTTON,
  createPublicRoom,
  joinRoom,
  memberCard,
  videoState,
  watchSharer,
} from './helpers';

// The manual two-tab checklist from CLAUDE.md, automated: two members in
// one room, each a separate browser context (own storage, own peer id).
// The sharer's screen capture and both peer connections are real; only
// the captured media is synthetic (the fake-device Chromium flags in
// playwright.config.ts).

test('two members: share, watch, real media flows, then teardown', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();

  // Ana creates a public room and is dropped straight into it.
  const ana = await anaCtx.newPage();
  await ana.goto('/');
  await ana.getByLabel('Nick').fill('Ana');
  await ana.getByLabel('Nome da sala').fill('Sala P2P');
  await ana.locator('label.switch').click();
  await expect(ana.getByLabel('Sala pública')).toBeChecked();
  await ana.locator('.panel', { hasText: 'Criar sala' }).getByRole('button', { name: 'Criar sala' }).click();
  await expect(ana).toHaveURL(/\/r\/[A-Z0-9]+$/);
  const roomUrl = ana.url();

  // Bob joins the same room.
  const bob = await joinRoom(bobCtx, roomUrl, 'Bob');
  await expect(ana.locator('.card__nick', { hasText: 'Bob' })).toBeVisible();
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();

  // Starting a share only lights up a "watch" affordance — it pushes no
  // video on its own.
  await ana.getByRole('button', { name: 'Compartilhar ou parar de compartilhar minha tela' }).click();
  await expect(bob.locator('.card', { hasText: 'Ana' }).locator('.card__watch-pill')).toBeVisible();
  expect((await videoState(bob, 'Ana')).readyState).toBe(0);

  // While sharing, the header shows a "Compartilhando" chip — and never
  // the old "select a screen" status (the browser picker already says that).
  await expect(ana.locator('.share-chip')).toBeVisible();
  await expect(ana.locator('.share-chip')).toContainText('Compartilhando');
  await expect(ana.locator('.stage-header')).not.toContainText('Selecione a tela');

  // Bob watches Ana's card — the whole tile is the watch affordance.
  await bob.locator('.card', { hasText: 'Ana' }).click();

  await expect
    .poll(async () => (await videoState(bob, 'Ana')).readyState, { timeout: MEDIA_SETTLE_MS })
    .toBeGreaterThanOrEqual(2);
  expect((await videoState(bob, 'Ana')).width).toBeGreaterThan(0);
  await expect(bob.locator('.card', { hasText: 'Ana' }).locator('.card__avatar')).toBeHidden();

  // Ana stops sharing from the in-app control — Bob's view tears down and
  // the card falls back to the avatar, and the "Compartilhando" chip goes.
  await ana.getByRole('button', { name: 'Compartilhar ou parar de compartilhar minha tela' }).click();
  // Bob only falls back to the avatar once `PeerStoppedSharing` has crossed
  // the socket and torn the incoming connection down — the same WebRTC
  // round trip the watch assertion above waits `MEDIA_SETTLE_MS` for, so
  // this one gets the same budget instead of the default 10s.
  await expect(bob.locator('.card', { hasText: 'Ana' }).locator('.card__avatar')).toBeVisible({
    timeout: MEDIA_SETTLE_MS,
  });
  await expect(ana.locator('.share-chip')).toBeHidden();

  await anaCtx.close();
  await bobCtx.close();
});

test('hiding your own preview while it is expanded drops focus, not a broken grid', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala foco');

  await ana.getByRole('button', { name: SHARE_BUTTON }).click();
  // Expand the sharer's own preview card.
  await ana.locator('.card', { hasText: 'Ana' }).click();
  await expect(ana.locator('#member-grid')).toHaveClass(/grid--focused/);

  // Hide the preview: its card leaves the grid, so focus must be released
  // rather than left pointing at a card that is no longer rendered.
  await ana.getByRole('button', { name: 'Esconder meu preview' }).click();
  await expect(ana.locator('#member-grid')).not.toHaveClass(/grid--focused/);

  await anaCtx.close();
});

test('a watcher reload mid-session silently rejoins and keeps the roster', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();

  const ana = await anaCtx.newPage();
  await ana.goto('/');
  await ana.getByLabel('Nick').fill('Ana');
  await ana.getByLabel('Nome da sala').fill('Sala reload');
  await ana.locator('label.switch').click();
  await expect(ana.getByLabel('Sala pública')).toBeChecked();
  await ana.locator('.panel', { hasText: 'Criar sala' }).getByRole('button', { name: 'Criar sala' }).click();
  await expect(ana).toHaveURL(/\/r\/[A-Z0-9]+$/);

  const bob = await joinRoom(bobCtx, ana.url(), 'Bob');
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();

  await bob.reload();

  // No nick gate this time — the tab-scoped session rejoins on its own.
  await expect(bob.locator('#member-grid')).toBeVisible();
  await expect(bob.getByRole('heading', { name: 'Entrar na sala' })).toBeHidden();
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();

  await anaCtx.close();
  await bobCtx.close();
});

test('a watcher whose connection drops reconnects on its own and rewatches', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();

  // Sever Bob's signaling socket on demand while the server stays up.
  // CDP offline emulation (`setOffline`) does not close a live WebSocket
  // and the protocol has no heartbeat, so the only way to exercise the
  // reconnect path from a test is to route the socket and close it.
  let severBobSocket: (() => Promise<void>) | undefined;
  await bobCtx.routeWebSocket(/\/ws(\?|$)/, (ws) => {
    ws.connectToServer();
    severBobSocket = () => ws.close();
  });

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala queda');
  const bob = await joinRoom(bobCtx, url, 'Bob');
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();

  await ana.getByRole('button', { name: SHARE_BUTTON }).click();
  await watchSharer(bob, 'Ana');

  // Bob's connection drops — the client should notice and start retrying.
  await severBobSocket?.();
  await expect(bob.locator('.stage-header')).toContainText(/Reconectando|Conexão perdida/, {
    timeout: 10_000,
  });

  // The roster comes back without a nick gate, and the watch re-establishes
  // itself (replayed intent) so frames flow again.
  await expect(bob.getByRole('heading', { name: 'Entrar na sala' })).toBeHidden();
  await expect(bob.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();
  await expect
    .poll(async () => (await videoState(bob, 'Ana')).readyState, { timeout: MEDIA_SETTLE_MS })
    .toBeGreaterThanOrEqual(2);

  await anaCtx.close();
  await bobCtx.close();
});

test('one watcher stopping does not disturb another watching the same sharer', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const caioCtx = await browser.newContext();

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala 3-vias');
  const bob = await joinRoom(bobCtx, url, 'Bob');
  const caio = await joinRoom(caioCtx, url, 'Caio');

  await ana.getByRole('button', { name: SHARE_BUTTON }).click();
  await watchSharer(bob, 'Ana');
  await watchSharer(caio, 'Ana');

  // Bob stops watching — his own tile falls back to the avatar once the
  // incoming connection is torn down.
  const anaCardOnBob = bob.locator('.card', { hasText: 'Ana' });
  await anaCardOnBob.hover();
  await anaCardOnBob.getByRole('button', { name: 'Parar de assistir' }).click();
  await expect(anaCardOnBob.locator('.card__avatar')).toBeVisible({ timeout: MEDIA_SETTLE_MS });

  // Caio's independent connection keeps decoding frames.
  await expect
    .poll(async () => (await videoState(caio, 'Ana')).readyState, { timeout: 5_000 })
    .toBeGreaterThanOrEqual(2);
  expect((await videoState(caio, 'Ana')).width).toBeGreaterThan(0);

  await anaCtx.close();
  await bobCtx.close();
  await caioCtx.close();
});

test('the watcher badge lists each viewer on its own line', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const caioCtx = await browser.newContext();

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala espectadores');
  const bob = await joinRoom(bobCtx, url, 'Bob');
  const caio = await joinRoom(caioCtx, url, 'Caio');

  await ana.getByRole('button', { name: SHARE_BUTTON }).click();
  await watchSharer(bob, 'Ana');
  await watchSharer(caio, 'Ana');

  const badge = memberCard(ana, 'Ana').locator('.watcher-badge');
  await expect(badge.locator('span').first()).toHaveText('2');

  await badge.hover();
  const names = badge.locator('.watcher-badge__name');
  await expect(names).toHaveCount(2);
  expect((await names.allTextContents()).sort()).toEqual(['Bob', 'Caio']);

  // Stacked vertically: the second name sits below the first, not beside it.
  const first = await names.nth(0).boundingBox();
  const second = await names.nth(1).boundingBox();
  if (!first || !second) throw new Error('no bounding box for a watcher name');
  expect(second.y).toBeGreaterThanOrEqual(first.y + first.height);

  await anaCtx.close();
  await bobCtx.close();
  await caioCtx.close();
});
