import { playwright } from '@vitest/browser-playwright';
import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config.js';

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      projects: [
        {
          test: {
            name: 'unit',
            environment: 'node',
            include: ['src/**/*.unit.test.ts'],
          },
        },
        {
          test: {
            name: 'component',
            include: ['src/**/*.component.test.ts'],
            browser: {
              enabled: true,
              provider: playwright(),
              instances: [{ browser: 'chromium' }],
              headless: true,
            },
          },
        },
      ],
    },
  }),
);
