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

  test('ships the launcher, VERSION, page JS, and command metadata', () => {
    for (const expected of [
      'impeccable', 'impeccable.cmd', 'VERSION', 'command-metadata.json',
      'live-browser.js', 'live-browser-dom.js', 'live-browser-session.js', 'modern-screenshot.umd.js',
    ]) {
      expect(names.has(expected)).toBe(true);
    }
  });

  test('ships no engine entry points and no bundled detector', () => {
    // The engine verbs live in the binary; the only Node scripts allowed in
    // the payload are the comp-fidelity build pipeline and its libs, which
    // have not moved into the engine yet.
    const allowedNodeScripts = new Set([
      'build-phase.mjs',
      'comp-diff.mjs',
      'comp-spec.mjs',
      'font-match.mjs',
      'lib/font-fingerprint.mjs',
      'lib/font-index.mjs',
      'lib/hero-checks.mjs',
      'lib/image-metrics.mjs',
      'lib/png.mjs',
      'lib/raster.mjs',
    ]);
    const stray = [...names].filter((n) =>
      (n.endsWith('.mjs') || n.startsWith('detector/') || n.startsWith('lib/')) && !allowedNodeScripts.has(n));
    expect(stray).toEqual([]);
  });

  test('never reads platform binaries as source', () => {
    expect([...names].filter((n) => n.startsWith('bin/'))).toEqual([]);
  });

  test('the launcher is executable and VERSION matches ENGINE_VERSION', () => {
    const launcher = scripts.find((s) => s.name === 'impeccable');
    expect(launcher.mode & 0o111).not.toBe(0);
    const version = scripts.find((s) => s.name === 'VERSION');
    expect(version.content.trim()).toBe(fs.readFileSync(path.join(ROOT_DIR, 'ENGINE_VERSION'), 'utf-8').trim());
  });
});

describe('universal skill source', () => {
  const ROOT = process.cwd();
  const SKILL_DIR = path.join(ROOT, 'skill');
  const markdown = utils.readFilesRecursive(SKILL_DIR);

  test('carries a real SKILL.md with only Agent Skills spec frontmatter', () => {
    const { frontmatter } = utils.parseFrontmatter(fs.readFileSync(path.join(SKILL_DIR, 'SKILL.md'), 'utf-8'));
    // Every harness reads these, and Codex's skill validator accepts only these.
    const spec = new Set(['name', 'description', 'license', 'compatibility', 'metadata']);
    for (const key of Object.keys(frontmatter)) expect(spec.has(key)).toBe(true);
    expect(frontmatter.metadata.version).toBeTruthy();
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
