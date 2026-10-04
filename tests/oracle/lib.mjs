/**
 * Oracle harness: records the observable behavior of every impeccino verb
 * (stdout, stderr, exit code, files written) against a fixed corpus, and
 * replays the same corpus against an alternate implementation to diff.
 *
 * Two implementations are addressable:
 *   - js  (default): the Node scripts in skill/scripts and cli/bin
 *   - bin: an executable at $IMPECCINO_BIN invoked as `<bin> <verb> ...args`
 *
 * A case is { id, verb, args, cwd?, stdin?, env?, files?, workspace? }:
 *   - workspace: name of a dir under tests/oracle/workspaces to copy into a
 *     temp dir and use as cwd (so writes never touch the repo)
 *   - cwd: subpath inside the staged workspace (default '.')
 *   - files: globs (relative to staged workspace) to snapshot after the run
 *   - args may contain <WS> and <REPO> placeholders
 *   - steps: multi-step cases share one staged workspace; a step may carry
 *     its own setup(ws) (run right before that step) and may set
 *     `daemon: true` to spawn its verb detached (see runDaemonStep) so later
 *     steps run against a live process; the daemon is killed after the last
 *     step and its captured output lands in the golden as `daemon`.
 *
 * Normalization replaces the staged workspace path with <WS>, the repo root
 * with <REPO>, $HOME with <HOME>, and masks ISO timestamps. Windows path
 * separators and CRLF are normalized only after these known roots are tagged.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';


export const ORACLE_DIR = path.dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = path.resolve(ORACLE_DIR, '..', '..');
export const GOLDEN_DIR = path.join(ORACLE_DIR, 'golden');
export const CASES_DIR = path.join(ORACLE_DIR, 'cases');
export const WORKSPACES_DIR = path.join(ORACLE_DIR, 'workspaces');

/** verb -> how the JS implementation is invoked */
export const JS_VERBS = {
  detect: ['node', path.join(REPO_ROOT, 'cli', 'bin', 'cli.js'), 'detect'],
  'cli-help': ['node', path.join(REPO_ROOT, 'cli', 'bin', 'cli.js'), '--help'],
  'cli-version': ['node', path.join(REPO_ROOT, 'cli', 'bin', 'cli.js'), '--version'],
  ignores: ['node', path.join(REPO_ROOT, 'cli', 'bin', 'cli.js'), 'ignores'],
};
for (const script of [
  'context', 'doctor', 'pin', 'surface-brief', 'palette',
  'context-signals', 'concept-seed',
  'hook', 'hook-before-edit', 'hook-admin',
]) {
  JS_VERBS[script] = ['node', path.join(REPO_ROOT, 'skill', 'scripts', `${script}.mjs`)];
}

/** verb -> argv for the binary implementation (verb name is the subcommand) */
export function binArgv(bin, verb) {
  if (verb === 'cli-help') return [bin, '--help'];
  if (verb === 'cli-version') return [bin, '--version'];
  return [bin, verb];
}

export async function allCases() {
  const out = [];
  for (const f of fs.readdirSync(CASES_DIR).sort()) {
    if (!f.endsWith('.mjs')) continue;
    const mod = await import(pathToFileURL(path.join(CASES_DIR, f)).href);
    const list = typeof mod.default === 'function' ? await mod.default() : mod.default;
    for (const item of Array.isArray(list) ? list : [list]) out.push({ ...item, sourceFile: f });
  }
  const seen = new Set();
  for (const c of out) {
    if (seen.has(c.id)) throw new Error(`duplicate oracle case id: ${c.id}`);
    seen.add(c.id);
  }
  return out;
}

export function stageWorkspace(name) {
  // realpath, so every path the verbs see and every <WS> the harness passes
  // (env, args, lock files) is the same string. macOS's tmpdir is a symlink
  // (/var -> /private/var); without this, goldens recorded there carried
  // symlink artifacts (`../../../../../../..<WS>/...` relative paths, lock
  // files that never matched their own file) that Linux does not reproduce.
  const tmp = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-oracle-')));
  if (name) {
    const src = path.join(WORKSPACES_DIR, name);
    if (!fs.existsSync(src)) throw new Error(`oracle workspace not found: ${name}`);
    fs.cpSync(src, tmp, { recursive: true });
  }
  return tmp;
}

// Replace `needle` only where it ends a path segment: at the end of the text
// or followed by anything but a name character (a separator, quote, dot,
// whitespace, JSON punctuation).
// A short home directory (`/root` in a container) is otherwise a substring of
// ordinary words, and `live/roots.json` came out as `live<HOME>s.json`.
function maskPath(text, needle, tag) {
  let out = '';
  let i = 0;
  while (i < text.length) {
    const j = text.indexOf(needle, i);
    if (j === -1) break;
    out += text.slice(i, j);
    const after = text[j + needle.length];
    const boundary = after === undefined || !/[A-Za-z0-9_-]/.test(after);
    out += boundary ? tag : needle;
    i = j + needle.length;
  }
  return out + text.slice(i);
}

