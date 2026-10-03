#!/usr/bin/env node
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isEntrypoint } from './lib/is-entrypoint.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const CARGO_ABOUT_VERSION = '0.8.4';
export const RELEASE_TARGET_LICENSE_CRATES = ['windows-link', 'libc'];

function runCargoAbout(cargoAbout, cwd, args) {
  const result = spawnSync(cargoAbout, args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
    timeout: 300_000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(result.stderr || 'cargo-about failed without diagnostics');
  }
  return result.stdout;
}

export function validateCargoAboutVersion(version) {
  const expected = `cargo-about ${CARGO_ABOUT_VERSION}`;
  if (version.trim() !== expected) {
    throw new Error(`Expected ${expected}, found: ${version.trim() || '(empty output)'}`);
  }
}

export function validateDependencyLicenses(dependencyLicenses) {
  if (!dependencyLicenses.includes('License:') || !dependencyLicenses.includes('Packages using this license:')) {
    throw new Error('cargo-about produced an empty or incomplete dependency license listing');
  }
  if (/&(?:amp|lt|gt|quot|#(?:x[0-9a-f]+|[0-9]+));/i.test(dependencyLicenses)) {
    throw new Error('cargo-about output contains HTML-escaped text instead of plain-text license notices');
  }
  for (const crate of RELEASE_TARGET_LICENSE_CRATES) {
    if (!dependencyLicenses.includes(`- ${crate} `)) {
      throw new Error(`cargo-about omitted the release target-union dependency ${crate}`);
    }
  }
}

export function buildEngineNotices({ projectLicense, projectNotices, dependencyLicenses }) {
  for (const notice of [
    'Copyright (C) 2016-2026 by Roman Dvornov',
    'Copyright (C) 1993-2004 by Sun Microsystems, Inc.',
    'Copyright 2014, the V8 project authors.',
  ]) {
    if (!projectNotices.includes(notice)) {
      throw new Error(`NOTICE.md is missing required upstream attribution: ${notice}`);
    }
  }
  return [
    'IMPECCINO ENGINE RELEASE LICENSES AND NOTICES',
    '',
    `This file is generated from the locked Cargo workspace with cargo-about ${CARGO_ABOUT_VERSION}.`,
    'The Cargo dependency section covers the union of all release targets in about.toml.',
    '',
    '================================================================================',
    'IMPECCINO PROJECT LICENSE (Apache-2.0)',
    '',
    projectLicense.trim(),
    '',
    '================================================================================',
    'PROJECT THIRD-PARTY NOTICES',
    '',
    projectNotices.trim(),
    '',
    '================================================================================',
    'RUST DEPENDENCY LICENSES',
    '',
    dependencyLicenses.trim(),
    '',
  ].join('\n');
}

export async function generateEngineNotices({
  root = repoRoot,
  outputArgument = process.argv[2] ?? 'out/THIRD-PARTY-NOTICES.txt',
  cargoAbout = process.env.CARGO_ABOUT_BIN ?? 'cargo-about',
  run = (args) => runCargoAbout(cargoAbout, root, args),
  read = readFile,
  makeDirectory = mkdir,
  write = writeFile,
} = {}) {
  validateCargoAboutVersion(await run(['--version']));
  const dependencyLicenses = (await run([
    'generate',
    '--workspace',
    '--frozen',
    '--fail',
    'about.hbs',
  ])).trim();
  validateDependencyLicenses(dependencyLicenses);

  const [projectLicense, projectNotices] = await Promise.all([
    read(path.join(root, 'LICENSE'), 'utf8'),
    read(path.join(root, 'NOTICE.md'), 'utf8'),
  ]);
  const contents = buildEngineNotices({ projectLicense, projectNotices, dependencyLicenses });
  const outputPath = path.resolve(root, outputArgument);
  await makeDirectory(path.dirname(outputPath), { recursive: true });
  await write(outputPath, contents, 'utf8');
  return outputPath;
}

if (isEntrypoint(import.meta.url)) {
  generateEngineNotices().then((outputPath) => {
    process.stdout.write(`Wrote ${path.relative(repoRoot, outputPath)}\n`);
  }, (error) => {
    process.stderr.write(`generate-engine-notices: ${error.message}\n`);
    process.exitCode = 1;
  });
}
