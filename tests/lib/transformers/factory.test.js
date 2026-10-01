import { describe, test, expect, beforeEach, afterEach, mock } from 'bun:test';
import fs from 'fs';
import path from 'path';
import { createTransformer } from '../../../scripts/lib/transformers/factory.js';
import { parseFrontmatter, readSourceFiles } from '../../../scripts/lib/utils.js';

const TEST_DIR = path.join(process.cwd(), 'test-tmp-factory');
const SRC_ROOT = path.join(TEST_DIR, 'src');
const DIST = path.join(TEST_DIR, 'dist');

const baseConfig = {
  provider: 'cursor',
  configDir: '.test',
  displayName: 'Test Provider',
};

const AGENT = `---
name: impeccable-reviewer
description: Reviews things.
tools: Read, Grep
model: inherit
effort: high
maxTurns: 30
---

# Reviewer

Run \`<scripts-path>/impeccable detect\`. <!-- rule:agent-detect -->
`;

function write(rel, content, mode) {
  const file = path.join(SRC_ROOT, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
  if (mode) fs.chmodSync(file, mode);
}

function fixtureSkill() {
  write('skill/SKILL.md', '---\nname: impeccable\ndescription: Design.\nlicense: Apache-2.0\nmetadata:\n  version: 1.2.3\n---\n\nRun `<skill-base-dir>/scripts/impeccable context`. <!-- rule:setup -->\n');
  write('skill/reference/polish.md', '# Polish\n');
  write('skill/agents/impeccable-reviewer.md', AGENT);
  write('skill/scripts/impeccable', '#!/bin/sh\n', 0o755);
  write('skill/scripts/data/font-index.json', '{}');
  write('skill/scripts/config.json', '{"project":"local"}');
  write('skill/scripts/bin/darwin-arm64/impeccable', 'binary', 0o755);
  return readSourceFiles(SRC_ROOT).skills;
}

const out = (...rel) => path.join(DIST, 'cursor', '.test', ...rel);

describe('createTransformer factory', () => {
  beforeEach(() => fs.rmSync(TEST_DIR, { recursive: true, force: true }));
  afterEach(() => fs.rmSync(TEST_DIR, { recursive: true, force: true }));

  test('creates the provider skills directory even without skills', () => {
    createTransformer(baseConfig)([], DIST);
    expect(fs.existsSync(out('skills'))).toBe(true);
  });

  test('copies skill/ verbatim: frontmatter, body, rule markers, references, agents', () => {
    const skills = fixtureSkill();
    createTransformer(baseConfig)(skills, DIST);
    for (const rel of ['SKILL.md', 'reference/polish.md', 'agents/impeccable-reviewer.md', 'scripts/data/font-index.json']) {
      expect(fs.readFileSync(out('skills/impeccable', rel), 'utf-8'))
        .toBe(fs.readFileSync(path.join(SRC_ROOT, 'skill', rel), 'utf-8'));
    }
  });

  test('keeps the launcher executable', () => {
    createTransformer(baseConfig)(fixtureSkill(), DIST);
    expect(fs.statSync(out('skills/impeccable/scripts/impeccable')).mode & 0o111).not.toBe(0);
  });

  test('leaves out fetched engine binaries and per-project script config', () => {
    createTransformer(baseConfig)(fixtureSkill(), DIST);
    expect(fs.existsSync(out('skills/impeccable/scripts/bin'))).toBe(false);
    expect(fs.existsSync(out('skills/impeccable/scripts/config.json'))).toBe(false);
  });

  test('cleans the provider directory before writing', () => {
    fs.mkdirSync(out('skills/stale'), { recursive: true });
    createTransformer(baseConfig)(fixtureSkill(), DIST);
    expect(fs.existsSync(out('skills/stale'))).toBe(false);
  });

  test('logs a one-line summary', () => {
    const log = mock(() => {});
    const original = console.log;
    console.log = log;
    try {
      createTransformer({ ...baseConfig, agentFormat: 'cursor-md' })(fixtureSkill(), DIST);
    } finally {
      console.log = original;
    }
    expect(log).toHaveBeenCalledWith('✓ Test Provider: 1 skill (1 agent files)');
  });

  test('claude-md agents are the source files, unchanged', () => {
    createTransformer({ ...baseConfig, agentFormat: 'claude-md' })(fixtureSkill(), DIST);
    expect(fs.readFileSync(out('agents/impeccable-reviewer.md'), 'utf-8')).toBe(AGENT);
  });

  test('cursor-md agents map tools to readonly and keep the body', () => {
    createTransformer({ ...baseConfig, agentFormat: 'cursor-md' })(fixtureSkill(), DIST);
    const { frontmatter, body } = parseFrontmatter(fs.readFileSync(out('agents/impeccable-reviewer.md'), 'utf-8'));
    expect(frontmatter).toEqual({
      name: 'impeccable-reviewer',
      description: 'Reviews things.',
      model: 'inherit',
      readonly: true,
      is_background: false,
    });
    expect(body).toContain('<scripts-path>/impeccable detect');
  });

  test('copilot agents carry only name and description', () => {
    createTransformer({ ...baseConfig, agentFormat: 'copilot-agent-md' })(fixtureSkill(), DIST);
    const { frontmatter } = parseFrontmatter(fs.readFileSync(out('agents/impeccable-reviewer.agent.md'), 'utf-8'));
    expect(frontmatter).toEqual({ name: 'impeccable-reviewer', description: 'Reviews things.' });
  });

  test('emits the hook manifest at the configured path', () => {
    createTransformer({ ...baseConfig, emitHooks: 'claude', hooksManifestRel: 'settings.json' })(fixtureSkill(), DIST);
    const manifest = JSON.parse(fs.readFileSync(out('settings.json'), 'utf-8'));
    expect(Object.keys(manifest.hooks)).toContain('PostToolUse');
  });
});
