import { afterEach, describe, expect, test, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { parse } from 'yaml';
import { createHash } from 'node:crypto';
import { checkProductTag, prepareSkill } from '../scripts/product-release.mjs';
import { openSkillPinPR, skillCandidate } from '../scripts/skill-pin-pr.mjs';
import { publishEngine, RELEASE_ASSETS } from '../scripts/publish-engine.mjs';
import { ENGINE_TARGETS, assetName } from '../scripts/fetch-engine.mjs';
import { checkEnginePins, writeEnginePins, pinEngine } from '../scripts/pin-engine.mjs';

const roots = [];
const temp = () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-product-'));
  roots.push(root);
  return root;
};
const write = (root, file, text) => {
  fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
  fs.writeFileSync(path.join(root, file), text);
};
afterEach(() => roots.splice(0).forEach(root => fs.rmSync(root, { recursive: true, force: true })));

function fixture() {
  const root = temp();
  write(root, '.release-please-version', '0.3.0\n');
  write(root, '.release-please-manifest.json', '{".":"0.3.0"}\n');
  write(root, 'Cargo.toml', '[workspace.package]\nversion = "0.3.0"\nedition = "2021"\n');
  write(root, 'skill/SKILL.md', '---\nname: impeccino\nmetadata:\n  version: 0.1.0\n---\n\nKeep this body.\n  version: body text\n');
  write(root, 'skill/scripts/VERSION', '0.2.0\n');
  write(root, 'skill/scripts/engine.sha256', '# old verified pins\n');
  writeEnginePins('0.2.0', new Map(ENGINE_TARGETS.map(target => [`engine-v0.2.0/${assetName(target)}`, 'a'.repeat(64)])), root);
  const git = (...args) => execFileSync('git', args, { cwd: root, stdio: 'pipe' });
  git('init');
  git('config', 'user.name', 'Release Test');
  git('config', 'user.email', 'test@example.com');
  git('config', 'commit.gpgsign', 'false');
  git('config', 'tag.gpgsign', 'false');
  git('add', '.');
  git('commit', '-m', 'fixture');
  git('tag', 'engine-v0.3.0');
  return root;
}

