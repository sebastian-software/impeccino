import { describe, test, expect, beforeEach, afterEach, spyOn } from 'bun:test';
import fs from 'fs';
import path from 'path';
import * as utils from '../scripts/lib/utils.js';
import * as transformers from '../scripts/lib/transformers/index.js';

const TEST_DIR = path.join(process.cwd(), 'test-tmp-build');

describe('build orchestration', () => {
  beforeEach(() => {
    if (fs.existsSync(TEST_DIR)) {
      fs.rmSync(TEST_DIR, { recursive: true, force: true });
    }
    fs.mkdirSync(TEST_DIR, { recursive: true });
  });

  afterEach(() => {
    if (fs.existsSync(TEST_DIR)) {
      fs.rmSync(TEST_DIR, { recursive: true, force: true });
    }
  });

  test('should call readSourceFiles with root directory', () => {
    const readSourceFilesSpy = spyOn(utils, 'readSourceFiles').mockReturnValue({
      skills: []
    });

    const transformCursorSpy = spyOn(transformers, 'transformCursor').mockImplementation(() => {});
    const transformClaudeCodeSpy = spyOn(transformers, 'transformClaudeCode').mockImplementation(() => {});
    const transformGeminiSpy = spyOn(transformers, 'transformGemini').mockImplementation(() => {});
    const transformCodexSpy = spyOn(transformers, 'transformCodex').mockImplementation(() => {});

    // Simulate the build process
    const ROOT_DIR = TEST_DIR;
    const DIST_DIR = path.join(ROOT_DIR, 'dist');

    const { skills } = utils.readSourceFiles(ROOT_DIR);
    const patterns = utils.readPatterns(ROOT_DIR);
    transformers.transformCursor(skills, DIST_DIR, patterns);
    transformers.transformClaudeCode(skills, DIST_DIR, patterns);
    transformers.transformGemini(skills, DIST_DIR, patterns);
    transformers.transformCodex(skills, DIST_DIR, patterns);

    expect(readSourceFilesSpy).toHaveBeenCalledWith(ROOT_DIR);

    readSourceFilesSpy.mockRestore();
    transformCursorSpy.mockRestore();
    transformClaudeCodeSpy.mockRestore();
    transformGeminiSpy.mockRestore();
    transformCodexSpy.mockRestore();
  });

  test('should call all transformers with correct arguments', () => {
    const skills = [
      { name: 'skill1', description: 'Skill 1', license: 'MIT', body: 'Skill body 1' }
    ];
    const patterns = { patterns: [], antipatterns: [] };

    const readSourceFilesSpy = spyOn(utils, 'readSourceFiles').mockReturnValue({
      skills
    });
    const readPatternsSpy = spyOn(utils, 'readPatterns').mockReturnValue(patterns);

    const transformCursorSpy = spyOn(transformers, 'transformCursor').mockImplementation(() => {});
    const transformClaudeCodeSpy = spyOn(transformers, 'transformClaudeCode').mockImplementation(() => {});
    const transformGeminiSpy = spyOn(transformers, 'transformGemini').mockImplementation(() => {});
    const transformCodexSpy = spyOn(transformers, 'transformCodex').mockImplementation(() => {});

    const ROOT_DIR = TEST_DIR;
    const DIST_DIR = path.join(ROOT_DIR, 'dist');

    const sourceFiles = utils.readSourceFiles(ROOT_DIR);
    const patternData = utils.readPatterns(ROOT_DIR);
    transformers.transformCursor(sourceFiles.skills, DIST_DIR, patternData);
    transformers.transformClaudeCode(sourceFiles.skills, DIST_DIR, patternData);
    transformers.transformGemini(sourceFiles.skills, DIST_DIR, patternData);
    transformers.transformCodex(sourceFiles.skills, DIST_DIR, patternData);

    expect(transformCursorSpy).toHaveBeenCalledWith(skills, DIST_DIR, patterns);
    expect(transformClaudeCodeSpy).toHaveBeenCalledWith(skills, DIST_DIR, patterns);
    expect(transformGeminiSpy).toHaveBeenCalledWith(skills, DIST_DIR, patterns);
    expect(transformCodexSpy).toHaveBeenCalledWith(skills, DIST_DIR, patterns);

    readSourceFilesSpy.mockRestore();
    readPatternsSpy.mockRestore();
    transformCursorSpy.mockRestore();
    transformClaudeCodeSpy.mockRestore();
    transformGeminiSpy.mockRestore();
    transformCodexSpy.mockRestore();
  });

  test('should handle empty source files', () => {
    const patterns = { patterns: [], antipatterns: [] };

    const readSourceFilesSpy = spyOn(utils, 'readSourceFiles').mockReturnValue({
      skills: []
    });
    const readPatternsSpy = spyOn(utils, 'readPatterns').mockReturnValue(patterns);

    const transformCursorSpy = spyOn(transformers, 'transformCursor').mockImplementation(() => {});
    const transformClaudeCodeSpy = spyOn(transformers, 'transformClaudeCode').mockImplementation(() => {});
    const transformGeminiSpy = spyOn(transformers, 'transformGemini').mockImplementation(() => {});
    const transformCodexSpy = spyOn(transformers, 'transformCodex').mockImplementation(() => {});

    const ROOT_DIR = TEST_DIR;
    const DIST_DIR = path.join(ROOT_DIR, 'dist');

    const { skills } = utils.readSourceFiles(ROOT_DIR);
    const patternData = utils.readPatterns(ROOT_DIR);
    transformers.transformCursor(skills, DIST_DIR, patternData);
    transformers.transformClaudeCode(skills, DIST_DIR, patternData);
    transformers.transformGemini(skills, DIST_DIR, patternData);
    transformers.transformCodex(skills, DIST_DIR, patternData);

    expect(transformCursorSpy).toHaveBeenCalledWith([], DIST_DIR, patterns);
    expect(transformClaudeCodeSpy).toHaveBeenCalledWith([], DIST_DIR, patterns);
    expect(transformGeminiSpy).toHaveBeenCalledWith([], DIST_DIR, patterns);
    expect(transformCodexSpy).toHaveBeenCalledWith([], DIST_DIR, patterns);

    readSourceFilesSpy.mockRestore();
    readPatternsSpy.mockRestore();
    transformCursorSpy.mockRestore();
    transformClaudeCodeSpy.mockRestore();
    transformGeminiSpy.mockRestore();
    transformCodexSpy.mockRestore();
  });

  test('integration: full build creates all expected outputs', () => {
    // Create test source files
    const skillContent = `---
name: test-skill
description: A test skill
license: MIT
---

This is a test skill body.`;

    const skillDir = path.join(TEST_DIR, 'skill');
    fs.mkdirSync(skillDir, { recursive: true });
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);

    // Run the build process
    const DIST_DIR = path.join(TEST_DIR, 'dist');
    const { skills } = utils.readSourceFiles(TEST_DIR);
    const patterns = utils.readPatterns(TEST_DIR);

    transformers.transformCursor(skills, DIST_DIR, patterns);
    transformers.transformClaudeCode(skills, DIST_DIR, patterns);
    transformers.transformGemini(skills, DIST_DIR, patterns);
    transformers.transformCodex(skills, DIST_DIR, patterns);
    transformers.transformAntigravity(skills, DIST_DIR, patterns);

    // Verify Cursor outputs
    expect(fs.existsSync(path.join(DIST_DIR, 'cursor/.cursor/skills/test-skill/SKILL.md'))).toBe(true);

    // Verify Claude Code outputs
    expect(fs.existsSync(path.join(DIST_DIR, 'claude-code/.claude/skills/test-skill/SKILL.md'))).toBe(true);

    // Verify Gemini outputs
    expect(fs.existsSync(path.join(DIST_DIR, 'gemini/.gemini/skills/test-skill/SKILL.md'))).toBe(true);

    // Verify Codex outputs
    expect(fs.existsSync(path.join(DIST_DIR, 'codex/.codex/skills/test-skill/SKILL.md'))).toBe(true);

    // Verify Antigravity outputs
    expect(fs.existsSync(path.join(DIST_DIR, 'antigravity/.agent/skills/test-skill/SKILL.md'))).toBe(true);
  });

  test('integration: emits native subagent files for Claude Code, GitHub Copilot, and Cursor', () => {
    const skillContent = `---
name: test-skill
description: A test skill
---

This is a test skill body.`;

    const agentContent = `---
name: asset-producer
description: Produces assets from approved crops
tools: Read, Write
model: inherit
effort: medium
maxTurns: 8
---

Do not redesign the approved crop.`;

    const skillDir = path.join(TEST_DIR, 'skill');
    fs.mkdirSync(path.join(skillDir, 'agents'), { recursive: true });
    fs.writeFileSync(path.join(skillDir, 'SKILL.md'), skillContent);
    fs.writeFileSync(path.join(skillDir, 'agents/asset-producer.md'), agentContent);

    const DIST_DIR = path.join(TEST_DIR, 'dist');
    const { skills } = utils.readSourceFiles(TEST_DIR);
    const patterns = utils.readPatterns(TEST_DIR);

    transformers.transformClaudeCode(skills, DIST_DIR, patterns);
    transformers.transformCodex(skills, DIST_DIR, patterns);
    transformers.transformGitHub(skills, DIST_DIR, patterns);
    transformers.transformCursor(skills, DIST_DIR, patterns);

    const claudeAgentPath = path.join(DIST_DIR, 'claude-code/.claude/agents/asset-producer.md');
    // GitHub Copilot discovers repo-level custom agents at .github/agents/<name>.agent.md.
    const copilotAgentPath = path.join(DIST_DIR, 'github/.github/agents/asset-producer.agent.md');
    // Cursor discovers repo-level subagents at .cursor/agents/<name>.md.
    const cursorAgentPath = path.join(DIST_DIR, 'cursor/.cursor/agents/asset-producer.md');

    expect(fs.existsSync(claudeAgentPath)).toBe(true);
    expect(fs.existsSync(copilotAgentPath)).toBe(true);
    expect(fs.existsSync(cursorAgentPath)).toBe(true);

    const claudeAgent = fs.readFileSync(claudeAgentPath, 'utf-8');
    expect(claudeAgent).toContain('name: asset-producer');
    expect(claudeAgent).toContain('tools: Read, Write');
    expect(claudeAgent).toContain('maxTurns: 8');

    // Copilot's portable frontmatter is name + description only: omitting
    // `tools` grants access to all tools, and there are no documented
    // model/effort/max-turns equivalents.
    const copilotAgent = fs.readFileSync(copilotAgentPath, 'utf-8');
    expect(copilotAgent).toContain('name: asset-producer');
    expect(copilotAgent).toContain('description: Produces assets from approved crops');
    expect(copilotAgent).toContain('Do not redesign the approved crop.');
    expect(copilotAgent).not.toContain('tools:');
    expect(copilotAgent).not.toContain('model:');
    expect(copilotAgent).not.toContain('effort:');
    expect(copilotAgent).not.toContain('maxTurns:');

    // Cursor keeps model (inherit maps directly) and derives readonly from the
    // tool list; this agent carries Write, so no readonly field is emitted.
    const cursorAgent = fs.readFileSync(cursorAgentPath, 'utf-8');
    expect(cursorAgent).toContain('name: asset-producer');
    expect(cursorAgent).toContain('description: Produces assets from approved crops');
    expect(cursorAgent).toContain('model: inherit');
    expect(cursorAgent).toContain('is_background: false');
    expect(cursorAgent).toContain('Do not redesign the approved crop.');
    expect(cursorAgent).not.toContain('readonly:');
    expect(cursorAgent).not.toContain('tools:');
    expect(cursorAgent).not.toContain('effort:');
    expect(cursorAgent).not.toContain('maxTurns:');
  });

  test('should call transformers in correct order', () => {
    const callOrder = [];

    const readSourceFilesSpy = spyOn(utils, 'readSourceFiles').mockReturnValue({
      skills: []
    });
    const readPatternsSpy = spyOn(utils, 'readPatterns').mockReturnValue({ patterns: [], antipatterns: [] });

    const transformCursorSpy = spyOn(transformers, 'transformCursor').mockImplementation(() => {
      callOrder.push('cursor');
    });
    const transformClaudeCodeSpy = spyOn(transformers, 'transformClaudeCode').mockImplementation(() => {
      callOrder.push('claude-code');
    });
    const transformGeminiSpy = spyOn(transformers, 'transformGemini').mockImplementation(() => {
      callOrder.push('gemini');
    });
    const transformCodexSpy = spyOn(transformers, 'transformCodex').mockImplementation(() => {
      callOrder.push('codex');
    });

    const ROOT_DIR = TEST_DIR;
    const DIST_DIR = path.join(ROOT_DIR, 'dist');

    const { skills } = utils.readSourceFiles(ROOT_DIR);
    const patterns = utils.readPatterns(ROOT_DIR);
    transformers.transformCursor(skills, DIST_DIR, patterns);
    transformers.transformClaudeCode(skills, DIST_DIR, patterns);
    transformers.transformGemini(skills, DIST_DIR, patterns);
    transformers.transformCodex(skills, DIST_DIR, patterns);

    expect(callOrder).toEqual(['cursor', 'claude-code', 'gemini', 'codex']);

    readSourceFilesSpy.mockRestore();
    readPatternsSpy.mockRestore();
    transformCursorSpy.mockRestore();
    transformClaudeCodeSpy.mockRestore();
    transformGeminiSpy.mockRestore();
    transformCodexSpy.mockRestore();
  });

  test('should include agents and kiro transformers', () => {
    const { skills } = utils.readSourceFiles(TEST_DIR);
    const patterns = utils.readPatterns(TEST_DIR);
    const DIST_DIR = path.join(TEST_DIR, 'dist');

    // These should not throw
    transformers.transformAgents(skills, DIST_DIR, patterns);
    transformers.transformGitHub(skills, DIST_DIR, patterns);
    transformers.transformKiro(skills, DIST_DIR, patterns);

    // Verify outputs
    expect(fs.existsSync(path.join(DIST_DIR, 'agents/.agents/skills'))).toBe(true);
    expect(fs.existsSync(path.join(DIST_DIR, 'github/.github/skills'))).toBe(true);
    expect(fs.existsSync(path.join(DIST_DIR, 'kiro/.kiro/skills'))).toBe(true);
  });

  test('Antigravity transformer emits skills under .agent/', () => {
    const { skills } = utils.readSourceFiles(TEST_DIR);
    const patterns = utils.readPatterns(TEST_DIR);
    const DIST_DIR = path.join(TEST_DIR, 'dist');

    // Should not throw
    transformers.transformAntigravity(skills, DIST_DIR, patterns);

    // Verify the harness directory is created at the correct path
    expect(fs.existsSync(path.join(DIST_DIR, 'antigravity/.agent/skills'))).toBe(true);
  });
});

