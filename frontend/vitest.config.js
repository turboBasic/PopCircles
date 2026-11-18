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
            include: ['src/**/*.unit.test.js'],
          },
        },
        {
          test: {
            name: 'component',
            include: ['src/**/*.component.test.js'],
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
