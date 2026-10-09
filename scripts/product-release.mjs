#!/usr/bin/env node
// Release Please chooses the product version. The installed skill advances
// only after all five binaries have verified provenance and committed pins.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { missingPins, PIN_FILE } from './fetch-engine.mjs';
import { isEntrypoint } from './lib/is-entrypoint.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

export function productVersion(root = ROOT) {
  const version = fs.readFileSync(path.join(root, '.release-please-version'), 'utf8').trim();
  if (!SEMVER.test(version)) throw new Error('Invalid product version');
  const manifest = JSON.parse(fs.readFileSync(path.join(root, '.release-please-manifest.json'), 'utf8'));
  if (manifest['.'] !== version) throw new Error('Release Please manifest disagrees with the product version');
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const workspace = cargo.match(/^\[workspace\.package\]\s*\n([\s\S]*?)(?=^\[|$)/m)?.[1];
  const cargoVersion = workspace?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (version !== cargoVersion) throw new Error(`Product ${version} disagrees with Cargo.toml ${cargoVersion}`);
  return version;
}

export function checkProductTag(tag, root = ROOT) {
  const version = productVersion(root);
  if (tag !== `engine-v${version}`) throw new Error(`Engine tag mismatch: expected engine-v${version}, got ${tag}`);
  return version;
}

export async function prepareSkill(tag, { root = ROOT, pin: suppliedPin } = {}) {
  const version = checkProductTag(tag, root);
  // Engine build runners check tags before any Node packages are installed.
  // Only skill preparation needs the YAML parser and pin-writing helpers.
  const { pinEngine, checkEnginePins, withoutReleasePins } = await import('./pin-engine.mjs');
  const pin = suppliedPin ?? pinEngine;
  const skillPath = path.join(root, 'skill/SKILL.md');
  const skill = fs.readFileSync(skillPath, 'utf8');
  const frontmatter = skill.match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[0];
  const versionLine = frontmatter?.match(/^  version:\s*[^\r\n]+/m)?.[0];
  if (!versionLine) throw new Error('Skill metadata.version is missing');
  const sourceDigest = execFileSync('git', ['rev-parse', `${tag}^{commit}`], { cwd: root, encoding: 'utf8' }).trim();
  const changed = execFileSync('git', ['diff', '--name-only', sourceDigest, 'HEAD', '--', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'skill'], { cwd: root, encoding: 'utf8' }).trim().split('\n').filter(Boolean);
  const pinFiles = [PIN_FILE, 'skill/scripts/VERSION', 'skill/SKILL.md'];
  if (changed.some(file => !pinFiles.includes(file))) throw new Error('Release sources changed on main; refusing to pin an older engine');
  const taggedSkill = execFileSync('git', ['show', `${sourceDigest}:skill/SKILL.md`], { cwd: root, encoding: 'utf8' });
  if (withoutReleasePins(skill) !== withoutReleasePins(taggedSkill)) throw new Error('Skill sources changed on main; finish the existing release first');
  const files = [PIN_FILE, 'skill/scripts/VERSION', 'skill/SKILL.md'];
  const originals = files.map(file => fs.readFileSync(path.join(root, file)));
  try {
    await pin(version, { root, sourceDigest });
    if (missingPins(version, root).length) throw new Error('The product engine is not fully pinned');
    checkEnginePins(version, root);
    fs.writeFileSync(path.join(root, 'skill/scripts/VERSION'), `${version}\n`);
    const pinnedSkill = fs.readFileSync(skillPath, 'utf8');
    fs.writeFileSync(skillPath, pinnedSkill.replace(/^---\r?\n([\s\S]*?)\r?\n---/, block => block.replace(/^  version:\s*[^\r\n]+/m, `  version: ${version}`)));
  } catch (error) {
    files.forEach((file, index) => fs.writeFileSync(path.join(root, file), originals[index]));
    throw error;
  }
  return version;
}

if (isEntrypoint(import.meta.url)) {
  const [command, tag] = process.argv.slice(2);
  try {
    if (command === 'check-tag') console.log(checkProductTag(tag));
    else if (command === 'prepare-skill') console.log(await prepareSkill(tag));
    else throw new Error('Usage: product-release.mjs <check-tag|prepare-skill> engine-v<version>');
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
