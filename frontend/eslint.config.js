import js from '@eslint/js';
import globals from 'globals';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';

export default [
  { ignores: ['dist/**', 'node_modules/**', 'test-results/**'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs.recommended,
  {
    files: ['**/*.ts'],
    languageOptions: { parser: tseslint.parser },
  },
  {
    files: ['**/*.js', '**/*.svelte', '**/*.ts'],
    languageOptions: { globals: globals.browser },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: { parserOptions: { parser: tseslint.parser } },
  },
];
