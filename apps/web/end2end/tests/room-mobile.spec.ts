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
    // A sheet: near full-width, sitting on the bottom edge.
    expect(box.width).toBeGreaterThan(viewport.width * 0.85);
    expect(box.y + box.height).toBeGreaterThan(viewport.height - 2);

    // A tap outside blurs the trigger, which closes it.
    await viewer.locator('.stage-header').tap();
    await expect(popup).not.toBeInViewport();

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
});
