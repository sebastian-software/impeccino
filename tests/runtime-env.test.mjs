import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SOURCE_ROOTS = ['crates', 'scripts', 'skill/scripts', 'tests'];
const SOURCE_EXTENSIONS = new Set(['.rs', '.js', '.mjs', '.sh', '.cmd']);
const KNOWN_HOST_ENV = new Set([
  'AGENT_BROWSER_SESSION', 'ANTHROPIC_API_KEY', 'CARGO_ABOUT_BIN', 'CI_CHANGED_FILES',
  'COMSPEC', 'ComSpec', 'CURSOR_PROJECT_DIR', 'DEEPSEEK_API_KEY', 'DO_NOT_TRACK', 'GITHUB_BASE_REF',
  'GITHUB_EVENT_BEFORE', 'GITHUB_EVENT_INPUTS_SKILL_BEHAVIOR', 'GITHUB_EVENT_NAME',
  'GITHUB_OUTPUT', 'GITHUB_SHA', 'GOOGLE_CLOUD_API_KEY', 'GOOGLE_GENERATIVE_AI_API_KEY',
  'HOME', 'OPENAI_API_KEY', 'OPENCODE_CONFIG_DIR', 'PATH', 'PATHEXT',
  'PROCESSOR_ARCHITECTURE', 'SystemRoot', 'TEMP', 'TMPDIR', 'USERPROFILE', 'XDG_CONFIG_HOME',
]);
// These names are assigned by the launcher for its own child-process handshake.
const INTERNAL_ENV = new Set(['IMPECCINO_COOLDOWN_FILE', 'IMPECCINO_LAUNCHER_PROBE']);

function listSourceFiles(relativePath) {
  return fs.readdirSync(path.join(REPO_ROOT, relativePath), { withFileTypes: true }).flatMap((entry) => {
    const child = path.posix.join(relativePath, entry.name);
    if (entry.isDirectory()) {
      if (relativePath === 'tests' && ['fixtures', 'oracle'].includes(entry.name)) return [];
      return listSourceFiles(child);
    }
    if (child === 'tests/runtime-env.test.mjs') return [];
    return SOURCE_EXTENSIONS.has(path.extname(entry.name)) || child === 'skill/scripts/impeccino' ? [child] : [];
  });
}

