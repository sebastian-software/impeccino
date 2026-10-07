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

    expect(newWork).toMatch(/host-permitted role routing/i);
    expect(newWork).toMatch(/Review is\s+read-only/i);
    expect(newWork).toMatch(/local review is not independent/i);
    expect(newWork).toMatch(/write boundary comes from\s+the task's existing authorization/i);

  });
});


describe('cross-reference contracts', () => {
  const skillDir = path.join(process.cwd(), 'skill');
  const read = (name) => fs.readFileSync(path.join(skillDir, name), 'utf-8');

  test('verification follows outcome and host budget rather than a fixed ceiling', () => {
    const overdrive = read('reference/overdrive.md');
    const skill = read('SKILL.md');
    expect(overdrive).toMatch(/host controls the inspection\s+budget/i);
    expect(skill).toMatch(/Stop when the requested outcome and checks pass/i);
    expect([skill, overdrive].join('\n')).not.toMatch(/at most one more round|two rounds is the ceiling|mandatory interview/i);
  });

  test('review and documentation distinguish style warnings from observed defects', () => {
    const craftFloor = read('reference/craft-floor.md');
    const reviewer = read('agents/impeccino-finish-reviewer.md');
    const documenter = read('agents/impeccino-documenter.md');

    expect(craftFloor).toMatch(/Style warnings are context-dependent/i);
    expect(reviewer).toMatch(/A style warning alone is not a material fix/i);
    expect(documenter).toMatch(/A style advisory alone is no reason to omit a\s+reusable rule/i);
    for (const text of [craftFloor, reviewer, documenter]) {
      expect(text).not.toMatch(/non-waivable ban|explicitly calls a ban|no brief earns it back/i);
    }
  });

  test('standalone seed and post-build documentation have separate timing', () => {
    const document = read('reference/document.md');
    const newWork = read('reference/new-work.md');
    const documenter = read('agents/impeccino-documenter.md');
    expect(document).toMatch(/document --seed.*provisional direction before implementation/i);
    expect(document).toMatch(/post-build documenter uses scan mode after implementation/is);
    expect(newWork).toMatch(/document.md.*after implementation/is);
    expect(documenter).toMatch(/Use scan mode here, not the standalone document --seed path/is);
  });

  test('the documenter uses approved handoff authority and reports gaps to its caller', () => {
    const documenter = read('agents/impeccino-documenter.md');

    expect(documenter).toMatch(/no user-facing channel/i);
    expect(documenter).toMatch(/report the missing\s+authority to the caller/i);
    expect(documenter).toMatch(/preserve incumbent decisions outside\s+that authorized scope and carry every existing waiver forward/i);
    expect(documenter).not.toMatch(/ask the user to clarify/i);
  });

  test('creative selection is optional and keys describe rolls that ran', () => {
    const newWork = read('reference/new-work.md');
    const reviewer = read('agents/impeccino-finish-reviewer.md');
    expect(newWork).toMatch(/optional local helper/i);
    expect(newWork).toMatch(/printed seed key.*only when a concept roll ran/is);
    expect(reviewer).toMatch(/When concept-seed ran, require the printed seed key/i);
    expect(reviewer).toMatch(/no-roll task\s+requires no key/i);
    expect(reviewer).toMatch(/Missing key alone never proves a roll was skipped/i);
  });

  test('critique grounds audience in product evidence instead of fictional personas', () => {
    const critique = read('reference/critique.md');
    expect(critique).toMatch(/PRODUCT.md.*Users.*ground audience/is);
    expect(critique).toMatch(/fictional personas\s+and numerical heuristic scores are not evidence/i);
    expect(critique).not.toMatch(/Auto-select.*personas/i);
  });

  test('source and rendered URL detector passes stay distinct', () => {
    const critique = read('reference/critique.md');
    expect(critique).toContain('detect --json <local source paths>');
    expect(critique).toContain('detect --viewport <W>x<H> <url>');
    expect(critique).toMatch(/source file is not a rendered-page target/i);
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


describe('skill text contracts cleaned up in issue 33', () => {
  const skillDir = path.join(process.cwd(), 'skill');
  const read = (name) => fs.readFileSync(path.join(skillDir, name), 'utf-8');

  test('argument hint preserves the command table category groups', () => {
    const parsed = utils.parseFrontmatter(read('SKILL.md'));
    const body = parsed.body;
    const categories = new Set(['Build', 'Evaluate', 'Refine', 'Enhance', 'Fix']);
    const tableGroups = [];
    for (const line of body.split(/\r?\n/)) {
      const columns = line.split('|').map((column) => column.trim());
      if (!categories.has(columns[2])) continue;
      const command = columns[1].replaceAll(String.fromCharCode(96), '').trim().split(/\s+/)[0];
      const previous = tableGroups.at(-1);
      if (!previous || previous.category !== columns[2]) {
        tableGroups.push({ category: columns[2], commands: [command] });
      } else {
        previous.commands.push(command);
      }
    }
    const hint = parsed.frontmatter['argument-hint'];
    const hintedGroups = hint.slice(1, hint.indexOf(']')).split(' · ').map((group) => group.split('|'));
    expect(hintedGroups).toEqual(tableGroups.map((group) => group.commands));

    const metadata = JSON.parse(fs.readFileSync(path.join(skillDir, 'scripts/command-metadata.json'), 'utf-8'));
    expect(metadata.init.description).toMatch(/does not create DESIGN[.]md/i);
    expect(metadata.init.description).not.toMatch(/offers DESIGN[.]md/i);
    expect(metadata.shape.description).toMatch(/one focused discovery round/i);
    expect(metadata.shape.description).not.toMatch(/multi-round|visual probes/i);
    expect(metadata.craft.description).toMatch(/deprecated compatibility alias/i);
  });

  test('skill source carries no unused rule markers or named host question APIs', () => {
    for (const file of utils.readFilesRecursive(skillDir)) {
      const text = fs.readFileSync(file, 'utf-8');
      expect(text).not.toContain('<!-- rule:');
      expect(text).not.toMatch(/AskUserQuestion|request_user_input/);
    }
  });

  test('document sidecar instructions contain only detector-consumed metadata', () => {
    const document = read('reference/document.md');
    const sidecar = document.split('### Step 4b: Write the DESIGN.json sidecar (detector metadata only)')[1]
      .split('### Step 5: Confirm and refine')[0];
    for (const field of ['extensions.colorMeta.<token>.canonical', 'extensions.colorMeta.<token>.tonalRamp',
      'extensions.roundedMeta.<token>', 'extensions.shadows[].value']) {
      expect(sidecar).toContain(field);
    }
    expect(sidecar).not.toMatch(/shadow DOM|5-10 components|narrative mapping/i);
    expect(document).not.toMatch(/button, input, and nav primitives/i);
    expect(sidecar).toMatch(/do not generate component snippets, narrative, motion, breakpoints/i);
  });

  test('hooks distinguish installed manifests from manually configured Grok', () => {
    const hooks = read('reference/hooks.md');
    expect(hooks).toMatch(/hooks on.{0,2} installs manifests for Claude Code.*Codex.*Cursor.*GitHub Copilot/i);
    expect(hooks).toMatch(/Grok Build can run the hook from a manually configured/i);
    expect(hooks).toMatch(/hooks on.*hooks off.*hooks reset.*hooks status.*do not write, manage, or report that manual manifest/i);
  });

  test('audit command menus include extract and follow their section order', () => {
    for (const file of ['reference/audit.md', 'reference/audit.native.md', 'reference/critique.md']) {
      expect(read(file)).toContain('/impeccino extract');
    }
    const audit = read('reference/audit.md');
    expect(audit.indexOf('### 3. Theming')).toBeLessThan(audit.indexOf('### 4. Responsive Design'));
    for (const heading of ['Audit Health', 'Implementation Integrity', 'Findings and Actions', 'Coverage']) {
      expect(audit).toContain(`### ${heading}`);
      expect(read('reference/audit.native.md')).toContain(`### ${heading}`);
    }
  });

  test('knowledge selection and source evidence stay task-scoped', () => {
    const operate = read('reference/operate.md');
    const typeset = read('reference/typeset.md');
    expect(operate).toMatch(/no mandatory font count, timing, palette, or measure/i);
    expect(typeset).toMatch(/fixed measure alone is\s+not a defect/i);
    const newWork = read('reference/new-work.md');
    const polish = read('reference/polish.md');
    expect(newWork).toMatch(/hooks have not already supplied.*source evidence/is);
    expect(polish).toMatch(/Reuse hooks.*source evidence/is);
    expect(polish).toMatch(/Rendered URL checks provide separate layout evidence/i);
  });
});
