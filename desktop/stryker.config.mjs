// StrykerJS — mutation testing for the Electron main process. Report-only
// (scheduled `stryker-desktop` job); the Vitest suite runs each mutant.
// @type {import('@stryker-mutator/api/core').PartialStrykerOptions}
export default {
  testRunner: 'vitest',
  // Explicit — pnpm's non-flat node_modules defeats Stryker's glob-based
  // plugin auto-discovery.
  plugins: ['@stryker-mutator/vitest-runner'],
  // Mutate in place instead of a sandbox copy: skips Stryker's tsconfig
  // rewriter, which calls `ts.parseConfigFileTextToJson` — absent from
  // the TypeScript 7 native build this project pins. Stryker restores
  // every file afterwards; CI runs on a throwaway checkout anyway.
  inPlace: true,
  reporters: ['progress', 'clear-text', 'html'],
  htmlReporter: { fileName: 'reports/mutation/index.html' },
  mutate: [
    'src/**/*.ts',
    '!src/**/*.test.ts',
    '!src/test-helpers/**',
    // Bootstrap wiring only — no branch logic worth mutating, and it
    // imports the whole app at load.
    '!src/main/index.ts',
    // Deferred (roadmap Phase 4): no unit tests yet, so every mutant
    // would "survive" and drown the signal.
    '!src/features/screen-share/picker.ts',
    '!src/features/screen-share/display-media.ts',
  ],
  incremental: true,
  incrementalFile: 'reports/mutation/stryker-incremental.json',
};
