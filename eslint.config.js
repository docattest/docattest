// @ts-check
// One lint configuration for the repository's TypeScript: the npm package
// (adapters/typescript), the docs' setup module, and the tools (tests/, scripts/, protocol/,
// core's fixture generators), all type-checked and held to the same rules. Python is linted
// by ruff (adapters/python/pyproject.toml), Rust by clippy (core/).
import js from '@eslint/js';
import { defineConfig } from 'eslint/config';
import tseslint from 'typescript-eslint';

/**
 * The compiler is `typescript`, which the adapter loads through its own ts.cjs.
 * @param {string} message
 * @returns {import('eslint').Linter.RulesRecord}
 */
const noCompiler = (message) => ({
  'no-restricted-imports': [
    'error',
    { paths: [{ name: 'typescript', message }], patterns: [{ group: ['**/ts.cjs'], message }] },
  ],
});

const PACKAGE = 'adapters/typescript';

export default defineConfig(
  {
    ignores: [
      '**/node_modules/',
      // Python virtualenvs (adapters/python's linter).
      '**/.venv/',
      `${PACKAGE}/dist/`,
      'core/target/',
      'core/vendor/',
      'adapters/python/',
      'tests/parity/recordings/',
      'tests/bench/results/',
      // Projects the docs run docattest in: what a user writes, not the repository's code.
      'docs/.fixtures/',
    ],
  },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: {
          // The docs' setup module is in no tsconfig of its own; the package's (which
          // type-checks it) serves. The tools have the root tsconfig.json.
          allowDefaultProject: ['docs/*.ts'],
          defaultProject: `${PACKAGE}/tsconfig.json`,
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // Line numbers and counts read naturally in messages.
      '@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
      // A body on its own line gets braces, so a wrapped condition never hides it.
      curly: ['error', 'multi-line'],
      'max-len': ['error', { code: 100, ignoreUrls: true, ignoreRegExpLiterals: true }],
    },
  },
  {
    // Only the TypeScript adapter loads the compiler: the launcher and the programmatic API
    // run the core and start in milliseconds, and the protocol server loads the adapter only
    // when a request needs it.
    files: [`${PACKAGE}/src/**/*.ts`],
    ignores: [`${PACKAGE}/src/language/**`],
    rules: noCompiler(
      'Only src/language may load the compiler; the launcher, the API and the ' +
        'protocol server must not.',
    ),
  },
  {
    // Workers run attested code; they get CommonJS from the session and never load a compiler.
    files: [`${PACKAGE}/src/language/worker/**`, `${PACKAGE}/src/language/node/**`],
    rules: noCompiler('Workers never load the compiler: the session transpiles for them.'),
  },
  {
    files: [`${PACKAGE}/test/**/*.ts`],
    rules: {
      // Test bodies are long lists of cases, and vitest's asymmetric matchers are typed `any`.
      'max-len': 'off',
      '@typescript-eslint/no-unsafe-assignment': 'off',
    },
  },
  {
    // This file: JavaScript, because ESLint loads a TypeScript config only through another
    // dependency; tsc checks it (`// @ts-check`), and typed linting needs a TypeScript file.
    files: ['eslint.config.js'],
    extends: [tseslint.configs.disableTypeChecked],
  },
  {
    // Test code is tables of cases (and the benchmark's scenarios are data), each case one
    // entry and many of them longer than a line: a line limit would only cut a table apart.
    files: ['tests/**/*.ts', 'core/tests/**/*.ts'],
    rules: { 'max-len': 'off' },
  },
);
