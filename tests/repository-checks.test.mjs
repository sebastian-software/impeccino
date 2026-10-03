import { describe, expect, test } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const skill = (description = 'Design guidance') => `---\nname: impeccino\ndescription: ${description}\n---\n\n| \`audit\` | Review |\n`;

function checkFixture({ readme = '# Project\n', mainSkill = skill(), reference = '' } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-repository-check-'));
  const write = (relative, content) => {
    const target = path.join(root, relative);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, content);
  };
  try {
    for (const relative of ['scripts/check.js', 'scripts/lib/utils.js']) {
      write(relative, fs.readFileSync(path.join(repoRoot, relative)));
    }
    write('package.json', '{"type":"module"}\n');
    write('README.md', readme);
    write('skill/SKILL.md', mainSkill);
    write('skill/reference/nested/guide.md', reference);
    write('crates/foundation/src/registry.rs', 'pub static ANTIPATTERNS = &[\n  id: "sample",\n];\n');
    return spawnSync(process.execPath, [path.join(root, 'scripts/check.js')], {
      cwd: root,
      encoding: 'utf8',
      timeout: 10_000,
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

describe('repository check entrypoint', () => {
  test('keeps the skill policy narrower than the README policy', () => {
    const valid = checkFixture({ reference: 'Use robust error handling.\n' });
    expect(valid.status, valid.stderr).toBe(0);

    const invalid = checkFixture({ readme: 'A robust product.\n', reference: 'Run npx impeccino.\n' });
    expect(invalid.status).toBe(1);
    expect(invalid.stderr).toContain('README.md:1: "robust"');
    expect(invalid.stderr).toMatch(/guide\.md:1: "npx impeccino"/);
  });

  test('checks nested Markdown and resets repeated punctuation checks', () => {
    const result = checkFixture({ reference: 'First — claim.\nSecond &mdash; claim.\nThird -- claim.\n' });
    expect(result.status).toBe(1);
    expect(result.stderr).toMatch(/guide\.md:1: em dash/);
    expect(result.stderr).toMatch(/guide\.md:2: em dash/);
    expect(result.stderr).toMatch(/guide\.md:3: ` -- ` em-dash substitute/);
  });

  test('continues enforcing counts and frontmatter limits', () => {
    const result = checkFixture({ readme: '3 commands and 61 deterministic rules.\n', mainSkill: skill('x'.repeat(1025)) });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('active');
    expect(result.stderr).toContain('detection count is 1');
    expect(result.stderr).toContain('exceeds maximum length of 1024');
  });

  test('requires the single current skill instead of the retired multi-skill fallback', () => {
    const result = checkFixture({ mainSkill: '---\nname: other\ndescription: Other\n---\n' });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('skill/SKILL.md must define the impeccino skill');
  });
});