function normalizeTaggedPathSeparators(text) {
  // JSON and quoted human-readable paths can contain spaces, so consume their
  // known-root suffix through a paired quote immediately surrounding the tag.
  // The unquoted pass below stops at the first token boundary.
  const quoted = text.replace(/(["'])(<(?:WS|REPO|HOME)>)([^"'\r\n]*)\1/g, (whole, quote, root, suffix) => {
    if (!/^[/\\]/.test(suffix)) return whole;
    const tag = root.slice(1, -1);
    return quote + '<' + tag + '>' + suffix.replace(/\\{1,2}/g, '/') + quote;
  });
  return quoted.replace(/<(WS|REPO|HOME)>([^\r\n]*)/g, (line, tag, suffix) => {
    const boundary = suffix.search(/[\s"'`<>),;\]}]/);
    const pathPart = boundary === -1 ? suffix : suffix.slice(0, boundary);
    if (!/^[/\\]/.test(pathPart)) return line;
    const rest = boundary === -1 ? '' : suffix.slice(boundary);
    return `<${tag}>${pathPart.replace(/\\{1,2}/g, '/')}${rest}`;
  });
}

function normalizeKnownWindowsJsonPathFields(text) {
  return text.replace(/("(?:productPath|designPath|surfaceBriefPath|path|file)"\s*:\s*)("(?:\\.|[^"\\])*")/g, (match, fieldText, encodedValue) => {
    const field = fieldText.match(/"([^"]+)"/)?.[1];
    let value;
    try { value = JSON.parse(encodedValue); } catch { return match; }
    if (typeof value !== 'string' || !value.includes('\\')) return match;

    const portable = value.replaceAll('\\', '/');
    const isRelative = !/^(?:[A-Za-z]:[\\/]|[\\/]{1,2}|[A-Za-z][A-Za-z0-9+.-]*:\/\/)/.test(value);
    const isKnownHiddenPath = /(?:^|\/)\.impeccino\/(?:config(?:\.local)?\.json|design\.json|hook\.cache\.json|surfaces\/|critique\/|live\/)/.test(portable) ||
      /(?:^|\/)\.claude\/(?:settings\.local\.json|hooks\.json(?:\.bak)?)/.test(portable) ||
      /(?:^|\/)\.cursor\/hooks\.json(?:\.bak)?/.test(portable);
    const isPrimaryDesignDocument = field === 'path' && isRelative && /(?:^|\/)(?:PRODUCT|DESIGN)\.md$/.test(portable);
    if (field === 'productPath' || field === 'designPath' ? !isRelative : !isKnownHiddenPath && !isPrimaryDesignDocument) return match;
    return `${fieldText}${JSON.stringify(portable)}`;
  });
}

function normalizeKnownWindowsDiagnosticSummaries(text) {
  return text.replace(/("summary"\s*:\s*)("(?:\\.|[^"\\])*")/g, (match, fieldText, encodedValue) => {
    let value;
    try { value = JSON.parse(encodedValue); } catch { return match; }
    const marker = value.match(/^\d+ persisted surface brief\(s\) name a primary target that no longer exists: /);
    if (!marker) return match;
    const remainder = value.slice(marker[0].length);
    // Keep this normalization limited to the known doctor diagnostic and its
    // generated .impeccino/surfaces/<slug>.md path.
    const surfacePath = remainder.match(/^(\.impeccino(?:\\+|\/+)surfaces(?:\\+|\/+)[^\\/\s→]+\.md)(\s→\s[\s\S]*)$/);
    if (!surfacePath) return match;
    const normalized = `${marker[0]}${surfacePath[1].replaceAll('\\', '/')}${surfacePath[2]}`;
    return `${fieldText}${JSON.stringify(normalized)}`;
  });
}

