import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'vitest';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const WINDOWS = process.platform === 'win32';
const PLATFORM = WINDOWS ? 'windows' : process.platform === 'darwin' ? 'darwin' : 'linux';
const ARCH = process.arch === 'arm64' ? 'arm64' : 'x64';
const COMSPEC = process.env.ComSpec || process.env.COMSPEC || 'C:\\Windows\\System32\\cmd.exe';
// Use the host's command interpreter as a harmless Windows executable. No
// downloaded release binary is executed, and all network traffic is loopback.
const PAYLOAD = WINDOWS ? fs.readFileSync(COMSPEC) : Buffer.from('#!/bin/sh\nif [ \"$1\" = engine-probe ]; then printf \'impeccino-engine 0.0.0-test\\n\'; else printf \'verified-engine\\n\'; fi\n');
const HASH = createHash('sha256').update(PAYLOAD).digest('hex');

async function exercise(t, scenario) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-launcher-'));
  t.onTestFinished(() => fs.rmSync(root, { recursive: true, force: true }));
  const scripts = path.join(root, 'skill scripts');
  const home = path.join(root, 'home');
  const cache = path.join(root, 'cache');
  fs.mkdirSync(scripts);
  fs.mkdirSync(home);
  fs.writeFileSync(path.join(scripts, 'VERSION'), '0.0.0-test\n');
  if (scenario !== 'no-pin') {
    // The skill's engine.sha256: one line per release asset. A pin for
    // another version must not apply to this one.
    const version = scenario === 'pinned-other-version' ? '9.9.9' : '0.0.0-test';
    const digest = scenario === 'mismatch' ? '0'.repeat(64) : HASH;
    const assets = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'windows-x64.exe', 'windows-arm64.exe'];
    fs.writeFileSync(path.join(scripts, 'engine.sha256'),
      `# engine-v${version}\n${assets.map(a => `${digest}  engine-v${version}/impeccino-${a}`).join('\n')}\n`);
  }
  const name = WINDOWS ? 'impeccino.cmd' : 'impeccino';
  const launcher = path.join(scripts, name);
  fs.copyFileSync(path.join(ROOT, 'skill/scripts', name), launcher);
  // Without IMPECCINO_HOME the launcher caches under the per-user cache:
  // $XDG_CACHE_HOME/impeccino, or %LOCALAPPDATA%\impeccino on Windows.
  const defaultCache = scenario === 'default-cache';
  const cacheDir = defaultCache
    ? path.join(cache, 'impeccino', 'bin', '0.0.0-test')
    : path.join(cache, 'bin', '0.0.0-test');
  const missingEnvBin = path.join(home, 'missing-engine');
  const overrideBin = path.join(home, WINDOWS ? 'override-engine.cmd' : 'override-engine');
  const daloScenarios = scenario.startsWith('dalo-');
  const project = path.join(root, 'project');
  const nested = path.join(project, 'nested');
  const customStore = path.join(root, 'custom store');
  let cwd = root;
  let daloStore = path.join(home, '.dalo');
  if (daloScenarios) {
    if (['dalo-custom', 'dalo-relative', 'dalo-explicit-over-project'].includes(scenario)) daloStore = customStore;
    if (['dalo-project', 'dalo-explicit-over-project', 'dalo-git-boundary'].includes(scenario)) {
      fs.mkdirSync(nested, { recursive: true });
      fs.writeFileSync(path.join(project, 'dalo-project.toml'), 'schema_version = 2\n');
      fs.mkdirSync(path.join(project, '.git'));
      cwd = nested;
      if (scenario === 'dalo-project') daloStore = path.join(project, '.dalo');
      if (scenario === 'dalo-git-boundary') {
        fs.writeFileSync(path.join(nested, '.git'), 'gitdir: unrelated worktree\n');
        // An ancestor's unrelated project store must not be consulted.
        fs.mkdirSync(path.join(project, '.dalo', 'bin'), { recursive: true });
        fs.writeFileSync(path.join(project, '.dalo', 'bin', 'impeccino'), PAYLOAD, { mode: 0o755 });
      }
    }
    fs.mkdirSync(path.join(daloStore, 'bin'), { recursive: true });
    if (scenario !== 'dalo-missing') {
      const payload = scenario === 'dalo-wrong-version'
        ? Buffer.from('#!/bin/sh\nif [ "$1" = engine-probe ]; then printf "impeccino-engine 9.9.9\\n"; else printf "wrong-engine\\n"; fi\n')
        : ['dalo-tampered', 'dalo-stale'].includes(scenario)
        ? Buffer.from(`#!/bin/sh\nprintf 'executed' > '${path.join(root, 'untrusted-executed')}'\nprintf 'impeccino-engine ${scenario === 'dalo-stale' ? '9.9.9' : '0.0.0-test'}\\n'\n`)
        : PAYLOAD;
      fs.writeFileSync(path.join(daloStore, 'bin', 'impeccino'), payload, { mode: 0o555 });
      if (scenario === 'dalo-wrong-version') {
        const pins = path.join(scripts, 'engine.sha256');
        fs.writeFileSync(pins, fs.readFileSync(pins, 'utf8').replaceAll(HASH, createHash('sha256').update(payload).digest('hex')));
      }
    }
    if (['dalo-priority', 'dalo-wrong-version'].includes(scenario)) {
      const sibling = path.join(scripts, 'bin', PLATFORM + '-' + ARCH, 'impeccino');
      fs.mkdirSync(path.dirname(sibling), { recursive: true });
      fs.writeFileSync(sibling, `#!/bin/sh\nif [ "$1" = engine-probe ]; then printf "impeccino-engine 0.0.0-test\\n"; else printf "${scenario === 'dalo-priority' ? 'sibling-engine' : 'verified-engine'}\\n"; fi\n`, { mode: 0o755 });
    }
    if (scenario === 'dalo-no-pin') fs.unlinkSync(path.join(scripts, 'engine.sha256'));
  }
  if (['sibling-marker-only', 'sibling-probe-failure', 'sibling-extra-output', 'cache-marker-only'].includes(scenario)) {
    const sibling = scenario === 'cache-marker-only'
      ? path.join(cacheDir, 'impeccino')
      : path.join(scripts, 'bin', PLATFORM + '-' + ARCH, 'impeccino');
    fs.mkdirSync(path.dirname(sibling), { recursive: true });
    fs.writeFileSync(sibling,
      scenario === 'sibling-probe-failure'
        ? '#!/bin/sh\nif [ "$1" = engine-probe ]; then printf \'impeccino-engine 0.0.0-test\\n\'; exit 1; else printf \'untrusted-sibling\\n\'; fi\n'
        : scenario === 'sibling-extra-output'
          ? '#!/bin/sh\nif [ "$1" = engine-probe ]; then printf \'impeccino-engine 0.0.0-test\\n\\n\'; else printf \'untrusted-sibling\\n\'; fi\n'
          : '#!/bin/sh\nif [ "$1" = engine-probe ]; then printf \'impeccino-engine\\n\'; else printf \'untrusted-sibling\\n\'; fi\n',
      { mode: 0o755 },
    );
  }
  if (scenario === 'expired-cooldown') {
    fs.mkdirSync(cacheDir, { recursive: true });
    fs.writeFileSync(path.join(cacheDir, '.impeccino-download-failed'), `${Math.floor(Date.now() / 1000) - 1}\n`);
  }
  if (scenario === 'cache-directory-failure') {
    // A file where the cache parent belongs makes mkdir fail on every OS,
    // including privileged test runners where chmod cannot deny writes.
    fs.mkdirSync(cache);
    fs.writeFileSync(path.join(cache, 'bin'), 'blocked');
  }
  if (scenario === 'cache-write-failure' || scenario === 'cache-readonly-file') {
    fs.mkdirSync(cacheDir, { recursive: true });
  }
  const tools = path.join(root, 'tools');
  fs.mkdirSync(tools);
  if (!WINDOWS && scenario === 'cache-write-failure') {
    // The POSIX staging name contains the launcher's PID, so intercept the
    // preceding mkdir to place a directory at precisely that file path.
    fs.writeFileSync(path.join(tools, 'mkdir'), '#!/bin/sh\n/bin/mkdir "$@" || exit $?\n/bin/mkdir "$IMPECCINO_HOME/bin/0.0.0-test/.impeccino.part.$PPID"\n', { mode: 0o755 });
  }
  if (WINDOWS && ['cache-write-failure', 'cache-readonly-file'].includes(scenario)) {
    // Windows cannot reliably deny writes when Vitest runs elevated. Pin the
    // source copy at the exact staging boundary and leave the error path real.
    const source = fs.readFileSync(launcher, 'utf8');
    const operation = '(type nul >"%stage%") 2>nul || goto cache_write_failed';
    assert.equal(source.split(operation).length, 2, 'instrument exactly one staging operation');
    fs.writeFileSync(launcher, source.replace(operation, 'goto cache_write_failed'));
  }
  if (!WINDOWS && ['hash-failure', 'removed-during-hash'].includes(scenario)) {
    fs.writeFileSync(path.join(tools, 'shasum'),
      `#!/bin/sh\n${scenario === 'removed-during-hash' ? 'rm -f "$3"\n' : ''}printf '%s  %s\\n' '${HASH}' "$3"\nexit ${scenario === 'hash-failure' ? 1 : 0}\n`,
      { mode: 0o755 });
  }
  const placementScenarios = ['removed-before-move', 'removed-after-move', 'emptied-after-move', 'move-failure'];
  if (!WINDOWS && placementScenarios.includes(scenario)) {
    const before = scenario === 'removed-before-move' ? 'rm -f "$2"\n' : '';
    const after = scenario === 'removed-after-move' ? 'rm -f "$3"\n' : scenario === 'emptied-after-move' ? ': > "$3"\n' : '';
    fs.writeFileSync(path.join(tools, 'mv'), scenario === 'move-failure' ? '#!/bin/sh\nexit 1\n' : `#!/bin/sh\n${before}/bin/mv "$@" || exit $?\n${after}`, { mode: 0o755 });
  }
  if (WINDOWS && (placementScenarios.includes(scenario) || ['hash-failure', 'removed-during-hash'].includes(scenario))) {
    // move is a cmd builtin and certutil is an .exe: PATH shims cannot
    // intercept them. Instrument ONLY the external-operation boundary in the
    // staged test copy, preserving the launcher's real labels/checks/status
    // handling. The separate valid case always runs the unmodified launcher.
    const fault = path.join(tools, 'fault.cmd');
    const hashFault = ['hash-failure', 'removed-during-hash'].includes(scenario);
    const operation = hashFault
      ? 'certutil -hashfile "%stage%" SHA256 >"%hash_tmp%" 2>nul'
      : 'move /y "%stage%" "%cached%" >nul 2>nul';
    let script;
    if (hashFault) {
      script = `@echo off\n${scenario === 'removed-during-hash' ? 'del "%stage%" >nul 2>nul\n' : ''}echo hash header\necho ${HASH}\nexit /b 1\n`;
    } else if (scenario === 'move-failure') {
      script = '@echo off\nexit /b 1\n';
    } else {
      const before = scenario === 'removed-before-move' ? 'del "%stage%" >nul 2>nul\n' : '';
      const after = scenario === 'removed-after-move' ? 'del "%cached%" >nul 2>nul\n' : scenario === 'emptied-after-move' ? 'type nul >"%cached%"\n' : '';
      script = `@echo off\n${before}${operation}\nif errorlevel 1 exit /b 1\n${after}exit /b 0\n`;
    }
    fs.writeFileSync(fault, script.replaceAll('\n', '\r\n'));
    const source = fs.readFileSync(launcher, 'utf8');
    assert.equal(source.split(operation).length, 2, 'instrument exactly one operation');
    const replacement = `call "${fault}"${hashFault ? ' >"%hash_tmp%" 2>nul' : ''}`;
    fs.writeFileSync(launcher, source.replace(operation, replacement));
  }
  const requests = [];
  const server = http.createServer((req, res) => {
    requests.push(req.url);
    if (scenario === 'transport-failure') {
      req.socket.destroy();
      return;
    }
    if (scenario === 'download-failure') res.writeHead(404);
    const respond = () => res.end(scenario === 'empty-download' ? '' : PAYLOAD);
    if (scenario === 'parallel-downloads') setTimeout(respond, 100);
    else respond();
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  t.onTestFinished(() => new Promise(resolve => server.close(resolve)));
  // Keep system tools, but exclude user/npm PATH candidates and all launcher
  // overrides so the test cannot accidentally execute an installed engine.
  const env = {
    PATH: WINDOWS ? `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}` : `${tools}:/usr/bin:/bin`,
    HOME: home, USERPROFILE: home, TEMP: root, TMP: root,
    ...(defaultCache ? (WINDOWS ? { LOCALAPPDATA: cache } : { XDG_CACHE_HOME: cache }) : { IMPECCINO_HOME: cache }),
    ...(scenario === 'missing-env-bin' ? { IMPECCINO_BIN: missingEnvBin } : {}),
    ...(scenario === 'override-other-version' ? { IMPECCINO_BIN: overrideBin } : {}),
    ...(['dalo-custom', 'dalo-explicit-over-project'].includes(scenario) ? { DALO_STORE: customStore } : {}),
    ...(scenario === 'dalo-relative' ? { DALO_STORE: 'custom store' } : {}),
    ...(scenario === 'dalo-override' ? { IMPECCINO_BIN: overrideBin } : {}),
    IMPECCINO_DOWNLOAD_BASE: `http://127.0.0.1:${server.address().port}`,
    ...(WINDOWS ? { SystemRoot: process.env.SystemRoot, ComSpec: COMSPEC, PROCESSOR_ARCHITECTURE: 'AMD64' } : {}),
  };
  const run = () => new Promise((resolve, reject) => {
    const child = WINDOWS
      ? spawn(COMSPEC, ['/d', '/s', '/c', `""${launcher}" /d /c echo verified-engine"`], { env, cwd: root, windowsVerbatimArguments: true, timeout: 20000 })
      : spawn('/bin/sh', [launcher], { env, cwd, timeout: 20000 });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', data => { stdout += data; });
    child.stderr.on('data', data => { stderr += data; });
    child.on('error', reject);
    child.on('close', (status, signal) => resolve({ status, signal, stdout, stderr }));
  });
  if (['override-other-version', 'dalo-override'].includes(scenario)) {
    fs.writeFileSync(overrideBin, WINDOWS
      ? '@echo off\r\necho override-engine\r\n'
      : "#!/bin/sh\n" + "printf 'override-engine\\n'\n",
      WINDOWS ? undefined : { mode: 0o755 },
    );
  }
  const results = scenario === 'parallel-downloads' ? await Promise.all([run(), run()]) : [await run()];
  const result = results[0];
  if ((scenario === 'valid' || defaultCache) && !WINDOWS) {
    const requestCount = requests.length;
    const cachedResult = await run();
    assert.equal(cachedResult.status, 0, cachedResult.stderr);
    assert.match(cachedResult.stdout, /verified-engine/);
    assert.equal(cachedResult.stderr, '', 'cached execution stays quiet');
    assert.equal(requests.length, requestCount, 'subsequent runs use the pinned cache without network');
  }
  assert.equal(result.signal, null, JSON.stringify(result));
  for (const item of results) assert.equal(item.signal, null, JSON.stringify(item));
  const noDownload = scenario.startsWith('cache-') && scenario !== 'cache-marker-only' || ['no-pin', 'pinned-other-version', 'missing-env-bin', 'override-other-version', 'dalo-default', 'dalo-custom', 'dalo-relative', 'dalo-project', 'dalo-explicit-over-project', 'dalo-git-boundary', 'dalo-priority', 'dalo-override', 'dalo-no-pin', 'dalo-wrong-version'].includes(scenario);
  const expectedRequests = scenario === 'parallel-downloads' ? 2 : noDownload ? 0 : 1;
  assert.equal(requests.length, expectedRequests,
    'cache failures, unusable overrides, and unpinned versions do not attempt a download; parallel downloads use separate staging files');
  assert.equal(fs.existsSync(path.join(root, 'untrusted-executed')), false, 'a mismatched Dalo candidate is never executed, even for the probe');
  let cooldownResult;
  if (['download-failure', 'transport-failure'].includes(scenario)) {
    cooldownResult = await run();
    assert.equal(cooldownResult.status, 127, cooldownResult.stderr);
    assert.match(cooldownResult.stderr, /five-minute cooldown/);
    assert.equal(requests.length, 1, 'the retry cooldown suppresses another network request');
  }
  return { ...result, results, cooldownResult, files: fs.existsSync(cacheDir) ? fs.readdirSync(cacheDir) : [], requests, cacheDir };
}

for (const scenario of ['cache-directory-failure', 'cache-write-failure', 'download-failure', 'transport-failure', ...(WINDOWS ? ['cache-readonly-file'] : [])]) {
  test(`launcher explains ${scenario} and how to retry setup`, async t => {
    const result = await exercise(t, scenario);
    assert.equal(result.status, 127, JSON.stringify(result));
    assert.doesNotMatch(result.stdout, /verified-engine/);
    assert.match(result.stderr, /engine 0\.0\.0-test/);
    assert.ok(result.stderr.includes(result.cacheDir), result.stderr);
    assert.match(result.stderr, /engine-probe/);
    assert.match(result.stderr, /IMPECCINO_HOME/);
    assert.match(result.stderr, /IMPECCINO_BIN/);
    if (['download-failure', 'transport-failure'].includes(scenario)) {
      assert.match(result.stderr, /could not download/);
      assert.match(result.stderr, /network/);
      assert.deepEqual(result.files, ['.impeccino-download-failed'], 'failed downloads leave only the cooldown marker');
      assert.equal(result.requests.length, 1, 'no verification without a download');
      assert.match(result.cooldownResult.stderr, /five-minute cooldown/);
    } else {
      assert.match(result.stderr, scenario === 'cache-directory-failure' ? /cannot create/ : /cannot write/);
    }
  });
}

for (const scenario of ['valid', 'default-cache']) {
  test(`launcher downloads and runs a verified executable (${scenario})`, async t => {
    const result = await exercise(t, scenario);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /verified-engine/);
    assert.deepEqual(result.files, [WINDOWS ? 'impeccino.exe' : 'impeccino'], `cached under ${result.cacheDir}`);
    assert.equal(result.requests.length, 1, 'the pinned digest needs no checksum download');
  });
}

