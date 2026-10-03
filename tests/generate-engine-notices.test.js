import { describe, expect, test } from 'vitest';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CARGO_ABOUT_VERSION, generateEngineNotices } from '../scripts/generate-engine-notices.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const licenseText = [
  'License: MIT License',
  '',
  'Packages using this license:',
  '- windows-link 0.2.1 (MIT OR Apache-2.0)',
  '- libc 0.2.189 (MIT OR Apache-2.0)',
  '',
  'Plain text retains <angle brackets>, an & ampersand, and "quotes".',
].join('\n');

const projectNotices = [
  'Copyright (C) 2016-2026 by Roman Dvornov',
  'Copyright (C) 1993-2004 by Sun Microsystems, Inc.',
  'Copyright 2014, the V8 project authors.',
].join('\n');

describe('engine notice generator', () => {
  test('keeps the installable skill license files complete and tied to their source', async () => {
    const [rootLicense, skillLicense, rootNotices, skillNotices] = await Promise.all([
      readFile(path.join(repoRoot, 'LICENSE')),
      readFile(path.join(repoRoot, 'skill', 'LICENSE')),
      readFile(path.join(repoRoot, 'NOTICE.md'), 'utf8'),
      readFile(path.join(repoRoot, 'skill', 'NOTICE.md'), 'utf8'),
    ]);

    expect(skillLicense).toEqual(rootLicense);
    const rootMit = extractPlatformMitNotice(rootNotices);
    const skillMit = extractPlatformMitNotice(skillNotices);
    expect(skillMit).toBe(rootMit);
    expect(skillMit).toMatch(/^MIT License\n\nCopyright \(c\) 2026/);
    expect(skillMit).toContain('Permission is hereby granted, free of charge');
    expect(skillMit).toContain('THE SOFTWARE IS PROVIDED "AS IS"');
    expect(skillNotices).toContain('dc2be825d8b439caea78e9eaa8fb3ac23b0ff3e9');
    expect(skillNotices).toContain('https://github.com/ehmo/platform-design-skills/blob/dc2be825d8b439caea78e9eaa8fb3ac23b0ff3e9/LICENSE');
  });

  test('writes the plain-text locked license listing after validating the pinned tool', async () => {
    const root = await createRoot();
    try {
      const calls = [];
      const outputPath = await generateEngineNotices({
        root,
        outputArgument: 'out/THIRD-PARTY-NOTICES.txt',
        run: async (args) => {
          calls.push(args);
          return args[0] === '--version' ? `cargo-about ${CARGO_ABOUT_VERSION}\n` : licenseText;
        },
      });

      expect(calls).toEqual([
        ['--version'],
        ['generate', '--workspace', '--frozen', '--fail', 'about.hbs'],
      ]);
      const generated = await readFile(outputPath, 'utf8');
      expect(generated).toContain('Apache project license');
      expect(generated).toContain(projectNotices);
      expect(generated).toContain(licenseText);
      expect(generated).not.toContain('&lt;');
      expect(generated).not.toContain('&quot;');
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test.each([
    {
      name: 'wrong cargo-about version',
      version: 'cargo-about 0.8.3',
      listing: licenseText,
      message: /Expected cargo-about 0\.8\.4/,
    },
    {
      name: 'missing target-specific packages',
      version: `cargo-about ${CARGO_ABOUT_VERSION}`,
      listing: licenseText.replace('- windows-link 0.2.1 (MIT OR Apache-2.0)\n', ''),
      message: /omitted the release target-union dependency windows-link/,
    },
    {
      name: 'HTML-escaped license text',
      version: `cargo-about ${CARGO_ABOUT_VERSION}`,
      listing: licenseText.replace('"quotes"', '&quot;quotes&quot;'),
      message: /HTML-escaped text/,
    },
  ])('fails on $name without writing an incomplete output file', async ({ version, listing, message }) => {
    const root = await createRoot();
    try {
      const outputPath = path.join(root, 'out', 'THIRD-PARTY-NOTICES.txt');
      await expect(generateEngineNotices({
        root,
        outputArgument: 'out/THIRD-PARTY-NOTICES.txt',
        run: async (args) => args[0] === '--version' ? version : listing,
      })).rejects.toThrow(message);
      expect(existsSync(outputPath)).toBe(false);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

async function createRoot() {
  const root = await mkdtemp(path.join(os.tmpdir(), 'impeccino-notices-test-'));
  await writeFile(path.join(root, 'LICENSE'), 'Apache project license\n');
  await writeFile(path.join(root, 'NOTICE.md'), projectNotices);
  return root;
}

function extractPlatformMitNotice(notices) {
  const match = notices.match(/The upstream MIT notice and permission terms are reproduced here[^\n]*\n\n```text\n([\s\S]*?)\n```/);
  if (!match) throw new Error('The complete platform MIT notice is missing');
  return match[1];
}
