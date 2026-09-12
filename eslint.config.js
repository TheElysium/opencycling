import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  // Svelte configs must come after typescript-eslint so their parser wins
  // for *.svelte and *.svelte.ts (runes) files.
  ...svelte.configs['flat/recommended'],
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
    // Mirrors the Rust gates: complexity threshold = clippy.toml (15);
    // file size stays in scripts/check_size.sh (shared with .rs files).
    rules: {
      complexity: ['error', 15],
      'max-lines-per-function': [
        'error',
        { max: 300, skipBlankLines: true, skipComments: true },
      ],
      // console.warn/error are legitimate error reporting paths.
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      // Desktop Tauri app: navigations are fixed internal routes given as
      // absolute paths; resolve() would add noise without benefit.
      'svelte/no-navigation-without-resolve': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_' },
      ],
    },
  },
  {
    // The svelte parser delegates inner TS (script blocks and .svelte.ts
    // rune modules) to the TS parser, per eslint-plugin-svelte README.
    files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: ['.svelte'],
      },
    },
  },
  {
    // bindings.ts is generated from Rust (never edit by hand) — not lintable source.
    ignores: [
      'build/*',
      'dist/*',
      '.svelte-kit/*',
      'static/*',
      'scripts/*',
      'src-tauri/*',
      'src/lib/bindings.ts',
    ],
  },
);
