import fs from 'fs';
import path from 'path';

// The launcher fetches platform binaries into `scripts/bin/<os>-<arch>/`;
// they are a per-machine cache, not part of the source skill scripts.
const SKILL_BINARY_DIR = 'bin';

function readSkillScripts(scriptsDir) {
  const scripts = [];

  const walk = (dir) => {
    const entries = fs.readdirSync(dir, { withFileTypes: true })
      .sort((a, b) => a.name.localeCompare(b.name));

    for (const entry of entries) {
      const entryPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        if (dir === scriptsDir && entry.name === SKILL_BINARY_DIR) continue;
        walk(entryPath);
        continue;
      }
      if (!entry.isFile()) continue;

      const relPath = path.relative(scriptsDir, entryPath).split(path.sep).join('/');
      // Capture executable bits so tests can verify the launcher stays executable.
      scripts.push({
        name: relPath,
        content: fs.readFileSync(entryPath, 'utf-8'),
        filePath: entryPath,
        mode: fs.statSync(entryPath).mode & 0o777,
      });
    }
  };

  walk(scriptsDir);
  return scripts;
}

/**
 * Parse frontmatter from markdown content
 * Returns { frontmatter: object, body: string }
 */
export function parseFrontmatter(content) {
  const frontmatterRegex = /^---\r?\n([\s\S]*?)\r?\n---\r?\n([\s\S]*)$/;
  const match = content.match(frontmatterRegex);

  if (!match) {
    return { frontmatter: {}, body: content };
  }

  const [, frontmatterText, body] = match;
  const frontmatter = {};

  // Simple YAML parser (handles basic key-value and arrays)
  const lines = frontmatterText.split(/\r?\n/);
  let currentKey = null;
  let currentArray = null;

  for (const line of lines) {
    if (!line.trim()) continue;

    // Calculate indent level
    const leadingSpaces = line.length - line.trimStart().length;
    const trimmed = line.trim();

    // Array item at level 2 (nested under a key)
    if (trimmed.startsWith('- ') && leadingSpaces >= 2) {
      if (currentArray) {
        if (trimmed.startsWith('- name:')) {
          // New object in array
          const obj = {};
          obj.name = trimmed.slice(7).trim();
          currentArray.push(obj);
        } else {
          // Simple string item in array
          currentArray.push(trimmed.slice(2));
        }
      }
      continue;
    }

    // One-level map under a key with no inline value (e.g. `metadata:` then
    // `  version: 4.4.0`). The key starts out as an empty array; the first
    // `key: value` child turns it into an object.
    if (leadingSpaces === 2 && currentKey && !trimmed.startsWith('- ')) {
      const holder = frontmatter[currentKey];
      const colonIndex = trimmed.indexOf(':');
      if (colonIndex > 0 && (Array.isArray(holder) ? holder.length === 0 : typeof holder === 'object')) {
        const map = Array.isArray(holder) ? {} : holder;
        const raw = trimmed.slice(colonIndex + 1).trim();
        map[trimmed.slice(0, colonIndex).trim()] = /^(".*"|'.*')$/.test(raw) ? raw.slice(1, -1) : raw;
        frontmatter[currentKey] = map;
        currentArray = null;
        continue;
      }
    }

    // Property of array object (indented further)
    if (leadingSpaces >= 4 && currentArray && currentArray.length > 0) {
      const colonIndex = trimmed.indexOf(':');
      if (colonIndex > 0) {
        const key = trimmed.slice(0, colonIndex).trim();
        const value = trimmed.slice(colonIndex + 1).trim();
        const lastObj = currentArray[currentArray.length - 1];
        lastObj[key] = value === 'true' ? true : value === 'false' ? false : value;
      }
      continue;
    }

    // Top-level key-value pair
    if (leadingSpaces === 0) {
      const colonIndex = trimmed.indexOf(':');
      if (colonIndex > 0) {
        const key = trimmed.slice(0, colonIndex).trim();
        const value = trimmed.slice(colonIndex + 1).trim();
        const isQuoted = /^(".*"|'.*')$/.test(value);
        const unquotedValue = isQuoted ? value.slice(1, -1) : value;
        const shouldCoerceBoolean =
          key === 'user-invocable' || key === 'user-invokable' || !isQuoted;

        if (value) {
          frontmatter[key] = shouldCoerceBoolean
            ? unquotedValue === 'true'
              ? true
              : unquotedValue === 'false'
                ? false
                : unquotedValue
            : unquotedValue;
          currentKey = key;
          currentArray = null;
        } else {
          // Start of array
          currentKey = key;
          currentArray = [];
          frontmatter[key] = currentArray;
        }
      }
    }
  }

  return { frontmatter, body: body.trim() };
}

