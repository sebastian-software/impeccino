import { describe, test, expect, beforeAll, afterAll } from 'bun:test';
import fs from 'fs';
import path from 'path';
import { PROVIDERS } from '../../../scripts/lib/transformers/providers.js';
import { createTransformer } from '../../../scripts/lib/transformers/factory.js';
import { readSourceFiles } from '../../../scripts/lib/utils.js';

const ROOT = path.resolve(import.meta.dir, '../../..');
const TEST_DIR = path.join(process.cwd(), 'test-tmp-providers');

function listFiles(dir, base = dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    return entry.isDirectory() ? listFiles(full, base) : [path.relative(base, full)];
  }).sort();
}

describe('provider configs', () => {
  test('every provider has a unique dist id and a dot-directory', () => {
    const ids = Object.values(PROVIDERS).map((c) => c.provider);
    expect(new Set(ids).size).toBe(ids.length);
    for (const config of Object.values(PROVIDERS)) {
      expect(config.configDir.startsWith('.')).toBe(true);
      expect(config.displayName).toBeTruthy();
    }
  });
});

describe('every provider receives the same skill', () => {
  const { skills } = readSourceFiles(ROOT);
  const skillDir = path.join(ROOT, 'skill');

  beforeAll(() => {
    fs.rmSync(TEST_DIR, { recursive: true, force: true });
    const log = console.log;
    console.log = () => {};
    try {
      for (const config of Object.values(PROVIDERS)) createTransformer(config)(skills, TEST_DIR);
    } finally {
      console.log = log;
    }
  });

  afterAll(() => fs.rmSync(TEST_DIR, { recursive: true, force: true }));

  for (const config of Object.values(PROVIDERS)) {
    test(`${config.displayName}: skill copy matches skill/ byte for byte`, () => {
      const copy = path.join(TEST_DIR, config.provider, config.configDir, 'skills', 'impeccable');
      const files = listFiles(copy);
      const sourceFiles = listFiles(skillDir).filter((rel) => !rel.startsWith(path.join('scripts', 'bin')) && rel !== path.join('scripts', 'config.json') && !rel.endsWith('.DS_Store'));
      expect(files).toEqual(sourceFiles);
      for (const rel of files) {
        expect(fs.readFileSync(path.join(copy, rel)).equals(fs.readFileSync(path.join(skillDir, rel)))).toBe(true);
      }
    });
  }
});
