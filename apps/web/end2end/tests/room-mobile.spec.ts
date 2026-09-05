import { type Browser, expect, type Page, test } from '@playwright/test';

import { createPublicRoom, joinRoom, memberCard, startSharing, watchSharer } from './helpers';

// Runs only under the `mobile-web` project (phone viewport + hasTouch) —
// see playwright.config.ts. The sharer context is a phone here too only
// because the fake-media Chrome flags let any viewport screen-share in
// test; a real phone can't. What matters is the viewer's touch UX.

async function room(browser: Browser) {
  const sharerCtx = await browser.newContext();
  const viewerCtx = await browser.newContext();
  const { url } = await createPublicRoom(sharerCtx, 'Sharer', 'Sala mobile');
  await startSharing(sharerCtx.pages()[0]);
  const viewer = await joinRoom(viewerCtx, url, 'Viewer');
  await watchSharer(viewer, 'Sharer');
  return { sharerCtx, viewerCtx, viewer };
}

/** A sharer plus two more members, none watching yet — for the roster-view
 * and filmstrip layout checks that need a third card. */
async function roomOfThree(browser: Browser) {
  const sharerCtx = await browser.newContext();
  const viewerCtx = await browser.newContext();
  const thirdCtx = await browser.newContext();
  const { url } = await createPublicRoom(sharerCtx, 'Sharer', 'Sala mobile 3');
  await startSharing(sharerCtx.pages()[0]);
  const viewer = await joinRoom(viewerCtx, url, 'Viewer');
  const third = await joinRoom(thirdCtx, url, 'Terceiro');
  await expect(viewer.locator('.card:not(.hidden)')).toHaveCount(3);
  return { sharerCtx, viewerCtx, thirdCtx, viewer, third };
}

/** In focus mode on touch the chrome auto-hides; wait for that, then tap
 * the video once to bring it back so a control can be tapped. */
async function revealChrome(viewer: Page, sharerNick: string) {
  const roomPage = viewer.locator('.room-page');
  await expect(roomPage).toHaveClass(/chrome-hidden/);
  await memberCard(viewer, sharerNick).locator('video').nth(1).tap();
  await expect(roomPage).not.toHaveClass(/chrome-hidden/);
}

