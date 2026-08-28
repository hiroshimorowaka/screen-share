import { resolve } from 'node:path';
import { defineConfig } from 'vitest/config';

// The main process runs in Node, and the source imports through the
// package.json `imports` map (`#platform/...`, `#native/...`) with `.js`
// specifiers that resolve to `dist/` at runtime. Tests run against the
// TypeScript source instead, so remap `#…` to `src/` (and `#native/…`
// stays under `native/`), rewriting the `.js` suffix to `.ts`.
const here = import.meta.dirname;

export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
    clearMocks: true,
    restoreMocks: true,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/**/*.ts'],
      exclude: ['src/**/*.test.ts', 'src/test-helpers/**'],
    },
  },
  resolve: {
    alias: [
      { find: /^#native\/(.*)\.js$/, replacement: resolve(here, 'native/$1.js') },
      { find: /^#(.*)\.js$/, replacement: resolve(here, 'src/$1.ts') },
    ],
  },
});
