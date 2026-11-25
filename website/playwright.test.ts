import { test, expect } from '@playwright/test';

const basePath = (process.env.BASE_PATH || '/phanalist/').replace(/\/$/, '');
const siteUrl = (process.env.SITE_URL || 'https://denzyldick.github.io').replace(/\/$/, '');
const e93Path = `${basePath}/rules/e0093.html`;

test.describe('Phanalist website', () => {
  test('homepage loads and has key sections', async ({ page }) => {
    await page.goto('./');
    await expect(page).toHaveTitle(/Phanalist/);
    await expect(page.locator('.brand')).toBeVisible();
    await expect(page.locator('#features')).toBeVisible();
    await expect(page.locator('#rules')).toBeVisible();
    await expect(page.locator('#playground')).toBeVisible();
    await expect(page.locator('#install')).toBeVisible();
  });

  test('playground mounts React editor', async ({ page }) => {
    await page.goto('./#playground');
    const root = page.locator('#root');
    await expect(root).toBeAttached();
    await expect(page.locator('#ruleSelect')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('.play-card').first()).toBeVisible();
  });

  test('rules explorer loads catalog with base-path links', async ({ page }) => {
    await page.goto('./');
    await expect(page.locator('#rulesContainer')).toBeVisible();
    await expect(page.locator('#rulesContainer button').first()).toBeVisible({ timeout: 15000 });
    await page.locator('#rulesContainer button').first().click();
    await expect(page.locator('.rule-card').first()).toBeVisible({ timeout: 15000 });

    await page.locator('#rulesSearch').fill('E0093');
    await expect(page.locator('.rule-card')).toHaveCount(1);
    await expect(page.locator('.rule-doc-link').first()).toHaveAttribute('href', e93Path);
  });

  test('install section contains curl command', async ({ page }) => {
    await page.goto('./');
    await expect(page.locator('#install')).toContainText('curl --proto');
  });

  test('open rule page link uses the configured base path', async ({ page }) => {
    await page.goto('./#playground');
    await expect(page.locator('#ruleSelect')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('#ruleSelect option[value="E0093"]')).toHaveCount(1, {
      timeout: 15000,
    });
    await page.locator('#ruleSelect').selectOption('E0093');

    const link = page.getByRole('link', { name: 'Open rule page' });
    await expect(link).toHaveAttribute('href', e93Path);
    await link.click();

    await expect(page).toHaveURL(new RegExp(`${e93Path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`));
    await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
      'href',
      `${siteUrl}${e93Path}`
    );
  });
});
