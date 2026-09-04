import { expect, test } from '@playwright/test';

import {
  MEDIA_SETTLE_MS,
  createPublicRoom,
  joinRoom,
  memberCard,
  startSharing,
  videoState,
  watchSharer,
} from './helpers';

// Interaction coverage for the room control bar and the per-card
// controls — the toggles, focus mode, menus, and their combinations,
// where a regression is easy to miss (e.g. hide-preview leaving the grid
// stuck in focus mode).

const HIDE_IDLE = 'Ocultar quem não está transmitindo';
const HIDE_PREVIEW = 'Esconder meu preview';
const SWITCH_SOURCE = 'Trocar a tela ou janela compartilhada';
const TRANSMISSION = 'Ajustes da transmissão';

test('hide-idle removes non-sharing cards and restores them', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const caioCtx = await browser.newContext();

  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala ocultar');
  await joinRoom(bobCtx, url, 'Bob');
  await joinRoom(caioCtx, url, 'Caio');
  await expect(ana.locator('.card__nick', { hasText: 'Caio' })).toBeVisible();

  await startSharing(ana);

  await ana.getByRole('button', { name: HIDE_IDLE }).click();
  await expect(ana.locator('.card', { hasText: 'Bob' })).toBeHidden();
  await expect(ana.locator('.card', { hasText: 'Caio' })).toBeHidden();
  // The sharer's own card stays — it is not idle.
  await expect(ana.locator('.card', { hasText: 'Ana' })).toBeVisible();

  await ana.getByRole('button', { name: HIDE_IDLE }).click();
  await expect(ana.locator('.card', { hasText: 'Bob' })).toBeVisible();
  await expect(ana.locator('.card', { hasText: 'Caio' })).toBeVisible();

  await anaCtx.close();
  await bobCtx.close();
  await caioCtx.close();
});

test('expanding your own preview and collapsing it again toggles focus mode', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala foco proprio');
  await startSharing(ana);

  const grid = ana.locator('#member-grid');
  await ana.locator('.card', { hasText: 'Ana' }).click();
  await expect(grid).toHaveClass(/grid--focused/);
  await expect(ana.locator('.card--focus', { hasText: 'Ana' })).toBeVisible();

  await ana.locator('.card', { hasText: 'Ana' }).click();
  await expect(grid).not.toHaveClass(/grid--focused/);

  await anaCtx.close();
});

test('expanding a watched sharer and collapsing it again toggles focus mode', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala foco assistido');
  const bob = await joinRoom(bobCtx, url, 'Bob');

  await startSharing(ana);
  await watchSharer(bob, 'Ana');

  const grid = bob.locator('#member-grid');
  await bob.locator('.card', { hasText: 'Ana' }).click();
  await expect(grid).toHaveClass(/grid--focused/);
  await bob.locator('.card', { hasText: 'Ana' }).click();
  await expect(grid).not.toHaveClass(/grid--focused/);
  // Frames keep flowing after the round trip through focus mode.
  expect((await videoState(bob, 'Ana')).readyState).toBeGreaterThanOrEqual(2);

  await anaCtx.close();
  await bobCtx.close();
});

test('switching the shared source keeps the share alive and the viewer decoding', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala troca');
  const bob = await joinRoom(bobCtx, url, 'Bob');

  await startSharing(ana);
  await watchSharer(bob, 'Ana');

  await ana.getByRole('button', { name: SWITCH_SOURCE }).click();
  // Still sharing (the chip stays), and Bob's tile never falls back to
  // the avatar.
  await expect(ana.locator('.share-chip')).toBeVisible();
  await expect
    .poll(async () => (await videoState(bob, 'Ana')).readyState, { timeout: MEDIA_SETTLE_MS })
    .toBeGreaterThanOrEqual(2);
  await expect(bob.locator('.card', { hasText: 'Ana' }).locator('.card__avatar')).toBeHidden();

  await anaCtx.close();
  await bobCtx.close();
});

