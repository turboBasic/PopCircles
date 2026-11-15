import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  snapshotPathTemplate: '{testDir}/{testFilePath}-snapshots/{arg}{ext}',
  expect: {
    toHaveScreenshot: { animations: 'disabled', maxDiffPixelRatio: 0.02 },
  },
  use: {
    baseURL: 'http://127.0.0.1:4173',
    browserName: 'chromium',
  },
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1 --port 4173',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: false,
  },
});