// The skill's scripts dir ships the launcher, its Windows twin, the pinned
// engine VERSION, the page JS, and command-metadata.json. Nothing else: the
// verbs live in the engine binary the launcher runs, and platform binaries
// (scripts/bin/) are fetched per machine, never read as source.
describe('skill scripts payload', () => {
  const ROOT_DIR = process.cwd();
  const { skills } = utils.readSourceFiles(ROOT_DIR);
  const scripts = skills[0]?.scripts ?? [];
  const names = new Set(scripts.map((s) => s.name));

  test('ships the launcher, VERSION, page JS, and command metadata', () => {
    for (const expected of [
      'impeccable', 'impeccable.cmd', 'VERSION', 'command-metadata.json',
      'live-browser.js', 'live-browser-dom.js', 'live-browser-session.js', 'modern-screenshot.umd.js',
    ]) {
      expect(names.has(expected)).toBe(true);
    }
  });

  test('ships no engine entry points and no bundled detector', () => {
    // The engine verbs live in the binary; the only Node scripts allowed in
    // the payload are the comp-fidelity build pipeline and its libs, which
    // have not moved into the engine yet.
    const allowedNodeScripts = new Set([
      'build-phase.mjs',
      'comp-diff.mjs',
      'comp-spec.mjs',
      'font-match.mjs',
      'lib/font-fingerprint.mjs',
      'lib/font-index.mjs',
      'lib/hero-checks.mjs',
      'lib/image-metrics.mjs',
      'lib/png.mjs',
      'lib/raster.mjs',
    ]);
    const stray = [...names].filter((n) =>
      (n.endsWith('.mjs') || n.startsWith('detector/') || n.startsWith('lib/')) && !allowedNodeScripts.has(n));
    expect(stray).toEqual([]);
  });

  test('never reads platform binaries as source', () => {
    expect([...names].filter((n) => n.startsWith('bin/'))).toEqual([]);
  });

  test('the launcher is executable and VERSION matches ENGINE_VERSION', () => {
    const launcher = scripts.find((s) => s.name === 'impeccable');
    expect(launcher.mode & 0o111).not.toBe(0);
    const version = scripts.find((s) => s.name === 'VERSION');
    expect(version.content.trim()).toBe(fs.readFileSync(path.join(ROOT_DIR, 'ENGINE_VERSION'), 'utf-8').trim());
  });
});