/**
 * Recursively read all .md files from a directory
 */
export function readFilesRecursive(dir, fileList = []) {
  if (!fs.existsSync(dir)) {
    return fileList;
  }

  const files = fs.readdirSync(dir);

  for (const file of files) {
    const filePath = path.join(dir, file);
    const stat = fs.statSync(filePath);

    if (stat.isDirectory()) {
      readFilesRecursive(filePath, fileList);
    } else if (file.endsWith('.md')) {
      fileList.push(filePath);
    }
  }

  return fileList;
}

/**
 * Read and parse the impeccino skill source.
 * The repo holds exactly one skill, flat at skill/. skill/ is also the
 * universal install payload, so it carries a real SKILL.md.
 * Returns { skills: [oneEntry] } so downstream array-shaped consumers stay happy.
 */
export function readSourceFiles(rootDir) {
  const skillDir = path.join(rootDir, 'skill');
  const skills = [];

  const skillMdPath = path.join(skillDir, 'SKILL.md');
  if (!fs.existsSync(skillMdPath)) {
    return { skills };
  }

  const content = fs.readFileSync(skillMdPath, 'utf-8');
  const { frontmatter, body } = parseFrontmatter(content);

  const references = [];
  const referenceDir = path.join(skillDir, 'reference');
  if (fs.existsSync(referenceDir)) {
    const refFiles = fs.readdirSync(referenceDir).filter(f => f.endsWith('.md'));
    for (const refFile of refFiles) {
      const refPath = path.join(referenceDir, refFile);
      references.push({
        name: path.basename(refFile, '.md'),
        content: fs.readFileSync(refPath, 'utf-8'),
        filePath: refPath
      });
    }
  }

  const scripts = [];
  const scriptsDir = path.join(skillDir, 'scripts');
  if (fs.existsSync(scriptsDir)) {
    scripts.push(...readSkillScripts(scriptsDir));
  }

  const agents = [];
  const agentsDir = path.join(skillDir, 'agents');
  if (fs.existsSync(agentsDir)) {
    const agentFiles = fs.readdirSync(agentsDir).filter(f => f.endsWith('.md'));
    for (const agentFile of agentFiles) {
      const agentPath = path.join(agentsDir, agentFile);
      const agentContent = fs.readFileSync(agentPath, 'utf-8');
      const { frontmatter: agentFrontmatter, body: agentBody } = parseFrontmatter(agentContent);
      const name = agentFrontmatter.name || path.basename(agentFile, '.md');
      agents.push({
        name,
        description: agentFrontmatter.description || '',
        tools: agentFrontmatter.tools || '',
        model: agentFrontmatter.model || '',
        effort: agentFrontmatter.effort || '',
        maxTurns: agentFrontmatter.maxTurns ? Number(agentFrontmatter.maxTurns) : '',
        body: agentBody,
        filePath: agentPath,
      });
    }
  }

  skills.push({
    name: frontmatter.name || 'impeccino',
    description: frontmatter.description || '',
    license: frontmatter.license || '',
    compatibility: frontmatter.compatibility || '',
    metadata: frontmatter.metadata || null,
    userInvocable: true,
    context: frontmatter.context || null,
    body,
    filePath: skillMdPath,
    references,
    scripts,
    agents
  });

  return { skills };
}