test('launcher refuses an unusable IMPECCINO_BIN instead of falling through', async t => {
  const result = await exercise(t, 'missing-env-bin');
  assert.equal(result.status, 127, JSON.stringify(result));
  assert.match(result.stderr, /IMPECCINO_BIN.*(missing|not found|usable|executable)/i);
  assert.deepEqual(result.requests, [], 'an explicit but unusable override must not download another engine');
});

test('launcher honors an explicit IMPECCINO_BIN even when it has another version', async t => {
  const result = await exercise(t, 'override-other-version');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /override-engine/);
  assert.deepEqual(result.requests, [], 'an explicit developer override is not version-pinned or replaced');
});

test('launcher recovers after an expired failure cooldown', async t => {
  const result = await exercise(t, 'expired-cooldown');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /verified-engine/);
  assert.equal(result.requests.length, 1, 'an expired marker permits one new fetch');
  assert.deepEqual(result.files, [WINDOWS ? 'impeccino.exe' : 'impeccino']);
});

if (!WINDOWS) {
  for (const scenario of ['dalo-default', 'dalo-custom', 'dalo-relative', 'dalo-project', 'dalo-explicit-over-project', 'dalo-git-boundary', 'dalo-priority']) {
    test(`launcher uses the pinned Dalo engine without downloading (${scenario})`, async t => {
      const result = await exercise(t, scenario);
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /verified-engine/);
      assert.deepEqual(result.requests, []);
      assert.deepEqual(result.files, [], 'Dalo execution does not copy into the launcher cache');
    });
  }
  for (const scenario of ['dalo-tampered', 'dalo-stale', 'dalo-missing']) {
    test(`launcher keeps its fallback for an unusable Dalo engine (${scenario})`, async t => {
      const result = await exercise(t, scenario);
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /verified-engine/);
      assert.equal(result.requests.length, 1);
    });
  }
  test('explicit engine override takes precedence over Dalo', async t => {
    const result = await exercise(t, 'dalo-override');
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /override-engine/);
    assert.deepEqual(result.requests, []);
  });
  test('launcher refuses to execute a Dalo engine without its skill digest', async t => {
    const result = await exercise(t, 'dalo-no-pin');
    assert.equal(result.status, 127, result.stderr);
    assert.deepEqual(result.requests, []);
  });
  test('launcher requires the exact version even when the Dalo digest matches', async t => {
    const result = await exercise(t, 'dalo-wrong-version');
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /verified-engine/);
    assert.doesNotMatch(result.stdout, /wrong-engine/);
    assert.deepEqual(result.requests, []);
  });
  test('parallel launcher downloads use distinct temporary files', async t => {
    const result = await exercise(t, 'parallel-downloads');
    assert.equal(result.results.length, 2);
    for (const run of result.results) {
      assert.equal(run.status, 0, run.stderr);
      assert.match(run.stdout, /verified-engine/);
    }
    assert.equal(result.requests.length, 2);
    assert.deepEqual(result.files, ['impeccino']);
  });

  for (const scenario of ['sibling-marker-only', 'sibling-probe-failure', 'sibling-extra-output', 'cache-marker-only']) {
    test(`launcher rejects an unpinned sibling (${scenario}) instead of running it as the pinned engine`, async t => {
      const result = await exercise(t, scenario);
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /verified-engine/);
      assert.doesNotMatch(result.stdout, /untrusted-sibling/);
      assert.equal(result.requests.length, 1, 'a sibling without a successful exact-version probe must fall through to download');
    });
  }
}