describe('GitHub Copilot custom agent generation', () => {
  const ROOT = process.cwd();
  const COPILOT_TEST_DIR = path.join(ROOT, 'test-tmp-copilot-agents');
  const DIST = path.join(COPILOT_TEST_DIR, 'dist');
  const AGENTS_DIR = path.join(DIST, 'github', '.github', 'agents');

  beforeEach(() => {
    if (fs.existsSync(COPILOT_TEST_DIR)) fs.rmSync(COPILOT_TEST_DIR, { recursive: true, force: true });
    fs.mkdirSync(COPILOT_TEST_DIR, { recursive: true });
    const { skills } = utils.readSourceFiles(ROOT);
    transformers.transformGitHub(skills, DIST);
  });

  afterEach(() => {
    if (fs.existsSync(COPILOT_TEST_DIR)) fs.rmSync(COPILOT_TEST_DIR, { recursive: true, force: true });
  });

  test('emits .github/agents/<name>.agent.md for every shipped agent', () => {
    const files = fs.readdirSync(AGENTS_DIR).sort();
    expect(files).toEqual([
      'impeccable-asset-producer.agent.md',
      'impeccable-documenter.agent.md',
      'impeccable-finish-reviewer.agent.md',
      'impeccable-manual-edit-applier.agent.md',
    ]);
  });

  test('frontmatter carries only name and description, description verbatim from the source', () => {
    const source = fs.readFileSync(path.join(ROOT, 'skill', 'agents', 'impeccable-finish-reviewer.md'), 'utf-8');
    const sourceDescription = source.match(/^description:\s*(.+)$/m)[1].trim();

    const content = fs.readFileSync(path.join(AGENTS_DIR, 'impeccable-finish-reviewer.agent.md'), 'utf-8');
    const frontmatter = content.split('---')[1];
    expect(frontmatter).toContain('name: impeccable-finish-reviewer');
    expect(frontmatter).toContain(`description: ${sourceDescription}`);
    // Copilot has no documented equivalents for these, and omitting `tools`
    // grants access to all tools; only portable fields are emitted.
    expect(frontmatter).not.toContain('tools:');
    expect(frontmatter).not.toContain('model:');
    expect(frontmatter).not.toContain('effort:');
    expect(frontmatter).not.toContain('maxTurns:');
    expect(frontmatter).not.toContain('nickname');
  });

});