test.describe('room on a touch device', () => {
  test('patching a sharer focuses it; the chrome auto-hides and a tap toggles it back', async ({
    browser,
  }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    const roomPage = viewer.locator('.room-page');

    // Watching one screen at a time: patching in goes straight to focus.
    await expect(viewer.locator('#member-grid')).toHaveClass(/grid--focused/);

    // The chrome gets out of the video's way on its own...
    await expect(roomPage).toHaveClass(/chrome-hidden/);
    await expect(viewer.locator('.room-controls')).toHaveClass(/room-controls--hidden/);

    // ...and a tap on the video brings it back.
    await memberCard(viewer, 'Sharer').locator('video').nth(1).tap();
    await expect(roomPage).not.toHaveClass(/chrome-hidden/);

    // The bar's back button is the way out of focus on touch.
    await viewer.getByRole('button', { name: 'Voltar para a grade' }).click();
    await expect(viewer.locator('#member-grid')).not.toHaveClass(/grid--focused/);

    await sharerCtx.close();
    await viewerCtx.close();
  });

  test('the quality picker opens as a bottom sheet and closes on an outside tap', async ({
    browser,
  }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    await revealChrome(viewer, 'Sharer');

    const viewport = viewer.viewportSize();
    if (!viewport) throw new Error('no viewport size');
    const popup = memberCard(viewer, 'Sharer').locator('.quality-menu__popup');

    await expect(popup).not.toBeInViewport();

    await viewer.getByRole('button', { name: 'Qualidade do vídeo' }).tap();
    await expect(popup).toBeInViewport();

    const box = await popup.boundingBox();
    if (!box) throw new Error('no bounding box for the open sheet');
    // A sheet: near full-width, sitting on the bottom edge — it covers the
    // trigger itself once open, so the way to close it on touch is tapping
    // outside, not tapping the (now hidden-behind-the-sheet) trigger again.
    expect(box.width).toBeGreaterThan(viewport.width * 0.85);
    expect(box.y + box.height).toBeGreaterThan(viewport.height - 2);

    // A tap outside blurs the trigger, which closes it.
    await viewer.locator('.stage-header').tap();
    await expect(popup).not.toBeInViewport();

    await sharerCtx.close();
    await viewerCtx.close();
  });

  test('the volume popup opens on the first tap; further taps on the same button mute instead of closing it', async ({
    browser,
  }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    await revealChrome(viewer, 'Sharer');

    const viewport = viewer.viewportSize();
    if (!viewport) throw new Error('no viewport size');
    const card = memberCard(viewer, 'Sharer');
    const trigger = card.locator('.volume-control > button');
    const popup = card.locator('.volume-control__popup');
    const peerVideo = card.locator('video').nth(1);

    // The popup is the same small anchored popup desktop has (not a
    // bottom sheet), so it sits within the viewport even closed — closed
    // is `opacity`/`pointer-events`, not geometry.
    await expect(popup).toHaveCSS('pointer-events', 'none');

    // First tap: opens the popup, does not mute.
    await trigger.tap();
    await expect(popup).toHaveCSS('pointer-events', 'auto');
    expect(await peerVideo.evaluate((v: HTMLVideoElement) => v.muted)).toBe(false);

    // Same small floating popup as desktop, anchored above the trigger —
    // not a bottom sheet: narrow, and clear of the viewport's bottom edge.
    const box = await popup.boundingBox();
    const triggerBox = await trigger.boundingBox();
    if (!box || !triggerBox) throw new Error('no bounding box for the open popup');
    expect(box.width).toBeLessThan(viewport.width * 0.5);
    expect(box.y + box.height).toBeLessThanOrEqual(triggerBox.y + 1);

    // Second tap on the same (already-open) button: mutes, popup stays open.
    await trigger.tap();
    await expect.poll(() => peerVideo.evaluate((v: HTMLVideoElement) => v.muted)).toBe(true);
    await expect(popup).toHaveCSS('pointer-events', 'auto');

    // Third tap: unmutes, still open — the trigger never closes it once open.
    await trigger.tap();
    await expect.poll(() => peerVideo.evaluate((v: HTMLVideoElement) => v.muted)).toBe(false);
    await expect(popup).toHaveCSS('pointer-events', 'auto');

    // The slider — the exact same one desktop has — still drives the real
    // <video>'s volume while the popup is open.
    await popup.locator('.volume-control__slider').evaluate((el: HTMLInputElement) => {
      el.value = '40';
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await expect
      .poll(() => peerVideo.evaluate((v: HTMLVideoElement) => v.volume))
      .toBeCloseTo(0.4, 1);

    // Only an outside tap closes it.
    await viewer.locator('.stage-header').tap();
    await expect(popup).toHaveCSS('pointer-events', 'none');

    await sharerCtx.close();
    await viewerCtx.close();
  });

  test('the focused card action buttons are at least 44px', async ({ browser }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    await revealChrome(viewer, 'Sharer');

    const buttons = memberCard(viewer, 'Sharer').locator('.card__actions .icon-btn');
    const count = await buttons.count();
    expect(count).toBeGreaterThan(0);
    for (let i = 0; i < count; i++) {
      const button = buttons.nth(i);
      if (!(await button.isVisible())) continue;
      const box = await button.boundingBox();
      if (!box) throw new Error('no bounding box for a visible action button');
      expect(box.width).toBeGreaterThanOrEqual(44);
      expect(box.height).toBeGreaterThanOrEqual(44);
    }

    await sharerCtx.close();
    await viewerCtx.close();
  });

  // A1: the quality bottom sheet must stack above the floating control bar
  // so every option is actually tappable.
  test('every quality-sheet option is hit-testable, not covered by the control bar', async ({
    browser,
  }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    await revealChrome(viewer, 'Sharer');

    await viewer.getByRole('button', { name: 'Qualidade do vídeo' }).tap();
    const popup = memberCard(viewer, 'Sharer').locator('.quality-menu__popup');
    await expect(popup).toBeInViewport();

    // The bar is pushed out of the way while the sheet is open.
    await expect(viewer.locator('.room-controls')).toHaveCSS('opacity', '0');

    const options = popup.locator('.quality-menu__option');
    await expect(options).toHaveCount(4);
    for (let i = 0; i < 4; i++) {
      const box = await options.nth(i).boundingBox();
      if (!box) throw new Error(`no bounding box for quality option ${i}`);
      const onTop = await viewer.evaluate(
        ({ x, y }) => document.elementFromPoint(x, y)?.closest('.quality-menu__option') !== null,
        { x: box.x + box.width / 2, y: box.y + box.height / 2 },
      );
      expect(onTop, `quality option ${i} is covered by another element`).toBe(true);
    }

    await sharerCtx.close();
    await viewerCtx.close();
  });

  // Picture-in-picture is dropped on touch; fullscreen stays.
  test('on touch the card offers fullscreen but not picture-in-picture', async ({ browser }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    await revealChrome(viewer, 'Sharer');

    const card = memberCard(viewer, 'Sharer');
    await expect(card.getByRole('button', { name: 'Tela cheia' })).toBeVisible();
    await expect(card.getByRole('button', { name: 'Picture-in-picture' })).toHaveCount(0);

    await sharerCtx.close();
    await viewerCtx.close();
  });

  // The regression this round fixes: a tap while fullscreen used to exit
  // fullscreen. On touch it must only reveal the idle-hidden controls;
  // leaving fullscreen is the button's job alone.
  test('on touch a tap in fullscreen reveals the controls without exiting', async ({ browser }) => {
    const { sharerCtx, viewerCtx, viewer } = await room(browser);
    await revealChrome(viewer, 'Sharer');
    const card = memberCard(viewer, 'Sharer');

    await card.getByRole('button', { name: 'Tela cheia' }).tap();
    const enteredFullscreen = await viewer
      .waitForFunction(() => document.fullscreenElement !== null, null, { timeout: 5000 })
      .then(() => true)
      .catch(() => false);
    test.skip(!enteredFullscreen, 'this headless environment does not grant Element.requestFullscreen');

    // The shared autohide idles the action row a few seconds in...
    await expect(card).toHaveClass(/card--controls-idle/);

    // ...and a tap on the video brings it back, staying in fullscreen.
    await card.locator('video').nth(1).tap();
    await expect(card).not.toHaveClass(/card--controls-idle/);
    expect(await viewer.evaluate(() => document.fullscreenElement !== null)).toBe(true);

    // Only the "Tela cheia" button leaves fullscreen.
    await card.getByRole('button', { name: 'Tela cheia' }).tap();
    await expect.poll(() => viewer.evaluate(() => document.fullscreenElement === null)).toBe(true);

    await sharerCtx.close();
    await viewerCtx.close();
  });

  // B2: filmstrip tiles are legible — the corner cluster is dropped and the
  // avatar no longer overlaps the nick pill.
  test('filmstrip tiles drop the corner cluster and do not overlap', async ({ browser }) => {
    const { sharerCtx, viewerCtx, thirdCtx, viewer } = await roomOfThree(browser);
    await watchSharer(viewer, 'Sharer');
    await expect(viewer.locator('#member-grid')).toHaveClass(/grid--focused/);

    const tiles = viewer.locator('.grid--focused .card:not(.card--focus):not(.hidden)');
    await expect(tiles).toHaveCount(2);

    for (let i = 0; i < 2; i++) {
      const tile = tiles.nth(i);
      await expect(tile.locator('.card__corner-start')).toBeHidden();
      const avatar = await tile.locator('.card__avatar').boundingBox();
      const nick = await tile.locator('.card__nick').boundingBox();
      if (!avatar || !nick) throw new Error('missing tile child box');
      expect(avatar.y + avatar.height, `tile ${i}: avatar overlaps the nick`).toBeLessThanOrEqual(
        nick.y + 3,
      );
    }

    await sharerCtx.close();
    await viewerCtx.close();
    await thirdCtx.close();
  });

  // A3 (roster) + C1 + C2: the roster view on a phone.
  test('roster view: last row clears the bar, orphan card is centred, watch pill not truncated', async ({
    browser,
  }) => {
    const { sharerCtx, viewerCtx, thirdCtx, viewer } = await roomOfThree(browser);

    await expect(viewer.locator('#member-grid')).not.toHaveClass(/grid--focused/);
    const cards = viewer.locator('.card:not(.hidden)');

    // A3: the last visible card is not behind the fixed control bar.
    const lastBox = await cards.nth(2).boundingBox();
    const barBox = await viewer.locator('.room-controls').boundingBox();
    if (!lastBox || !barBox) throw new Error('missing layout box');
    expect(lastBox.y + lastBox.height).toBeLessThanOrEqual(barBox.y + 1);

    // C1: 3 members in a 2-column narrow grid (4 half-column tracks) — the
    // lone last-row card is centred, i.e. starts at track line 2.
    await expect
      .poll(() => cards.nth(2).evaluate((el) => getComputedStyle(el).gridColumnStart))
      .toBe('2');

    // C2: the sharer's watch pill shows the whole label (wrapped), not
    // "Assistir tra…".
    const pill = memberCard(viewer, 'Sharer').locator('.card__watch-pill');
    await expect(pill).toBeVisible();
    const truncated = await pill.evaluate((el) => el.scrollWidth > el.clientWidth + 1);
    expect(truncated, 'the watch pill is truncated').toBe(false);

    await sharerCtx.close();
    await viewerCtx.close();
    await thirdCtx.close();
  });
});
