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
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { ENGINE_MISSING_MESSAGE, findEngineBinary } from './lib/engine-bin.mjs';
import { allCases, assertRecordableCases, caseRunsHere, diffResults, expectedForPlatform, GOLDEN_DIR, normalize, normalizeProjectCacheDir, normalizeSnapshotFiles, serializeOracleStdin } from './oracle/lib.mjs';
import { claudeEdit, hookPath } from './oracle/cases/hooks.mjs';

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

it('keeps multi-project cache snapshots distinct and rejects normalized path collisions', () => {
  const files = {
    '.oracle-home/.cache/impeccino/projects/workspace-11111111/hook.cache.json': '{"sessions":{"workspace":{}}}',
    '.oracle-home/.cache/impeccino/projects/apps-a-22222222/hook.cache.json': '{"sessions":{"app-a":{}}}',
    '.oracle-home/.cache/impeccino/projects/apps-b-33333333/hook.cache.json': '{"sessions":{"app-b":{}}}',
  };
  const labels = new Map([
    ['11111111', 'workspace'],
    ['22222222', 'appA'],
    ['33333333', 'appB'],
  ]);

  assert.deepEqual(Object.keys(normalizeSnapshotFiles(files, labels, 'multi-project')),
    [
      '.oracle-home/.cache/impeccino/projects/<PROJECT:workspace>/hook.cache.json',
      '.oracle-home/.cache/impeccino/projects/<PROJECT:appA>/hook.cache.json',
      '.oracle-home/.cache/impeccino/projects/<PROJECT:appB>/hook.cache.json',
    ]);
  assert.throws(() => normalizeSnapshotFiles(files), /normalized file snapshot collision in oracle case/);
  assert.equal(
    normalizeProjectCacheDir(String.raw`.oracle-home\AppData\Local\impeccino\projects\workspace-11111111\hook.cache.json`, labels),
    String.raw`.oracle-home\AppData\Local\impeccino\projects\<PROJECT:workspace>\hook.cache.json`,
  );
  assert.deepEqual(expectedForPlatform({}, {
    files: {
      '.oracle-home/AppData/Local/impeccino/projects/<PROJECT:workspace>/hook.cache.json': '{"sessions":{}}',
      '.oracle-home/AppData/Local/impeccino/projects/<PROJECT:appA>/hook.cache.json': '{"sessions":{"a":{}}}',
    },
  }, 'win32'), {
    files: {
      '.oracle-home/AppData/Local/impeccino/projects/<PROJECT:workspace>/hook.cache.json': '{"sessions":{}}',
      '.oracle-home/AppData/Local/impeccino/projects/<PROJECT:appA>/hook.cache.json': '{"sessions":{"a":{}}}',
    },
  });
});

it('substitutes placeholders in object stdin before JSON serialization', () => {
  const paths = {
    ws: String.raw`D:\a\impeccino project\stage`,
    repo: String.raw`D:\a\impeccino`,
  };
  const input = {
    cwd: '<WS>',
    '<WS>-key': 'placeholder in a property name',
    workspace_roots: ['<WS>'],
    tool_input: {
      file_path: '<WS>/src/new.css',
      content: String.raw`keep\literal`,
      source: '<REPO>/tests/fixture.css',
    },
  };

  assert.deepEqual(JSON.parse(serializeOracleStdin(input, paths)), {
    cwd: paths.ws,
    [`${paths.ws}-key`]: 'placeholder in a property name',
    workspace_roots: [paths.ws],
    tool_input: {
      file_path: `${paths.ws}/src/new.css`,
      content: String.raw`keep\literal`,
      source: `${paths.repo}/tests/fixture.css`,
    },
  });
  assert.equal(serializeOracleStdin('{not json', paths), '{not json');
});

it('builds Windows-native hook event paths while preserving content strings', () => {
  const event = claudeEdit('src/new.css', {
    tool_response: {
      type: 'create',
      filePath: hookPath('win32', '<WS>', 'src/new.css'),
      content: String.raw`keep\literal and C:\assets\logo.svg`,
    },
  }, 'win32');
  const decoded = JSON.parse(serializeOracleStdin(event, {
    ws: String.raw`D:\a\hook-project`,
    repo: String.raw`D:\a\impeccino`,
  }));

  assert.equal(decoded.tool_input.file_path, String.raw`D:\a\hook-project\src\new.css`);
  assert.equal(decoded.tool_response.filePath, String.raw`D:\a\hook-project\src\new.css`);
  assert.equal(decoded.tool_response.content, String.raw`keep\literal and C:\assets\logo.svg`);
});

