#!/usr/bin/env node
// Deliver verified pins through the same required CI gate as other changes.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { productVersion } from './product-release.mjs';
import { missingPins, readEngineVersion, PIN_FILE } from './fetch-engine.mjs';
import { isEntrypoint } from './lib/is-entrypoint.mjs';
import { checkEnginePins } from './pin-engine.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REPO = 'sebastian-software/impeccino';
const FILES = ['skill/SKILL.md', 'skill/scripts/VERSION', PIN_FILE];
const command = (root, binary) => args => execFileSync(binary, args, { cwd: root, encoding: 'utf8' }).trim();
const skillVersion = root => fs.readFileSync(path.join(root, 'skill/SKILL.md'), 'utf8').match(/^---\r?\n([\s\S]*?)\r?\n---/)?.[1].match(/^  version:\s*(\S+)/m)?.[1];

export function sharedPinsReady(root = ROOT) {
  const version = productVersion(root);
  if (readEngineVersion(root) !== version || skillVersion(root) !== version || missingPins(version, root).length) return false;
  try {
    checkEnginePins(version, root);
    return true;
  } catch {
    return false;
  }
}

export function skillCandidate(testedSha, { root = ROOT, git = command(root, 'git'), gh = command(root, 'gh') } = {}) {
  // A newer main commit must get its own successful push CI before publication.
  if (git(['rev-parse', 'HEAD']) !== testedSha || !sharedPinsReady(root)) return false;
  const tag = `skill-v${productVersion(root)}`;
  const releases = JSON.parse(gh(['release', 'list', '--repo', REPO, '--limit', '1000', '--json', 'tagName,isDraft']));
  return !releases.some(release => release.tagName === tag && !release.isDraft);
}

export function openSkillPinPR({ root = ROOT, git = command(root, 'git'), gh = command(root, 'gh') } = {}) {
  if (!sharedPinsReady(root)) throw new Error('Shared skill version and complete engine pins are required');
  const version = productVersion(root);
  const changed = git(['diff', '--name-only', 'HEAD']).split('\n').filter(Boolean);
  if (changed.some(file => !FILES.includes(file))) throw new Error('Refusing to include non-pin changes in the release PR');
  if (!changed.length) return 'Skill pins are already committed on main';
  const branch = `codex/release-skill-${version}`;
  if (git(['ls-remote', '--heads', 'origin', `refs/heads/${branch}`])) {
    git(['fetch', 'origin', branch]);
    const branchChanges = git(['diff', '--name-only', 'HEAD', 'FETCH_HEAD']).split('\n').filter(Boolean);
    if (branchChanges.some(file => !FILES.includes(file))) throw new Error('Existing pin branch has unrelated changes; refusing to replace it');
    for (const file of FILES) {
      if (git(['show', `FETCH_HEAD:${file}`]) !== fs.readFileSync(path.join(root, file), 'utf8').trim()) {
        throw new Error('Existing pin branch has different verified files; refusing to replace it');
      }
    }
  } else {
    git(['config', 'user.name', 'github-actions[bot]']);
    git(['config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com']);
    git(['add', '--', ...FILES]);
    git(['commit', '-m', `chore: pin engine and release skill ${version}`, '-m', 'AI assistance: Codex helped implement this release automation.']);
    git(['push', 'origin', `HEAD:refs/heads/${branch}`]);
  }
  const existing = JSON.parse(gh(['pr', 'list', '--repo', REPO, '--state', 'open', '--base', 'main', '--head', branch, '--json', 'url']));
  if (existing.length) return existing[0].url;
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-pin-pr-'));
  try {
    const body = path.join(dir, 'body.md');
    fs.writeFileSync(body, `I am advancing the skill and launcher to the shared ${version} release after verifying all five published engine assets against their signing workflow, source tag, and commit.\n\nThe pin PR runs the required CI. After merge and successful main CI, release-skill.yml publishes skill-v${version} from the tested commit.\n\nAI assistance: Codex helped implement this release automation.\n`);
    return gh(['pr', 'create', '--repo', REPO, '--base', 'main', '--head', branch, '--title', `chore: pin engine and release skill ${version}`, '--body-file', body]);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

if (isEntrypoint(import.meta.url)) {
  try {
    if (process.argv[2] === 'candidate') console.log(`ready=${skillCandidate(process.argv[3])}`);
    else console.log(openSkillPinPR());
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
