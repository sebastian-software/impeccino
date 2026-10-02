import { defineConfig } from 'vitest/config';

// One runner for every suite; scripts/run-tests.mjs picks the files per suite
// (scripts/test-suites.mjs) and sets the per-suite timeouts.
export default defineConfig({
  test: {
    include: ['tests/**/*.test.{js,mjs}'],
    exclude: ['**/node_modules/**', 'tests/fixtures/**', 'tests/oracle/workspaces/**'],
    // Tests spawn the engine, launchers, and git; separate processes keep a
    // wedged child from taking the whole run with it.
    pool: 'forks',
    testTimeout: 180_000,
    hookTimeout: 180_000,
  },
});
