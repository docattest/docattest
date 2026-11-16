import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// Acceptance tests import `docattest` and get the built package, exactly as published: it
// runs the docattest core (DOCATTEST_CORE, which `npm test` sets to the core built in this
// checkout). Unit tests import modules from src directly; there the adapter's ts.cjs, which
// loads the compiler with `require` in the built package, is the compiler itself. The global
// setup builds dist before any test runs.
export default defineConfig({
  resolve: {
    alias: [
      {
        find: /^docattest$/,
        replacement: fileURLToPath(new URL('./dist/index.js', import.meta.url)),
      },
      { find: /^(\.\.?\/)+(.*\/)?ts\.cjs$/, replacement: 'typescript' },
    ],
  },
  test: {
    include: ['test/**/*.test.ts'],
    globalSetup: ['test/global-setup.ts'],
  },
});
