import { expect, test } from '@playwright/test';

// A room code is 4+ uppercase alphanumerics (see `generate_room_code`).
const ROOM_URL = /\/r\/[A-Z0-9]+$/;

function createPanel(page: import('@playwright/test').Page) {
  return page.locator('.panel', { hasText: 'Criar sala' });
}

function joinPanel(page: import('@playwright/test').Page) {
  return page.locator('.panel', { hasText: 'Entrar em uma sala' });
}

test.describe('home page — create room', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { name: 'Criar sala' })).toBeVisible();
  });

  test('a blank password without the public checkbox is a validation error', async ({ page }) => {
    await page.getByLabel('Nick').fill('Ana');
    await page.getByLabel('Nome da sala').fill('Sala da Ana');
    await createPanel(page).getByRole('button', { name: 'Criar sala' }).click();

    const status = createPanel(page).locator('.status-text');
    await expect(status).toContainText('Digite uma senha ou marque');
    // Every validation/protocol error renders in the error (red) style —
    // it used to fall through to the neutral "tip" color instead.
    await expect(status).toHaveClass(/status-text--error/);
    await expect(page).toHaveURL(/\/$/);
  });

  test('a nick over the length limit is a validation error, shown in red', async ({ page }) => {
    await page.getByLabel('Nick').fill('a'.repeat(40));
    await page.getByLabel('Nome da sala').fill('Sala da Ana');
    await page.getByLabel('Senha da sala').fill('senha123');
    await createPanel(page).getByRole('button', { name: 'Criar sala' }).click();

    const status = createPanel(page).locator('.status-text');
    await expect(status).toContainText('Nick vazio, muito longo');
    await expect(status).toHaveClass(/status-text--error/);
  });

  test('a dismissible validation error reverts on its own after a delay', async ({ page }) => {
    await page.getByLabel('Nick').fill('Ana');
    await page.getByLabel('Nome da sala').fill('Sala da Ana');
    await createPanel(page).getByRole('button', { name: 'Criar sala' }).click();

    const status = createPanel(page).locator('.status-text');
    await expect(status).toContainText('Digite uma senha ou marque');
    // ERROR_DISMISS_MS in `client::dom` — generous margin above it.
    await expect(status).toHaveText('Pronto para criar uma sala.', { timeout: 8000 });
    await expect(status).not.toHaveClass(/status-text--error/);
  });

  test('a password room is created and the browser lands inside it', async ({ page }) => {
    await page.getByLabel('Nick').fill('Ana');
    await page.getByLabel('Nome da sala').fill('Sala fechada');
    await page.getByLabel('Senha da sala').fill('senha123');
    await createPanel(page).getByRole('button', { name: 'Criar sala' }).click();

    await expect(page).toHaveURL(ROOM_URL);
    await expect(page.locator('#member-grid')).toBeVisible();
    await expect(page.locator('.card__nick', { hasText: 'Ana' })).toBeVisible();
  });

  test('toggling "sala pública" creates a room with no password', async ({ page }) => {
    await page.getByLabel('Nick').fill('Bia');
    await page.getByLabel('Nome da sala').fill('Sala aberta');
    // The checkbox is visually hidden behind the switch UI — click the
    // switch itself, the way a user does, then assert the state took.
    await createPanel(page).locator('label.switch').click();
    await expect(page.getByLabel('Sala pública')).toBeChecked();
    // The password field is hidden once the room is public.
    await expect(page.getByLabel('Senha da sala')).toBeHidden();
    await createPanel(page).getByRole('button', { name: 'Criar sala' }).click();

    await expect(page).toHaveURL(ROOM_URL);
    await expect(page.locator('#member-grid')).toBeVisible();
  });
});