for (const scenario of ['no-pin', 'pinned-other-version']) {
  test(`launcher refuses to download an engine version the skill does not pin (${scenario})`, async t => {
    const result = await exercise(t, scenario);
    assert.equal(result.status, 127, JSON.stringify(result));
    assert.doesNotMatch(result.stdout, /verified-engine/);
    assert.match(result.stderr, /no digest pinned for engine-v0\.0\.0-test/);
    assert.match(result.stderr, /IMPECCINO_BIN/);
    assert.deepEqual(result.files, []);
  });
}

for (const scenario of ['empty-download', 'mismatch', 'hash-failure', 'removed-during-hash', 'removed-before-move', 'removed-after-move', 'emptied-after-move', 'move-failure']) {
  test(`launcher refuses ${scenario} with an accurate diagnostic`, async t => {
    const result = await exercise(t, scenario);
    assert.equal(result.status, 127, JSON.stringify(result));
    assert.doesNotMatch(result.stdout, /verified-engine/);
    assert.deepEqual(result.files, [], 'no unverified file left behind');
    if (scenario.startsWith('removed')) {
      assert.match(result.stderr, /download completed but the file was removed before (verification|execution)/);
      assert.match(result.stderr, /antivirus.*logs/i);
      assert.doesNotMatch(result.stderr, /checksum mismatch/);
    } else if (scenario.startsWith('emptied') || scenario === 'empty-download') {
      assert.match(result.stderr, /downloaded file is empty/);
      assert.doesNotMatch(result.stderr, /checksum mismatch/);
    } else if (scenario === 'mismatch') {
      assert.match(result.stderr, /checksum mismatch/);
      assert.match(result.stderr, /engine\.sha256/);
    } else if (scenario === 'move-failure') {
      assert.match(result.stderr, /could not cache the verified download/);
    } else {
      assert.match(result.stderr, /refusing the unverified download/);
    }
  });
}
