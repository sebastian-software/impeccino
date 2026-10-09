#!/usr/bin/env node
/**
 * Pin attested engine bytes in engine.sha256, VERSION, and SKILL.md's Dalo declaration.
 *
 * The release workflow calls pinEngine after engine-v<VERSION> is published. For each
 * of the five release binaries this script downloads the asset, checks its
 * GitHub build attestation (`gh attestation verify`: built by this repo's
 * release-engine workflow), and only then writes
 * `<sha256>  engine-v<version>/<asset>` lines and Dalo's four-platform declaration. The launchers
 * verify downloads against these pins, so the skill commit a skill manager
 * pins also pins the binary bytes (docs/adr/0010).
 *
 *   node scripts/pin-engine.mjs              # pin skill/scripts/VERSION
 *   node scripts/pin-engine.mjs --check      # verify both routes against VERSION (no network)
 *
 * Environment:
 *   IMPECCINO_DOWNLOAD_BASE  release root (default: this repo's GitHub Releases)
 */
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import { parseDocument, stringify } from 'yaml';
import { ENGINE_TARGETS, PIN_FILE, assetName, assetUrl, missingPins, readEngineVersion, readPins } from './fetch-engine.mjs';
import { isEntrypoint } from './lib/is-entrypoint.mjs';

export { PIN_FILE, missingPins };

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REPO = 'sebastian-software/impeccino';
const WORKFLOW = `${REPO}/.github/workflows/release-engine.yml`;
const SKILL_FILE = 'skill/SKILL.md';
const FRONTMATTER = /^---\r?\n([\s\S]*?)\r?\n---/;

function skillDocument(skill) {
  const frontmatter = skill.match(FRONTMATTER);
  if (!frontmatter) throw new Error('Skill frontmatter is missing');
  const document = parseDocument(frontmatter[1]);
  if (document.errors.length) throw new Error(`Invalid skill frontmatter: ${document.errors[0].message}`);
  return { document, frontmatter: frontmatter[0] };
}

function declaration(version, pins) {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error('Invalid engine version');
  const assets = {};
  for (const target of ENGINE_TARGETS) {
    const asset = assetName(target);
    const sha256 = pins.get(`engine-v${version}/${asset}`);
    if (!/^[0-9a-f]{64}$/.test(sha256 || '')) throw new Error(`Missing or invalid pin for engine-v${version}/${asset}`);
    if (!target.startsWith('windows-')) assets[target.replace('darwin-', 'macos-')] = { asset, sha256 };
  }
  return { source: 'github-release', repo: REPO, tag: `engine-v${version}`, availability: 'optional', assets };
}

/** Check both distribution routes against the installed engine version, offline. */
export function checkEnginePins(version = readEngineVersion(), root = ROOT, skill = fs.readFileSync(path.join(root, SKILL_FILE), 'utf8')) {
  const expected = declaration(version, readPins(root));
  const { document } = skillDocument(skill);
  if (!isDeepStrictEqual(document.toJS().binaries?.impeccino, expected)) {
    throw new Error(`SKILL.md binaries.impeccino disagrees with ${PIN_FILE} for engine-v${version}`);
  }
}

