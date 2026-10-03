import { describe, test, expect } from 'vitest';
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

  test('ships only the launcher, VERSION, engine pins, and command metadata', () => {
    // Browser-run assets and comp data left the skill with the features that
    // used them (docs/adr/0011, 0012); the skill carries no page JS or data.
    // engine.sha256 pins the release binaries the launcher downloads (0010).
    expect([...names].sort()).toEqual(['VERSION', 'command-metadata.json', 'engine.sha256', 'impeccino', 'impeccino.cmd']);
  });

  test('never reads platform binaries as source', () => {
    expect([...names].filter((n) => n.startsWith('bin/'))).toEqual([]);
  });

  test('the launcher is executable and VERSION matches the engine crates', () => {
    const launcher = scripts.find((s) => s.name === 'impeccino');
    expect(launcher.mode & 0o111).not.toBe(0);
    const version = scripts.find((s) => s.name === 'VERSION').content.trim();
    // `impeccino --version` prints the crate version; the launcher pins VERSION.
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
      expect(text).not.toMatch(/\.(claude|agents|cursor|github)\/skills\/impeccino\/scripts\/impeccino /);
    }
  });

  test('agent roles do not depend on the launcher path', () => {
    for (const file of fs.readdirSync(path.join(SKILL_DIR, 'agents'))) {
      const source = fs.readFileSync(path.join(SKILL_DIR, 'agents', file), 'utf-8');
      expect(source).not.toContain('<skill-base-dir>');
      expect(source).not.toContain('<scripts-path>');
    }
  });

  test('dispatches shipped roles without harness-specific agent names and reads generic fallback instructions', () => {
    const skill = fs.readFileSync(path.join(SKILL_DIR, 'SKILL.md'), 'utf-8');
    const dispatch = skill.split('## Shipped agents')[1].split(/\n## /)[0];
    const newWork = fs.readFileSync(path.join(SKILL_DIR, 'reference/new-work.md'), 'utf-8');

    expect(dispatch).not.toMatch(/impeccino_finish_reviewer|\/impeccino-finish-reviewer|GitHub Copilot/);
    expect(dispatch).toMatch(/installed role definition when the host exposes it/i);
    expect(dispatch).toMatch(/read the matching file.*pass its full Markdown body.*fresh general-purpose subagent/i);
    expect(dispatch).toMatch(/no inherited conversation history/i);
    expect(dispatch).toMatch(/tools.*frontmatter.*child tool limits/i);
    expect(dispatch).toMatch(/complete the role locally according to its full output contract before resuming the parent workflow/i);
    expect(dispatch).toMatch(/inline finish review is not independent/i);
    expect(dispatch).toMatch(/keep that disclosure outside the role's contracted return/i);

    expect(newWork).not.toMatch(/Never read the shipped agents' definition files before spawning/);
    expect(newWork).toMatch(/<skill-base-dir>\/agents\/impeccino-finish-reviewer\.md/);
    expect(newWork).toMatch(/<skill-base-dir>\/agents\/impeccino-documenter\.md/);
    expect(newWork).toMatch(/pass its full Markdown body.*general-purpose subagent/i);
    expect(newWork).toMatch(/role file's `tools` frontmatter as child tool limits/i);
    expect(newWork).toMatch(/complete its full five-section review locally before acting on its disposition/i);
    expect(newWork).toMatch(/read the same role file and \[document\.md\]\(document\.md\) in full before writing/i);
    expect(newWork).toMatch(/Perform the documentation pass locally.*then produce the role's full output contract/i);
    expect(newWork).toMatch(/label it as a local pass, outside the role's contracted return/i);
    expect(newWork).toMatch(/keep this disclosure outside the reviewer's contracted return/i);
  });
});
