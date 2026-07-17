// Flat ESLint config (ESLint 10 + typescript-eslint 8). The project is ESM
// ("type": "module"), so this file is loaded as ESM.
//
// Philosophy: catch real maintainability/correctness issues without drowning a
// deliberately hand-tuned codebase in style noise. The high-value rules here are
// the TYPE-AWARE async ones (no-floating-promises / no-misused-promises) — this
// bot is async-heavy and unhandled/misused promises are exactly the bug class
// that has bitten it. Those need type information, so type-aware linting is
// scoped to `src/**/*.ts` (the only files tsconfig.json includes); scripts and
// plain JS are linted with the non-type-aware rules only.
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import globals from 'globals';

export default tseslint.config(
  // Never lint build output, dependencies, the vendored ffmpeg binary, or assets.
  {
    // web-client/ is a standalone frontend package with its own tooling/tsconfig.
    ignores: ['dist/**', 'node_modules/**', 'bin/**', 'assets/**', 'coverage/**', 'web-client/**'],
  },

  // Baseline recommended rules for every linted file.
  js.configs.recommended,
  ...tseslint.configs.recommended,

  // Application source — type-aware linting.
  {
    files: ['src/**/*.ts'],
    languageOptions: {
      globals: globals.node,
      parserOptions: {
        // Resolves each file's tsconfig automatically (typescript-eslint v8).
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // The reason type-aware linting is worth the cost here.
      '@typescript-eslint/no-floating-promises': 'error',
      // Allow async callbacks passed as arguments (discord.js event handlers are
      // async and catch internally); still flag `if (promise)`-style misuse.
      '@typescript-eslint/no-misused-promises': ['error', { checksVoidReturn: { arguments: false } }],
      '@typescript-eslint/await-thenable': 'error',
      // Keep the existing `import type` discipline consistent (auto-fixable).
      '@typescript-eslint/consistent-type-imports': ['warn', { prefer: 'type-imports', fixStyle: 'inline-type-imports' }],
      // Intentionally-unused args/vars/catch bindings may be prefixed with `_`.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' },
      ],
      // `any` shows up at untyped 3rd-party seams (yt-dlp/soundcloud/etc.) — warn,
      // don't fail the build.
      '@typescript-eslint/no-explicit-any': 'warn',
    },
  },

  // Scripts and plain JS/config files — lint, but non-type-aware (not in tsconfig).
  {
    files: ['**/*.{js,mjs,cjs}', 'scripts/**/*.ts'],
    languageOptions: {
      globals: globals.node,
      sourceType: 'module',
    },
    rules: {
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
    },
  },
);
