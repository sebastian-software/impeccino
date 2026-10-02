/**
 * Guard tests for scripts/release.mjs, the tagging/publishing script for the
 * independently versioned components (skill, cli, engine). It owns every
 * refusal that protects a public release: dirty tree, unpushed HEAD, existing
 * tag, missing version, and engine pins that disagree.
 *
 * The script resolves repoRoot from its own file location and runs top-level
 * code on import, so these tests copy it into a disposable git repo (with a
 * local bare `origin`) and spawn it exactly as a maintainer would. Every run
 * uses --dry-run, which skips all mutating steps (tag, push, gh release,
 * builds) but exercises every guard on the way there.
 */
import { describe, it, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

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
      // The D4 engine release-order guard would otherwise probe the network for
      // published engine assets; these guards predate it and only exercise the
      // version and tag checks, so take its documented escape hatch.
      env: { ...process.env, IMPECCINO_SKIP_ENGINE_CHECK: '1' },
    });
    return { code: 0, stdout, stderr: '' };
  } catch (err) {
    return { code: err.status ?? 1, stdout: err.stdout ?? '', stderr: err.stderr ?? '' };
  }
}

describe('release.mjs guards', () => {
  let root;
  let workDir;
  let bareDir;
  let baselineSha;

  const write = (rel, contents) => {
    const abs = path.join(workDir, rel);
    fs.mkdirSync(path.dirname(abs), { recursive: true });
    fs.writeFileSync(abs, contents);
  };

  before(() => {
    root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-release-'));
    bareDir = path.join(root, 'origin.git');
    workDir = path.join(root, 'work');
    execFileSync('git', ['init', '--bare', bareDir]);
    fs.mkdirSync(workDir);
    git(workDir, 'init', '-b', 'main');
    git(workDir, 'config', 'user.email', 'test@example.com');
    git(workDir, 'config', 'user.name', 'Release Test');
    // A maintainer's global signing setup (1Password, gpg) would block on a
    // prompt for every fixture commit and tag; this repo is disposable.
    git(workDir, 'config', 'commit.gpgsign', 'false');
    git(workDir, 'config', 'tag.gpgsign', 'false');

    fs.mkdirSync(path.join(workDir, 'scripts'));
    fs.copyFileSync(RELEASE_SCRIPT, path.join(workDir, 'scripts', 'release.mjs'));
    // release.mjs imports ./check-engine-release.mjs and ./fetch-engine.mjs
    // (and check-engine-release.mjs imports fetch-engine.mjs), so stage them
    // too or the dry runs fail to resolve the modules instead of exercising
    // the guard.
    for (const dep of ['check-engine-release.mjs', 'fetch-engine.mjs', 'pin-engine.mjs']) {
      fs.copyFileSync(path.join(REPO_ROOT, 'scripts', dep), path.join(workDir, 'scripts', dep));
    }
    write('skill/SKILL.md', '---\nname: impeccino\ndescription: Design.\nmetadata:\n  version: 1.2.3\n---\n\nBody.\n');
    write('skill/scripts/VERSION', '0.1.0\n');
    const pins = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'windows-x64.exe']
      .map((a) => `${'a'.repeat(64)}  engine-v0.1.0/impeccino-${a}`).join('\n');
    write('skill/scripts/engine.sha256', `# fixture\n${pins}\n`);

    git(workDir, 'add', '-A');
    git(workDir, 'commit', '-m', 'fixture');
    git(workDir, 'remote', 'add', 'origin', bareDir);
    git(workDir, 'push', '-u', 'origin', 'main');
    baselineSha = git(workDir, 'rev-parse', 'HEAD');
  });

  after(() => {
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
    assert.match(stdout, /\[dry-run\] gh release create skill-v1\.2\.3/);
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

});
