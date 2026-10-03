import { describe, expect, test } from 'vitest';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { CARGO_ABOUT_VERSION, generateEngineNotices } from '../scripts/generate-engine-notices.mjs';

const licenseText = [
  'License: MIT License',
  '',
  'Packages using this license:',
  '- windows-link 0.2.1 (MIT OR Apache-2.0)',
  '- core-foundation-sys 0.8.7 (MIT OR Apache-2.0)',
  '',
  'Plain text retains <angle brackets>, an & ampersand, and "quotes".',
].join('\n');

const projectNotices = [
  'Copyright (C) 2016-2026 by Roman Dvornov',
  'Copyright (C) 1993-2004 by Sun Microsystems, Inc.',
  'Copyright 2014, the V8 project authors.',
].join('\n');

describe('engine notice generator', () => {
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
      listing: licenseText.replace('- core-foundation-sys 0.8.7 (MIT OR Apache-2.0)\n', ''),
      message: /omitted the release target-union dependency core-foundation-sys/,
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
