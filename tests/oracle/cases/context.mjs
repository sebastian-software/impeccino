/**
 * Corpus for the context-and-helper verbs: `context`, `doctor`, `pin`,
 * `surface-brief`, `palette`, `context-signals`, `concept-seed`, and the
 * removed `critique-storage`.
 *
 * Workspaces (tests/oracle/workspaces/ctx-*):
 *   ctx-empty          package.json only
 *   ctx-visual-only    index.html + src/*.css, no PRODUCT.md
 *   ctx-product-only   stamped PRODUCT.md (web), visual code, no DESIGN.md
 *   ctx-full           PRODUCT.md + DESIGN.md + DESIGN.json v2 + SURFACES.md with two briefs
 *   ctx-native-ios     PRODUCT.md `## Platform` ios, no visual code
 *   ctx-adaptive       PRODUCT.md `## Platform` "ios, android"
 *   ctx-bad-platform   PRODUCT.md `## Platform` flutter + pubspec.yaml
 *   ctx-monorepo       pnpm-workspace + apps/a (own PRODUCT/DESIGN) + apps/b (inherits)
 *   ctx-legacy         unstamped PRODUCT.md with ## Register, DESIGN.json sidecar v1, a DESIGN.md
 *                      waiver naming an unknown rule, an orphan brief in SURFACES.md, and a
 *                      leftover `.impeccino/` from the layout before docs/adr/0020
 *   ctx-signals        git-initialised in setup() with fixed author/committer dates
 *   ctx-pin            .claude/.agents/.cursor skills dirs with impeccino installed
 *
 * Only offline, deterministic paths are exercised: no listening server. Env
 * vars that would change behaviour on the recording machine (OPENAI_API_KEY,
 * context overrides, CI) are pinned per case through BASE_ENV.
 */

import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { execFileSync } from 'node:child_process';

const WS = '<WS>';

// Env the recording machine may carry that would leak into output.
const BASE_ENV = {
  OPENAI_API_KEY: null,
  IMPECCINO_CONTEXT_DIR: null,
  IMPECCINO_STALENESS_CACHE: null,
  IMPECCINO_UPDATE_CACHE: null,
  IMPECCINO_NO_STALENESS_CHECK: null,
  IMPECCINO_HOOK_DISABLED: null,
  IMPECCINO_PROVIDER_ID: null,
  OPENCODE_CONFIG_DIR: null,
  XDG_CONFIG_HOME: null,
  IMPECCINO_PALETTE_SEED: null,
  IMPECCINO_CONCEPT_SEED: null,
  IMPECCINO_IMAGE_GEN_FAKE: null,
  IMPECCINO_QUESTION_DISABLED: null,
  IMPECCINO_QUESTION_FORCE: null,
  CI: null,
  SSH_CONNECTION: null,
};
const env = (extra = {}) => ({ ...BASE_ENV, ...extra });

const IMPECCINO_FILES = ['SURFACES.md', 'PRODUCT.md', 'DESIGN.md', 'DESIGN.json', '.impeccino/**'];

// ---- setup helpers ---------------------------------------------------------

const write = (ws, rel, body) => {
  const abs = path.join(ws, rel);
  fs.mkdirSync(path.dirname(abs), { recursive: true });
  fs.writeFileSync(abs, body);
  return abs;
};

// A `.git` directory marking a repository boundary. Written at run time so
// the fixture tree stays a plain directory in this repo.
const gitBoundary = (ws, rel) => {
  fs.mkdirSync(path.join(ws, rel, '.git'), { recursive: true });
};

// An installed Claude Code Stop hook in the launcher spelling, which is what
// `context` counts as active coverage.
const claudeStopHook = (ws, rel) => write(
  ws,
  path.join(rel, '.claude/settings.local.json'),
  JSON.stringify({
    hooks: { Stop: [{ hooks: [{ command: '.claude/skills/impeccino/scripts/impeccino hook' }] }] },
  }) + '\n',
);

// Fixed mtimes so DESIGN.md-vs-sidecar age comparisons never depend on copy
// order or filesystem timestamp granularity.
const T_OLD = new Date('2026-01-01T00:00:00Z');
const T_NEW = new Date('2026-06-01T00:00:00Z');
const touch = (abs, when) => { if (fs.existsSync(abs)) fs.utimesSync(abs, when, when); };
const sidecarNewer = (ws) => { touch(path.join(ws, 'DESIGN.md'), T_OLD); touch(path.join(ws, 'DESIGN.json'), T_NEW); };
const sidecarOlder = (ws) => { touch(path.join(ws, 'DESIGN.md'), T_NEW); touch(path.join(ws, 'DESIGN.json'), T_OLD); };

// ctx-legacy carries a DESIGN.md newer than its sidecar.
const legacySetup = (ws) => sidecarOlder(ws);

// What an install from before docs/adr/0020 left at a project root.
const leftoverStateDir = (ws) => {
  write(ws, '.impeccino/config.json', JSON.stringify({ detector: { ignoreRules: ['side-tab'] } }, null, 2) + '\n');
  write(ws, '.impeccino/design.json', JSON.stringify({ schemaVersion: 2 }) + '\n');
  write(ws, '.impeccino/surfaces/src-app-tsx.md', '---\nprimary_target: "src/App.tsx"\n---\n\n# Surface brief: App\n');
};

const GIT_ENV = {
  GIT_AUTHOR_NAME: 'Oracle', GIT_AUTHOR_EMAIL: 'oracle@example.com', GIT_AUTHOR_DATE: '2026-01-02T03:04:05Z',
  GIT_COMMITTER_NAME: 'Oracle', GIT_COMMITTER_EMAIL: 'oracle@example.com', GIT_COMMITTER_DATE: '2026-01-02T03:04:05Z',
  GIT_CONFIG_NOSYSTEM: '1', HOME: '/nonexistent-home',
};
const git = (ws, ...args) => execFileSync('git', ['-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', '-c', 'init.defaultBranch=main', ...args], { cwd: ws, env: { ...process.env, ...GIT_ENV }, stdio: 'ignore' });
const gitInit = (ws) => {
  git(ws, 'init', '-q');
  git(ws, 'symbolic-ref', 'HEAD', 'refs/heads/main');
  git(ws, 'add', '.');
  git(ws, 'commit', '-qm', 'init');
};
const gitDirty = (ws) => { gitInit(ws); fs.appendFileSync(path.join(ws, 'src/styles.css'), 'p { margin: 0; }\n'); write(ws, 'src/New.tsx', 'export const New = () => null;\n'); };
const gitFeature = (ws) => {
  gitInit(ws);
  git(ws, 'checkout', '-qb', 'feature/hero');
  fs.appendFileSync(path.join(ws, 'src/App.tsx'), '// feature\n');
  write(ws, 'src/util.ts', 'export const add = (a: number, b: number) => a + b + 0;\n');
  git(ws, 'add', '.');
  git(ws, 'commit', '-qm', 'feature');
};

