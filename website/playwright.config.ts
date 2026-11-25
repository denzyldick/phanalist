import { defineConfig } from '@playwright/test';

const basePath = (process.env.BASE_PATH || '/phanalist/').replace(/\/$/, '');
const port = 4173;

export default defineConfig({
  testDir: '.',
  testMatch: 'playwright.test.ts',
  timeout: 30_000,
  use: {
    baseURL: `http://127.0.0.1:${port}${basePath}/`,
    headless: true,
  },
  webServer: {
    command: `npm run build && npm run preview -- --host 127.0.0.1 --port ${port}`,
    env: {
      BASE_PATH: `${basePath}/`,
      SITE_URL: 'https://denzyldick.github.io',
    },
    url: `http://127.0.0.1:${port}${basePath}/`,
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
