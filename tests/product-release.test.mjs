import { afterEach, describe, expect, test, vi } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { parse } from 'yaml';
import { checkProductTag, prepareSkill } from '../scripts/product-release.mjs';
import { publishEngine, RELEASE_ASSETS } from '../scripts/publish-engine.mjs';
import { ENGINE_TARGETS, assetName } from '../scripts/fetch-engine.mjs';

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
      expect(fs.readFileSync(path.join(root, 'skill/scripts/VERSION'), 'utf8')).toBe('0.2.0\n');
      expect(options.sourceDigest).toMatch(/^[a-f0-9]{40}$/);
      write(root, 'skill/scripts/engine.sha256', ENGINE_TARGETS.map(target => `${'a'.repeat(64)}  engine-v${version}/${assetName(target)}`).join('\n'));
    });
    await expect(prepareSkill('engine-v0.3.0', { root, pin })).resolves.toBe('0.3.0');
    expect(pin).toHaveBeenCalledOnce();
    expect(fs.readFileSync(path.join(root, 'skill/scripts/VERSION'), 'utf8')).toBe('0.3.0\n');
    expect(fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8')).toBe('---\nname: impeccino\nmetadata:\n  version: 0.3.0\n---\n\nKeep this body.\n  version: body text\n');
  });

  test.each(['provenance failure', 'incomplete pins'])('restores the installed skill after %s', async (failure) => {
    const root = fixture();
    const files = ['skill/SKILL.md', 'skill/scripts/VERSION', 'skill/scripts/engine.sha256'];
    const before = files.map(file => fs.readFileSync(path.join(root, file), 'utf8'));
    const pin = async () => {
      write(root, 'skill/scripts/engine.sha256', '# partial\n');
      if (failure === 'provenance failure') throw new Error('untrusted artifact');
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