function isInsideQuotedOrCodeText(text, index) {
  let doubleQuoted = false;
  let inCode = false;
  let escaped = false;
  for (let i = 0; i < index; i++) {
    const char = text[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if ((doubleQuoted || inCode) && char === '\\') {
      escaped = true;
      continue;
    }
    if (char === '"' && !inCode) doubleQuoted = !doubleQuoted;
    else if (char === '`' && !doubleQuoted) inCode = !inCode;
  }
  return doubleQuoted || inCode;
}

function normalizeWindowsPathOutput(text, caseId) {
  if (!text) return text;
  const match = text.match(/^([^\r\n]+)(\r?\n?)$/);
  if (!match || !/(?:^|[\\/])SURFACES\.md$/.test(match[1])) {
    throw new Error(`Expected ${caseId}'s Windows stdout to be one surface path line`);
  }
  const [, pathLine, lineEnding] = match;
  const portablePath = caseId === 'surface-brief-path-slash'
    ? pathLine.replace(/^(?:\.\.\\){2,}(?=SURFACES\.md$)/, '<UP_TO_ROOT>/')
    : pathLine;
  return portablePath.replaceAll('\\', '/') + lineEnding;
}

function normalizeKnownWindowsHiddenPaths(text) {
  const contracts = {
    '.impeccino': /^(?:config(?:\.local)?\.json|design\.json|hook\.cache\.json|surfaces|critique|live)$/,
    '.claude': /^(?:settings\.local\.json|hooks\.json(?:\.bak)?)$/,
    '.cursor': /^hooks\.json(?:\.bak)?$/,
  };
  const pathPattern = /(^|[^A-Za-z0-9_.-])(\.impeccino|\.claude|\.cursor)(\\{1,2}|\/)([^"'`()[\]{}<>\s,;]+)/g;
  return text.replace(pathPattern, (match, boundary, root, separator, suffix, offset) => {
    if (isInsideQuotedOrCodeText(text, offset + boundary.length)) return match;
    const punctuation = suffix.match(/[.!?]+$/)?.[0] || '';
    const pathSuffix = punctuation ? suffix.slice(0, -punctuation.length) : suffix;
    const firstSegment = pathSuffix.match(/^[^\\/]+/)?.[0];
    if (!firstSegment || !contracts[root].test(firstSegment)) return match;
    return `${boundary}${root}${separator}${pathSuffix}`.replace(/\\{1,2}/g, '/') + punctuation;
  });
}

function maskJsonEscapedHookAdminCommand(text, binaryPath) {
  if (!binaryPath) return text;
  const pathForms = new Set([
    binaryPath,
    binaryPath.replaceAll('\\', '/'),
    binaryPath.replaceAll('/', '\\'),
  ]);
  const commandForms = new Set(pathForms);
  for (const form of pathForms) commandForms.add(form.replaceAll('\\', '\\\\'));
  let out = text;
  for (const form of commandForms) {
    const encodedCommand = JSON.stringify(`"${form}" hooks`).slice(1, -1);
    const escapedCommand = encodedCommand.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    out = out.replace(new RegExp(`${escapedCommand}(?![A-Za-z0-9_-])`, 'g'), '<HOOK_ADMIN_CMD>');
  }
  return out;
}

export function normalize(text, {
  ws,
  home = os.homedir(),
  windowsPowerShellGuidance = false,
  platform = process.platform,
  binaryPath = process.env.IMPECCINO_BIN,
  caseId,
  pathOutput = false,
}) {
  if (typeof text !== 'string') return text;
  let out = text.replaceAll('\r\n', '\n');
  // The hook footer embeds the admin command: JS prints "node '<scripts>/hook-admin.mjs'",
  // the binary prints "'<bin>' hooks". Both collapse to <HOOK_ADMIN_CMD>. This must run
  // before the generic binary-path mask below.
  out = out.replace(/node '[^']*\/hook-admin\.mjs'/g, '<HOOK_ADMIN_CMD>');
  out = out.replace(/node "[^"]*\/hook-admin\.mjs"/g, '<HOOK_ADMIN_CMD>');
  out = out.replace(/'[^']*\/impeccino(?:\.exe)?' hooks/g, '<HOOK_ADMIN_CMD>');
  out = out.replace(/"[^"]*\\impeccino(?:\.exe)?" hooks/g, '<HOOK_ADMIN_CMD>');
  // Audit entries record the rendered message length, which includes that command's path.
  out = out.replace(/"chars":\s*\d+/g, '"chars": <N>');
  // The Windows engine quotes and JSON-escapes its hook-admin command path.
  // Collapse only the exact binary path followed by the `hooks` verb, before
  // the generic binary path mask can hide part of that command.
  if (platform === 'win32') out = maskJsonEscapedHookAdminCommand(out, binaryPath);
  // The binary's own path: it may sit under $HOME or the repo.
  if (binaryPath) {
    const bin = binaryPath;
    if (platform === 'win32' && windowsPowerShellGuidance) {
      const quotedCommand = `"${bin}" detect http://localhost:`;
      out = out.split(quotedCommand).join('<WINDOWS_QUOTED_IMPECCINO> detect http://localhost:');
    }
    for (const form of [`'${bin}'`, `"${bin}"`, bin]) out = out.split(form).join('<IMPECCINO>');
  }
  let wsReal = null;
  try { wsReal = ws ? fs.realpathSync(ws) : null; } catch { /* staged dir already gone */ }
  for (const [needle, tag] of [
    [wsReal, '<WS>'], [ws, '<WS>'], [REPO_ROOT, '<REPO>'], [home, '<HOME>'],
  ]) {
    if (!needle) continue;
    const spellings = new Set([needle]);
    if (needle.includes('\\')) {
      spellings.add(needle.replaceAll('\\', '/'));
      spellings.add(needle.replaceAll('\\', '\\\\'));
    }
    for (const spelling of [...spellings].sort((a, b) => b.length - a.length)) {
      out = maskPath(out, spelling, tag);
    }
  }
  // Keep path separator normalization scoped to roots already identified
  // above. Other backslashes in output (including Windows shell guidance)
  // remain observable.
  out = normalizeTaggedPathSeparators(out);
  if (platform === 'win32') {
    out = normalizeKnownWindowsJsonPathFields(out);
    out = normalizeKnownWindowsDiagnosticSummaries(out);
    if (pathOutput) out = normalizeWindowsPathOutput(out, caseId);
    out = normalizeKnownWindowsHiddenPaths(out);
  }
  // Self-referential command lines: the JS prints "node <scripts>/<verb>.mjs", the
  // binary prints "<bin> <verb>". Both collapse to "<IMPECCINO> <verb>".
  out = out.replace(/node ['"]?<REPO>\/skill\/scripts\/([a-z-]+)\.mjs['"]?/g, (m, v) => `<IMPECCINO> ${v === 'context-signals' ? 'signals' : v === 'hook-admin' ? 'hooks' : v}`);
  // context.mjs probes `which cwebp/sips/magick/ffmpeg`; the set found is a
  // property of the recording machine, not of the implementation.
  out = out.replace(/IMAGE_TOOLS: available image converters on this machine: [^.]*\. Use the first suitable one; never probe again this session\./g, 'IMAGE_TOOLS: <IMAGE_TOOLS_PROBE>');
  out = out.replace(/IMAGE_TOOLS: no image converter found \(cwebp, sips, magick, ffmpeg\)\. Ship PNG output unconverted rather than probing per image\./g, 'IMAGE_TOOLS: <IMAGE_TOOLS_PROBE>');
  // A target given as an absolute path outside the workspace makes the verb
  // print the surface path relative to the root, which climbs as many levels
  // as the staged tmpdir is deep (7 on macOS, 2 on Linux). The climb is a
  // property of the machine, not of the verb.
  out = out.replace(/(?:\.\.\/){2,}(?=SURFACES\.md)/g, '<UP_TO_ROOT>/');
  // Hook state lives in a per-project dir of the user cache, named after the
  // project's absolute path plus an 8-hex digest of it (docs/adr/0020).
  out = normalizeProjectCacheDir(out);
  // Hook audit entries carry wall-clock durations.
  out = out.replace(/"durationMs":\s*\d+(?:\.\d+)?/g, '"durationMs": <MS>');
  // ISO timestamps and epoch millis are run-dependent.
  out = out.replace(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z/g, '<ISO>');
  out = out.replace(/"(updatedAt|createdAt|checkedAt|lastCheck|lastChecked|timestamp|ts|mtimeMs|mtime|startedAt|endedAt)":\s*\d{10,}/g, '"$1": <EPOCH>');
  // The staleness notice cache (<user cache>/impeccino/staleness-check.json) keys epoch
  // stamps by finding id: { projects: { "<root>": { "<finding-id>": ms } } }.
  out = out.replace(/"([a-z][a-z0-9-]*)":\s*1[6-9]\d{11}(?=[,}\s])/g, '"$1": <EPOCH>');
  // Live mode: server.json, the inject journal, and source locks record the
  // writing process's pid; the helper server mints a UUID token. Both vary
  // per run. Ports and lease stamps are per-case (see `normalize` on a case).
  out = out.replace(/"pid":(\s*)\d+/g, '"pid":$1<PID>');
  out = out.replace(/\(pid \d+\)/g, '(pid <PID>)');
  out = out.replace(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi, '<UUID>');
  return out;
}

/** `<cache>/impeccino/projects/<slug>-<hash>/` -> `.../projects/<PROJECT>/`. */
export function normalizeProjectCacheDir(text, labels = new Map()) {
  return text.replace(/(impeccino[\\/]projects[\\/])[^\\/\s"'`]+-([0-9a-f]{8})(?=[\\/])/g,
    (_match, prefix, digest) => `${prefix}<PROJECT${labels.has(digest) ? `:${labels.get(digest)}` : ''}>`);
}

/**
 * Normalize file snapshot paths and fail loudly if the normalizer would
 * discard one file by mapping two real paths to the same golden key.
 */
export function normalizeSnapshotFiles(files, labels = new Map(), caseId = 'oracle case') {
  const normalized = {};
  for (const [file, contents] of Object.entries(files)) {
    const key = normalizeProjectCacheDir(file, labels);
    if (Object.hasOwn(normalized, key)) {
      throw new Error(`normalized file snapshot collision in ${caseId}: ${key}`);
    }
    normalized[key] = contents;
  }
  return normalized;
}

function projectCacheLabels(c, ws) {
  if (!c.projectCacheRoots) return new Map();
  const labels = new Map();
  for (const [label, root] of Object.entries(c.projectCacheRoots)) {
    const resolved = path.resolve(ws, root);
    const digest = createHash('sha256').update(resolved).digest('hex').slice(0, 8);
    if (labels.has(digest)) {
      throw new Error(`project cache label collision in ${c.id}: ${labels.get(digest)} and ${label}`);
    }
    labels.set(digest, label);
  }
  return labels;
}

/**
 * Case-scoped replacements: `c.normalize` is a list of [regexSource, flags,
 * replacement] applied after the global pass to stdout, stderr, files, and
 * daemon output of that case only. Keeps run-dependent values that only one
 * flow produces (a dynamically chosen server port, lease deadlines) from
 * widening the global normalizer and masking real diffs elsewhere.
 */
function applyCaseNormalizers(text, rules) {
  if (typeof text !== 'string' || !rules?.length) return text;
  let out = text;
  for (const [src, flags, repl] of rules) out = out.replace(new RegExp(src, flags), repl);
  return out;
}

function globToRegex(glob) {
  let re = "";
  for (let i = 0; i < glob.length; i++) {
    const ch = glob[i];
    if (ch === "*") {
      if (glob[i + 1] === "*") {
        i++;
        if (glob[i + 1] === "/") { i++; re += "(?:.*/)?"; } else re += ".*";
      } else re += "[^/]*";
    } else if (ch === "?") re += "[^/]";
    else re += ch.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp("^" + re + "$");
}

export function replaceOraclePlaceholders(value, { ws, repo = REPO_ROOT }) {
  if (typeof value === 'string') {
    return value.replaceAll('<WS>', ws).replaceAll('<REPO>', repo);
  }
  if (Array.isArray(value)) return value.map((item) => replaceOraclePlaceholders(item, { ws, repo }));
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [
      replaceOraclePlaceholders(key, { ws, repo }),
      replaceOraclePlaceholders(item, { ws, repo }),
    ]));
  }
  return value;
}

export function serializeOracleStdin(stdin, paths) {
  if (typeof stdin === 'string') {
    return replaceOraclePlaceholders(stdin, paths);
  }
  return stdin == null ? '' : JSON.stringify(replaceOraclePlaceholders(stdin, paths));
}

export function snapshotFiles(ws, globs) {
  const out = {};
  if (!globs || !globs.length) return out;
  const regs = globs.map(globToRegex);
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, ent.name);
      const rel = path.relative(ws, full).split(path.sep).join('/');
      if (ent.isDirectory()) {
        if (ent.name === '.git') continue;
        if (ent.name === 'node_modules') {
          // Only the live-mode preview tree is ours; never walk installed or
          // symlinked packages.
          const preview = path.join(full, '.impeccino-live');
          if (fs.existsSync(preview)) walk(preview);
          continue;
        }
        walk(full);
      } else if (regs.some(r => r.test(rel))) {
        const buf = fs.readFileSync(full);
        out[rel] = isProbablyText(buf) ? buf.toString('utf8') : `<binary ${buf.length} bytes>`;
      }
    }
  };
  walk(ws);
  return Object.fromEntries(Object.entries(out).sort(([a], [b]) => a.localeCompare(b)));
}