it('normalizes only recognized Windows oracle path fields and hidden-path tokens', () => {
  const sample = [
    String.raw`{"productPath":"apps\\a\\PRODUCT.md","designPath":"apps\\a\\DESIGN.md","surfaceBriefPath":".impeccino\\surfaces\\x.md","path":".impeccino\\surfaces\\x.md","file":".impeccino\\critique\\<STAMP>__x.md","message":"keep\\literal"}`,
    String.raw`{"path":".impeccino\\hook.cache.json"}`,
    String.raw`{"path":"..\\dashboard\\PRODUCT.md"}`,
    String.raw`{"path":"docs\\notes\\keep.txt"}`,
    String.raw`human paths: .impeccino\config.json, .impeccino\hook.cache.json, .claude\settings.local.json, .cursor\hooks.json.bak.`,
    String.raw`literal text: keep\literal and .impeccino\not-a-path\keep`,
    'code: `.impeccino\\config.json`',
    'quoted text: ".impeccino\\config.json"',
    '',
  ].join('\n');
  const actual = normalize(sample, {
    ws: String.raw`D:\a\impeccino`,
    platform: 'win32',
  });

  assert.equal(actual, [
    String.raw`{"productPath":"apps/a/PRODUCT.md","designPath":"apps/a/DESIGN.md","surfaceBriefPath":".impeccino/surfaces/x.md","path":".impeccino/surfaces/x.md","file":".impeccino/critique/<STAMP>__x.md","message":"keep\\literal"}`,
    String.raw`{"path":".impeccino/hook.cache.json"}`,
    String.raw`{"path":"../dashboard/PRODUCT.md"}`,
    String.raw`{"path":"docs\\notes\\keep.txt"}`,
    'human paths: .impeccino/config.json, .impeccino/hook.cache.json, .claude/settings.local.json, .cursor/hooks.json.bak.',
    String.raw`literal text: keep\literal and .impeccino\not-a-path\keep`,
    'code: `.impeccino\\config.json`',
    'quoted text: ".impeccino\\config.json"',
    '',
  ].join('\n'));
});

it('normalizes only the known Windows persisted-surface doctor summary path', () => {
  const sharedSummary = '1 persisted surface brief(s) name a primary target that no longer exists: .impeccino\\surfaces\\src-old-astro.md → src/old.astro.';
  const unrelatedSummary = String.raw`keep\literal and .impeccino\not-a-surface\file.txt`;
  const input = { summary: sharedSummary, unrelatedSummary };
  const normalized = normalize(JSON.stringify(input, null, 2), { platform: 'win32' });

  assert.deepEqual(JSON.parse(normalized), {
    summary: '1 persisted surface brief(s) name a primary target that no longer exists: .impeccino/surfaces/src-old-astro.md → src/old.astro.',
    unrelatedSummary,
  });
});

it('masks only the quoted engine binary path inside JSON-escaped output strings', () => {
  const binaryPath = String.raw`D:\a\runner/impeccino/target/release/impeccino.exe`;
  const quotedCommandArg = binaryPath.replaceAll('\\', '\\\\');
  const payload = JSON.stringify({
    user_message: `Run "${quotedCommandArg}" hooks ignore-value <rule>`,
    unrelated: `Run "${quotedCommandArg}" status unchanged`,
    wrong_subcommand: `Run "${quotedCommandArg}" hooks-extra unchanged`,
    other_command: String.raw`Run "D:\a\other\tool.exe" hooks unchanged`,
  });
  const actual = normalize(payload, { platform: 'win32', binaryPath });
  const expected = JSON.stringify({
    user_message: 'Run <HOOK_ADMIN_CMD> ignore-value <rule>',
    unrelated: `Run "${quotedCommandArg}" status unchanged`,
    wrong_subcommand: `Run "${quotedCommandArg}" hooks-extra unchanged`,
    other_command: String.raw`Run "D:\a\other\tool.exe" hooks unchanged`,
  });

  assert.equal(actual, expected);

  const slashBinaryPath = 'D:/a/runner/impeccino/target/release/impeccino.exe';
  const slashCommand = JSON.stringify({
    user_message: `Run "${slashBinaryPath}" hooks ignore-value <rule>`,
    wrong_subcommand: `Run "${slashBinaryPath}" hooks-extra unchanged`,
  });
  assert.equal(normalize(slashCommand, { platform: 'win32', binaryPath: slashBinaryPath }), JSON.stringify({
    user_message: 'Run <HOOK_ADMIN_CMD> ignore-value <rule>',
    wrong_subcommand: 'Run "<IMPECCINO>" hooks-extra unchanged',
  }));
});