describe('shared product release', () => {
  test('writes both pin routes only after every asset verifies', async () => {
    const root = fixture();
    const verifyAsset = vi.fn();
    const downloadAsset = async url => Buffer.from(url.split('/').at(-1));
    await pinEngine('0.3.0', { root, sourceDigest: 'c'.repeat(40), downloadAsset, verifyAsset });
    expect(verifyAsset).toHaveBeenCalledTimes(5);
    for (const [, version, sourceDigest] of verifyAsset.mock.calls) {
      expect(version).toBe('0.3.0');
      expect(sourceDigest).toBe('c'.repeat(40));
    }
    const frontmatter = parse(fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8').split('---')[1]);
    const engine = frontmatter.binaries.impeccino;
    expect(Object.keys(engine.assets).sort()).toEqual(['linux-arm64', 'linux-x64', 'macos-arm64', 'macos-x64']);
    expect(engine.assets['macos-arm64'].sha256).toBe(createHash('sha256').update('impeccino-darwin-arm64').digest('hex'));
    expect(engine.availability).toBe('optional');
    expect(() => checkEnginePins('0.3.0', root)).not.toThrow();
  });

  test('a late attestation failure preserves all installed pins', async () => {
    const root = fixture();
    const files = ['skill/SKILL.md', 'skill/scripts/VERSION', 'skill/scripts/engine.sha256'];
    const before = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'));
    let count = 0;
    await expect(pinEngine('0.3.0', {
      root,
      downloadAsset: async () => Buffer.from('engine'),
      verifyAsset: () => { if (++count === 5) throw new Error('invalid provenance'); },
    })).rejects.toThrow('build attestation did not verify');
    expect(files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))).toEqual(before);
  });

  test.each(['tag', 'digest', 'asset', 'platform', 'unknown field'])('offline pin validation rejects declaration drift: %s', (change) => {
    const root = fixture();
    const file = path.join(root, 'skill/SKILL.md');
    const skill = fs.readFileSync(file, 'utf8');
    const replacements = {
      tag: ['tag: engine-v0.2.0', 'tag: engine-v9.9.9'],
      digest: ['a'.repeat(64), 'b'.repeat(64)],
      asset: ['asset: impeccino-darwin-arm64', 'asset: another-binary'],
      platform: ['macos-arm64:', 'windows-x64:'],
      'unknown field': ['availability: optional', 'availability: optional\n    unexpected: true'],
    };
    fs.writeFileSync(file, skill.replace(...replacements[change]));
    expect(() => checkEnginePins('0.2.0', root)).toThrow('disagrees');
  });

  test('rejects incomplete verified pins before writing the skill', () => {
    const root = fixture();
    const skill = fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8');
    expect(() => writeEnginePins('0.3.0', new Map(), root)).toThrow('Missing or invalid pin');
    expect(fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8')).toBe(skill);
  });

  test('validates the candidate before changing the installed engine pin', () => {
    const root = fixture();
    expect(checkProductTag('engine-v0.3.0', root)).toBe('0.3.0');
    expect(() => checkProductTag('engine-v0.2.0', root)).toThrow('Engine tag mismatch');
    write(root, 'Cargo.toml', '[workspace.package]\nversion = "0.4.0"\n');
    expect(() => checkProductTag('engine-v0.3.0', root)).toThrow('disagrees with Cargo.toml');
    write(root, '.release-please-manifest.json', '{".":"0.4.0"}\n');
    expect(() => checkProductTag('engine-v0.3.0', root)).toThrow('manifest disagrees');
  });

  test('advances metadata and VERSION only after all five pins verify against the tag commit', async () => {
    const root = fixture();
    const pin = vi.fn(async (version, options) => {
      expect(fs.readFileSync(path.join(root, 'skill/scripts/VERSION'), 'utf8')).toBe(pin.mock.calls.length === 1 ? '0.2.0\n' : '0.3.0\n');
      expect(options.sourceDigest).toMatch(/^[a-f0-9]{40}$/);
      writeEnginePins(version, new Map(ENGINE_TARGETS.map(target => [`engine-v${version}/${assetName(target)}`, 'a'.repeat(64)])), root);
    });
    await expect(prepareSkill('engine-v0.3.0', { root, pin })).resolves.toBe('0.3.0');
    expect(pin).toHaveBeenCalledOnce();
    expect(fs.readFileSync(path.join(root, 'skill/scripts/VERSION'), 'utf8')).toBe('0.3.0\n');
    const skill = fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8');
    expect(skill).toContain('  version: 0.3.0\n');
    expect(skill).toContain('tag: engine-v0.3.0');
    expect(skill).toContain('\nKeep this body.\n  version: body text\n');
    expect(() => checkEnginePins('0.3.0', root)).not.toThrow();
    // Recovery after the mechanical pin commit must preserve identical files.
    execFileSync('git', ['add', '.'], { cwd: root });
    execFileSync('git', ['commit', '-m', 'chore: pin engine'], { cwd: root });
    await expect(prepareSkill('engine-v0.3.0', { root, pin })).resolves.toBe('0.3.0');
    expect(fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8')).toBe(skill);
  });

  test.each(['provenance failure', 'incomplete pins', 'declaration drift'])('restores the installed skill after %s', async (failure) => {
    const root = fixture();
    const files = ['skill/SKILL.md', 'skill/scripts/VERSION', 'skill/scripts/engine.sha256'];
    const before = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'));
    const pin = async () => {
      write(root, 'skill/scripts/engine.sha256', '# partial\n');
      if (failure === 'provenance failure') throw new Error('untrusted artifact');
      if (failure === 'declaration drift') {
        write(root, 'skill/scripts/engine.sha256', ENGINE_TARGETS.map(target => `${'a'.repeat(64)}  engine-v0.3.0/${assetName(target)}`).join('\n'));
        write(root, 'skill/scripts/VERSION', '0.3.0\n');
      }
    };
    await expect(prepareSkill('engine-v0.3.0', { root, pin })).rejects.toThrow();
    expect(files.map(file => fs.readFileSync(path.join(root, file), 'utf8'))).toEqual(before);
  });

  test('refuses an outdated recovery before downloading or modifying the skill', async () => {
    const root = fixture();
    const pin = vi.fn();
    await expect(prepareSkill('engine-v0.2.0', { root, pin })).rejects.toThrow('Engine tag mismatch');
    expect(pin).not.toHaveBeenCalled();
  });

  test.each(['crates/cli/src/main.rs', 'skill/reference/audit.md', 'skill/SKILL.md'])('refuses source drift in %s before pinning', async (file) => {
    const root = fixture();
    const text = file === 'skill/SKILL.md' ? fs.readFileSync(path.join(root, file), 'utf8') + '\nNew command.\n' : 'changed source';
    write(root, file, text);
    execFileSync('git', ['add', '.'], { cwd: root });
    execFileSync('git', ['commit', '-m', 'feat: change sources'], { cwd: root });
    const pin = vi.fn();
    await expect(prepareSkill('engine-v0.3.0', { root, pin })).rejects.toThrow(/sources changed/);
    expect(pin).not.toHaveBeenCalled();
  });

  test.each(['repo: sebastian-software/impeccino', 'availability: optional', 'asset: impeccino-darwin-arm64'])('recovery refuses an authored binary contract change: %s', async value => {
    const root = fixture();
    const file = path.join(root, 'skill/SKILL.md');
    fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replace(value, `${value}-changed`));
    execFileSync('git', ['add', '.'], { cwd: root });
    execFileSync('git', ['commit', '-m', 'change binary contract'], { cwd: root });
    const pin = vi.fn();
    await expect(prepareSkill('engine-v0.3.0', { root, pin })).rejects.toThrow('Skill sources changed');
    expect(pin).not.toHaveBeenCalled();
  });

  test('Release Please updates every local crate in the virtual workspace lockfile', () => {
    const config = JSON.parse(fs.readFileSync(new URL('../release-please-config.json', import.meta.url)));
    const product = config.packages['.'];
    expect(config['release-type']).toBe('simple');
    expect(Object.keys(config.packages)).toEqual(['.']);
    expect(product.draft).toBe(true);
    expect(product['force-tag-creation']).toBe(true);
    expect(product['version-file']).toBe('.release-please-version');
    expect(`${product.component}${product['tag-separator']}v0.3.0`).toBe('engine-v0.3.0');
    const lock = fs.readFileSync(new URL('../Cargo.lock', import.meta.url), 'utf8');
    const names = [...lock.matchAll(/^name = "(impeccino(?:-[^"]+)?)"$/gm)].map(match => match[1]);
    const updates = product['extra-files'].filter(file => file.path === 'Cargo.lock');
    expect(updates.map(update => update.jsonpath)).toEqual(names.map(name => `$.package[?(@.name.value=="${name}")].version`));
    expect(product['extra-files'].some(file => file.path.startsWith('skill/'))).toBe(false);
  });

  test('dispatches the immutable engine tag and requires a token that starts PR checks', () => {
    const workflow = parse(fs.readFileSync(new URL('../.github/workflows/release-please.yml', import.meta.url), 'utf8'));
    const steps = workflow.jobs['release-please'].steps;
    expect(steps.find(step => step.id === 'release').with.token).toBe('${{ secrets.RELEASE_PLEASE_TOKEN }}');
    const dispatch = steps.find(step => step.name === 'Build the tagged engine release');
    expect(dispatch.run).toBe('gh workflow run release-engine.yml --ref "$RELEASE_TAG"');
    expect(dispatch.env.RELEASE_TAG).toBe('${{ steps.release.outputs.tag_name }}');
  });
});

