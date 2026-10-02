import fs from 'node:fs';
import path from 'node:path';

export const DEFAULT_SUITES = ['core', 'oracle'];
export const OPT_IN_SUITES = [
  'skill-behavior',
  'skill-workflow',
];

const COMMON_INFRA_PATTERNS = [
  /^package\.json$/,
  /^pnpm-lock\.yaml$/,
  /^vitest\.config\.mjs$/,
  /^scripts\/run-tests\.mjs$/,
  /^scripts\/test-suites\.mjs$/,
  /^scripts\/ci-test-plan\.mjs$/,
  /^scripts\/lib\/process-group\.mjs$/,
  /^\.github\/workflows\/ci\.yml$/,
];

export const SUITES = {
  core: {
    description: 'Skill source rules, repository checks, workflows, launcher, and release tooling.',
    triggers: [
      ...COMMON_INFRA_PATTERNS,
      /^scripts\//,
      /^skill\/(SKILL\.md|agents\/|reference\/|scripts\/)/,
      /^skill\/scripts\/VERSION$/,
      /^README\.md$/,
      /^\.github\/workflows\/release-engine\.yml$/,
    ],
    commands: [
      {
        runner: 'vitest',
        // The slowest core test is ~11s; the cap only catches a hang.
        timeoutMs: 180000,
        files: [
          'tests/skill-source.test.js',
          'tests/lib/utils.test.js',
          'tests/release-engine-workflow.test.js',
          'tests/workflow-security.test.js',
          'tests/ci-test-plan.test.mjs',
          'tests/launcher-download.test.mjs',
          'tests/process-group.test.mjs',
          'tests/release.test.mjs',
          'tests/skill-reference.test.mjs',
          'tests/skill-behavior-harness.test.mjs',
          'tests/test-suites.test.mjs',
        ],
      },
    ],
  },
  // The verbs live in the engine binary; this repo pins its behavior with the
  // oracle goldens (tests/oracle), which skip when no binary is present
  // (pnpm run fetch:engine, or IMPECCINO_BIN).
  oracle: {
    description: 'Oracle corpus replay against the engine binary; skips without a binary.',
    triggers: [
      ...COMMON_INFRA_PATTERNS,
      /^skill\/scripts\/VERSION$/,
      /^tests\/oracle\//,
      /^tests\/fixtures\//,
      /^tests\/lib\/engine-bin\.mjs$/,
      /^skill\/(reference\/|scripts\/)/,
    ],
    commands: [
      {
        runner: 'vitest',
        timeoutMs: 900000,
        files: ['tests/oracle.test.mjs'],
      },
    ],
  },
  'skill-behavior': {
    description: 'LLM-backed protocol checkpoints, not full builds.',
    optIn: true,
    triggers: [
      ...COMMON_INFRA_PATTERNS,
      /^skill\/SKILL\.md$/,
      /^skill\/reference\//,
      /^skill\/scripts\/VERSION$/,
      /^tests\/skill-behavior\//,
    ],
    commands: [{
      runner: 'vitest',
      timeoutMs: 240000,
      wallClockMs: 1_800_000,
      files: ['tests/skill-behavior/scenarios.test.mjs'],
    }],
  },
  'skill-workflow': {
    description: 'Explicitly opt-in completed workflows with a preflighted browser.',
    optIn: true,
    needsPlaywright: true,
    triggers: [
      ...COMMON_INFRA_PATTERNS,
      /^skill\//,
      /^skill\/scripts\/VERSION$/,
      /^tests\/skill-workflow\//,
      /^tests\/skill-behavior\//,
    ],
    commands: [
      { runner: 'vitest', files: ['tests/skill-workflow-browser.test.mjs'] },
      {
        runner: 'vitest', timeoutMs: 240000, wallClockMs: 600000,
        files: ['tests/skill-workflow/finish-handoff.test.mjs'],
      },
      {
        runner: 'vitest',
        timeoutMs: 900000,
        wallClockMs: 3_600_000,
        files: ['tests/skill-workflow/full-build.test.mjs'],
      },
    ],
  },
};

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// Every suite must select itself when one of its own test files changes.
// Generated from the files lists so the hand-written trigger patterns above
// only carry source paths and fixture directories; before this, four test
// files were registered in a suite that change-based CI could never select
// by editing them (serve-question, ci-test-plan, both validate-plugin-*),
// and tests/lib/detector-bundle.test.js triggered core while running in
// detector. The meta-test in tests/test-suites.test.mjs pins this invariant.
for (const suite of Object.values(SUITES)) {
  const ownFiles = suite.commands.flatMap((command) => command.files);
  suite.triggers = [
    ...(suite.triggers ?? []),
    ...ownFiles.map((file) => new RegExp(`^${escapeRegExp(file)}$`)),
  ];
}

export function expandSuites(requested) {
  const names = requested.length === 0 ? ['default'] : requested;
  const expanded = [];
  for (const name of names) {
    if (name === 'default' || name === 'all-local') {
      expanded.push(...DEFAULT_SUITES);
    } else if (name === 'all') {
      expanded.push(...DEFAULT_SUITES, ...OPT_IN_SUITES);
    } else if (SUITES[name]) {
      expanded.push(name);
    } else {
      throw new Error(`Unknown test suite "${name}". Run: node scripts/run-tests.mjs --list`);
    }
  }
  return [...new Set(expanded)];
}

export function suiteFiles(suiteNames) {
  const files = [];
  for (const name of suiteNames) {
    const suite = SUITES[name];
    if (!suite) throw new Error(`Unknown test suite "${name}"`);
    for (const command of suite.commands) {
      files.push(...command.files);
    }
  }
  return files;
}

export function findTestFiles(root = process.cwd()) {
  const out = [];
  const stack = [path.join(root, 'tests')];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const abs = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(abs);
      } else if (/\.test\.(js|mjs)$/.test(entry.name)) {
        out.push(path.relative(root, abs).split(path.sep).join('/'));
      }
    }
  }
  return out.sort();
}

export function matchesSuiteTriggers(suiteName, changedFiles) {
  const suite = SUITES[suiteName];
  if (!suite) throw new Error(`Unknown test suite "${suiteName}"`);
  return changedFiles.some((file) => suite.triggers?.some((pattern) => pattern.test(file)));
}