describe('Cursor subagent generation', () => {
  const ROOT = process.cwd();
  const CURSOR_TEST_DIR = path.join(ROOT, 'test-tmp-cursor-agents');
  const DIST = path.join(CURSOR_TEST_DIR, 'dist');
  const AGENTS_DIR = path.join(DIST, 'cursor', '.cursor', 'agents');

  beforeEach(() => {
    if (fs.existsSync(CURSOR_TEST_DIR)) fs.rmSync(CURSOR_TEST_DIR, { recursive: true, force: true });
    fs.mkdirSync(CURSOR_TEST_DIR, { recursive: true });
    const { skills } = utils.readSourceFiles(ROOT);
    transformers.transformCursor(skills, DIST);
  });

  afterEach(() => {
    if (fs.existsSync(CURSOR_TEST_DIR)) fs.rmSync(CURSOR_TEST_DIR, { recursive: true, force: true });
  });

  test('emits .cursor/agents/<name>.md for every shipped agent', () => {
    const files = fs.readdirSync(AGENTS_DIR).sort();
    expect(files).toEqual([
      'impeccable-asset-producer.md',
      'impeccable-documenter.md',
      'impeccable-finish-reviewer.md',
      'impeccable-manual-edit-applier.md',
    ]);
  });

  test('frontmatter maps name, description, model inherit, is_background false; readonly only on the reviewer', () => {
    for (const name of fs.readdirSync(AGENTS_DIR)) {
      const content = fs.readFileSync(path.join(AGENTS_DIR, name), 'utf-8');
      const frontmatter = content.split('---')[1];
      expect(frontmatter).toContain(`name: ${name.replace(/\.md$/, '')}`);
      expect(frontmatter).toContain('description: ');
      expect(frontmatter).toContain('model: inherit');
      expect(frontmatter).toContain('is_background: false');
      // Cursor's effort option requires an explicit model id, incompatible
      // with inherit, and our tool names are not Cursor's vocabulary.
      expect(frontmatter).not.toContain('tools:');
      expect(frontmatter).not.toContain('effort:');
      expect(frontmatter).not.toContain('maxTurns:');
      // The finish reviewer is the only role whose tool list has no Write or
      // Edit; it reviews, the other three write.
      if (name === 'impeccable-finish-reviewer.md') {
        expect(frontmatter).toContain('readonly: true');
      } else {
        expect(frontmatter).not.toContain('readonly:');
      }
    }
  });

});

