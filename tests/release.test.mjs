/**
 * Guard tests for scripts/release.mjs, the tagging/publishing script for the
 * independently versioned components (skill and engine). It owns every
 * refusal that protects a public release: dirty tree, unpushed HEAD, existing
 * tag, missing version, and engine pins that disagree.
 *
 * The script resolves repoRoot from its own file location and runs top-level
 * code on import, so these tests copy it into a disposable git repo (with a
 * local bare `origin`) and spawn it exactly as a maintainer would. Every run
 * uses --dry-run, which skips all mutating steps (tag, push, gh release,
 * builds) but exercises every guard on the way there.
 */
import { describe, it, beforeAll, afterAll, beforeEach } from 'vitest';
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { checkEngineRelease } from '../scripts/check-engine-release.mjs';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const RELEASE_SCRIPT = path.join(REPO_ROOT, 'scripts', 'release.mjs');


function git(cwd, ...args) {
  return execFileSync('git', args, { cwd, encoding: 'utf-8' }).trim();
}

function runRelease(cwd, ...args) {
  try {
    const stdout = execFileSync(process.execPath, ['scripts/release.mjs', ...args, '--dry-run'], {
      cwd,
      encoding: 'utf-8',
      timeout: 60000,
      // The engine release-order guard would otherwise probe the network for
      // published engine assets; these tests only exercise the
      // version and tag checks, so take its documented escape hatch.
      env: { ...process.env, IMPECCINO_SKIP_ENGINE_CHECK: '1' },
    });
    return { code: 0, stdout, stderr: '' };
  } catch (err) {
    return { code: err.status ?? 1, stdout: err.stdout ?? '', stderr: err.stderr ?? '' };
  }
}

function runNode(args, { cwd, env = process.env, timeoutMs = 60000 }) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8').on('data', (chunk) => { stdout += chunk; });
    child.stderr.setEncoding('utf8').on('data', (chunk) => { stderr += chunk; });
    const timer = setTimeout(() => child.kill(), timeoutMs);
    child.on('error', reject);
    child.on('close', (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
  });
}

