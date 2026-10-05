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


describe('cross-reference contracts', () => {
  const skillDir = path.join(process.cwd(), 'skill');
  const read = (name) => fs.readFileSync(path.join(skillDir, name), 'utf-8');

  test('all commands follow the shared bounded inspection cycle', () => {
    const overdrive = read('reference/overdrive.md');
    expect(overdrive).toMatch(/bounded finish cycle/i);
    expect(overdrive).toMatch(/at most one more round/i);
    expect(overdrive).not.toMatch(/Expect multiple rounds of refinement/i);
  });

  test('review and documentation honor brief-earned craft-floor defaults', () => {
    const craftFloor = read('reference/craft-floor.md');
    const reviewer = read('agents/impeccino-finish-reviewer.md');
    const documenter = read('agents/impeccino-documenter.md');

    expect(craftFloor).toMatch(/defaults, not bans: the brief's own words can earn any/i);
    expect(reviewer).toMatch(/keep a default the brief or world explicitly earns/i);
    expect(reviewer).toMatch(/explicitly calls a ban/i);
    expect(documenter).toMatch(/preserve one when the approved brief or world deliberately chooses it/i);
    expect(documenter).toMatch(/explicitly labels a ban/i);
  });

  test('standalone document seed and post-build documentation have separate timing', () => {
    const document = read('reference/document.md');
    const newWork = read('reference/new-work.md');
    const documenter = read('agents/impeccino-documenter.md');

    expect(document).toContain('This is the explicit standalone');
    expect(document).toContain('document --seed');
    expect(document).toContain('workflow.');
    expect(newWork).toContain('post-build handoff is separate from the standalone');
    expect(newWork).toContain('document --seed');
    expect(newWork).toMatch(/DESIGN\.md is written at finish from the built world/i);
    expect(documenter).toContain('standalone questions, overwrite confirmation');
    expect(documenter).toContain('do not run them or ask the user');
  });

  test('the documenter uses approved handoff authority and reports gaps to its caller', () => {
    const documenter = read('agents/impeccino-documenter.md');

    expect(documenter).toMatch(/no user-facing channel/i);
    expect(documenter).toMatch(/report the missing authority to the caller/i);
    expect(documenter).toMatch(/preserve incumbent decisions outside that approved scope and carry every existing waiver forward/i);
    expect(documenter).not.toMatch(/ask the user to clarify/i);
  });

  test('concept-seed keys are required only for a roll that ran', () => {
    const newWork = read('reference/new-work.md');
    const reviewer = read('agents/impeccino-finish-reviewer.md');

    expect(newWork).toMatch(/concept-seed.*ran, include the printed seed key/i);
    expect(newWork).toMatch(/No roll: <reason>/);
    expect(reviewer).toMatch(/When concept-seed ran, require the printed seed key/i);
    expect(reviewer).toMatch(/when FORM records an allowed no-roll reason.*no key is required/i);
    expect(reviewer).toMatch(/Missing key alone never proves a roll was skipped/i);
  });

  test('critique personas use PRODUCT.md audience truth', () => {
    const critique = read('reference/critique.md');

    expect(critique).toMatch(/PRODUCT\.md.*Users.*section/i);
    expect(critique).toMatch(/PRODUCT\.md.*concrete information under.*Users/i);
    expect(critique).not.toMatch(/Design Context.*impeccino init/i);
  });

  test('source and rendered URL detector passes stay distinct', () => {
    const critique = read('reference/critique.md');

    expect(critique).toMatch(/source pass scans local files and directories, not URLs/i);
    expect(critique).toMatch(/rendered-page detector remains a separate required web pass on the URL/i);
  });

  test('native projects skip web-only detector instructions', () => {
    for (const file of ['reference/critique.md', 'reference/layout.md', 'reference/typeset.md']) {
      expect(read(file)).toMatch(/(?:native[\s\S]*skip[^.]*detect|skip[^.]*web-only detector)/i);
    }
    for (const file of ['reference/layout.md', 'reference/typeset.md']) {
      expect(read(file)).toMatch(/Mechanical scan \(web only\)/i);
      expect(read(file)).toMatch(/On web targets, rerun the scan; on native targets, recheck the device captures/i);
    }
  });
});