describe('draft publication recovery', () => {
  const assets = () => {
    const root = temp();
    RELEASE_ASSETS.forEach(name => write(root, name, name));
    return root;
  };

  test('does not modify an already published immutable release', () => {
    const gh = vi.fn(() => JSON.stringify({ isDraft: false }));
    expect(publishEngine('engine-v0.3.0', temp(), { gh })).toBe('published');
    expect(gh).toHaveBeenCalledOnce();
  });

  test('publishes only after every expected binary and notice has been attached', () => {
    const directory = assets();
    const attached = [];
    const gh = vi.fn(args => {
      if (args[1] === 'view') return JSON.stringify({ isDraft: true, assets: attached });
      if (args[1] === 'upload') attached.push({ name: path.basename(args[3]) });
      if (args[1] === 'edit') expect(attached.map(asset => asset.name)).toEqual(RELEASE_ASSETS);
      return '';
    });
    publishEngine('engine-v0.3.0', directory, { gh });
    expect(gh.mock.calls.at(-1)[0]).toEqual(['release', 'edit', 'engine-v0.3.0', '--draft=false']);
  });

  test('rejects a changed draft asset and leaves the draft unpublished', () => {
    const directory = assets();
    const name = RELEASE_ASSETS[0];
    const gh = vi.fn(args => {
      if (args[1] === 'view') return JSON.stringify({ isDraft: true, assets: [{ name }] });
      if (args[1] === 'download') write(args[args.indexOf('--dir') + 1], name, 'different bytes');
      return '';
    });
    expect(() => publishEngine('engine-v0.3.0', directory, { gh })).toThrow('refusing to replace');
    expect(gh.mock.calls.some(([args]) => ['upload', 'edit'].includes(args[1]))).toBe(false);
  });

  test('reuses identical draft assets on a retry without uploading them again', () => {
    const directory = assets();
    const existing = RELEASE_ASSETS.map(name => ({ name }));
    const gh = vi.fn(args => {
      if (args[1] === 'view') return JSON.stringify({ isDraft: true, assets: existing });
      if (args[1] === 'download') {
        const name = args[args.indexOf('--pattern') + 1];
        write(args[args.indexOf('--dir') + 1], name, name);
      }
      return '';
    });
    publishEngine('engine-v0.3.0', directory, { gh });
    expect(gh.mock.calls.some(([args]) => args[1] === 'upload')).toBe(false);
    expect(gh.mock.calls.at(-1)[0][1]).toBe('edit');
  });

  test('a missing local asset or incomplete remote release stays a draft', () => {
    const directory = assets();
    const gh = vi.fn(() => JSON.stringify({ isDraft: true, assets: [] }));
    expect(() => publishEngine('engine-v0.3.0', directory, { gh })).toThrow('incomplete');
    expect(gh.mock.calls.some(([args]) => args[1] === 'edit')).toBe(false);
    fs.unlinkSync(path.join(directory, RELEASE_ASSETS.at(-1)));
    gh.mockClear();
    expect(() => publishEngine('engine-v0.3.0', directory, { gh })).toThrow('Missing local release asset');
    expect(gh).toHaveBeenCalledOnce();
  });
});