function stripSlashComments(source, rust = false) {
  let result = '';
  for (let index = 0; index < source.length; index += 1) {
    const raw = rust && source.slice(index).match(/^r(#{0,})"/);
    if (raw) {
      const closing = `"${raw[1]}`;
      const end = source.indexOf(closing, index + raw[0].length);
      if (end !== -1) {
        result += ' ';
        index = end + closing.length - 1;
        continue;
      }
    }
    const quote = source[index];
    if (quote === '"' || (!rust && (quote === "'" || quote === '`'))) {
      const start = index;
      let escaped = false;
      for (index += 1; index < source.length; index += 1) {
        if (escaped) escaped = false;
        else if (source[index] === '\\') escaped = true;
        else if (source[index] === quote) break;
      }
      result += source.slice(start, index + 1);
      continue;
    }
    if (source.startsWith('//', index)) {
      const end = source.indexOf('\n', index);
      if (end === -1) break;
      result += '\n';
      index = end;
      continue;
    }
    if (source.startsWith('/*', index)) {
      const end = source.indexOf('*/', index + 2);
      if (end === -1) break;
      result += ' ';
      index = end + 1;
      continue;
    }
    result += source[index];
  }
  return result;
}

function stripShellComments(source) {
  return source.split('\n').map((line) => {
    let quote = '';
    let escaped = false;
    for (let index = 0; index < line.length; index += 1) {
      const current = line[index];
      if (escaped) escaped = false;
      else if (current === '\\' && quote !== "'") escaped = true;
      else if (quote && current === quote) quote = '';
      else if (!quote && (current === '"' || current === "'")) quote = current;
      else if (!quote && current === '#' && (index === 0 || /\s/.test(line[index - 1]))) return line.slice(0, index);
    }
    return line;
  }).join('\n');
}

function stripComments(source, extension) {
  if (extension === '.cmd') return source.split('\n').filter((line) => !/^\s*(?:rem\b|::)/i.test(line)).join('\n');
  if (extension === '.sh' || extension === '') return stripShellComments(source);
  return stripSlashComments(source, extension === '.rs');
}

function readEnvironmentNames(source, extension) {
  const code = stripComments(source, extension);
  const names = new Set();
  const patterns = [
    /\bstd::env::(?:var|var_os)\s*\(\s*"([A-Za-z_][A-Za-z0-9_]*)"/g,
    /\.(?:get|env|env_remove)\s*\(\s*"([A-Za-z_][A-Za-z0-9_]*)"/g,
    /\bprocess\.env\.([A-Za-z_][A-Za-z0-9_]*)/g,
    /\bprocess\.env\s*\[\s*["']([A-Za-z_][A-Za-z0-9_]*)["']\s*\]/g,
    /\benvKey\s*:\s*["']([A-Za-z_][A-Za-z0-9_]*)["']/g,
    /\$\{?([A-Za-z_][A-Za-z0-9_]*)/g,
    /%([A-Za-z_][A-Za-z0-9_]*)%/g,
  ];
  for (const pattern of patterns) {
    for (const match of code.matchAll(pattern)) names.add(match[1]);
  }
  const constants = new Map([...code.matchAll(/\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"(IMPECCINO_[A-Z0-9_]+)"/g)]
    .map((match) => [match[1], match[2]]));
  for (const [constant, name] of constants) {
    if (new RegExp(`std::env::(?:var|var_os)\\s*\\(\\s*${constant}\\s*\\)`).test(code)) names.add(name);
  }
  return names;
}

function trackedNames(names) {
  return new Set([...names].filter((name) =>
    (name.startsWith('IMPECCINO_') && !INTERNAL_ENV.has(name)) || KNOWN_HOST_ENV.has(name)));
}

function documentedNames(markdown) {
  return new Set([...markdown.matchAll(/`([A-Za-z_][A-Za-z0-9_]*)`/g)].map((match) => match[1]));
}

describe('runtime environment inventory', () => {
  it('covers source readers and excludes comment-only mentions', () => {
    const rust = [
      '// std::env::var("COMMENT_ONLY")',
      "fn example<'a>(value: &'a str) { let _ = value; }",
      'let _ = r###"// std::env::var("RAW_STRING_ONLY")"###;',
      'std::env::var("ACTUAL_RUST_READ");',
      'std::env::var_os("SystemRoot");',
    ].join('\n');
    expect([...readEnvironmentNames(rust, '.rs')].sort()).toEqual(['ACTUAL_RUST_READ', 'SystemRoot']);
    expect([...readEnvironmentNames('# ${COMMENT_ONLY}\necho ${ACTUAL_SHELL_READ}', '.sh')]).toEqual(['ACTUAL_SHELL_READ']);
    expect([...readEnvironmentNames('// process.env.COMMENT_ONLY\nprocess.env.ACTUAL_JS_READ;', '.mjs')]).toEqual(['ACTUAL_JS_READ']);

    const sourceNames = new Set();
    for (const root of SOURCE_ROOTS) {
      for (const file of listSourceFiles(root)) {
        const extension = path.extname(file);
        const source = fs.readFileSync(path.join(REPO_ROOT, file), 'utf8');
        for (const name of readEnvironmentNames(source, extension)) sourceNames.add(name);
      }
    }
    const documented = documentedNames(fs.readFileSync(path.join(REPO_ROOT, 'docs/RUNTIME-ENV.md'), 'utf8'));
    const missing = [...trackedNames(sourceNames)].filter((name) => !documented.has(name)).sort();
    expect(missing, `undocumented environment reads: ${missing.join(', ')}`).toEqual([]);
  });
});