test('switching to an audio-less source updates the audio chip', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  // First getDisplayMedia keeps the fake audio track (shared "tab" with
  // audio); the second strips it (switching to a whole-screen share).
  await anaCtx.addInitScript(() => {
    const devices = navigator.mediaDevices;
    const original = devices.getDisplayMedia.bind(devices);
    let call = 0;
    devices.getDisplayMedia = async (...args: Parameters<typeof original>) => {
      const stream = await original(...args);
      if (++call >= 2) {
        for (const track of stream.getAudioTracks()) {
          track.stop();
          stream.removeTrack(track);
        }
      }
      return stream;
    };
  });
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala chip');

  await startSharing(ana);
  await expect(ana.locator('.audio-chip')).toContainText('Áudio ligado');

  await ana.getByRole('button', { name: SWITCH_SOURCE }).click();
  await expect(ana.locator('.share-chip')).toBeVisible();
  // The chip must reflect the new source, not stay stuck on "ligado".
  await expect(ana.locator('.audio-chip')).toContainText('Áudio desligado');

  await anaCtx.close();
});

test('a source switch that gains audio reaches a watcher without them re-watching', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  // First getDisplayMedia strips the audio track (share starts silent);
  // the second keeps it (switched to a shared "tab" with audio).
  await anaCtx.addInitScript(() => {
    const devices = navigator.mediaDevices;
    const original = devices.getDisplayMedia.bind(devices);
    let call = 0;
    devices.getDisplayMedia = async (...args: Parameters<typeof original>) => {
      const stream = await original(...args);
      if (++call < 2) {
        for (const track of stream.getAudioTracks()) {
          track.stop();
          stream.removeTrack(track);
        }
      }
      return stream;
    };
  });
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala troca audio');
  const bob = await joinRoom(bobCtx, url, 'Bob');

  await startSharing(ana);
  await watchSharer(bob, 'Ana');

  // Bob is watching a silent share; a live, unmuted audio track only
  // appears on his received stream after Ana switches source — and it must
  // arrive without him stopping and restarting the watch.
  const receivedAudio = () =>
    memberCard(bob, 'Ana')
      .locator('video')
      .nth(1)
      .evaluate((v: HTMLVideoElement) => {
        const [audio] = (v.srcObject as MediaStream | null)?.getAudioTracks() ?? [];
        return { present: !!audio, muted: audio?.muted ?? true };
      });

  await ana.getByRole('button', { name: SWITCH_SOURCE }).click();
  await expect(ana.locator('.share-chip')).toBeVisible();

  await expect
    .poll(receivedAudio, { timeout: MEDIA_SETTLE_MS })
    .toEqual({ present: true, muted: false });
  // Video keeps decoding across the switch.
  await expect
    .poll(async () => (await videoState(bob, 'Ana')).readyState, { timeout: MEDIA_SETTLE_MS })
    .toBeGreaterThanOrEqual(2);

  await anaCtx.close();
  await bobCtx.close();
});

test('leaving the room navigates back to the home page', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala saida');

  await ana.getByRole('button', { name: 'Sair da sala' }).click();
  await expect(ana).toHaveURL(/\/$/);
  await expect(ana.getByRole('heading', { name: 'Criar sala' })).toBeVisible();

  await anaCtx.close();
});

test('a mouse move after leaving the room does not hit a disposed signal', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala saida 2');

  // A WASM panic (e.g. touching an already-disposed reactive value from a
  // leaked window listener) surfaces here.
  const panics: string[] = [];
  ana.on('pageerror', (err) => panics.push(String(err)));

  await ana.getByRole('button', { name: 'Sair da sala' }).click();
  await expect(ana).toHaveURL(/\/$/);

  // The room grid's auto-hide / adaptive-grid listeners were attached to
  // `window`; if they outlive RoomPage, this fires them against disposed
  // signals.
  for (let i = 0; i < 5; i++) {
    await ana.mouse.move(100 + i * 40, 100 + i * 30);
  }
  await ana.waitForTimeout(100);

  expect(panics, `page errors after leaving the room:\n${panics.join('\n')}`).toEqual([]);
  // Still interactive.
  await ana.getByLabel('Nick').fill('Ana again');
  await expect(ana.getByLabel('Nick')).toHaveValue('Ana again');

  await anaCtx.close();
});

