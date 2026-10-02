import { describe, test, expect } from 'bun:test';
import fs from 'fs';
import path from 'path';
import * as utils from '../scripts/lib/utils.js';

// skill/ is the install payload for every harness (docs/adr/0001), so these
// checks guard the folder itself rather than any build output.

describe('skill scripts payload', () => {
  const ROOT_DIR = process.cwd();
  const { skills } = utils.readSourceFiles(ROOT_DIR);
  const scripts = skills[0]?.scripts ?? [];
  const names = new Set(scripts.map((s) => s.name));

  test('ships only the launcher, VERSION, and command metadata', () => {
    // Browser-run assets and comp data left the skill with the features that
    // used them (docs/adr/0011, 0012); the skill carries no page JS or data.
    expect([...names].sort()).toEqual(['VERSION', 'command-metadata.json', 'impeccable', 'impeccable.cmd']);
  });

  test('never reads platform binaries as source', () => {
    expect([...names].filter((n) => n.startsWith('bin/'))).toEqual([]);
  });

  test('the launcher is executable and VERSION matches the engine crates', () => {
    const launcher = scripts.find((s) => s.name === 'impeccable');
    expect(launcher.mode & 0o111).not.toBe(0);
    const version = scripts.find((s) => s.name === 'VERSION').content.trim();
    // `impeccable --version` prints the crate version; the launcher pins VERSION.
    const cargo = fs.readFileSync(path.join(ROOT_DIR, 'Cargo.toml'), 'utf-8');
    expect(cargo.match(/^version = "([^"]+)"/m)[1]).toBe(version);
  });
});

describe('universal skill source', () => {
  const ROOT = process.cwd();
  const SKILL_DIR = path.join(ROOT, 'skill');
  const markdown = utils.readFilesRecursive(SKILL_DIR);

  test('carries spec frontmatter plus the harness keys runtimes tolerate (ADR 0008)', () => {
    const { frontmatter } = utils.parseFrontmatter(fs.readFileSync(path.join(SKILL_DIR, 'SKILL.md'), 'utf-8'));
    // Spec fields, plus Claude Code's `user-invocable` and `argument-hint`.
    // `allowed-tools` stays out: Claude Code then blocks non-interactive activation.
    const allowed = new Set(['name', 'description', 'license', 'compatibility', 'metadata', 'user-invocable', 'argument-hint']);
    for (const key of Object.keys(frontmatter)) expect(allowed.has(key)).toBe(true);
    expect(frontmatter.metadata.version).toBeTruthy();
    // The hint names every command, and only those.
    const commands = Object.keys(JSON.parse(fs.readFileSync(path.join(SKILL_DIR, 'scripts/command-metadata.json'), 'utf-8')));
    const hinted = frontmatter['argument-hint'].replace(/\[target\]$/, '').replace(/[\[\]]/g, '').split(/[·|]/).map(s => s.trim()).filter(Boolean);
    expect([...hinted].sort()).toEqual([...commands].sort());
  });

  test('leaves no build-time placeholders or provider blocks', () => {
    for (const file of markdown) {
      const text = fs.readFileSync(file, 'utf-8');
      expect(text).not.toMatch(/\{\{[a-z_]+\}\}/);
      expect(text).not.toMatch(/^[ \t]*<\/?(claude|claude-code|codex|cursor|gemini)>[ \t]*$/m);
    }
  });

  test('never hardcodes one harness install path for the launcher', () => {
    for (const file of markdown) {
      const text = fs.readFileSync(file, 'utf-8');
      expect(text).not.toMatch(/\.(claude|agents|cursor|github)\/skills\/impeccable\/scripts\/impeccable /);
    }
  });

  test('agents get the scripts path from the parent, not from SKILL.md', () => {
    for (const file of fs.readdirSync(path.join(SKILL_DIR, 'agents'))) {
      expect(fs.readFileSync(path.join(SKILL_DIR, 'agents', file), 'utf-8')).not.toContain('<skill-base-dir>');
    }
  });
});