// Regression guard for the gap that shipped literal `{{scripts_path}}` inside
// the Codex dists' nested agent .toml: three separate code paths emit an agent
// body, and one of them skipped placeholder substitution and rule-marker
// stripping. Assert every surface, not just the one that was broken.

describe('universal skill source', () => {
  const ROOT = process.cwd();
  const SKILL_DIR = path.join(ROOT, 'skill');
  const markdown = utils.readFilesRecursive(SKILL_DIR);

  test('carries a real SKILL.md with only Agent Skills spec frontmatter', () => {
    const { frontmatter } = utils.parseFrontmatter(fs.readFileSync(path.join(SKILL_DIR, 'SKILL.md'), 'utf-8'));
    // Codex rejects unknown top-level keys; every harness reads these.
    const spec = new Set(['name', 'description', 'license', 'compatibility', 'metadata']);
    for (const key of Object.keys(frontmatter)) expect(spec.has(key)).toBe(true);
    expect(frontmatter.metadata.version).toBeTruthy();
  });

  test('leaves no build-time placeholders or provider blocks', () => {
    for (const file of markdown) {
      const text = fs.readFileSync(file, 'utf-8');
      expect(text).not.toMatch(/\{\{[a-z_]+\}\}/);
      expect(text).not.toMatch(/^[ \t]*<\/?(claude|claude-code|codex|cursor|gemini)>[ \t]*$/m);
    }
  });

  test('never hardcodes one harness install path for the launcher', () => {
    for (const file of markdown) {
      const text = fs.readFileSync(file, 'utf-8');
      expect(text).not.toMatch(/\.(claude|agents|cursor|github)\/skills\/impeccable\/scripts\/impeccable /);
    }
  });

  test('agents get the scripts path from the parent, not from SKILL.md', () => {
    for (const file of fs.readdirSync(path.join(SKILL_DIR, 'agents'))) {
      expect(fs.readFileSync(path.join(SKILL_DIR, 'agents', file), 'utf-8')).not.toContain('<skill-base-dir>');
    }
  });
});