test('the ping loop stops after leaving the room', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala saida 3');

  const wsErrors: string[] = [];
  ana.on('console', (msg) => {
    if (msg.text().includes('CLOSING or CLOSED')) wsErrors.push(msg.text());
  });

  await ana.getByRole('button', { name: 'Sair da sala' }).click();
  await expect(ana).toHaveURL(/\/$/);

  // The self-ping interval fires every 5s; if it outlives the room it
  // calls `WsClient::send` on the closed socket.
  await ana.waitForTimeout(6000);

  expect(wsErrors, wsErrors.join('\n')).toEqual([]);

  await anaCtx.close();
});

test('leaving the room while sharing tears the share down and still navigates home', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala saida share');

  await startSharing(ana);
  await ana.getByRole('button', { name: 'Sair da sala' }).click();

  // The leave handler runs `stop_sharing`'s teardown before disconnecting
  // (so Chrome's native "you're sharing" indicator is released instead of
  // stranded); a regression there throws in the handler and the navigation
  // never happens.
  await expect(ana).toHaveURL(/\/$/);
  await expect(ana.getByRole('heading', { name: 'Criar sala' })).toBeVisible();

  await anaCtx.close();
});

test("the card's own stop-watching button ends just that watch", async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const caioCtx = await browser.newContext();
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala parar');
  const bob = await joinRoom(bobCtx, url, 'Bob');
  // A third member so the grid is 2 columns — Ana's card and its corner
  // controls then sit clear of the bottom-centre control bar.
  await joinRoom(caioCtx, url, 'Caio');

  await startSharing(ana);
  await watchSharer(bob, 'Ana');

  const anaCard = memberCard(bob, 'Ana');
  await anaCard.hover();
  await anaCard.getByRole('button', { name: 'Parar de assistir' }).click();
  // Falls back to the avatar once the incoming connection is torn down.
  await expect(anaCard.locator('.card__avatar')).toBeVisible({ timeout: MEDIA_SETTLE_MS });
  // Ana is still sharing — the watch pill comes back for a re-watch.
  await expect(anaCard.locator('.card__watch-pill')).toBeVisible();

  await anaCtx.close();
  await bobCtx.close();
  await caioCtx.close();
});

test('the per-card quality menu opens on hover and closes when the pointer leaves', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const caioCtx = await browser.newContext();
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala qualidade');
  const bob = await joinRoom(bobCtx, url, 'Bob');
  await joinRoom(caioCtx, url, 'Caio'); // 2-column grid — see the note above.

  await startSharing(ana);
  await watchSharer(bob, 'Ana');

  const anaCard = memberCard(bob, 'Ana');
  await anaCard.hover();
  const menu = anaCard.locator('.quality-menu');
  const popup = menu.locator('.quality-menu__popup');

  await expect(popup).toHaveCSS('pointer-events', 'none');

  // Hovering the trigger reveals the list.
  await menu.locator('.quality-menu__trigger').hover();
  await expect(popup).toHaveCSS('pointer-events', 'auto');
  await popup.getByRole('button', { name: 'Baixa' }).click();
  await expect(menu.locator('.quality-menu__current')).toHaveText('Baixa');

  // Moving the pointer off the whole menu closes it again.
  await bob.mouse.move(4, 4);
  await expect(popup).toHaveCSS('pointer-events', 'none');

  await anaCtx.close();
  await bobCtx.close();
  await caioCtx.close();
});

