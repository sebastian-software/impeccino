/**
 * Behavior gate: replays the oracle corpus (tests/oracle) against the engine
 * binary and asserts every runnable case matches its golden exactly.
 *
 * Skips cleanly when no binary is available: set IMPECCINO_BIN or run
 * `pnpm run fetch:engine` (which writes skill/scripts/bin/<os>-<arch>/).
 *
 * Run with: pnpm exec vitest run tests/oracle.test.mjs
 * Scope:    IMPECCINO_ORACLE_PREFIX=detect- pnpm exec vitest run tests/oracle.test.mjs
 */
import { describe, it } from 'vitest';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ENGINE_MISSING_MESSAGE, findEngineBinary } from './lib/engine-bin.mjs';
import { allCases, assertRecordableCases, diffResults, expectedForPlatform, normalize } from './oracle/lib.mjs';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const ENGINE_BIN = findEngineBinary();

it('normalizes Windows path separators and line endings before comparing goldens', () => {
  const sample = String.raw`root: C:\runner\repo\skill\scripts
json: {"root":"C:\\runner\\repo\\skill\\scripts"}
json path with spaces: {"root":"C:\\runner\\repo\\skill docs\\scripts"}
mixed: C:\runner\repo\skill\scripts keep\literal "quoted\thing"
literal: keep\this\text
`.replaceAll('\n', '\r\n');
  const actual = normalize(sample, {
    ws: String.raw`C:\runner\repo`,
    home: String.raw`C:\Users\runner`,
  });

  assert.equal(actual, 'root: <WS>/skill/scripts\njson: {"root":"<WS>/skill/scripts"}\n' +
    'json path with spaces: {"root":"<WS>/skill docs/scripts"}\n' +
    'mixed: <WS>/skill/scripts keep\\literal "quoted\\thing"\nliteral: keep\\this\\text\n');
});

it('keeps the PowerShell launcher note and quoted command in explicit Windows expectations', () => {
  const item = { id: 'detect-framework-vite-text', windowsPowerShellGuidance: true };
  const shared = { stderr: '  <IMPECCINO> detect http://localhost:5173\n\n' };

  assert.deepEqual(expectedForPlatform(item, shared, 'linux'), shared);
  assert.deepEqual(expectedForPlatform(item, shared, 'win32'), {
    stderr: '  <WINDOWS_QUOTED_IMPECCINO> detect http://localhost:5173\n' +
      'In PowerShell, prefix the quoted launcher path with `&`.\n\n',
  });
  const expectedWindows = expectedForPlatform(item, shared, 'win32');
  const misplaced = {
    stderr: expectedWindows.stderr.replace('\nIn PowerShell', ' In PowerShell'),
  };
  const duplicated = {
    stderr: `${expectedWindows.stderr}In PowerShell, prefix the quoted launcher path with \`&\`.\n`,
  };
  assert.ok(diffResults(expectedWindows, misplaced).length > 0);
  assert.ok(diffResults(expectedWindows, duplicated).length > 0);
});

it('prevents Windows runs from overwriting shared goldens with platform-specific guidance', () => {
  const item = { id: 'detect-framework-vite-text', windowsPowerShellGuidance: true };

  assert.doesNotThrow(() => assertRecordableCases([item], 'darwin'));
  assert.doesNotThrow(() => assertRecordableCases([{ id: 'detect-help' }], 'win32'));
  assert.throws(
    () => assertRecordableCases([item], 'win32'),
    /Cannot record shared oracle goldens on Windows.*Record these cases on Linux or macOS/,
  );
});

it('limits platform-specific oracle skips to the documented case-folding probe', async () => {
  const cases = await allCases();
  const platformSpecific = cases
    .filter((item) => Array.isArray(item.platforms))
    .map(({ id, platforms, platformSkipReason }) => ({ id, platforms, platformSkipReason }));

  assert.deepEqual(platformSpecific, [{
    id: 'context-lowercase-product-name',
    platforms: ['darwin', 'win32'],
    platformSkipReason: 'This case tests case-insensitive PRODUCT.md discovery, which Linux filesystems do not provide.',
  }]);
});

it('limits explicit PowerShell golden differences to framework scan guidance', async () => {
  const cases = await allCases();
  const guidanceCases = cases.filter((item) => item.windowsPowerShellGuidance).map((item) => item.id).sort();

  assert.deepEqual(guidanceCases, [
    'detect-fixture-text-framework-next-cssinjs',
    'detect-fixture-text-framework-next-modules',
    'detect-fixture-text-framework-next-tailwind',
    'detect-fixture-text-framework-vite',
    'detect-framework-next-modules-text',
  ]);
});

describe.skipIf(!ENGINE_BIN)('oracle corpus against the engine binary', () => {
  it('replays every recorded case with zero differences', () => {
    const args = [path.join(REPO_ROOT, 'tests', 'oracle', 'run.mjs')];
    if (process.env.IMPECCINO_ORACLE_PREFIX) args.push(process.env.IMPECCINO_ORACLE_PREFIX);
    const result = spawnSync(process.execPath, args, {
      cwd: REPO_ROOT,
      encoding: 'utf-8',
      env: { ...process.env, IMPECCINO_BIN: ENGINE_BIN },
      maxBuffer: 64 * 1024 * 1024,
    });
    const summary = (result.stdout || '').trim().split('\n').pop();
    const failures = (result.stdout || '').split('\n').filter((line) => line.startsWith('XX ') || line.startsWith('?? '));
    assert.equal(
      result.status,
      0,
      `oracle run exited ${result.status}: ${summary}\n${failures.slice(0, 40).join('\n')}\n${(result.stderr || '').slice(-4000)}`,
    );
  });
});