function isProbablyText(buf) {
  const n = Math.min(buf.length, 512);
  for (let i = 0; i < n; i++) if (buf[i] === 0) return false;
  return true;
}

/**
 * Run one case with the given implementation ('js' | 'bin').
 * Returns { stdout, stderr, exit, signal, files } normalized.
 */
/**
 * A case may declare `platforms: ['darwin', 'win32']` when its behavior is a
 * property of the host (case-insensitive file systems, for example) rather
 * than of the implementation. Such a case runs only on those platforms; the
 * runner reports it as skipped elsewhere instead of failing.
 */
export function caseRunsHere(c, platform = process.platform) {
  return !Array.isArray(c.platforms) || c.platforms.includes(platform);
}

/**
 * The engine uses native Windows cache paths and deliberately prints a
 * PowerShell note and quoted launcher command for framework scans. Keep
 * these contracts in explicit expectations beside the shared POSIX golden.
 */
export function expectedForPlatform(c, golden, platform = process.platform) {
  if (platform !== 'win32') return golden;
  const expected = structuredClone(golden);
  // The isolated user cache follows the engine's native Windows fallback.
  // Keep the actual storage location observable, with explicit expectations
  // for cache-file snapshots and the engine's displayed absolute cache paths.
  const windowsCachePath = (text) => text.replaceAll(
    '<WS>/.oracle-home/.cache/impeccino/',
    '<WS>/.oracle-home/AppData/Local/impeccino/',
  );
  for (const result of expected.steps || [expected]) {
    for (const stream of ['stdout', 'stderr']) {
      if (typeof result[stream] === 'string') result[stream] = windowsCachePath(result[stream]);
    }
  }
  if (expected.files) {
    expected.files = Object.fromEntries(Object.entries(expected.files).map(([key, value]) => [
      key.replace(/^\.oracle-home\/\.cache\/impeccino\//, '.oracle-home/AppData/Local/impeccino/'),
      value,
    ]));
  }
  if (c.windowsClaudeExecForm && typeof expected.files?.['.claude/settings.local.json'] === 'string') {
    const key = '.claude/settings.local.json';
    const settings = JSON.parse(expected.files[key]);
    const sharedCommand = 'if [ -x "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" ]; then "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" hook; fi';
    const launcher = '<WS>/.claude/skills/impeccino/scripts/impeccino.cmd';
    const args = ['-NoProfile', '-Command', `if (Test-Path -LiteralPath '${launcher}' -PathType Leaf) { & '${launcher}' hook }`];
    let replacements = 0;
    const rewrite = (value) => {
      if (Array.isArray(value)) return value.map(rewrite);
      if (!value || typeof value !== 'object') return value;
      if (value.command === sharedCommand) {
        replacements++;
        return Object.fromEntries(Object.entries(value).flatMap(([field, item]) => field === 'command'
          ? [['command', 'powershell.exe'], ['args', args]]
          : [[field, rewrite(item)]]));
      }
      return Object.fromEntries(Object.entries(value).map(([field, item]) => [field, rewrite(item)]));
    };
    const rewritten = rewrite(settings);
    if (replacements === 0) {
      throw new Error(`Expected generated Claude launcher entries in ${c.id}'s shared golden`);
    }
    expected.files[key] = JSON.stringify(rewritten, null, 2) + '\n';
  }
  if (c.windowsPowerShellGuidance) {
    const guidance = 'In PowerShell, prefix the quoted launcher path with `&`.';
    let totalReplacements = 0;
    for (const stream of ['stdout', 'stderr']) {
      if (typeof expected[stream] !== 'string') continue;
      expected[stream] = expected[stream].replace(
        /(^[ \t]*)<IMPECCINO> detect (http:\/\/localhost:\d+)\n\n/gm,
        (_match, indent, url) => {
          totalReplacements++;
          return `${indent}<WINDOWS_QUOTED_IMPECCINO> detect ${url}\n${guidance}\n\n`;
        },
      );
    }
    if (totalReplacements !== 1) {
      throw new Error(`Expected one framework launcher command in ${c.id}'s shared golden; found ${totalReplacements}`);
    }
  }

  if (c.windowsDrivePathQuoteField) {
    const field = c.windowsDrivePathQuoteField;
    const quotePath = (text) => {
      if (typeof text !== 'string') return { text, replacements: 0 };
      let replacements = 0;
      const pattern = new RegExp(`(^[ \\t]*${field}:[ \\t]*)(<WS>\\/[^\\r\\n]+)$`, 'gm');
      return {
        text: text.replace(pattern, (_match, prefix, value) => {
          replacements++;
          return `${prefix}"${value}"`;
        }),
        replacements,
      };
    };
    let totalReplacements = 0;
    if (Array.isArray(expected.steps)) {
      for (const step of expected.steps) {
        for (const stream of ['stdout', 'stderr']) {
          if (typeof step[stream] !== 'string') continue;
          const result = quotePath(step[stream]);
          step[stream] = result.text;
          totalReplacements += result.replacements;
        }
      }
    } else {
      for (const stream of ['stdout', 'stderr']) {
        if (typeof expected[stream] !== 'string') continue;
        const result = quotePath(expected[stream]);
        expected[stream] = result.text;
        totalReplacements += result.replacements;
      }
    }
    if (totalReplacements !== 1) {
      throw new Error(`Expected one ${field} path in ${c.id}'s shared golden; found ${totalReplacements}`);
    }
  }

  if (c.windowsQuotedIgnoreValue) {
    const { rule, value } = c.windowsQuotedIgnoreValue;
    const sharedHint = `ignore-value ${rule} '${value}'`;
    const windowsHint = `ignore-value ${rule} "${value}"`;
    const lineEnding = expected.stdout.match(/(?:\r?\n)+$/)?.[0] || '';
    const sharedOutput = expected.stdout.slice(0, expected.stdout.length - lineEnding.length);
    let payload;
    try { payload = JSON.parse(sharedOutput); } catch {
      throw new Error(`Expected ${c.id}'s shared golden to contain JSON hook output`);
    }
    const context = payload?.hookSpecificOutput?.additionalContext;
    const occurrences = typeof context === 'string' ? context.split(sharedHint).length - 1 : 0;
    if (occurrences !== 1) {
      throw new Error(`Expected one ${rule} quoted-value example in ${c.id}'s shared golden; found ${occurrences}`);
    }
    payload.hookSpecificOutput.additionalContext = context.replace(sharedHint, windowsHint);
    expected.stdout = JSON.stringify(payload) + lineEnding;
  }
  return expected;
}

export function assertRecordableCases(cases, platform = process.platform) {
  const windowsExpectationCases = platform === 'win32'
    ? cases.filter((c) => c.windowsPowerShellGuidance || c.windowsDrivePathQuoteField || c.windowsQuotedIgnoreValue || c.windowsClaudeExecForm).map((c) => c.id)
    : [];
  if (windowsExpectationCases.length) {
    throw new Error(
      `Cannot record shared oracle goldens on Windows for ${windowsExpectationCases.join(', ')}. ` +
      'Record these cases on Linux or macOS; Windows output is checked against an explicit platform expectation.',
    );
  }
}

export function runCase(c, { impl = 'js', bin = process.env.IMPECCINO_BIN } = {}) {
  const ws = stageWorkspace(c.workspace);
  try {
    const isolatedHome = path.join(ws, '.oracle-home');
    if (c.isolateHome !== false) fs.mkdirSync(isolatedHome, { recursive: true });
    if (typeof c.setup === 'function') c.setup(ws);
    const steps = c.steps || [c];
    const results = [];
    const daemons = [];
    try {
      for (const step of steps) {
        const merged = { ...c, ...step, verb: step.verb || c.verb };
        // A step-level setup stages state between verbs (e.g. the agent's
        // variant files between wrap and accept).
        if (c.steps && typeof step.setup === 'function') step.setup(ws);
        if (step.daemon) {
          daemons.push(runDaemonStep(merged, { impl, bin, ws, isolatedHome }));
          results.push({ stdout: '', stderr: '', status: null, signal: null, daemon: true });
          continue;
        }
        results.push(runStep(merged, { impl, bin, ws, isolatedHome }));
      }
    } finally {
      for (const d of daemons) stopDaemon(d);
    }
    const files = snapshotFiles(ws, c.files);
    const ctx = { ws };
    const N = (text, { pathOutput = false } = {}) => applyCaseNormalizers(normalize(text, {
      ...ctx,
      binaryPath: bin,
      windowsPowerShellGuidance: c.windowsPowerShellGuidance,
      caseId: c.id,
      pathOutput,
    }), c.normalize);
    const norm = (r, step) => ({
      stdout: N(r.stdout ?? '', { pathOutput: step?.windowsPathOutput || (!c.steps && c.windowsPathOutput) }),
      stderr: N(r.stderr ?? ''),
      exit: r.status,
      signal: r.signal || null,
      ...(r.daemon ? { daemon: true } : {}),
    });
    const labels = projectCacheLabels(c, ws);
    const normalizedFiles = normalizeSnapshotFiles(files, labels, c.id);
    const filesNorm = Object.fromEntries(Object.entries(normalizedFiles).map(([k, v]) => [k, N(v)]));
    const daemonOut = daemons.length
      ? { daemon: daemons.map((d) => ({ stdout: N(d.stdout()), stderr: N(d.stderr()) })) }
      : {};
    if (c.steps) return { steps: results.map((result, index) => norm(result, c.steps[index])), files: filesNorm, ...daemonOut };
    return { ...norm(results[0]), files: filesNorm, ...daemonOut };
  } finally {
    fs.rmSync(ws, { recursive: true, force: true });
  }
}

function buildInvocation(c, { impl, bin, ws, isolatedHome }) {
  const cwd = path.join(ws, c.cwd || '.');
  let argv;
  if (impl === 'js') {
    const base = JS_VERBS[c.verb];
    if (!base) throw new Error(`no JS invocation for verb ${c.verb}`);
    argv = [...base];
  } else {
    if (!bin) throw new Error('IMPECCINO_BIN not set');
    argv = binArgv(bin, c.verb);
  }
  const sub = (v) => String(v).replaceAll('<WS>', ws).replaceAll('<REPO>', REPO_ROOT);
  argv.push(...(c.args || []).map(sub));
  const env = {
    ...process.env,
    NO_COLOR: '1',
    FORCE_COLOR: '0',
    IMPECCINO_NO_UPDATE_CHECK: '1',
    // Context reports a missing agent-browser; any existing file counts as
    // installed, so goldens do not depend on the recording machine's PATH.
    IMPECCINO_AGENT_BROWSER: process.execPath,
    // The per-user cache (hook state, the staleness throttle) follows the
    // isolated home: XDG_CACHE_HOME and LOCALAPPDATA from the recording
    // machine must not leak in.
    ...(c.isolateHome === false ? {} : { HOME: isolatedHome, USERPROFILE: isolatedHome, XDG_CACHE_HOME: null, LOCALAPPDATA: null }),
    // What the launcher exports for the binary (see launcher/impeccino in the engine repo).
    ...(impl === 'bin' ? { IMPECCINO_SKILL_DIR: path.join(REPO_ROOT, 'skill'), IMPECCINO_SELF: bin } : {}),
    ...Object.fromEntries(Object.entries(c.env || {}).map(([k, v]) => [k, v == null ? v : sub(v)])),
  };
  for (const [k, v] of Object.entries(env)) if (v == null) delete env[k];
  const stdin = serializeOracleStdin(c.stdin, { ws, repo: REPO_ROOT });
  return { argv, cwd, env, stdin };
}

/**
 * Spawn a step's verb detached and wait until `readyFile` (relative to the
 * staged workspace) exists. stdout/stderr go to files under
 * <ws>/.oracle-daemon/ and are read back at teardown so the golden records
 * what the daemon printed over its whole life.
 */
function runDaemonStep(c, opts) {
  const { argv, cwd, env } = buildInvocation(c, opts);
  const outDir = path.join(opts.ws, '.oracle-daemon');
  fs.mkdirSync(outDir, { recursive: true });
  const n = fs.readdirSync(outDir).length;
  const outPath = path.join(outDir, `${n}.stdout`);
  const errPath = path.join(outDir, `${n}.stderr`);
  const outFd = fs.openSync(outPath, 'w');
  const errFd = fs.openSync(errPath, 'w');
  const child = (spawn(argv[0], argv.slice(1), {
    cwd, env, stdio: ['ignore', outFd, errFd], detached: true, windowsHide: true,
  }));
  fs.closeSync(outFd);
  fs.closeSync(errFd);
  const readyFile = path.join(opts.ws, c.readyFile);
  const deadline = Date.now() + (c.readyTimeoutMs || 10_000);
  while (!fs.existsSync(readyFile)) {
    if (child.exitCode !== null || Date.now() > deadline) break;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 25);
  }
  if (!fs.existsSync(readyFile)) {
    throw new Error(`daemon ${c.verb} for case ${c.id} did not create ${c.readyFile}\n${safeRead(errPath)}`);
  }
  return {
    child,
    stdout: () => safeRead(outPath),
    stderr: () => safeRead(errPath),
  };
}

