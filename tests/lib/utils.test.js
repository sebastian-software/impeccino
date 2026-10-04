import { describe, test, expect, beforeEach, afterEach } from 'vitest';
import fs from 'fs';
import path from 'path';
import {
  parseFrontmatter,
  readFilesRecursive,
  readSourceFiles,
} from '../../scripts/lib/utils.js';

// Temporary test directory
const TEST_DIR = path.join(process.cwd(), 'test-tmp');

function mkdirp(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function writeFixture(filePath, content) {
  mkdirp(path.dirname(filePath));
  fs.writeFileSync(filePath, content, 'utf-8');
}

describe('parseFrontmatter', () => {
  test('should parse basic frontmatter with simple key-value pairs', () => {
    const content = `---
name: test-skill
description: A test skill
---

This is the body content.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter.name).toBe('test-skill');
    expect(result.frontmatter.description).toBe('A test skill');
    expect(result.body).toBe('This is the body content.');
  });

  test('should parse frontmatter with argument-hint', () => {
    const content = `---
name: test-skill
description: A test skill
argument-hint: <output> [TARGET=<value>]
---

Body here.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter.name).toBe('test-skill');
    expect(result.frontmatter['argument-hint']).toBe('<output> [TARGET=<value>]');
  });

  test('should return empty frontmatter when no frontmatter present', () => {
    const content = 'Just some content without frontmatter.';
    const result = parseFrontmatter(content);

    expect(result.frontmatter).toEqual({});
    expect(result.body).toBe(content);
  });

  test('should handle empty body', () => {
    const content = `---
name: test
---
`;
    const result = parseFrontmatter(content);

    expect(result.frontmatter.name).toBe('test');
    expect(result.body).toBe('');
  });

  test('should handle frontmatter with license field', () => {
    const content = `---
name: skill-name
description: A skill
license: MIT
---

Skill body.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter.license).toBe('MIT');
  });

  test('should parse user-invocable boolean', () => {
    const content = `---
name: test-skill
user-invocable: true
---

Body.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter['user-invocable']).toBe(true);
  });

  test('should parse quoted user-invocable boolean as true', () => {
    const content = `---
name: test-skill
user-invocable: 'true'
---

Body.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter['user-invocable']).toBe(true);
  });

  test('should keep quoted non-user-invocable booleans as plain strings', () => {
    const content = `---
name: test-skill
description: 'true'
---

Body.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter.description).toBe('true');
  });

  test('should parse allowed-tools field', () => {
    const content = `---
name: test-skill
allowed-tools: Bash
---

Body.`;

    const result = parseFrontmatter(content);
    expect(result.frontmatter['allowed-tools']).toBe('Bash');
  });
});

describe('readFilesRecursive', () => {
  beforeEach(() => {
    mkdirp(TEST_DIR);
  });

  afterEach(() => {
    if (fs.existsSync(TEST_DIR)) {
      fs.rmSync(TEST_DIR, { recursive: true, force: true });
    }
  });

  test('should find all markdown files in directory', () => {
    writeFixture(path.join(TEST_DIR, 'file1.md'), 'content1');
    writeFixture(path.join(TEST_DIR, 'file2.md'), 'content2');
    writeFixture(path.join(TEST_DIR, 'file3.txt'), 'not markdown');

    const files = readFilesRecursive(TEST_DIR);
    expect(files).toHaveLength(2);
    expect(files.some(f => f.endsWith('file1.md'))).toBe(true);
    expect(files.some(f => f.endsWith('file2.md'))).toBe(true);
  });

  test('should find markdown files in nested directories', () => {
    writeFixture(path.join(TEST_DIR, 'root.md'), 'root');
    writeFixture(path.join(TEST_DIR, 'sub', 'nested.md'), 'nested');
    writeFixture(path.join(TEST_DIR, 'sub', 'deep', 'deeper.md'), 'deeper');

    const files = readFilesRecursive(TEST_DIR);
    expect(files).toHaveLength(3);
    expect(files.some(f => f.endsWith('root.md'))).toBe(true);
    expect(files.some(f => f.endsWith('nested.md'))).toBe(true);
    expect(files.some(f => f.endsWith('deeper.md'))).toBe(true);
  });

  test('should return empty array for non-existent directory', () => {
    const files = readFilesRecursive(path.join(TEST_DIR, 'does-not-exist'));
    expect(files).toEqual([]);
  });

  test('should return empty array for directory with no markdown files', () => {
    writeFixture(path.join(TEST_DIR, 'file.txt'), 'text');
    writeFixture(path.join(TEST_DIR, 'file.js'), 'code');

    const files = readFilesRecursive(TEST_DIR);
    expect(files).toEqual([]);
  });
});