describe('protected-main skill delivery', () => {
  async function prepared() {
    const root = fixture();
    await prepareSkill('engine-v0.3.0', { root, pin: async version => {
      writeEnginePins(version, new Map(ENGINE_TARGETS.map(target => [`engine-v${version}/${assetName(target)}`, 'a'.repeat(64)])), root);
    } });
    return root;
  }
  const localGit = root => args => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();

  test('only publishes the successful main SHA with complete shared pins', async () => {
    const root = fixture();
    const git = localGit(root);
    const sha = git(['rev-parse', 'HEAD']);
    const gh = vi.fn(() => '[]');
    expect(skillCandidate(sha, { root, git, gh })).toBe(false);
    expect(gh).not.toHaveBeenCalled();
    await prepareSkill('engine-v0.3.0', { root, pin: async version => {
      writeEnginePins(version, new Map(ENGINE_TARGETS.map(target => [`engine-v${version}/${assetName(target)}`, 'a'.repeat(64)])), root);
    } });
    expect(skillCandidate('older-sha', { root, git, gh })).toBe(false);
    expect(gh).not.toHaveBeenCalled();
    expect(skillCandidate(sha, { root, git, gh })).toBe(true);
    gh.mockReturnValue('[{"tagName":"skill-v0.3.0","isDraft":false}]');
    expect(skillCandidate(sha, { root, git, gh })).toBe(false);
    gh.mockReturnValue('[{"tagName":"skill-v0.3.0","isDraft":true}]');
    expect(skillCandidate(sha, { root, git, gh })).toBe(true);
    const skill = fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8');
    write(root, 'skill/SKILL.md', skill.replace('tag: engine-v0.3.0', 'tag: engine-v0.2.0'));
    expect(skillCandidate(sha, { root, git, gh })).toBe(false);
    write(root, 'skill/SKILL.md', skill);
    write(root, 'skill/scripts/engine.sha256', '# incomplete');
    expect(skillCandidate(sha, { root, git, gh })).toBe(false);
  });

  test('opens a pin-only branch and PR, never pushing main or forcing a ref', async () => {
    const root = await prepared();
    const git = vi.fn(args => ['ls-remote', 'push'].includes(args[0]) ? '' : localGit(root)(args));
    const gh = vi.fn(args => {
      if (args[1] === 'list') return '[]';
      const body = fs.readFileSync(args[args.indexOf('--body-file') + 1], 'utf8');
      expect(body).toContain('I am advancing');
      expect(body).toContain('AI assistance:');
      return 'https://example.com/pin-pr';
    });
    expect(openSkillPinPR({ root, git, gh })).toBe('https://example.com/pin-pr');
    expect(git.mock.calls.filter(([args]) => args[0] === 'push')).toEqual([[['push', 'origin', 'HEAD:refs/heads/codex/release-skill-0.3.0']]]);
    expect(localGit(root)(['show', '--format=', '--name-only', 'HEAD']).split('\n')).toEqual(['skill/SKILL.md', 'skill/scripts/VERSION', 'skill/scripts/engine.sha256']);
  });

  test('reuses an identical pending pin PR without another push', async () => {
    const root = await prepared();
    const git = vi.fn(args => {
      if (args[0] === 'ls-remote') return 'remote branch';
      if (args[0] === 'fetch') return '';
      if (args[0] === 'diff' && args.includes('FETCH_HEAD')) return 'skill/scripts/VERSION';
      if (args[0] === 'show') return fs.readFileSync(path.join(root, args[1].split(':')[1]), 'utf8').trim();
      return localGit(root)(args);
    });
    const gh = vi.fn(() => '[{"url":"https://example.com/existing-pr"}]');
    expect(openSkillPinPR({ root, git, gh })).toBe('https://example.com/existing-pr');
    expect(git.mock.calls.some(([args]) => ['push', 'commit'].includes(args[0]))).toBe(false);
    expect(gh).toHaveBeenCalledOnce();
    git.mockImplementation(args => args[0] === 'ls-remote' ? 'remote branch' : args[0] === 'fetch' ? '' : args[0] === 'diff' && args.includes('FETCH_HEAD') ? 'crates/cli/src/main.rs' : localGit(root)(args));
    expect(() => openSkillPinPR({ root, git, gh })).toThrow('unrelated changes');
  });

  test('refuses unrelated changes in a prepared pin PR', async () => {
    const root = await prepared();
    write(root, 'Cargo.toml', fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8') + '# unrelated\n');
    expect(() => openSkillPinPR({ root, git: localGit(root), gh: vi.fn() })).toThrow('non-pin changes');
  });

  test('sets a tagger identity on a fresh publication runner', () => {
    const root = fixture();
    const git = localGit(root);
    git(['config', '--unset', 'user.name']);
    git(['config', '--unset', 'user.email']);
    git(['config', 'user.useConfigOnly', 'true']);
    const skill = parse(fs.readFileSync(new URL('../.github/workflows/release-skill.yml', import.meta.url), 'utf8'));
    for (const job of ['prepare', 'publish']) {
      const steps = skill.jobs[job].steps;
      const install = steps.findIndex(step => step.run === 'pnpm install --frozen-lockfile');
      const firstPinScript = steps.findIndex(step => step.run?.includes('node scripts/'));
      expect(install).toBeGreaterThan(-1);
      expect(install).toBeLessThan(firstPinScript);
    }
    const publish = skill.jobs.publish.steps.find(step => step.name === 'Publish the skill with the tested shared version');
    const identity = publish.run.slice(0, publish.run.indexOf('node scripts/release.mjs'));
    execFileSync('sh', ['-c', identity], { cwd: root });
    git(['tag', '-a', 'skill-v0.3.0', '-m', 'Skill 0.3.0']);
    expect(git(['for-each-ref', '--format=%(taggername) %(taggeremail)', 'refs/tags/skill-v0.3.0'])).toBe('github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>');
  });

  test('dispatches preparation on main and gates publication on trusted push CI', () => {
    const engine = parse(fs.readFileSync(new URL('../.github/workflows/release-engine.yml', import.meta.url), 'utf8'));
    expect(engine.jobs['pin-skill'].steps[0].run).toContain('gh workflow run release-skill.yml --ref main');
    expect(engine.jobs['pin-skill'].permissions).toEqual({ actions: 'write' });
    const skill = parse(fs.readFileSync(new URL('../.github/workflows/release-skill.yml', import.meta.url), 'utf8'));
    expect(skill.on.workflow_run).toEqual({ workflows: ['CI'], branches: ['main'], types: ['completed'] });
    for (const guard of ["event == 'push'", "head_branch == 'main'", "head_repository.full_name == github.repository", "conclusion == 'success'"]) {
      expect(skill.jobs.publish.if).toContain(guard);
    }
    expect(skill.jobs.publish.steps[0].with.ref).toBe('main');
    expect(skill.jobs.publish.steps[0].with.token).toBe('${{ secrets.RELEASE_PLEASE_TOKEN }}');
    const publication = skill.jobs.publish.steps.find(step => step.name === 'Publish the skill with the tested shared version');
    expect(publication.env.GH_TOKEN).toBe('${{ secrets.RELEASE_PLEASE_TOKEN }}');
    const candidate = skill.jobs.publish.steps.find(step => step.id === 'candidate');
    expect(candidate.env.TESTED_SHA).toBe('${{ github.event.workflow_run.head_sha }}');
    expect(skill.jobs.prepare.steps.at(-1).env.GH_TOKEN).toBe('${{ secrets.RELEASE_PLEASE_TOKEN }}');
    const verify = skill.jobs.publish.steps.find(step => step.name === 'Reverify the published engine and committed pins');
    expect(verify.run).toContain('git diff --exit-code');
  });
});