it('normalizes only declared Windows surface-path output lines', () => {
  const slash = normalize(String.raw`..\..\..\..\..\..\SURFACES.md
`, {
    platform: 'win32',
    caseId: 'surface-brief-path-slash',
    pathOutput: true,
  });
  const outside = normalize(String.raw`..\elsewhere\SURFACES.md
`, {
    platform: 'win32',
    caseId: 'surface-brief-path-outside',
    pathOutput: true,
  });

  assert.equal(normalize('SURFACES.md\n', { platform: 'win32', caseId: 'surface-brief-path-file', pathOutput: true }), 'SURFACES.md\n');
  assert.equal(slash, '<UP_TO_ROOT>/SURFACES.md\n');
  assert.equal(outside, '../elsewhere/SURFACES.md\n');
  assert.equal(normalize(String.raw`keep\literal`, { platform: 'win32' }), String.raw`keep\literal`);
  assert.throws(
    () => normalize('not a path line\nsecond line\n', { platform: 'win32', caseId: 'surface-brief-path-file', pathOutput: true }),
    /one surface path line/,
  );
});

it('expects the native Windows user cache without changing shared goldens', () => {
  const shared = {
    steps: [{ stdout: 'cache file: <WS>/.oracle-home/.cache/impeccino/projects/<PROJECT>/hook.cache.json\n', stderr: '', exit: 0 }],
    files: {
      '.oracle-home/.cache/impeccino/projects/<PROJECT>/hook.cache.json': '{"sessions":{}}',
      'docs/.cache/impeccino/example.txt': 'keep literal cache prose',
    },
  };
  const original = structuredClone(shared);
  assert.deepEqual(expectedForPlatform({}, shared, 'win32'), {
    steps: [{ stdout: 'cache file: <WS>/.oracle-home/AppData/Local/impeccino/projects/<PROJECT>/hook.cache.json\n', stderr: '', exit: 0 }],
    files: {
      '.oracle-home/AppData/Local/impeccino/projects/<PROJECT>/hook.cache.json': '{"sessions":{}}',
      'docs/.cache/impeccino/example.txt': 'keep literal cache prose',
    },
  });
  assert.deepEqual(shared, original);
  assert.deepEqual(expectedForPlatform({}, shared, 'linux'), original);
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

it('quotes the Windows drive-colon target_path without changing the shared golden', () => {
  const item = { id: 'critique-write-monorepo-child', windowsDrivePathQuoteField: 'target_path' };
  const shared = { stdout: 'target_path: <WS>/apps/a/src/App.tsx\n' };
  const expectedWindows = expectedForPlatform(item, shared, 'win32');

  assert.deepEqual(expectedForPlatform(item, shared, 'linux'), shared);
  assert.deepEqual(expectedWindows, { stdout: 'target_path: "<WS>/apps/a/src/App.tsx"\n' });
  assert.equal(diffResults(expectedWindows, { stdout: 'target_path: "<WS>/apps/a/src/App.tsx"\n' }).length, 0);
  assert.ok(diffResults(expectedWindows, shared).length > 0);
});

it('prevents Windows runs from overwriting shared goldens with platform-specific guidance', () => {
  const guidance = { id: 'detect-framework-vite-text', windowsPowerShellGuidance: true };
  const driveQuote = { id: 'critique-write-monorepo-child', windowsDrivePathQuoteField: 'target_path' };
  const quotedIgnore = { id: 'hook-config-per-edit-all', windowsQuotedIgnoreValue: { rule: 'bounce-easing', value: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)' } };

  assert.doesNotThrow(() => assertRecordableCases([guidance, driveQuote], 'darwin'));
  assert.doesNotThrow(() => assertRecordableCases([{ id: 'detect-help' }], 'win32'));
  assert.throws(
    () => assertRecordableCases([guidance, driveQuote, quotedIgnore], 'win32'),
    /Cannot record shared oracle goldens on Windows.*detect-framework-vite-text, critique-write-monorepo-child, hook-config-per-edit-all.*Record these cases on Linux or macOS/,
  );
});

it('limits platform-specific oracle skips to exact case-folding and POSIX-only probes', async () => {
  const cases = await allCases();
  const platformSpecific = cases
    .filter((item) => Array.isArray(item.platforms))
    .map(({ id, platforms, platformSkipReason }) => ({ id, platforms, platformSkipReason }));
  platformSpecific.sort((a, b) => a.id.localeCompare(b.id));

  assert.deepEqual(platformSpecific, [{
    id: 'context-lowercase-product-name',
    platforms: ['darwin', 'win32'],
    platformSkipReason: 'This case tests case-insensitive PRODUCT.md discovery, which Linux filesystems do not provide.',
  }, {
    id: 'detect-unreadable-file-in-dir',
    platforms: ['linux', 'darwin'],
    platformSkipReason: 'This case uses POSIX chmod(0) to deny reads, which Windows does not enforce.',
  }, {
    id: 'detect-unreadable-file-json',
    platforms: ['linux', 'darwin'],
    platformSkipReason: 'This case uses POSIX chmod(0) to deny reads, which Windows does not enforce.',
  }, {
    id: 'surface-brief-write-route',
    platforms: ['linux', 'darwin'],
    platformSkipReason: 'On Windows, the slash-only target resolves to the drive root; this fixture must not write outside its staged workspace.',
  }]);
  for (const item of platformSpecific) {
    assert.equal(caseRunsHere(item, 'win32'), item.platforms.includes('win32'), `${item.id} Windows contract`);
    assert.equal(caseRunsHere(item, 'linux'), item.platforms.includes('linux'), `${item.id} Linux contract`);
    assert.equal(caseRunsHere(item, 'darwin'), item.platforms.includes('darwin'), `${item.id} macOS contract`);
  }
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

it('limits Windows path-output contracts to exact single-path cases and steps', async () => {
  const cases = await allCases();
  const pathOutputCases = cases.filter((item) => item.windowsPathOutput).map((item) => item.id).sort();
  const pathOutputSteps = cases.flatMap((item) => (item.steps || [])
    .map((step, index) => step.windowsPathOutput ? `${item.id}[${index}]` : null)
    .filter(Boolean)).sort();
  const driveQuoteCases = cases.filter((item) => item.windowsDrivePathQuoteField).map((item) => item.id).sort();
  const quotedIgnoreCases = cases.filter((item) => item.windowsQuotedIgnoreValue).map((item) => item.id).sort();
  const listCase = cases.find((item) => item.id === 'surface-brief-write-read-list');

  assert.deepEqual(pathOutputCases, [
    'surface-brief-path-file',
    'surface-brief-path-from-subdir',
    'surface-brief-path-outside',
    'surface-brief-path-route',
    'surface-brief-path-slash',
    'surface-brief-path-url',
    'surface-brief-write-monorepo-child',
    'surface-brief-write-url',
  ]);
  assert.deepEqual(pathOutputSteps, ['surface-brief-write-read-list[0]', 'surface-brief-write-read-list[3]']);
  assert.deepEqual(driveQuoteCases, []);
  assert.deepEqual(quotedIgnoreCases, []);
  assert.ok(listCase.files.includes('SURFACES.md'));
  assert.equal(normalize('Root route brief.', { platform: 'win32' }), 'Root route brief.');
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

it('expects Windows Claude exec-form only for declared generated hook manifests', () => {
  const command = 'if [ -x "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" ]; then "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" hook; fi';
  const entry = { type: 'command', command, timeout: 10, statusMessage: 'checking' };
  const settings = { permissions: { allow: ['Bash(ls)'] }, hooks: { PostToolUse: [{ hooks: [entry, { type: 'command', command: 'echo other' }] }], Stop: [{ hooks: [entry] }] } };
  const shared = { files: { '.claude/settings.local.json': JSON.stringify(settings, null, 2) + '\n', 'other.json': 'keep' } };
  const original = structuredClone(shared);
  const item = { id: 'hadmin-on', windowsClaudeExecForm: true };
  const expected = expectedForPlatform(item, shared, 'win32');
  const windows = JSON.parse(expected.files['.claude/settings.local.json']);
  const generated = windows.hooks.PostToolUse[0].hooks[0];
  assert.equal(generated.command, 'powershell.exe');
  assert.deepEqual(Object.keys(generated), ['type', 'command', 'args', 'timeout', 'statusMessage']);
  assert.deepEqual(generated.args, ['-NoProfile', '-Command', "if (Test-Path -LiteralPath '<WS>/.claude/skills/impeccino/scripts/impeccino.cmd' -PathType Leaf) { & '<WS>/.claude/skills/impeccino/scripts/impeccino.cmd' hook }"]);
  assert.deepEqual(windows.hooks.Stop[0].hooks[0], generated);
  assert.deepEqual(windows.permissions, settings.permissions);
  assert.equal(windows.hooks.PostToolUse[0].hooks[1].command, 'echo other');
  assert.equal(expected.files['other.json'], 'keep');
  assert.deepEqual(shared, original);
  assert.deepEqual(expectedForPlatform(item, shared, 'linux'), original);
  assert.deepEqual(expectedForPlatform({}, shared, 'win32'), original);
  assert.deepEqual(expectedForPlatform(item, { files: {} }, 'win32'), { files: {} });
  assert.throws(() => expectedForPlatform(item, { files: { '.claude/settings.local.json': '{}' } }, 'win32'), /Expected generated Claude/);
  assert.throws(() => assertRecordableCases([item], 'win32'), /Cannot record shared oracle goldens.*hadmin-on/);
});
