#!/usr/bin/env node
// A retry may fill a draft, but cannot replace assets or mutate a public release.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { ENGINE_TARGETS, assetName } from './fetch-engine.mjs';
import { checkProductTag } from './product-release.mjs';
import { isEntrypoint } from './lib/is-entrypoint.mjs';

const digest = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const runGh = args => execFileSync('gh', args, { encoding: 'utf8' });
export const RELEASE_ASSETS = [...ENGINE_TARGETS.map(assetName), 'THIRD-PARTY-NOTICES.txt'];

export function publishEngine(tag, directory, { gh = runGh } = {}) {
  const release = JSON.parse(gh(['release', 'view', tag, '--json', 'isDraft,assets']));
  if (!release.isDraft) return 'published';
  for (const name of RELEASE_ASSETS) {
    if (!fs.existsSync(path.join(directory, name))) throw new Error(`Missing local release asset: ${name}`);
  }
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-release-'));
  try {
    for (const name of RELEASE_ASSETS) {
      const local = path.join(directory, name);
      if (release.assets.some(asset => asset.name === name)) {
        gh(['release', 'download', tag, '--pattern', name, '--dir', scratch]);
        if (digest(local) !== digest(path.join(scratch, name))) {
          throw new Error(`Draft asset ${name} differs; refusing to replace it`);
        }
      } else {
        gh(['release', 'upload', tag, local]);
      }
    }
    const complete = JSON.parse(gh(['release', 'view', tag, '--json', 'assets']));
    if (RELEASE_ASSETS.some(name => !complete.assets.some(asset => asset.name === name))) {
      throw new Error('The draft release is incomplete');
    }
    gh(['release', 'edit', tag, '--draft=false']);
    return 'published';
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

if (isEntrypoint(import.meta.url)) {
  try {
    const [tag, directory] = process.argv.slice(2);
    checkProductTag(tag);
    console.log(publishEngine(tag, directory));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
