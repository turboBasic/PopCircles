import js from '@eslint/js';
import globals from 'globals';
import svelte from 'eslint-plugin-svelte';

export default [
  { ignores: ['dist/**', 'node_modules/**', 'test-results/**'] },
  js.configs.recommended,
  ...svelte.configs.recommended,
  {
    files: ['**/*.js', '**/*.svelte'],
    languageOptions: { globals: globals.browser },
  },
];
