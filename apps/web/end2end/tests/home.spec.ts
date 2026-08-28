import { expect, test } from '@playwright/test';

// A room code is 4+ uppercase alphanumerics (see `generate_room_code`).
const ROOM_URL = /\/r\/[A-Z0-9]+$/;

function createPanel(page: import('@playwright/test').Page) {
  return page.locator('.panel', { hasText: 'Criar sala' });
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

    await expect(createPanel(page).locator('.status-text')).toContainText(
      'Digite uma senha ou marque',
    );
    await expect(page).toHaveURL(/\/$/);
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

  test('a full invite link is accepted and its code is uppercased', async ({ page }) => {
    await page.goto('/');
    await page
      .getByLabel('Código ou link da sala')
      .fill('http://127.0.0.1:3000/r/abcd1234?foo=bar');
    await page.getByRole('button', { name: 'Entrar na sala' }).click();

    await expect(page).toHaveURL('/r/ABCD1234');
  });
});