test.describe('home page — join form', () => {
  test('an unknown room code lands on the "not found" screen, not a nick form', async ({ page }) => {
    await page.goto('/');
    await page.getByLabel('Código ou link da sala').fill('ZZZZ9999');
    await page.getByRole('button', { name: 'Entrar na sala' }).click();

    await expect(page).toHaveURL('/r/ZZZZ9999');
    await expect(page.getByRole('heading', { name: 'Sala não encontrada' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Entrar na sala' })).toBeHidden();
  });

  test('an input with no resolvable code is a red, dismissible validation error', async ({
    page,
  }) => {
    await page.goto('/');
    // Non-empty (passes the field's `required`) but resolves to no code.
    await page.getByLabel('Código ou link da sala').fill('///');
    await page.getByRole('button', { name: 'Entrar na sala' }).click();

    const status = joinPanel(page).locator('.status-text');
    await expect(status).toContainText('Informe o código da sala');
    await expect(status).toHaveClass(/status-text--error/);
    // ERROR_DISMISS_MS in `client::dom` — generous margin above it.
    await expect(status).toBeHidden({ timeout: 8000 });
  });

  test('a full invite link is accepted and its code is uppercased', async ({ page }) => {
    await page.goto('/');
    await page
      .getByLabel('Código ou link da sala')
      .fill('http://127.0.0.1:3000/r/abcd1234?foo=bar');
    await page.getByRole('button', { name: 'Entrar na sala' }).click();

    await expect(page).toHaveURL('/r/ABCD1234');
  });

  test('the "Sala não encontrada" screen links back to the home page', async ({ page }) => {
    await page.goto('/r/ZZZZ9999');
    await expect(page.getByRole('heading', { name: 'Sala não encontrada' })).toBeVisible();
    await page.getByRole('link', { name: 'Voltar à página principal' }).click();
    await expect(page).toHaveURL(/\/$/);
    await expect(page.getByRole('heading', { name: 'Criar sala' })).toBeVisible();
  });
});

test.describe('room gate — validation', () => {
  test('a password room with no password filled in is a red, dismissible error', async ({
    browser,
  }) => {
    const creatorCtx = await browser.newContext();
    const creator = await creatorCtx.newPage();
    await creator.goto('/');
    await creator.getByLabel('Nick').fill('Ana');
    await creator.getByLabel('Nome da sala').fill('Sala fechada');
    await creator.getByLabel('Senha da sala').fill('senha123');
    await createPanel(creator).getByRole('button', { name: 'Criar sala' }).click();
    await expect(creator).toHaveURL(ROOM_URL);
    const roomUrl = creator.url();

    // A separate context — the creator's own browser would silently
    // rejoin with its stored session instead of showing the gate form.
    const joinerCtx = await browser.newContext();
    const joiner = await joinerCtx.newPage();
    await joiner.goto(roomUrl);
    await expect(joiner.getByRole('heading', { name: 'Entrar na sala' })).toBeVisible();
    await joiner.getByLabel('Nick').fill('Bob');
    await joiner.getByRole('button', { name: 'Entrar' }).click();

    // Scoped to the gate panel: the (hidden, pre-auth) stage header also
    // has a `.status-text` span elsewhere in the DOM.
    const status = joiner
      .locator('.panel', { hasText: 'Entrar na sala' })
      .locator('.status-text');
    await expect(status).toContainText('Preencha nick e senha.');
    await expect(status).toHaveClass(/status-text--error/);
    // ERROR_DISMISS_MS in `client::dom` — generous margin above it.
    await expect(status).toHaveText('Informe o nick da sala.', { timeout: 8000 });
    await expect(status).not.toHaveClass(/status-text--error/);
  });
});

test.describe('unknown route', () => {
  test('an unknown path renders the 404 page, not a blank string', async ({ page }) => {
    await page.goto('/isto-nao-existe');
    await expect(page.locator('.not-found')).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Esta página não existe.' })).toBeVisible();

    await page.getByRole('link', { name: 'Voltar ao início' }).click();
    await expect(page).toHaveURL(/\/$/);
    await expect(page.getByRole('heading', { name: 'Criar sala' })).toBeVisible();
  });
});