// Tiny valid rasters for embed-prompt.
function pngChunk(type, data) {
  const t = Buffer.from(type, 'latin1');
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length, 0);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([t, data])), 0);
  return Buffer.concat([len, t, data, crc]);
}
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) { c ^= buf[i]; for (let k = 0; k < 8; k++) c = (c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1); }
  return (c ^ 0xffffffff) >>> 0;
}
function tinyPng() {
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(1, 0); ihdr.writeUInt32BE(1, 4); ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const idat = zlib.deflateSync(Buffer.from([0, 255, 0, 0]));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), pngChunk('IHDR', ihdr), pngChunk('IDAT', idat), pngChunk('IEND', Buffer.alloc(0))]);
}
function tinyJpeg() {
  // SOI, APP0 (JFIF), SOS, EOI. Enough structure for the COM reader/writer.
  const app0 = Buffer.from([0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
  const sos = Buffer.from([0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f, 0x00]);
  return Buffer.concat([Buffer.from([0xff, 0xd8]), app0, sos, Buffer.from([0x00, 0xff, 0xd9])]);
}
const imagesSetup = (ws) => {
  fs.mkdirSync(path.join(ws, 'assets/nested'), { recursive: true });
  fs.mkdirSync(path.join(ws, 'assets/.hidden'), { recursive: true });
  fs.mkdirSync(path.join(ws, 'assets/node_modules'), { recursive: true });
  fs.writeFileSync(path.join(ws, 'assets/a.png'), tinyPng());
  fs.writeFileSync(path.join(ws, 'assets/b.jpg'), tinyJpeg());
  fs.writeFileSync(path.join(ws, 'assets/c.webp'), Buffer.from('RIFF....WEBPVP8 ', 'latin1'));
  fs.writeFileSync(path.join(ws, 'assets/nested/d.jpeg'), tinyJpeg());
  fs.writeFileSync(path.join(ws, 'assets/.hidden/e.png'), tinyPng());
  fs.writeFileSync(path.join(ws, 'assets/node_modules/f.png'), tinyPng());
  fs.writeFileSync(path.join(ws, 'assets/notes.txt'), 'not a raster\n');
  fs.writeFileSync(path.join(ws, 'prompt.txt'), 'A prompt read from a file.\nSecond line.\n');
};


const cases = [
  // ======================================================================
  // context
  // ======================================================================
  { id: 'context-empty', verb: 'context', workspace: 'ctx-empty', env: env(), files: IMPECCINO_FILES },
  { id: 'context-visual-only', verb: 'context', workspace: 'ctx-visual-only', env: env(), files: IMPECCINO_FILES },
  { id: 'context-product-only', verb: 'context', workspace: 'ctx-product-only', env: env(), files: IMPECCINO_FILES },
  { id: 'context-full', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, env: env(), files: IMPECCINO_FILES },
  // No agent-browser: context says so up front instead of the first `detect <url>` failing.
  { id: 'context-no-agent-browser', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, env: env({ IMPECCINO_AGENT_BROWSER: '<WS>/no-such-agent-browser' }), files: IMPECCINO_FILES },
  { id: 'context-full-target-brief', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target', 'src/pages/index.astro'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-full-target-related', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['-t', 'src/components/Hero.astro'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-full-target-route', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target=/pricing'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-full-target-missing-file', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target', 'src/pages/nope.astro'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-full-target-last-wins', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target', 'src/pages/nope.astro', '--target', 'src/pages/index.astro'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-full-from-subdir', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, cwd: 'src/pages', env: env(), files: IMPECCINO_FILES },
  { id: 'context-target-missing-value', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target'], env: env() },
  { id: 'context-target-eq-empty', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target='], env: env() },
  { id: 'context-target-followed-by-flag', verb: 'context', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target', '--help'], env: env() },
  { id: 'context-native-ios', verb: 'context', workspace: 'ctx-native-ios', env: env(), files: IMPECCINO_FILES },
  { id: 'context-adaptive', verb: 'context', workspace: 'ctx-adaptive', env: env(), files: IMPECCINO_FILES },
  { id: 'context-bad-platform', verb: 'context', workspace: 'ctx-bad-platform', env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-root', verb: 'context', workspace: 'ctx-monorepo', env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-a', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', 'apps/a/src/App.tsx'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-b-inherits', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', 'apps/b'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-dot', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', '.'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-missing', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', 'apps/zzz/src/App.tsx'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-from-child-cwd', verb: 'context', workspace: 'ctx-monorepo', cwd: 'apps/b', env: env(), files: IMPECCINO_FILES },
  // #706: a bare child name resolves to the one workspace candidate with that
  // name; an absolutized single-segment path that does not exist takes the
  // same route; an ambiguous or unknown name still reports the miss.
  { id: 'context-monorepo-target-bare-name', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', 'a'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-bare-name-abs', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', `${WS}/b`], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-bare-unknown', verb: 'context', workspace: 'ctx-monorepo', args: ['--target', 'zzz'], env: env(), files: IMPECCINO_FILES },
  { id: 'context-monorepo-target-bare-from-child-cwd', verb: 'context', workspace: 'ctx-monorepo', cwd: 'apps/b', args: ['--target', 'a'], env: env(), files: IMPECCINO_FILES },
  // #710: the hook manifest can live at an enclosing git root, the lifecycle
  // config beside it is honored, and an explicit target never borrows a
  // manifest from the caller or an outer workspace across a git boundary.
  {
    id: 'context-hook-at-enclosing-git-root', verb: 'context', workspace: 'ctx-empty', cwd: 'web',
    setup: (ws) => { gitBoundary(ws, '.'); write(ws, 'web/PRODUCT.md', '# Nested web product\n'); claudeStopHook(ws, '.'); },
    env: env({ IMPECCINO_PROVIDER_ID: 'claude-code' }),
  },
  {
    id: 'context-hook-not-borrowed-from-caller', verb: 'context', workspace: 'ctx-empty', cwd: 'apps/marketing',
    args: ['--target', `${WS}/apps/dashboard/src/App.jsx`],
    setup: (ws) => {
      gitBoundary(ws, '.');
      write(ws, 'package.json', JSON.stringify({ private: true, workspaces: ['apps/*'] }) + '\n');
      write(ws, 'turbo.json', JSON.stringify({ tasks: {} }) + '\n');
      write(ws, 'apps/marketing/package.json', JSON.stringify({ name: 'marketing' }) + '\n');
      write(ws, 'apps/dashboard/package.json', JSON.stringify({ name: 'dashboard' }) + '\n');
      write(ws, 'apps/dashboard/PRODUCT.md', '# Dashboard\n');
      write(ws, 'apps/dashboard/src/App.jsx', 'export default function App() { return "dashboard"; }\n');
      claudeStopHook(ws, 'apps/marketing');
    },
    env: env({ IMPECCINO_PROVIDER_ID: 'claude-code' }),
  },
  {
    id: 'context-hook-not-borrowed-across-nested-git', verb: 'context', workspace: 'ctx-empty',
    args: ['--target', 'repos/standalone/src/App.jsx'],
    setup: (ws) => {
      gitBoundary(ws, '.');
      gitBoundary(ws, 'repos/standalone');
      write(ws, 'package.json', JSON.stringify({ private: true, workspaces: ['repos/*'] }) + '\n');
      claudeStopHook(ws, '.');
      write(ws, 'repos/standalone/package.json', JSON.stringify({ name: 'standalone' }) + '\n');
      write(ws, 'repos/standalone/PRODUCT.md', '# Standalone\n');
      write(ws, 'repos/standalone/src/App.jsx', 'export default function App() { return "standalone"; }\n');
    },
    env: env({ IMPECCINO_PROVIDER_ID: 'claude-code' }),
  },
  {
    id: 'context-markerless-nested-git-target', verb: 'context', workspace: 'ctx-empty',
    args: ['--target', 'repos/standalone/src/App.jsx'],
    setup: (ws) => {
      gitBoundary(ws, '.');
      gitBoundary(ws, 'repos/standalone');
      write(ws, 'package.json', JSON.stringify({ private: true, workspaces: ['repos/*'] }) + '\n');
      write(ws, 'PRODUCT.md', '# Outer product\n');
      claudeStopHook(ws, '.');
      write(ws, 'repos/standalone/src/App.jsx', 'export default function App() { return "standalone"; }\n');
    },
    env: env({ IMPECCINO_PROVIDER_ID: 'claude-code' }),
  },
  { id: 'context-legacy', verb: 'context', workspace: 'ctx-legacy', setup: legacySetup, env: env(), files: IMPECCINO_FILES },
  { id: 'context-hook-disabled-env', verb: 'context', workspace: 'ctx-product-only', env: env({ IMPECCINO_HOOK_DISABLED: 'yes' }), files: IMPECCINO_FILES },
  { id: 'context-openai-key', verb: 'context', workspace: 'ctx-product-only', env: env({ OPENAI_API_KEY: 'sk-oracle' }), files: IMPECCINO_FILES },
  { id: 'context-no-staleness-check-env', verb: 'context', workspace: 'ctx-legacy', setup: legacySetup, env: env({ IMPECCINO_NO_STALENESS_CHECK: '1' }), files: IMPECCINO_FILES },
  {
    // Tier-1 throttling: the first boot reports mention/route findings, the
    // second boot within a week reports only `auto` ones. Both steps share
    // the isolated HOME, so the notice cache carries between them.
    id: 'context-staleness-throttle', verb: 'context', workspace: 'ctx-legacy', setup: legacySetup, env: env(), files: IMPECCINO_FILES,
    steps: [{}, {}],
  },
  {
    id: 'context-staleness-cache-env', verb: 'context', workspace: 'ctx-legacy', setup: legacySetup,
    env: env({ IMPECCINO_STALENESS_CACHE: `${WS}/.oracle-cache/notice.json` }), files: [...IMPECCINO_FILES, '.oracle-cache/**'],
    steps: [{}, {}],
  },
  {
    id: 'context-dir-override', verb: 'context', workspace: 'ctx-empty',
    setup: (ws) => { write(ws, 'elsewhere/PRODUCT.md', '# Elsewhere\n\n<!-- impeccino:product-schema 1 -->\n\n## Platform\n\nweb\n\n## Positioning\nFound through IMPECCINO_CONTEXT_DIR.\n'); write(ws, 'elsewhere/DESIGN.md', '# Design: Elsewhere\n\n## Colors\n- **Ink** (#111): Text.\n'); },
    env: env({ IMPECCINO_CONTEXT_DIR: `${WS}/elsewhere` }), files: IMPECCINO_FILES,
  },
  { id: 'context-dir-override-relative', verb: 'context', workspace: 'ctx-empty', setup: (ws) => write(ws, 'ctx/PRODUCT.md', '# Rel\n\n<!-- impeccino:product-schema 1 -->\n\n## Positioning\nRelative override.\n'), env: env({ IMPECCINO_CONTEXT_DIR: 'ctx' }), files: IMPECCINO_FILES },
  { id: 'context-dir-override-ignored-when-project-has-product', verb: 'context', workspace: 'ctx-product-only', setup: (ws) => write(ws, 'elsewhere/PRODUCT.md', '# Should not load\n'), env: env({ IMPECCINO_CONTEXT_DIR: `${WS}/elsewhere` }), files: IMPECCINO_FILES },
  { id: 'context-dir-override-missing', verb: 'context', workspace: 'ctx-empty', env: env({ IMPECCINO_CONTEXT_DIR: `${WS}/nowhere` }), files: IMPECCINO_FILES },
  { id: 'context-fallback-dir-docs', verb: 'context', workspace: 'ctx-empty', setup: (ws) => write(ws, 'docs/PRODUCT.md', '# Docs product\n\n<!-- impeccino:product-schema 1 -->\n\n## Positioning\nLives under docs/.\n'), env: env(), files: IMPECCINO_FILES },
  // `product.md` is found through the case-insensitive lookup of PRODUCT.md on
  // macOS and Windows and reported under the canonical name; on a
  // case-sensitive file system the fallback scan finds it as `product.md`.
  // Both are right for their host, so the case runs only where the golden
  // was recorded.
  {
    id: 'context-lowercase-product-name',
    platforms: ['darwin', 'win32'],
    platformSkipReason: 'This case tests case-insensitive PRODUCT.md discovery, which Linux filesystems do not provide.',
    verb: 'context', workspace: 'ctx-empty',
    setup: (ws) => write(ws, 'product.md', '# lower\n\n<!-- impeccino:product-schema 1 -->\n\n## Positioning\nLowercase filename.\n'),
    env: env(), files: IMPECCINO_FILES,
  },
  { id: 'context-design-only', verb: 'context', workspace: 'ctx-empty', setup: (ws) => write(ws, 'DESIGN.md', '---\nname: Only\n---\n# Design: Only\n\n## Colors\n- **Ink** (#111): Text.\n'), env: env(), files: IMPECCINO_FILES },
  { id: 'context-empty-platform-section', verb: 'context', workspace: 'ctx-empty', setup: (ws) => write(ws, 'PRODUCT.md', '# P\n\n<!-- impeccino:product-schema 1 -->\n\n## Platform\n\n## Positioning\nEmpty platform section.\n'), env: env(), files: IMPECCINO_FILES },
  { id: 'context-android', verb: 'context', workspace: 'ctx-empty', setup: (ws) => write(ws, 'PRODUCT.md', '# P\n\n<!-- impeccino:product-schema 1 -->\n\n## Platform\n\nAndroid\n\n## Positioning\nNative android.\n'), env: env(), files: IMPECCINO_FILES },
  { id: 'context-adaptive-word', verb: 'context', workspace: 'ctx-empty', setup: (ws) => write(ws, 'PRODUCT.md', '# P\n\n<!-- impeccino:product-schema 1 -->\n\n## Platform\n\nadaptive\n\n## Positioning\nAdaptive keyword.\n'), env: env(), files: IMPECCINO_FILES },
  { id: 'context-native-evidence-web', verb: 'context', workspace: 'ctx-product-only', setup: (ws) => write(ws, 'ios/Podfile', "platform :ios, '15.0'\n"), env: env(), files: IMPECCINO_FILES },
  { id: 'context-surfaces-only-brief', verb: 'context', workspace: 'ctx-product-only', setup: (ws) => write(ws, 'SURFACES.md', '# Surfaces\n\n## src/App.tsx\n<!-- impeccino:surface {"target":"src/App.tsx","related":[]} -->\n\nMode: Operate\n'), env: env(), files: IMPECCINO_FILES },
  // A `.impeccino/` left by the layout before docs/adr/0020: one mention,
  // through the same throttle as every boot finding.
  { id: 'context-legacy-state-dir', verb: 'context', workspace: 'ctx-product-only', setup: leftoverStateDir, env: env(), files: IMPECCINO_FILES, steps: [{}, {}] },
  // The home directory is never reported: older launchers cached the engine
  // in ~/.impeccino/.
  { id: 'context-legacy-state-dir-home-skipped', verb: 'context', workspace: 'ctx-product-only', setup: leftoverStateDir, env: env({ HOME: WS }), files: IMPECCINO_FILES },
  { id: 'context-legacy-state-dir-no-staleness-check', verb: 'context', workspace: 'ctx-product-only', setup: leftoverStateDir, env: env({ IMPECCINO_NO_STALENESS_CHECK: '1' }), files: IMPECCINO_FILES },
  { id: 'context-hook-manifest-source-provider', verb: 'context', workspace: 'ctx-product-only', setup: (ws) => write(ws, '.claude/settings.local.json', JSON.stringify({ hooks: { PostToolUse: [{ hooks: [{ type: 'command', command: 'node .claude/skills/impeccino/scripts/hook.mjs' }] }] } }, null, 2) + '\n'), env: env(), files: IMPECCINO_FILES },
  // Upgrade path (triage E8): a v3 install left a `.claude/settings.local.json`
  // naming the retired `node .../hook.mjs` script. Under the real provider the
  // stale marker must NOT count as an active hook, so MANUAL_DETECTOR_REQUIRED
  // still fires (the launcher-era script no longer exists; the hook is dead).
  { id: 'context-stale-hook-manifest', verb: 'context', workspace: 'ctx-product-only', setup: (ws) => write(ws, '.claude/settings.local.json', JSON.stringify({ hooks: { PostToolUse: [{ matcher: 'Edit', hooks: [{ type: 'command', command: 'node "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/hook.mjs"' }] }] } }, null, 2) + '\n'), env: env({ IMPECCINO_PROVIDER_ID: 'claude-code' }), files: IMPECCINO_FILES },
  // Control: the launcher-era manifest still counts as an active hook, so
  // MANUAL_DETECTOR_REQUIRED is suppressed exactly as before.
  { id: 'context-launcher-hook-active', verb: 'context', workspace: 'ctx-product-only', setup: (ws) => write(ws, '.claude/settings.local.json', JSON.stringify({ hooks: { PostToolUse: [{ matcher: 'Edit', hooks: [{ type: 'command', command: '[ ! -f "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" ] || "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" hook' }] }] } }, null, 2) + '\n'), env: env({ IMPECCINO_PROVIDER_ID: 'claude-code' }), files: IMPECCINO_FILES },

  // ======================================================================
  // doctor
  // ======================================================================
  { id: 'doctor-help', verb: 'doctor', workspace: 'ctx-empty', args: ['--help'], env: env() },
  { id: 'doctor-help-short', verb: 'doctor', workspace: 'ctx-empty', args: ['-h', '--json'], env: env() },
  { id: 'doctor-empty-text', verb: 'doctor', workspace: 'ctx-empty', env: env() },
  { id: 'doctor-empty-json', verb: 'doctor', workspace: 'ctx-empty', args: ['--json'], env: env() },
  { id: 'doctor-visual-only-text', verb: 'doctor', workspace: 'ctx-visual-only', env: env() },
  { id: 'doctor-visual-only-json', verb: 'doctor', workspace: 'ctx-visual-only', args: ['--json'], env: env() },
  { id: 'doctor-product-only-text', verb: 'doctor', workspace: 'ctx-product-only', env: env() },
  { id: 'doctor-product-only-json', verb: 'doctor', workspace: 'ctx-product-only', args: ['--json'], env: env() },
  { id: 'doctor-full-text', verb: 'doctor', workspace: 'ctx-full', setup: sidecarNewer, env: env() },
  { id: 'doctor-full-json', verb: 'doctor', workspace: 'ctx-full', setup: sidecarNewer, args: ['--json'], env: env() },
  // Boot and deep findings keep their established artifact order (the JS
  // shared the boot policy with doctor in 80997663).
  {
    id: 'doctor-order-boot-and-deep', verb: 'doctor', workspace: 'ctx-empty',
    setup: (ws) => {
      write(ws, 'PRODUCT.md', '# Product\n\n## Register\n\nbrand\n\n## Users\nDesigners.\n');
      write(ws, 'DESIGN.md', '---\nname: Example\n---\n\n# Design System: Example\n\n<!-- impeccino-disable no-such-rule -->\n');
      write(ws, 'DESIGN.json', JSON.stringify({ schemaVersion: 1 }));
      leftoverStateDir(ws);
    },
    args: ['--json'], env: env(),
  },
  { id: 'doctor-full-sidecar-stale', verb: 'doctor', workspace: 'ctx-full', setup: (ws) => { touch(path.join(ws, 'DESIGN.md'), T_NEW); touch(path.join(ws, 'DESIGN.json'), T_OLD); }, args: ['--json'], env: env() },
  { id: 'doctor-native-ios-text', verb: 'doctor', workspace: 'ctx-native-ios', env: env() },
  { id: 'doctor-native-ios-json', verb: 'doctor', workspace: 'ctx-native-ios', args: ['--json'], env: env() },
  { id: 'doctor-adaptive-text', verb: 'doctor', workspace: 'ctx-adaptive', env: env() },
  { id: 'doctor-adaptive-json', verb: 'doctor', workspace: 'ctx-adaptive', args: ['--json'], env: env() },
  { id: 'doctor-bad-platform-text', verb: 'doctor', workspace: 'ctx-bad-platform', env: env() },
  { id: 'doctor-bad-platform-json', verb: 'doctor', workspace: 'ctx-bad-platform', args: ['--json'], env: env() },
  { id: 'doctor-monorepo-text', verb: 'doctor', workspace: 'ctx-monorepo', env: env() },
  { id: 'doctor-monorepo-json', verb: 'doctor', workspace: 'ctx-monorepo', args: ['--json'], env: env() },
  { id: 'doctor-monorepo-target-a', verb: 'doctor', workspace: 'ctx-monorepo', args: ['--json', '--target', 'apps/a'], env: env() },
  { id: 'doctor-monorepo-target-b', verb: 'doctor', workspace: 'ctx-monorepo', args: ['--target', 'apps/b/src/App.tsx'], env: env() },
  { id: 'doctor-monorepo-child-cwd', verb: 'doctor', workspace: 'ctx-monorepo', cwd: 'apps/a', args: ['--json'], env: env() },
  { id: 'doctor-legacy-text', verb: 'doctor', workspace: 'ctx-legacy', setup: legacySetup, env: env(), files: IMPECCINO_FILES },
  { id: 'doctor-legacy-json', verb: 'doctor', workspace: 'ctx-legacy', setup: legacySetup, args: ['--json'], env: env(), files: IMPECCINO_FILES },
  { id: 'doctor-legacy-fix', verb: 'doctor', workspace: 'ctx-legacy', setup: legacySetup, args: ['--fix'], env: env(), files: IMPECCINO_FILES },
  { id: 'doctor-legacy-fix-json', verb: 'doctor', workspace: 'ctx-legacy', setup: legacySetup, args: ['--fix', '--json'], env: env(), files: IMPECCINO_FILES },
  { id: 'doctor-legacy-fix-twice', verb: 'doctor', workspace: 'ctx-legacy', setup: legacySetup, args: ['--fix'], env: env(), files: IMPECCINO_FILES, steps: [{}, {}, { args: ['--json'] }] },
  {
    // Unstamped PRODUCT.md that already has a v4 section: --fix stamps it.
    id: 'doctor-fix-stamps-product', verb: 'doctor', workspace: 'ctx-product-only',
    setup: (ws) => write(ws, 'PRODUCT.md', '# Unstamped\n\n## Platform\n\nweb\n\n## Positioning\nHas a v4 section but no stamp.\n'),
    args: ['--fix'], env: env(), files: IMPECCINO_FILES, steps: [{}, {}],
  },
  { id: 'doctor-fix-clean', verb: 'doctor', workspace: 'ctx-full', setup: sidecarNewer, args: ['--fix'], env: env(), files: IMPECCINO_FILES },
  { id: 'doctor-target-missing-value', verb: 'doctor', workspace: 'ctx-full', setup: sidecarNewer, args: ['--json', '--target'], env: env() },
  { id: 'doctor-target-eq-empty', verb: 'doctor', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target='], env: env() },
  { id: 'doctor-target-file', verb: 'doctor', workspace: 'ctx-full', setup: sidecarNewer, args: ['--target=src/pages/index.astro'], env: env() },
  { id: 'doctor-legacy-state-dir', verb: 'doctor', workspace: 'ctx-product-only', setup: leftoverStateDir, env: env(), files: IMPECCINO_FILES, steps: [{}, { args: ['--fix', '--json'] }] },
  { id: 'doctor-design-seed-marker', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => write(ws, 'DESIGN.md', "<!-- SEED: established with the user before implementation; re-run /impeccino document once there's code to capture the actual tokens and components. -->\n# Seed\n\n## Colors\n- **Ink** (#111): Text.\n\n## Typography\n**Body Font:** Inter\n"), args: ['--json'], env: env() },
  { id: 'doctor-design-coverage-missing-all', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => write(ws, 'DESIGN.md', '# Thin\n\nNo canonical sections at all.\n'), env: env() },
  { id: 'doctor-sidecar-schema-missing', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => { write(ws, 'DESIGN.md', '# D\n\n## Colors\n- x\n\n## Typography\n- y\n\n## Components\n- z\n'); write(ws, 'DESIGN.json', '{"title":"x"}\n'); sidecarNewer(ws); }, args: ['--json'], env: env() },
  // The sidecar lives next to DESIGN.md, wherever DESIGN.md lives.
  { id: 'doctor-sidecar-next-to-docs-design', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => { write(ws, 'docs/DESIGN.md', '# D\n\n## Colors\n- x\n\n## Typography\n- y\n\n## Components\n- z\n'); write(ws, 'docs/DESIGN.json', '{"title":"x"}\n'); write(ws, 'DESIGN.json', '{"schemaVersion":2}\n'); }, args: ['--json'], env: env() },
  // Project-wide waivers live in DESIGN.md; the deep pass validates their ids.
  { id: 'doctor-design-waiver-ids', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => write(ws, 'DESIGN.md', '# D\n\n## Colors\n- x\n\n## Typography\n- y\n\n## Components\n- z\n\n<!-- impeccino-disable GRADIENT-TEXT, side-tabs -- typo for side-tab -->\n<!-- impeccino-disable -->\n'), env: env() },
  { id: 'doctor-hook-script-missing', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => write(ws, '.claude/settings.json', JSON.stringify({ hooks: { Stop: [{ hooks: [{ type: 'command', command: 'node "${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/hook.mjs"' }] }] } }, null, 2) + '\n'), args: ['--json'], env: env() },
  { id: 'doctor-hook-script-present', verb: 'doctor', workspace: 'ctx-product-only', setup: (ws) => { write(ws, '.claude/settings.json', JSON.stringify({ hooks: { Stop: [{ hooks: [{ type: 'command', command: 'node .claude/skills/impeccino/scripts/hook.mjs' }] }] } }, null, 2) + '\n'); write(ws, '.claude/skills/impeccino/scripts/hook.mjs', '// present\n'); }, args: ['--json'], env: env() },
  {
    id: 'doctor-design-drift', verb: 'doctor', workspace: 'ctx-signals',
    setup: (ws) => {
      write(ws, 'DESIGN.md', '# D\n\n## Colors\n- x\n\n## Typography\n- y\n\n## Components\n- z\n');
      gitInit(ws);
      for (let i = 0; i < 26; i++) {
        write(ws, `src/c${i}.tsx`, `export const C${i} = () => null;\n`);
        git(ws, 'add', '.');
        git(ws, 'commit', '-qm', `change ${i}`);
      }
    },
    args: ['--json'], env: env(),
  },
  { id: 'doctor-design-no-drift', verb: 'doctor', workspace: 'ctx-signals', setup: (ws) => { write(ws, 'DESIGN.md', '# D\n\n## Colors\n- x\n\n## Typography\n- y\n\n## Components\n- z\n'); gitInit(ws); }, args: ['--json'], env: env() },

  // ======================================================================
  // pin
  // ======================================================================
  { id: 'pin-usage-no-args', verb: 'pin', workspace: 'ctx-pin', env: env() },
  { id: 'pin-usage-one-arg', verb: 'pin', workspace: 'ctx-pin', args: ['pin'], env: env() },
  { id: 'pin-bad-action', verb: 'pin', workspace: 'ctx-pin', args: ['toggle', 'audit'], env: env() },
  { id: 'pin-bad-command', verb: 'pin', workspace: 'ctx-pin', args: ['pin', 'doctor'], env: env() },
  { id: 'pin-bad-command-teach', verb: 'pin', workspace: 'ctx-pin', args: ['pin', 'teach'], env: env() },
  { id: 'pin-no-harness', verb: 'pin', workspace: 'ctx-empty', args: ['pin', 'audit'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-unpin-no-harness', verb: 'pin', workspace: 'ctx-empty', args: ['unpin', 'audit'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-polish', verb: 'pin', workspace: 'ctx-pin', args: ['pin', 'polish'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-audit-skips-existing', verb: 'pin', workspace: 'ctx-pin', args: ['pin', 'audit'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-polish-from-subdir', verb: 'pin', workspace: 'ctx-pin', cwd: 'sub/deeper', setup: (ws) => fs.mkdirSync(path.join(ws, 'sub/deeper'), { recursive: true }), args: ['pin', 'polish'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-unpin-nothing-pinned', verb: 'pin', workspace: 'ctx-pin', args: ['unpin', 'polish'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-unpin-skips-non-pinned', verb: 'pin', workspace: 'ctx-pin', args: ['unpin', 'audit'], env: env(), files: ['.*/skills/**'] },
  { id: 'pin-then-unpin', verb: 'pin', workspace: 'ctx-pin', env: env(), files: ['.*/skills/**'], steps: [{ args: ['pin', 'critique'] }, { args: ['pin', 'critique'] }, { args: ['unpin', 'critique'] }, { args: ['unpin', 'critique'] }] },
  { id: 'pin-i-impeccino-alias', verb: 'pin', workspace: 'ctx-empty', setup: (ws) => write(ws, '.codex/skills/i-impeccino/SKILL.md', '---\nname: i-impeccino\n---\n'), args: ['pin', 'shape'], env: env(), files: ['.*/skills/**'] },
  // #483: OpenCode does not surface a pinned SKILL.md in its slash menu, so a
  // pin there writes `commands/impeccino-<cmd>.md` instead, in the project
  // scope and in the user config dir, and never a `.opencode/skills/<cmd>`.
  {
    id: 'pin-opencode-project', verb: 'pin', workspace: 'ctx-empty',
    setup: (ws) => write(ws, '.opencode/skills/impeccino/SKILL.md', '---\nname: impeccino\n---\n'),
    args: ['pin', 'polish'], env: env(), files: ['.*/skills/**', '.*/commands/**'],
  },
  {
    id: 'pin-opencode-user-scope', verb: 'pin', workspace: 'ctx-empty',
    args: ['pin', 'polish'],
    env: env({ OPENCODE_CONFIG_DIR: `${WS}/oc-config` }),
    setup: (ws) => write(ws, 'oc-config/skills/impeccino/SKILL.md', '---\nname: impeccino\n---\n'),
    files: ['oc-config/**'],
  },
  {
    id: 'pin-opencode-skips-foreign-command', verb: 'pin', workspace: 'ctx-empty',
    setup: (ws) => {
      write(ws, '.opencode/skills/impeccino/SKILL.md', '---\nname: impeccino\n---\n');
      write(ws, '.opencode/commands/impeccino-polish.md', '---\ndescription: hand written\n---\n');
    },
    args: ['pin', 'polish'], env: env(), files: ['.*/skills/**', '.*/commands/**'],
  },
  {
    id: 'pin-opencode-then-unpin', verb: 'pin', workspace: 'ctx-empty',
    setup: (ws) => write(ws, '.opencode/skills/impeccino/SKILL.md', '---\nname: impeccino\n---\n'),
    env: env(), files: ['.*/skills/**', '.*/commands/**'],
    steps: [{ args: ['pin', 'polish'] }, { args: ['unpin', 'polish'] }, { args: ['unpin', 'polish'] }],
  },
  {
    id: 'pin-opencode-unpin-skips-foreign', verb: 'pin', workspace: 'ctx-empty',
    setup: (ws) => write(ws, '.opencode/commands/impeccino-polish.md', '---\ndescription: hand written\n---\n'),
    args: ['unpin', 'polish'], env: env(), files: ['.*/skills/**', '.*/commands/**'],
  },

  // ======================================================================
  // surface-brief
  // ======================================================================
  { id: 'surface-brief-usage', verb: 'surface-brief', workspace: 'ctx-full', env: env() },
  { id: 'surface-brief-unknown', verb: 'surface-brief', workspace: 'ctx-full', args: ['delete', 'x'], env: env() },
  { id: 'surface-brief-path-file', verb: 'surface-brief', workspace: 'ctx-full', args: ['path', 'src/pages/index.astro'], env: env(), windowsPathOutput: true },
  { id: 'surface-brief-path-route', verb: 'surface-brief', workspace: 'ctx-full', args: ['path', 'route:/docs/intro/'], env: env(), windowsPathOutput: true },
  { id: 'surface-brief-path-slash', verb: 'surface-brief', workspace: 'ctx-full', args: ['path', '/'], env: env(), windowsPathOutput: true },
  { id: 'surface-brief-path-url', verb: 'surface-brief', workspace: 'ctx-full', args: ['path', 'https://Impeccino.Style/docs/audit/?x=1#top'], env: env(), windowsPathOutput: true },
  { id: 'surface-brief-path-outside', verb: 'surface-brief', workspace: 'ctx-full', args: ['path', '../elsewhere/x.astro'], env: env(), windowsPathOutput: true },
  { id: 'surface-brief-path-missing-target', verb: 'surface-brief', workspace: 'ctx-full', args: ['path'], env: env() },
  { id: 'surface-brief-path-from-subdir', verb: 'surface-brief', workspace: 'ctx-full', cwd: 'src', args: ['path', 'pages/index.astro'], env: env(), windowsPathOutput: true },
  { id: 'surface-brief-list', verb: 'surface-brief', workspace: 'ctx-full', args: ['list'], env: env() },
  { id: 'surface-brief-list-empty', verb: 'surface-brief', workspace: 'ctx-empty', args: ['list'], env: env() },
  { id: 'surface-brief-read-primary', verb: 'surface-brief', workspace: 'ctx-full', args: ['read', 'src/pages/index.astro'], env: env() },
  { id: 'surface-brief-read-related', verb: 'surface-brief', workspace: 'ctx-full', args: ['read', 'src/components/Hero.astro'], env: env() },
  { id: 'surface-brief-read-route', verb: 'surface-brief', workspace: 'ctx-full', args: ['read', '/pricing'], env: env() },
  { id: 'surface-brief-read-route-prefixed', verb: 'surface-brief', workspace: 'ctx-full', args: ['read', 'route:/pricing/'], env: env() },
  { id: 'surface-brief-read-not-found', verb: 'surface-brief', workspace: 'ctx-full', args: ['read', 'src/pages/about.astro'], env: env() },
  { id: 'surface-brief-read-no-target-ambiguous', verb: 'surface-brief', workspace: 'ctx-full', args: ['read'], env: env() },
  { id: 'surface-brief-read-no-target-only-brief', verb: 'surface-brief', workspace: 'ctx-legacy', args: ['read'], env: env() },
  { id: 'surface-brief-read-none', verb: 'surface-brief', workspace: 'ctx-empty', args: ['read', 'src/x.tsx'], env: env() },
  { id: 'surface-brief-read-invalid-target', verb: 'surface-brief', workspace: 'ctx-full', args: ['read', 'route:../etc'], env: env() },
  { id: 'surface-brief-read-monorepo-child', verb: 'surface-brief', workspace: 'ctx-monorepo', setup: (ws) => write(ws, 'apps/a/SURFACES.md', '# Surfaces\n\n## src/App.tsx\n<!-- impeccino:surface {"target":"src/App.tsx","related":[]} -->\n\nMode: Operate\n'), args: ['read', 'apps/a/src/App.tsx'], env: env() },
  { id: 'surface-brief-write-usage', verb: 'surface-brief', workspace: 'ctx-full', args: ['write', 'src/pages/about.astro'], env: env() },
  {
    id: 'surface-brief-write-read-list', verb: 'surface-brief', workspace: 'ctx-full',
    setup: (ws) => write(ws, 'body.md', 'Mode: Read\n\n### Product strategy\nTell the story.\n\n'),
    env: env(), files: ['SURFACES.md'],
    steps: [
      { args: ['write', 'src/pages/about.astro', `${WS}/body.md`, 'src/components/Team.astro', 'src/pages/about.astro', 'src/components/Team.astro'], windowsPathOutput: true },
      { args: ['read', 'src/components/Team.astro'] },
      { args: ['list'] },
      { args: ['write', 'src/pages/about.astro', `${WS}/body.md`], windowsPathOutput: true },
      { args: ['read', 'src/components/Team.astro'] },
    ],
  },
  // `/` resolves the project root to the filesystem root (#710), where
  // SURFACES.md cannot be written; the OS names that failure differently
  // (read-only on macOS, permission denied on Linux).
  { id: 'surface-brief-write-route', verb: 'surface-brief', workspace: 'ctx-empty', setup: (ws) => write(ws, 'body.md', 'Root route brief.'), args: ['write', '/', `${WS}/body.md`, 'route:/home/'], env: env(), files: ['SURFACES.md'], platforms: ['linux', 'darwin'], platformSkipReason: 'On Windows, the slash-only target resolves to the drive root; this fixture must not write outside its staged workspace.',
    normalize: [['(?:Read-only file system|Permission denied|Operation not permitted) \\(os error \\d+\\)', 'g', '<UNWRITABLE>']] },
  { id: 'surface-brief-write-url', verb: 'surface-brief', workspace: 'ctx-empty', setup: (ws) => write(ws, 'body.md', 'URL brief.'), args: ['write', 'https://example.com/pricing/#plans', `${WS}/body.md`], env: env(), files: ['SURFACES.md'], windowsPathOutput: true },
  { id: 'surface-brief-write-invalid-target', verb: 'surface-brief', workspace: 'ctx-empty', setup: (ws) => write(ws, 'body.md', 'x'), args: ['write', '../outside.astro', `${WS}/body.md`], env: env(), files: ['SURFACES.md'] },
  { id: 'surface-brief-write-missing-body', verb: 'surface-brief', workspace: 'ctx-empty', args: ['write', 'src/x.tsx', `${WS}/nope.md`], env: env(), files: ['SURFACES.md'] },
  { id: 'surface-brief-write-monorepo-child', verb: 'surface-brief', workspace: 'ctx-monorepo', setup: (ws) => write(ws, 'body.md', 'Child brief.'), args: ['write', 'apps/b/src/App.tsx', `${WS}/body.md`], env: env(), files: ['**/SURFACES.md'], windowsPathOutput: true },
  // Writing one surface replaces exactly its own section: the other sections,
  // the preamble, and a hand-written note inside a section stay byte for byte.
  {
    id: 'surface-brief-write-replaces-one-section', verb: 'surface-brief', workspace: 'ctx-full',
    setup: (ws) => {
      fs.appendFileSync(path.join(ws, 'SURFACES.md'), '\n### Direction contract\nTHESIS: a price list that reads like a menu.\n\n## Notes kept by hand\nThe pricing page also ships in German.\n');
      write(ws, 'home.md', 'Mode: Persuade\n\n### Direction contract\nTHESIS: the install command is the hero.\nFINISH: unreviewed and undocumented is unfinished.\n');
      write(ws, 'about.md', 'Mode: Read\n');
    },
    env: env(), files: ['SURFACES.md'],
    steps: [
      { args: ['write', 'src/pages/index.astro', `${WS}/home.md`, '/'] },
      { args: ['write', 'src/pages/about.astro', `${WS}/about.md`] },
      { args: ['read', 'route:/'] },
      { args: ['read', 'route:/pricing'] },
      { args: ['list'] },
    ],
  },
  // Markers are the authority: a body heading, or a marker inside a code
  // fence, never starts a section of its own.
  {
    id: 'surface-brief-read-fenced-marker', verb: 'surface-brief', workspace: 'ctx-empty',
    setup: (ws) => write(ws, 'SURFACES.md', '# Surfaces\n\n## index.html\n<!-- impeccino:surface {"target":"index.html","related":[]} -->\n\n## How a marker looks\n```md\n## other.html\n<!-- impeccino:surface {"target":"other.html","related":[]} -->\n```\n'),
    env: env(), steps: [{ args: ['list'] }, { args: ['read', 'index.html'] }, { args: ['read', 'other.html'] }],
  },

  // ======================================================================
  // critique-storage (removed: critiques are not archived, docs/adr/0020)
  // ======================================================================
  { id: 'critique-storage-removed', verb: 'critique-storage', workspace: 'ctx-full', args: ['latest', 'src/pages/index.astro'], env: env() },

  // ======================================================================
  // palette
  // ======================================================================
  { id: 'palette-id-known', verb: 'palette', args: ['--id', 'seed-002'], env: env() },
  { id: 'palette-id-unknown', verb: 'palette', args: ['--id', 'no-such-seed'], env: env() },
  { id: 'palette-from-key', verb: 'palette', args: ['--from', 'oracle-fixture-key'], env: env() },
  { id: 'palette-from-key-2', verb: 'palette', args: ['--from', 'another key with spaces'], env: env() },
  { id: 'palette-env-seed', verb: 'palette', args: [], env: env({ IMPECCINO_PALETTE_SEED: 'env-seed-key' }) },
  { id: 'palette-from-overrides-env', verb: 'palette', args: ['--from', 'oracle-fixture-key'], env: env({ IMPECCINO_PALETTE_SEED: 'env-seed-key' }) },
  { id: 'palette-id-overrides-from', verb: 'palette', args: ['--from', 'oracle-fixture-key', '--id', 'no-such-seed'], env: env() },

  // ======================================================================
  // context-signals
  // ======================================================================
  { id: 'signals-empty', verb: 'context-signals', workspace: 'ctx-empty', env: env() },
  { id: 'signals-visual-only', verb: 'context-signals', workspace: 'ctx-visual-only', env: env() },
  { id: 'signals-full', verb: 'context-signals', workspace: 'ctx-full', setup: sidecarNewer, env: env() },
  { id: 'signals-native-ios', verb: 'context-signals', workspace: 'ctx-native-ios', env: env() },
  { id: 'signals-git-clean-main', verb: 'context-signals', workspace: 'ctx-signals', setup: gitInit, env: env() },
  { id: 'signals-git-dirty-main', verb: 'context-signals', workspace: 'ctx-signals', setup: gitDirty, env: env() },
  { id: 'signals-git-feature-branch', verb: 'context-signals', workspace: 'ctx-signals', setup: gitFeature, env: env() },
  { id: 'signals-git-dirty-non-ui', verb: 'context-signals', workspace: 'ctx-signals', setup: (ws) => { gitInit(ws); write(ws, 'src/util.ts', 'export const x = 2;\n'); write(ws, 'dist/bundle.css', 'a{}\n'); write(ws, 'README.md', 'x\n'); }, env: env() },
  { id: 'signals-git-dirty-renamed', verb: 'context-signals', workspace: 'ctx-signals', setup: (ws) => { gitInit(ws); git(ws, 'mv', 'src/App.tsx', 'src/Main.tsx'); }, env: env() },

  // ======================================================================
  // concept-seed (local only: the roll assigns an index into the model's own list)
  // ======================================================================
  { id: 'seed-scope-invalid', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'world', '--from', 'k1'], env: env() },
  { id: 'seed-reroll-invalid', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'k1', '--reroll', '-1'], env: env() },
  { id: 'seed-register-invalid', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'k1', '--reroll', '1', '--register', 'wild'], env: env() },
  { id: 'seed-register-without-reroll', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'k1', '--register', 'bolder'], env: env() },
  { id: 'seed-register-surface', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'surface', '--from', 'k1', '--reroll', '1', '--register', 'bolder'], env: env() },
  { id: 'seed-mode-invalid', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'k1', '--mode', 'sell'], env: env() },
  { id: 'seed-candidate-count-invalid', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'k1', '--candidate-count', '9'], env: env() },
  { id: 'seed-no-product-gate', verb: 'concept-seed', workspace: 'ctx-visual-only', args: ['--scope', 'direction', '--from', 'k1'], env: env() },
  { id: 'seed-no-product-gate-empty', verb: 'concept-seed', workspace: 'ctx-empty', args: ['--scope', 'direction', '--from', 'k1'], env: env() },
  { id: 'seed-direction', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--mode', 'persuade', '--from', 'oracle-key-1'], env: env() },
  { id: 'seed-direction-reroll', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--mode', 'persuade', '--from', 'oracle-key-1', '--reroll', '1'], env: env() },
  { id: 'seed-direction-bolder', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'oracle-key-1', '--reroll', '1', '--register', 'bolder'], env: env() },
  { id: 'seed-direction-safer', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'oracle-key-1', '--reroll', '1', '--register', 'safer'], env: env() },
  { id: 'seed-direction-count-5', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--mode', 'operate', '--from', 'oracle-key-2', '--candidate-count', '5'], env: env() },
  { id: 'seed-direction-unscoped', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction', '--from', 'oracle-key-3'], env: env() },
  { id: 'seed-direction-env-key', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'direction'], env: env({ IMPECCINO_CONCEPT_SEED: 'oracle-key-1' }) },
  { id: 'seed-surface', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'surface', '--mode', 'operate', '--from', 'oracle-key-1'], env: env() },
  { id: 'seed-surface-default-scope', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--mode', 'operate', '--from', 'oracle-key-1'], env: env() },
  { id: 'seed-surface-reroll', verb: 'concept-seed', workspace: 'ctx-product-only', args: ['--scope', 'surface', '--mode', 'experience', '--from', 'oracle-key-4', '--reroll', '1'], env: env() },
  // The choice ping is gone (docs/adr/0019); its flags answer with a removal note.
  { id: 'seed-removed-chosen', verb: 'concept-seed', workspace: 'ctx-empty', args: ['--chosen', 'some-id', '--kind', 'challenger', '--from', 'k1', '--scope', 'direction'], env: env() },
  { id: 'seed-removed-kind', verb: 'concept-seed', workspace: 'ctx-empty', args: ['--kind', 'assigned', '--from', 'k1', '--scope', 'direction'], env: env() },

];

export default cases;