function stopDaemon(d) {
  const { child } = d;
  if (child.exitCode !== null || child.signalCode) return;
  try { child.kill('SIGTERM'); } catch { /* already gone */ }
  const deadline = Date.now() + 3000;
  while (child.exitCode === null && !child.signalCode && Date.now() < deadline) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 25);
    // A detached child never reports exit to a synchronous loop; probe the pid.
    try { process.kill(child.pid, 0); } catch { break; }
  }
  try { process.kill(child.pid, 0); child.kill('SIGKILL'); } catch { /* exited */ }
  // Give the OS a beat to release the port and flush the output files.
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 50);
}

function safeRead(p) {
  try { return fs.readFileSync(p, 'utf8'); } catch { return ''; }
}

function runStep(c, opts) {
  const { argv, cwd, env, stdin } = buildInvocation(c, opts);
  return spawnSync(argv[0], argv.slice(1), {
    cwd, env, input: stdin, encoding: 'utf8', timeout: c.timeoutMs || 60_000,
    windowsHide: true, maxBuffer: 64 * 1024 * 1024,
  });
}

export function goldenPath(id) {
  return path.join(GOLDEN_DIR, `${id}.json`);
}

export function writeGolden(id, result) {
  fs.mkdirSync(GOLDEN_DIR, { recursive: true });
  fs.writeFileSync(goldenPath(id), JSON.stringify(result, null, 2) + '\n');
}

