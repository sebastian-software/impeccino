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
 * Exits 0 when every asset is present, 1 when an asset returns 404, and 2 when
 * a request remains unverifiable after retries. release.mjs requires a verified
 * release; CI warns on an unverifiable probe and fails on confirmed missing assets.
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
import { isEntrypoint } from './lib/is-entrypoint.mjs';

const DEFAULT_RETRIES = 2;
const DEFAULT_RETRY_DELAY_MS = 250;
const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;

// A ranged GET follows GitHub's redirect without pulling the whole binary.
// Only 404 proves an asset is absent; request failures and other HTTP errors
// remain unverified and are retried before being reported separately.
async function probeAsset(url, fetchImpl, retries, retryDelayMs, timeoutMs) {
  let lastError;
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const res = await fetchImpl(url, {
        redirect: 'follow',
        headers: { Range: 'bytes=0-0' },
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (res.status === 404) return { state: 'missing' };
      if (res.ok || res.status === 206) return { state: 'present' };
      lastError = new Error(`HTTP ${res.status}${res.statusText ? ` ${res.statusText}` : ''}`);
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
    }
    if (attempt < retries) await new Promise((resolve) => setTimeout(resolve, retryDelayMs * (attempt + 1)));
  }
  return { state: 'unreachable', error: lastError?.message || 'request failed' };
}

/**
 * Check every asset for one engine version. Returns missing 404 assets and
 * unverified assets separately.
 */
export async function checkEngineRelease({
  version = readEngineVersion(),
  base = process.env.IMPECCINO_DOWNLOAD_BASE || DEFAULT_DOWNLOAD_BASE,
  fetchImpl = fetch,
  retries = DEFAULT_RETRIES,
  retryDelayMs = DEFAULT_RETRY_DELAY_MS,
  timeoutMs = DEFAULT_REQUEST_TIMEOUT_MS,
} = {}) {
  const missing = [];
  const unreachable = [];

  await Promise.all(
    ENGINE_TARGETS.map(async (target) => {
      const binUrl = assetUrl(version, target, base);
      const probe = await probeAsset(binUrl, fetchImpl, retries, retryDelayMs, timeoutMs);
      if (probe.state === 'missing') {
        missing.push({ kind: 'binary', target, what: `impeccino-${target} binary`, url: binUrl });
      } else if (probe.state === 'unreachable') {
        unreachable.push({ kind: 'binary', target, what: `impeccino-${target} binary`, url: binUrl, error: probe.error });
      }
    })
  );

  missing.sort((a, b) => ENGINE_TARGETS.indexOf(a.target) - ENGINE_TARGETS.indexOf(b.target));
  unreachable.sort((a, b) => ENGINE_TARGETS.indexOf(a.target) - ENGINE_TARGETS.indexOf(b.target));

  return { ok: missing.length === 0 && unreachable.length === 0, version, base, missing, unreachable };
}

function report(result) {
  const { ok, version, base, missing, unreachable } = result;
  if (ok) {
    console.log(`✓ engine v${version} release is complete: all ${ENGINE_TARGETS.length} binaries are published.`);
    console.log(`  release base: ${base}`);
    return;
  }
  if (missing.length) {
    console.error(`✗ engine v${version} release is INCOMPLETE — ${missing.length} asset(s) missing:`);
    for (const m of missing) {
      console.error(`  · ${m.what}`);
      console.error(`      ${m.url}`);
    }
  }
  if (unreachable.length) {
    console.error(`✗ could not verify ${unreachable.length} engine v${version} asset(s) after retries:`);
    for (const m of unreachable) {
      console.error(`  · ${m.what}: ${m.error}`);
      console.error(`      ${m.url}`);
    }
  }
  if (missing.length) {
    console.error('');
    console.error(`Publish engine v${version} (tag engine-v${version}, pnpm run release:engine)`);
    console.error('BEFORE releasing the skill. See AGENTS.md "Releases".');
  }
  if (unreachable.length) console.error('Check network access and rerun this probe before releasing the skill.');
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
  return result.ok ? 0 : result.missing.length ? 1 : 2;
}

// Run only when invoked directly, not when imported by release.mjs.
if (isEntrypoint(import.meta.url)) {
  main().then((code) => process.exit(code));
}