describe('readSourceFiles', () => {
  const testRootDir = TEST_DIR;

  beforeEach(() => {
    mkdirp(testRootDir);
  });

  afterEach(() => {
    if (fs.existsSync(testRootDir)) {
      fs.rmSync(testRootDir, { recursive: true, force: true });
    }
  });

  test('should read and parse SKILL.md from skill/', () => {
    const skillContent = `---
name: test-skill
description: A test skill
license: MIT
---

Skill instructions here.`;

    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);

    const { skills } = readSourceFiles(testRootDir);

    expect(skills).toHaveLength(1);
    expect(skills[0].name).toBe('test-skill');
    expect(skills[0].description).toBe('A test skill');
    expect(skills[0].license).toBe('MIT');
    expect(skills[0].body).toBe('Skill instructions here.');
  });

  test('should read skill with user-invocable flag', () => {
    const skillContent = `---
name: audit
description: Run technical quality checks
user-invocable: true
---

Audit the code.`;

    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);

    const { skills } = readSourceFiles(testRootDir);

    expect(skills).toHaveLength(1);
    expect(skills[0].userInvocable).toBe(true);
  });

  test('should read skill with quoted user-invocable flag', () => {
    const skillContent = `---
name: audit
description: Run technical quality checks
user-invocable: 'true'
---

Audit the code.`;

    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);

    const { skills } = readSourceFiles(testRootDir);

    expect(skills).toHaveLength(1);
    expect(skills[0].userInvocable).toBe(true);
  });

  test('should read skill with reference files', () => {
    const skillContent = `---
name: impeccino
description: Impeccino design skill
---

Impeccino design instructions.`;

    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);

    const refDir = path.join(skillDir, 'reference');
    mkdirp(refDir);
    fs.writeFileSync(path.join(refDir, 'typography.md'), 'Typography reference content.');
    fs.writeFileSync(path.join(refDir, 'color.md'), 'Color reference content.');

    const { skills } = readSourceFiles(testRootDir);

    expect(skills).toHaveLength(1);
    expect(skills[0].references).toHaveLength(2);
    // References may not be in a specific order due to fs.readdirSync
    const refNames = skills[0].references.map(r => r.name).sort();
    expect(refNames).toEqual(['color', 'typography']);
  });

  test('should fall back to "impeccino" when frontmatter has no name', () => {
    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), 'Just body, no frontmatter.');

    const { skills } = readSourceFiles(testRootDir);

    expect(skills).toHaveLength(1);
    expect(skills[0].name).toBe('impeccino');
  });

  test('should ignore non-md files in skill/reference', () => {
    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), '---\nname: test-skill\n---\nBody');

    const refDir = path.join(skillDir, 'reference');
    mkdirp(refDir);
    fs.writeFileSync(path.join(refDir, 'readme.txt'), 'Not a markdown file');
    fs.writeFileSync(path.join(refDir, 'typography.md'), 'Valid reference');

    const { skills } = readSourceFiles(testRootDir);

    expect(skills).toHaveLength(1);
    expect(skills[0].references).toHaveLength(1);
    expect(skills[0].references[0].name).toBe('typography');
  });

  test('should read nested skill script files with portable relative names', () => {
    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), '---\nname: test-skill\n---\nBody');

    const scriptsDir = path.join(skillDir, 'scripts');
    mkdirp(path.join(scriptsDir, 'live'));
    fs.writeFileSync(path.join(scriptsDir, 'context.mjs'), 'export const context = true;\n');
    fs.writeFileSync(path.join(scriptsDir, 'live/session-store.mjs'), 'export const nested = true;\n');

    const { skills } = readSourceFiles(testRootDir);
    const scripts = skills[0].scripts;
    const scriptNames = scripts.map(script => script.name).sort();

    expect(scriptNames).toEqual(['context.mjs', 'live/session-store.mjs']);
    expect(scripts.find(script => script.name === 'live/session-store.mjs').content).toContain('nested = true');
  });

  test('should handle missing skill directory', () => {
    const { skills } = readSourceFiles(testRootDir);
    expect(skills).toEqual([]);
  });

  test('should parse all frontmatter fields correctly', () => {
    const skillContent = `---
name: test-skill
description: A comprehensive test skill
license: Apache-2.0
compatibility: claude-code
metadata:
  version: 4.4.0
---

Body content.`;

    const skillDir = path.join(testRootDir, 'skill');
    mkdirp(skillDir);
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);

    const { skills } = readSourceFiles(testRootDir);

    expect(skills[0].name).toBe('test-skill');
    expect(skills[0].description).toBe('A comprehensive test skill');
    expect(skills[0].license).toBe('Apache-2.0');
    expect(skills[0].compatibility).toBe('claude-code');
    expect(skills[0].metadata).toEqual({ version: '4.4.0' });
  });
});