export function readGolden(id) {
  const p = goldenPath(id);
  if (!fs.existsSync(p)) return null;
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}

/** Return a list of human-readable differences, empty if equal. */
export function diffResults(golden, actual) {
  const diffs = [];
  if (golden.steps || actual.steps) {
    const g = golden.steps || [], a = actual.steps || [];
    if (g.length !== a.length) diffs.push(`steps: expected ${g.length}, got ${a.length}`);
    for (let i = 0; i < Math.min(g.length, a.length); i++) {
      for (const d of diffResults({ ...g[i], files: {} }, { ...a[i], files: {} })) diffs.push(`step ${i + 1} ${d}`);
    }
    for (const d of diffResults({ files: golden.files, exit: 0, signal: null, stdout: '', stderr: '' }, { files: actual.files, exit: 0, signal: null, stdout: '', stderr: '' })) diffs.push(d);
    const gd = golden.daemon || [], ad = actual.daemon || [];
    if (gd.length !== ad.length) diffs.push(`daemons: expected ${gd.length}, got ${ad.length}`);
    for (let i = 0; i < Math.min(gd.length, ad.length); i++) {
      for (const k of ['stdout', 'stderr']) {
        if (gd[i][k] !== ad[i][k]) diffs.push(`daemon ${i + 1} ${k} differs:\n${firstDiff(gd[i][k], ad[i][k])}`);
      }
    }
    return diffs;
  }
  for (const k of ['exit', 'signal']) {
    if (golden[k] !== actual[k]) diffs.push(`${k}: expected ${golden[k]}, got ${actual[k]}`);
  }
  for (const k of ['stdout', 'stderr']) {
    if (golden[k] !== actual[k]) diffs.push(`${k} differs:\n${firstDiff(golden[k], actual[k])}`);
  }
  const keys = new Set([...Object.keys(golden.files || {}), ...Object.keys(actual.files || {})]);
  for (const k of [...keys].sort()) {
    const g = golden.files?.[k], a = actual.files?.[k];
    if (g === undefined) diffs.push(`file ${k}: unexpected (written by actual only)`);
    else if (a === undefined) diffs.push(`file ${k}: missing (golden has it)`);
    else if (g !== a) diffs.push(`file ${k} differs:\n${firstDiff(g, a)}`);
  }
  return diffs;
}

function firstDiff(a, b) {
  const al = String(a).split('\n'), bl = String(b).split('\n');
  const n = Math.max(al.length, bl.length);
  for (let i = 0; i < n; i++) {
    if (al[i] !== bl[i]) {
      return `  line ${i + 1}\n  - ${JSON.stringify(al[i] ?? '<EOF>')}\n  + ${JSON.stringify(bl[i] ?? '<EOF>')}`;
    }
  }
  return '  (lengths differ)';
}
