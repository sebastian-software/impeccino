#!/usr/bin/env node
/**
 * Release-order guard.
 *
 * The launcher (skill/scripts/impeccino) dead-ends unless the engine release
 * for the pinned skill/scripts/VERSION exists FIRST: the five platform binaries
 * in the engine-v<version> GitHub Release. Nothing else mechanically stops a
 * maintainer from tagging the skill release before those assets are published.
 *
 * This script verifies, for the pinned engine version, that each of the five
 * release binaries impeccino-<os>-<arch>[.exe] is fetchable. Their digests and
 * build attestations are checked by scripts/pin-engine.mjs.
 *
 * Exits 0 when everything is present, non-zero (naming exactly what is missing)
 * otherwise. release.mjs runs it before an engine-dependent release; CI runs it
 * as a soft warning until the first engine release exists.
 *
 *   node scripts/check-engine-release.mjs            # check the pinned skill/scripts/VERSION
 *   node scripts/check-engine-release.mjs --json     # machine-readable report
 *
 * Environment:
 *   IMPECCINO_DOWNLOAD_BASE  release root (default: the public repo's GitHub Releases)
 */
import {
  ENGINE_TARGETS,
  DEFAULT_DOWNLOAD_BASE,
  readEngineVersion,
  assetUrl,
} from './fetch-engine.mjs';

// A ranged GET is the most portable existence probe: GitHub release downloads
// answer HEAD inconsistently across their 302 to object storage, but a
// `Range: bytes=0-0` GET follows the redirect and returns 200/206 for a real
// asset and 404 for a missing one without pulling the whole binary.
async function urlExists(url) {
  try {
    const res = await fetch(url, { redirect: 'follow', headers: { Range: 'bytes=0-0' } });
    return res.ok || res.status === 206;
  } catch (err) {
    return false;
  }
}

/**
 * Check every asset for one engine version. Returns { ok, version, base, missing }
 * where missing is a list of { kind, target, what, url } entries.
 */
export async function checkEngineRelease({
  version = readEngineVersion(),
  base = process.env.IMPECCINO_DOWNLOAD_BASE || DEFAULT_DOWNLOAD_BASE,
} = {}) {
  const missing = [];

  await Promise.all(
    ENGINE_TARGETS.map(async (target) => {
      const binUrl = assetUrl(version, target, base);
      const binOk = await urlExists(binUrl);

      if (!binOk) missing.push({ kind: 'binary', target, what: `impeccino-${target} binary`, url: binUrl });
    })
  );

  missing.sort((a, b) => ENGINE_TARGETS.indexOf(a.target) - ENGINE_TARGETS.indexOf(b.target));

  return { ok: missing.length === 0, version, base, missing };
}

function report(result) {
  const { ok, version, base, missing } = result;
  if (ok) {
    console.log(`✓ engine v${version} release is complete: all ${ENGINE_TARGETS.length} binaries are published.`);
    console.log(`  release base: ${base}`);
    return;
  }
  console.error(`✗ engine v${version} release is INCOMPLETE — ${missing.length} asset(s) missing:`);
  for (const m of missing) {
    console.error(`  · ${m.what}`);
    console.error(`      ${m.url}`);
  }
  console.error('');
  console.error(`Publish engine v${version} (tag engine-v${version}, bun run release:engine)`);
  console.error('BEFORE releasing the skill. See CLAUDE.md "Releases".');
  console.error(`  release base: ${base}`);
}

async function main(argv = process.argv.slice(2)) {
  const json = argv.includes('--json');
  const result = await checkEngineRelease();
  if (json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    report(result);
  }
  return result.ok ? 0 : 1;
}

// Run only when invoked directly, not when imported by release.mjs.
if (import.meta.url === `file://${process.argv[1]}`) {
  main().then((code) => process.exit(code));
}
