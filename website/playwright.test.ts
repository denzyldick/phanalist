import { test, expect } from '@playwright/test';

test.describe('Phanalist website', () => {
  test('homepage loads and has key sections', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle(/Phanalist/);
    await expect(page.locator('.brand')).toBeVisible();
    await expect(page.locator('#features')).toBeVisible();
    await expect(page.locator('#rules')).toBeVisible();
    await expect(page.locator('#playground')).toBeVisible();
    await expect(page.locator('#install')).toBeVisible();
  });

  test('playground mounts React editor', async ({ page }) => {
    await page.goto('/#playground');
    const root = page.locator('#root');
    await expect(root).toBeAttached();
    await expect(page.locator('#ruleSelect')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('.play-card').first()).toBeVisible();
  });

  test('rules explorer loads catalog', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#rulesContainer')).toBeVisible();
    await expect(page.locator('#rulesContainer h3').first()).toBeVisible({ timeout: 15000 });
    await page.locator('#rulesContainer h3').first().click();
    await expect(page.locator('.rule-card').first()).toBeVisible({ timeout: 15000 });
  });

  test('install section contains curl command', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#install')).toContainText('curl --proto');
  });
});