test('the transmission menu switches the video mode', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala transmissao');
  await startSharing(ana);

  // Wake the auto-hiding control bar before focusing (focus moves no mouse).
  await ana.mouse.move(300, 300);
  const trigger = ana.getByRole('button', { name: TRANSMISSION });
  await trigger.focus();
  const popup = ana.locator('.transmission-menu__popup');
  await expect(popup).toHaveCSS('pointer-events', 'auto');

  // Motion ("Vídeo e jogos") is the default; switch to detail.
  await expect(popup.getByRole('button', { name: 'Vídeo e jogos' })).toHaveClass(
    /transmission-menu__opt--on/,
  );
  await popup.getByRole('button', { name: 'Textos e código' }).click();
  await expect(popup.getByRole('button', { name: 'Textos e código' })).toHaveClass(
    /transmission-menu__opt--on/,
  );

  await anaCtx.close();
});

test('the transmission menu hides the audio rows when the share carries no audio', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  // The fake capture always hands back an audio track; a real whole-screen
  // or window share never does (Chrome only offers "share tab audio" for a
  // shared tab). Strip audio from the captured stream to reproduce that.
  await anaCtx.addInitScript(() => {
    const devices = navigator.mediaDevices;
    const original = devices.getDisplayMedia.bind(devices);
    devices.getDisplayMedia = async (...args: Parameters<typeof original>) => {
      const stream = await original(...args);
      for (const track of stream.getAudioTracks()) {
        track.stop();
        stream.removeTrack(track);
      }
      return stream;
    };
  });
  const { page: ana } = await createPublicRoom(anaCtx, 'Ana', 'Sala sem audio');
  await startSharing(ana);

  // The header chip is the sharer-visible confirmation there is no audio.
  await expect(ana.locator('.audio-chip')).toContainText('Áudio desligado');

  await ana.mouse.move(300, 300);
  await ana.getByRole('button', { name: TRANSMISSION }).focus();
  const popup = ana.locator('.transmission-menu__popup');
  await expect(popup).toHaveCSS('pointer-events', 'auto');

  // Video mode is always offered; the audio-quality and mute rows are not,
  // since there is no audio track for them to act on.
  await expect(popup.getByText('Modo de vídeo')).toBeVisible();
  await expect(popup.getByText('Qualidade do áudio')).toBeHidden();
  await expect(popup.locator('.transmission-menu__mute')).toBeHidden();

  await anaCtx.close();
});

test('the invite button copies the room link and confirms it', async ({ browser }) => {
  const anaCtx = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'] });
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala convite');

  await ana.getByRole('button', { name: 'Copiar link de convite da sala' }).click();
  await expect(ana.locator('.invite-btn')).toContainText('Link copiado!');
  await expect(ana.locator('.invite-btn')).toHaveClass(/invite-btn--copied/);
  expect(await ana.evaluate(() => navigator.clipboard.readText())).toBe(url);

  await anaCtx.close();
});

test('the member count follows joins and leaves', async ({ browser }) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala contagem');
  await expect(ana.locator('.room-member-count')).toHaveText('1/10');

  await joinRoom(bobCtx, url, 'Bob');
  await expect(ana.locator('.room-member-count')).toHaveText('2/10');

  await bobCtx.close();
  await expect(ana.locator('.room-member-count')).toHaveText('1/10');

  await anaCtx.close();
});

test('card state classes mark who is sharing, who is watched, and which card is yours', async ({
  browser,
}) => {
  const anaCtx = await browser.newContext();
  const bobCtx = await browser.newContext();
  const { page: ana, url } = await createPublicRoom(anaCtx, 'Ana', 'Sala estados');
  const bob = await joinRoom(bobCtx, url, 'Bob');

  await startSharing(ana);
  // Ana's own card: hers, and live.
  await expect(memberCard(ana, 'Ana')).toHaveClass(/card--self/);
  await expect(memberCard(ana, 'Ana')).toHaveClass(/card--live/);

  await watchSharer(bob, 'Ana');
  // On Bob's side: Ana's card is live and patched; Bob's own card is his.
  await expect(memberCard(bob, 'Ana')).toHaveClass(/card--live/);
  await expect(memberCard(bob, 'Ana')).toHaveClass(/card--patched/);
  await expect(memberCard(bob, 'Bob')).toHaveClass(/card--self/);
  await expect(memberCard(bob, 'Bob')).not.toHaveClass(/card--patched/);

  await anaCtx.close();
  await bobCtx.close();
});
