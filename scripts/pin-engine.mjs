#!/usr/bin/env node
/**
 * Pin the published engine release into the skill: skill/scripts/engine.sha256.
 *
 * Run after `pnpm run release:engine` has published engine-v<VERSION>. For each
 * of the five release binaries this script downloads the asset, checks its
 * GitHub build attestation (`gh attestation verify`: built by this repo's
 * release-engine workflow), and only then writes
 * `<sha256>  engine-v<version>/<asset>` lines. The launchers
 * verify downloads against these pins, so the skill commit a skill manager
 * pins also pins the binary bytes (docs/adr/0010).
 *
 *   node scripts/pin-engine.mjs              # pin skill/scripts/VERSION
 *   node scripts/pin-engine.mjs --check      # verify the pins cover VERSION (no network)
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
import { ENGINE_TARGETS, PIN_FILE, assetName, assetUrl, missingPins, readEngineVersion } from './fetch-engine.mjs';
import { isEntrypoint } from './lib/is-entrypoint.mjs';

export { PIN_FILE, missingPins };

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REPO = 'sebastian-software/impeccino';
const WORKFLOW = `${REPO}/.github/workflows/release-engine.yml`;

async function download(url) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

async function pin(version) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-pin-'));
  const lines = [];
  try {
    for (const target of ENGINE_TARGETS) {
      const url = assetUrl(version, target);
      const bytes = await download(url);
      const digest = createHash('sha256').update(bytes).digest('hex');
      const file = path.join(dir, assetName(target));
      fs.writeFileSync(file, bytes);
      try {
        execFileSync('gh', ['attestation', 'verify', file, '--repo', REPO, '--signer-workflow', WORKFLOW], { stdio: 'pipe' });
      } catch (err) {
        const detail = String(err.stderr || err.message).trim().split('\n').slice(-3).join('\n');
        throw new Error(`${url}: build attestation did not verify (needs the gh CLI, signed in):\n${detail}`);
      }
      console.log(`✓ ${assetName(target)}  ${digest}  (attested by ${WORKFLOW})`);
      lines.push(`${digest}  engine-v${version}/${assetName(target)}`);
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  const header = [
    `# Engine release engine-v${version}, pinned by scripts/pin-engine.mjs after verifying`,
    `# each asset's build attestation (${WORKFLOW}).`,
    '# The launchers check downloads against these digests.',
  ];
  fs.writeFileSync(path.join(ROOT, PIN_FILE), `${[...header, ...lines].join('\n')}\n`);
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
    console.log(`✓ ${PIN_FILE} pins all ${ENGINE_TARGETS.length} assets of engine-v${version}`);
    return 0;
  }
  await pin(version);
  return 0;
}

if (isEntrypoint(import.meta.url)) {
  main().then((code) => process.exit(code), (err) => { console.error(`✗ ${err.message}`); process.exit(1); });
}
