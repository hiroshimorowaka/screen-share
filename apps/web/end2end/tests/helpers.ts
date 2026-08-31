import { type BrowserContext, type Page, expect } from '@playwright/test';

// Shared fixtures for the room specs. Every room flow runs with two or
// more browser contexts (own storage, own peer id); screen capture and
// the peer connections are real, only the captured media is synthetic
// (the fake-device Chromium flags in playwright.config.ts).

// A watched stream should paint within this window once the peer
// connection is up — generous for a CI box under xvfb.
export const MEDIA_SETTLE_MS = 15_000;

export const SHARE_BUTTON = 'Compartilhar ou parar de compartilhar minha tela';
export const ROOM_URL = /\/r\/[A-Z0-9]+$/;

/**
 * The card for the member with this nick, matched on the nick pill rather
 * than any text — a watcher tooltip elsewhere can also contain the nick.
 */
export function memberCard(page: Page, nick: string) {
  return page.locator('.card').filter({
    has: page.locator('.card__nick', { hasText: nick }),
  });
}

/** `readyState` / `videoWidth` of the peer `<video>` on a member's card. */
export async function videoState(page: Page, cardNick: string) {
  return memberCard(page, cardNick)
    .locator('video')
    .nth(1) // [0] is the self-preview slot, [1] is the peer slot
    .evaluate((v: HTMLVideoElement) => ({
      readyState: v.readyState,
      width: v.videoWidth,
    }));
}

/** Joins an existing room through the nick gate. */
export async function joinRoom(
  context: BrowserContext,
  url: string,
  nick: string,
): Promise<Page> {
  const page = await context.newPage();
  await page.goto(url);
  await expect(page.getByRole('heading', { name: 'Entrar na sala' })).toBeVisible();
  await page.getByLabel('Nick').fill(nick);
  await page.getByRole('button', { name: 'Entrar' }).click();
  await expect(page.locator('#member-grid')).toBeVisible();
  return page;
}

/** Creates a public room and lands inside it. */
export async function createPublicRoom(
  context: BrowserContext,
  nick: string,
  roomName: string,
) {
  const page = await context.newPage();
  await page.goto('/');
  await page.getByLabel('Nick').fill(nick);
  await page.getByLabel('Nome da sala').fill(roomName);
  // The "sala pública" checkbox is hidden behind the switch UI — click the
  // switch the way a user does, then confirm the state took.
  await page.locator('label.switch').click();
  await expect(page.getByLabel('Sala pública')).toBeChecked();
  await page
    .locator('.panel', { hasText: 'Criar sala' })
    .getByRole('button', { name: 'Criar sala' })
    .click();
  await expect(page).toHaveURL(ROOM_URL);
  return { page, url: page.url() };
}

/** Starts sharing from the given page and waits for the "on air" chip. */
export async function startSharing(page: Page) {
  await page.getByRole('button', { name: SHARE_BUTTON }).click();
  await expect(page.locator('.share-chip')).toBeVisible();
}

/** Clicks a sharer's tile to watch and waits for frames to decode. */
export async function watchSharer(viewer: Page, sharerNick: string) {
  const card = memberCard(viewer, sharerNick);
  await expect(card.locator('.card__watch-pill')).toBeVisible();
  await card.click();
  await expect
    .poll(async () => (await videoState(viewer, sharerNick)).readyState, {
      timeout: MEDIA_SETTLE_MS,
    })
    .toBeGreaterThanOrEqual(2);
}

/** Moves the pointer away and waits for the auto-hiding control bar to fade,
 * so a click on a card-corner control below it is not intercepted. */
export async function dismissControlBar(page: Page) {
  await page.mouse.move(4, 4);
  await expect(page.locator('.room-controls')).toHaveClass(/room-controls--hidden/);
}