describe('release.mjs guards', () => {
  let root;
  let workDir;
  let bareDir;
  let upstreamDir;
  let baselineSha;

  const write = (rel, contents) => {
    const abs = path.join(workDir, rel);
    fs.mkdirSync(path.dirname(abs), { recursive: true });
    fs.writeFileSync(abs, contents);
  };

  beforeAll(() => {
    root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino release café-'));
    bareDir = path.join(root, 'origin.git');
    upstreamDir = path.join(root, 'upstream.git');
    workDir = path.join(root, 'work');
    execFileSync('git', ['init', '--bare', bareDir]);
    execFileSync('git', ['init', '--bare', upstreamDir]);
    fs.mkdirSync(workDir);
    git(workDir, 'init', '-b', 'main');
    git(workDir, 'config', 'user.email', 'test@example.com');
    git(workDir, 'config', 'user.name', 'Release Test');
    // A maintainer's global signing setup (1Password, gpg) would block on a
    // prompt for every fixture commit and tag; this repo is disposable.
    git(workDir, 'config', 'commit.gpgsign', 'false');
    git(workDir, 'config', 'tag.gpgsign', 'false');

    fs.mkdirSync(path.join(workDir, 'scripts', 'lib'), { recursive: true });
    fs.copyFileSync(RELEASE_SCRIPT, path.join(workDir, 'scripts', 'release.mjs'));
    // release.mjs imports ./check-engine-release.mjs and ./fetch-engine.mjs
    // (and check-engine-release.mjs imports fetch-engine.mjs), so stage them
    // too or the dry runs fail to resolve the modules instead of exercising
    // the guard.
    for (const dep of ['check-engine-release.mjs', 'fetch-engine.mjs', 'pin-engine.mjs']) {
      fs.copyFileSync(path.join(REPO_ROOT, 'scripts', dep), path.join(workDir, 'scripts', dep));
    }
    fs.copyFileSync(
      path.join(REPO_ROOT, 'scripts', 'lib', 'is-entrypoint.mjs'),
      path.join(workDir, 'scripts', 'lib', 'is-entrypoint.mjs'),
    );
    write('skill/SKILL.md', '---\nname: impeccino\ndescription: Design.\nmetadata:\n  version: 1.2.3\n---\n\nBody.\n');
    write('skill/scripts/VERSION', '0.1.0\n');
    write('Cargo.toml', '[workspace.package]\nversion = "0.1.0"\n');
    const pins = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'windows-x64.exe']
      .map((a) => `${'a'.repeat(64)}  engine-v0.1.0/impeccino-${a}`).join('\n');
    write('skill/scripts/engine.sha256', `# fixture\n${pins}\n`);

    git(workDir, 'add', '-A');
    git(workDir, 'commit', '-m', 'fixture');
    git(workDir, 'remote', 'add', 'origin', bareDir);
    git(workDir, 'remote', 'add', 'upstream', upstreamDir);
    git(workDir, 'push', '-u', 'origin', 'main');
    baselineSha = git(workDir, 'rev-parse', 'HEAD');
  });

  afterAll(() => {
    fs.rmSync(root, { recursive: true, force: true });
  });

  beforeEach(() => {
    // Undo whatever the previous scenario staged, on both ends: local tree
    // and tags back to the baseline commit, and origin force-reset too, since
    // several scenarios push commits or tags that would poison later ones.
    git(workDir, 'checkout', '--', '.');
    git(workDir, 'clean', '-fd');
    git(workDir, 'reset', '--hard', baselineSha);
    git(workDir, 'push', '--force', 'origin', 'main');
    for (const tag of git(workDir, 'tag').split('\n').filter(Boolean)) {
      git(workDir, 'tag', '-d', tag);
    }
    // --refs excludes the peeled `^{}` lines annotated tags produce, which
    // are not deletable refs and would abort the cleanup.
    for (const line of git(workDir, 'ls-remote', '--refs', '--tags', 'origin').split('\n').filter(Boolean)) {
      const ref = line.split('\t')[1];
      if (ref) git(workDir, 'push', 'origin', `:${ref}`);
    }
  });

  it('dry-runs a clean engine release: tags only, CI publishes', () => {
    const { code, stdout } = runRelease(workDir, 'engine');
    assert.equal(code, 0, stdout);
    assert.match(stdout, /Engine 0\.1\.0/);
    assert.match(stdout, /\[dry-run\] git tag -a engine-v0\.1\.0/);
    assert.match(stdout, /\[dry-run\] git push origin engine-v0\.1\.0/);
    assert.doesNotMatch(stdout, /gh release create/);
    assert.match(stdout, /release-engine workflow/);
  });

  it('allows matching engine prerelease versions', () => {
    write('skill/scripts/VERSION', '0.2.0-rc.1\n');
    write('Cargo.toml', '[workspace.package]\nversion = "0.2.0-rc.1"\n');
    git(workDir, 'add', 'Cargo.toml', 'skill/scripts/VERSION');
    git(workDir, 'commit', '-m', 'matching engine prerelease versions');
    git(workDir, 'push', 'origin', 'main');

    const { code, stdout } = runRelease(workDir, 'engine');
    assert.equal(code, 0, stdout);
    assert.match(stdout, /Engine 0\.2\.0-rc\.1/);
    assert.match(stdout, /\[dry-run\] git tag -a engine-v0\.2\.0-rc\.1/);
  });

  it('refuses an engine tag when Cargo.toml and skill/scripts/VERSION disagree', () => {
    write('Cargo.toml', '[workspace.package]\nversion = "0.2.0"\n');
    const { code, stderr } = runRelease(workDir, 'engine');
    assert.equal(code, 1);
    assert.match(stderr, /Engine version mismatch/);
  });

  it('engine: refuses when the tag already exists on origin', () => {
    git(workDir, 'tag', 'engine-v0.1.0');
    git(workDir, 'push', 'origin', 'engine-v0.1.0');
    git(workDir, 'tag', '-d', 'engine-v0.1.0');
    const { code, stderr } = runRelease(workDir, 'engine');
    assert.notEqual(code, 0);
    assert.match(stderr, /engine-v0\.1\.0 already exists on origin/);
  });

  it('dry-runs a clean skill release end to end', () => {
    const { code, stdout } = runRelease(workDir, 'skill');
    assert.equal(code, 0, stdout);
    assert.match(stdout, /Skill 1\.2\.3/);
    assert.match(stdout, /tag is free/);
    assert.match(stdout, /\[dry-run\] git tag -a skill-v1\.2\.3/);
    assert.match(stdout, /\[dry-run\] gh release create skill-v1\.2\.3 --repo sebastian-software\/impeccino --verify-tag/);
    assert.match(stdout, /gh release create skill-v1\.2\.3 [^\n]*--generate-notes/);
    assert.doesNotMatch(stdout, /universal\.zip|1Password|changelog/);
  });

  it('refuses a skill release whose engine version has no pins', () => {
    write('skill/scripts/engine.sha256', '# no pins\n');
    git(workDir, 'commit', '-am', 'drop pins');
    git(workDir, 'push', 'origin', 'main');
    const { code, stderr } = runRelease(workDir, 'skill');
    assert.equal(code, 1);
    assert.match(stderr, /engine\.sha256 has no pin for engine-v0\.1\.0/);
  });

  it('starts the release notes from the previous release on origin, not a local-only tag', () => {
    git(workDir, 'tag', 'skill-v1.0.0');
    git(workDir, 'push', 'origin', 'skill-v1.0.0');
    // A tag that only exists locally (an upstream remote's, say) is not a release here.
    git(workDir, 'tag', 'skill-v9.9.9');
    const { code, stdout } = runRelease(workDir, 'skill');
    assert.equal(code, 0, stdout);
    assert.match(stdout, /--notes-start-tag skill-v1\.0\.0/);
    assert.doesNotMatch(stdout, /skill-v9\.9\.9/);
  });

  it('refuses an unknown component', () => {
    const { code, stderr } = runRelease(workDir, 'website');
    assert.equal(code, 1);
    assert.match(stderr, /usage: release\.mjs/);
  });

  it('refuses a dirty working tree', () => {
    write('README.md', 'uncommitted');
    const { code, stderr } = runRelease(workDir, 'skill');
    assert.equal(code, 1);
    assert.match(stderr, /Working tree is dirty/);
  });

  it('refuses when HEAD is ahead of origin', () => {
    write('note.txt', 'ahead');
    git(workDir, 'add', '-A');
    git(workDir, 'commit', '-m', 'unpushed');
    const { code, stderr } = runRelease(workDir, 'skill');
    assert.equal(code, 1);
    assert.match(stderr, /Push your commits first/);
  });

  it('refuses when the tag already exists locally', () => {
    git(workDir, 'tag', 'skill-v1.2.3');
    const { code, stderr } = runRelease(workDir, 'skill');
    assert.equal(code, 1);
    assert.match(stderr, /already exists locally/);
  });

  it('refuses when the tag already exists on origin', () => {
    git(workDir, 'tag', 'skill-v1.2.3');
    git(workDir, 'push', 'origin', 'skill-v1.2.3');
    git(workDir, 'tag', '-d', 'skill-v1.2.3');
    const { code, stderr } = runRelease(workDir, 'skill');
    assert.equal(code, 1);
    assert.match(stderr, /already exists on origin/);
  });

  it('refuses a skill release when SKILL.md carries no metadata.version', () => {
    write('skill/SKILL.md', '---\nname: impeccino\ndescription: Design.\n---\n\nBody.\n');
    git(workDir, 'add', '-A');
    git(workDir, 'commit', '-m', 'no version');
    git(workDir, 'push', 'origin', 'main');
    const { code, stderr } = runRelease(workDir, 'skill');
    assert.equal(code, 1);
    assert.match(stderr, /No version field in skill\/SKILL\.md/);
  });

  it('retries transient release probes and treats only 404 as a missing asset', async () => {
    const calls = new Map();
    const result = await checkEngineRelease({
      version: '0.1.0',
      base: 'https://release.test',
      retries: 2,
      retryDelayMs: 0,
      fetchImpl: async (url) => {
        const count = (calls.get(url) || 0) + 1;
        calls.set(url, count);
        if (url.endsWith('impeccino-linux-x64')) {
          return count < 3
            ? { ok: false, status: 503, statusText: 'Service Unavailable' }
            : { ok: true, status: 206, statusText: 'Partial Content' };
        }
        return { ok: false, status: 404, statusText: 'Not Found' };
      },
    });

    assert.equal(result.ok, false);
    assert.deepEqual(result.missing.map((asset) => asset.target), [
      'darwin-arm64', 'darwin-x64', 'linux-arm64', 'windows-x64',
    ]);
    assert.deepEqual(result.unreachable, []);
    assert.equal(calls.get('https://release.test/engine-v0.1.0/impeccino-linux-x64'), 3);
  });

  it('bounds hung release probes and reports them as unverifiable', async () => {
    let aborted = 0;
    const result = await checkEngineRelease({
      version: '0.1.0',
      base: 'https://release.test',
      retries: 0,
      timeoutMs: 10,
      fetchImpl: async (_url, { signal }) => new Promise((_resolve, reject) => {
        signal.addEventListener('abort', () => {
          aborted++;
          reject(signal.reason);
        }, { once: true });
      }),
    });

    assert.equal(result.missing.length, 0);
    assert.equal(result.unreachable.length, 5);
    assert.equal(aborted, 5);
  });

  it('runs release helpers through spaced, Unicode, and symlinked paths', async () => {
    const scriptsDir = path.join(workDir, 'scripts');
    const noNetworkBase = 'http://127.0.0.1:1/releases/download';
    const cases = [
      {
        name: 'pin-engine.mjs', args: ['--check'],
        env: process.env,
        assertResult: ({ code, stdout }) => {
          assert.equal(code, 0);
          assert.match(stdout, /pins all 5 assets/);
        },
      },
      {
        name: 'check-engine-release.mjs', args: ['--json'],
        env: { ...process.env, IMPECCINO_DOWNLOAD_BASE: noNetworkBase },
        assertResult: ({ code, stdout }) => {
          assert.equal(code, 2);
          assert.equal(JSON.parse(stdout).unreachable.length, 5);
        },
      },
      {
        name: 'fetch-engine.mjs', args: ['--help'],
        env: process.env,
        assertResult: ({ code, stdout }) => {
          assert.equal(code, 0);
          assert.match(stdout, /Usage: node scripts\/fetch-engine\.mjs/);
        },
      },
    ];

    for (const testCase of cases) {
      const script = path.join(scriptsDir, testCase.name);
      const direct = await runNode([script, ...testCase.args], { cwd: workDir, env: testCase.env });
      testCase.assertResult(direct);
    }

    const linksDir = path.join(workDir, 'entry links');
    fs.mkdirSync(linksDir, { recursive: true });
    for (const testCase of cases) {
      const script = path.join(scriptsDir, testCase.name);
      const link = path.join(linksDir, testCase.name);
      try {
        fs.symlinkSync(script, link, 'file');
      } catch (err) {
        if (process.platform === 'win32') return;
        throw err;
      }
      const linked = await runNode([link, ...testCase.args], { cwd: workDir, env: testCase.env });
      testCase.assertResult(linked);
    }
  });

  it('keeps release helper imports side-effect free', async () => {
    for (const name of ['pin-engine.mjs', 'check-engine-release.mjs', 'fetch-engine.mjs']) {
      const url = pathToFileURL(path.join(workDir, 'scripts', name)).href;
      const result = await runNode(['--input-type=module', '-e', `await import(${JSON.stringify(url)});`], {
        cwd: workDir,
      });
      assert.equal(result.code, 0, result.stderr);
      assert.equal(result.stdout, '');
      assert.equal(result.stderr, '');
    }
  });

});
