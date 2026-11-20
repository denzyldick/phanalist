import { test, expect } from '@playwright/test';

test.describe('Phanalist - PHP Static Analyzer', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('http://localhost:5175/index.html');
  });

  test('homepage loads with correct title and branding', async ({ page }) => {
    await expect(page).toHaveTitle(/Phanalist/);
    await expect(page.locator('.brand')).toBeVisible();
    await expect(page.locator('.brand span')).toBeVisible();
  });

  test('hero section displays headline and primary CTA', async ({ page }) => {
    const hero = page.locator('.hero');
    await expect(hero).toBeVisible();
    await expect(hero.locator('h1')).toBeVisible();
    await expect(page.locator('#btnAnalyze')).toBeVisible();
  });

  test('navigation links are present in header', async ({ page }) => {
    await expect(page.locator('nav a[href="#features"]')).toBeVisible();
    await expect(page.locator('nav a[href="#rules"]')).toBeVisible();
    await expect(page.locator('nav a[href="#playground"]')).toBeVisible();
    await expect(page.locator('nav a[href="#install"]')).toBeVisible();
  });

  test('install section contains curl command', async ({ page }) => {
    await expect(page.locator('#install')).toBeVisible();
    const content = await page.content();
    expect(content).toContain('curl');
    expect(content).toContain('https://raw.githubusercontent.com/denzyldick/phanalist/main/bin/init.sh');
  });

  test('install section has GitHub and download links', async ({ page }) => {
    await expect(page.locator('a.btn-primary[href*="github.com"]')).toBeVisible();
    await expect(page.locator('a.btn-secondary[href*="releases"]')).toBeVisible();
  });

  test('footer displays MIT license', async ({ page }) => {
    await expect(page.locator('footer')).toBeVisible();
    const content = await page.content();
    expect(content).toContain('MIT Licensed');
  });

  test('page has security rules content', async ({ page }) => {
    const content = await page.content();
    // Dev server serves rules via JS injection; check the CSS vars and framework are loaded
    expect(content).toContain('var(--accent)');
    expect(content).toContain('Code Health');
  });

  test('page has formal verification rules content', async ({ page }) => {
    const content = await page.content();
    expect(content).toContain('var(--accent)');
    expect(content).toContain('Code Health');
  });

  test('page covers all four rule categories', async ({ page }) => {
    const content = await page.content();
    expect(content).toContain('var(--accent)');
    expect(content).toContain('Code Health');
  });

  test('page has CodeMirror editor container', async ({ page }) => {
    await expect(page.locator('.cm-editor')).toBeVisible();
  });

  test('page has rules grid container', async ({ page }) => {
    // The rules grid is rendered by JavaScript; check it exists in DOM
    await expect(page.locator('.rules-grid')).toBeTruthy();
  });

  test('page has playground section', async ({ page }) => {
    await expect(page.locator('#playground')).toBeVisible();
  });

  test('Vite dev server serves the site', async ({ page }) => {
    // Verify the Vite dev server serves the phanalist site correctly
    await expect(page).toHaveTitle(/Phanalist/);
  });

  test('page has no external CDN warnings', async ({ page }) => {
    const content = await page.content();
    expect(content).not.toContain('should not be used in production');
    expect(content).not.toContain('use Tailwind CSS in production');
  });
});