/** Publish the same verified bytes to the launcher pins and Dalo declaration. */
export function writeEnginePins(version, pins, root = ROOT) {
  const engine = declaration(version, pins);
  const skillPath = path.join(root, SKILL_FILE);
  const skill = fs.readFileSync(skillPath, 'utf8');
  const { frontmatter } = skillDocument(skill);
  const block = stringify({ binaries: { impeccino: engine } }, { lineWidth: 0 });
  // Only this generated section changes; keep authored frontmatter and body bytes.
  const contents = frontmatter.match(FRONTMATTER)[1].replace(/\r\n/g, '\n');
  const binaries = /^binaries:[^\n]*(?:\n(?:[ \t].*|[ \t]*))*\n?/m;
  const updated = binaries.test(contents) ? contents.replace(binaries, block) : `${contents.trimEnd()}\n${block}`;
  const header = [
    `# Engine release engine-v${version}, pinned by scripts/pin-engine.mjs after verifying`,
    `# each asset's build attestation (${WORKFLOW}).`,
    '# The launchers check downloads against these digests.',
  ];
  const lines = ENGINE_TARGETS.map(target => `${pins.get(`engine-v${version}/${assetName(target)}`)}  engine-v${version}/${assetName(target)}`);
  const files = [PIN_FILE, SKILL_FILE, 'skill/scripts/VERSION'];
  const originals = files.map(file => fs.readFileSync(path.join(root, file)));
  try {
    fs.writeFileSync(path.join(root, PIN_FILE), `${[...header, ...lines].join('\n')}\n`);
    fs.writeFileSync(skillPath, skill.replace(frontmatter, `---\n${updated.trimEnd()}\n---`));
    fs.writeFileSync(path.join(root, 'skill/scripts/VERSION'), `${version}\n`);
  } catch (error) {
    files.forEach((file, index) => fs.writeFileSync(path.join(root, file), originals[index]));
    throw error;
  }
}

/** Normalize only generated version/digest values for release recovery comparisons. */
export function withoutReleasePins(skill) {
  const { document, frontmatter } = skillDocument(skill);
  document.setIn(['metadata', 'version'], 'product');
  const engine = document.toJS().binaries?.impeccino;
  if (engine) {
    document.setIn(['binaries', 'impeccino', 'tag'], 'engine-product');
    for (const platform of Object.keys(engine.assets || {})) {
      document.setIn(['binaries', 'impeccino', 'assets', platform, 'sha256'], 'digest');
    }
  }
  return skill.replace(frontmatter, `---\n${document.toString({ lineWidth: 0 }).trimEnd()}\n---`);
}

async function download(url) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

function verify(file, version, sourceDigest) {
  const args = ['attestation', 'verify', file, '--repo', REPO, '--signer-workflow', WORKFLOW];
  if (sourceDigest) args.push('--source-digest', sourceDigest, '--source-ref', `refs/tags/engine-v${version}`);
  execFileSync('gh', args, { stdio: 'pipe' });
}

export async function pinEngine(version, { root = ROOT, sourceDigest, downloadAsset = download, verifyAsset = verify } = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-pin-'));
  const pins = new Map();
  try {
    for (const target of ENGINE_TARGETS) {
      const url = assetUrl(version, target);
      const bytes = await downloadAsset(url);
      const digest = createHash('sha256').update(bytes).digest('hex');
      const file = path.join(dir, assetName(target));
      fs.writeFileSync(file, bytes);
      try {
        await verifyAsset(file, version, sourceDigest);
      } catch (err) {
        const detail = String(err.stderr || err.message).trim().split('\n').slice(-3).join('\n');
        throw new Error(`${url}: build attestation did not verify (needs the gh CLI, signed in):\n${detail}`);
      }
      console.log(`✓ ${assetName(target)}  ${digest}  (attested by ${WORKFLOW})`);
      pins.set(`engine-v${version}/${assetName(target)}`, digest);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  writeEnginePins(version, pins, root);
  console.log(`\n✓ wrote ${PIN_FILE} for engine-v${version}`);
}

async function main(argv = process.argv.slice(2)) {
  const version = readEngineVersion();
  if (argv.includes('--check')) {
    const missing = missingPins(version);
    if (missing.length) {
      console.error(`✗ ${PIN_FILE} has no pin for engine-v${version}: ${missing.map(assetName).join(', ')}. Run node scripts/pin-engine.mjs.`);
      return 1;
    }
    checkEnginePins(version);
    console.log(`✓ ${PIN_FILE} pins all ${ENGINE_TARGETS.length} assets of engine-v${version}`);
    return 0;
  }
  await pinEngine(version);
  return 0;
}

if (isEntrypoint(import.meta.url)) {
  main().then((code) => process.exit(code), (err) => { console.error(`✗ ${err.message}`); process.exit(1); });
}
