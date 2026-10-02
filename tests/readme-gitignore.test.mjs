import { describe, it, afterAll } from 'vitest';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

describe('README gitignore block', () => {
  let tmp;

  afterAll(() => {
    if (tmp) rmSync(tmp, { recursive: true, force: true });
  });

  it('ignores ephemeral review and questions dirs while keeping shared artifacts tracked', () => {
    const readme = readFileSync(join(ROOT, 'README.md'), 'utf-8').replace(/\r\n?/g, '\n');
    const match = readme.match(/```gitignore\n([\s\S]*?)```/);
    assert.ok(match, 'README.md should contain a fenced gitignore block');
    const block = match[1];
    assert.match(block, /# impeccino-ignore-start/);

    tmp = mkdtempSync(join(tmpdir(), 'impeccino-readme-gitignore-'));
    writeFileSync(join(tmp, '.gitignore'), block);
    execFileSync('git', ['init'], { cwd: tmp });

    for (const rel of [
      '.impeccino/review/desktop.png',
      '.impeccino/questions/fb63f8a6.log',
      'apps/web/.impeccino/review/desktop.png',
      'apps/web/.impeccino/questions/fb63f8a6.log',
    ]) {
      const ignored = execFileSync('git', ['check-ignore', rel], {
        cwd: tmp,
        encoding: 'utf-8',
      });
      assert.equal(ignored.trim(), rel, `${rel} should be ignored independently`);
    }

    for (const rel of [
      '.impeccino/config.json',
      '.impeccino/critique/report.md',
      'apps/web/.impeccino/config.json',
      'apps/web/.impeccino/critique/report.md',
    ]) {
      const result = spawnSync('git', ['check-ignore', rel], { cwd: tmp });
      assert.notEqual(result.status, 0, `${rel} should not be ignored`);
    }
  });
});
