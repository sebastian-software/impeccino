/**
 * `impeccino hook`, `hook-before-edit`, `hook-admin` corpus.
 * Workspace: hook-project (package.json, PRODUCT.md web, UI files with and
 * without findings). Multi-step cases share one staged workspace so the
 * session cache carries between steps.
 *
 * Hook state lives in the user cache (docs/adr/0020), which the harness puts
 * under the isolated home: `.oracle-home/.cache/impeccino/projects/<PROJECT>/`.
 * The snapshot also lists the retired `.impeccino/`, so a golden would show
 * it if anything wrote project-local state again.
 */

import fs from 'node:fs';
import path from 'node:path';

const WS = '<WS>';
const CACHE_FILES = ['.oracle-home/.cache/impeccino/**', '.impeccino/**', '.claude/settings.local.json', '.codex/hooks.json', '.cursor/hooks.json', '.github/hooks/impeccino.json'];
const write = (ws, rel, body) => {
  const abs = `${ws}/${rel}`;
  fs.mkdirSync(abs.slice(0, abs.lastIndexOf('/')), { recursive: true });
  fs.writeFileSync(abs, body);
};
// A `.git` directory marking the repository root, so the project's
// .gitignore and .gitattributes apply.
const gitRepo = (ws) => fs.mkdirSync(`${ws}/.git`, { recursive: true });

export function hookPath(platform, ...segments) {
  return (platform === 'win32' ? path.win32 : path).join(...segments);
}

export const claudeEdit = (file, extra = {}, platform = process.platform) => ({
  session_id: 's1', cwd: WS, hook_event_name: 'PostToolUse', tool_name: 'Edit',
  tool_input: { file_path: hookPath(platform, WS, file) }, ...extra,
});
const stop = (extra = {}) => ({ session_id: 's1', cwd: WS, hook_event_name: 'Stop', stop_hook_active: false, ...extra });

export default [
  {
    id: 'hook-stop-baseline-new-finding', workspace: 'hook-project', files: CACHE_FILES,
    normalize: [['("stopBaseline":\\{"version":1,"engine":")[^"]+', 'g', '$1<ENGINE_VERSION>']],
    setup(ws) {
      fs.writeFileSync(`${ws}/src/new.css`, '.card { border-left: 4px solid #6366f1; border-radius: 8px; }\n');
    },
    steps: [
      { verb: 'hook', stdin: claudeEdit('src/new.css', {
        tool_name: 'Write',
        tool_response: {
          type: 'create', filePath: hookPath(process.platform, WS, 'src/new.css'), originalFile: null,
          content: '.card { border-left: 4px solid #6366f1; border-radius: 8px; }\n',
        },
      }) },
      { verb: 'hook', stdin: stop() },
    ],
  },
  {
    id: 'hook-stop-baseline-import-only', workspace: 'hook-project', files: CACHE_FILES,
    normalize: [['("stopBaseline":\\{"version":1,"engine":")[^"]+', 'g', '$1<ENGINE_VERSION>']],
    setup(ws) {
      fs.writeFileSync(`${ws}/src/report.ts`, "const report = `<style>body { font-family: Fraunces; }</style>`;\n");
    },
    steps: [
      { verb: 'hook', stdin: claudeEdit('src/report.ts', {
        tool_response: {
          filePath: hookPath(process.platform, WS, 'src/report.ts'),
          originalFile: "import dead from 'dead';\nconst report = `<style>body { font-family: Fraunces; }</style>`;\n",
          oldString: "import dead from 'dead';\n", newString: '', replaceAll: false, userModified: false,
        },
      }) },
      { verb: 'hook', stdin: stop() },
    ],
  },
  // --- hook.mjs: per-edit ---
  { id: 'hook-edit-tsx-fresh', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.tsx'), files: CACHE_FILES },
  {
    id: 'hook-edit-catch-all-route', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    setup(ws) {
      const route = `${ws}/app/[...slug]/page.tsx`;
      fs.mkdirSync(`${ws}/app/[...slug]`, { recursive: true });
      fs.writeFileSync(route, 'const Page = () => <div className="title">Title</div>;\nconst styles = css`\n.title { background: linear-gradient(90deg, #f472b6, #a78bfa); -webkit-background-clip: text; color: transparent; }\n`;\n');
    },
    stdin: claudeEdit('app/[...slug]/page.tsx'),
  },
  { id: 'hook-edit-css-fresh', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.module.css'), files: CACHE_FILES },
  { id: 'hook-edit-html-fresh', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/page.html'), files: CACHE_FILES },
  { id: 'hook-edit-clean-tsx', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Clean.tsx'), files: CACHE_FILES },
  { id: 'hook-edit-non-ui-ts', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/util.ts'), files: CACHE_FILES },
  { id: 'hook-edit-missing-file', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/nope.tsx'), files: CACHE_FILES },
  { id: 'hook-edit-outside-project', verb: 'hook', workspace: 'hook-project', stdin: { ...claudeEdit('x'), tool_input: { file_path: '<REPO>/tests/fixtures/antipatterns/blinking-cursor.html' } }, files: CACHE_FILES },
  { id: 'hook-edit-sensitive', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('.env.local'), files: CACHE_FILES },
  { id: 'hook-edit-generated', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('dist/bundle.css'), files: CACHE_FILES },
  { id: 'hook-edit-write-tool', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.tsx', { tool_name: 'Write' }), files: CACHE_FILES },
  { id: 'hook-edit-multiedit', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.tsx', { tool_name: 'MultiEdit' }), files: CACHE_FILES },
  { id: 'hook-edit-no-file-path', verb: 'hook', workspace: 'hook-project', stdin: { session_id: 's1', cwd: WS, hook_event_name: 'PostToolUse', tool_name: 'Edit', tool_input: {} }, files: CACHE_FILES },
  { id: 'hook-stdin-empty', verb: 'hook', workspace: 'hook-project', stdin: '', files: CACHE_FILES },
  { id: 'hook-stdin-malformed', verb: 'hook', workspace: 'hook-project', stdin: '{not json', files: CACHE_FILES },
  { id: 'hook-stdin-array', verb: 'hook', workspace: 'hook-project', stdin: '[1,2]', files: CACHE_FILES },
  { id: 'hook-env-disabled', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.tsx'), env: { IMPECCINO_HOOK_DISABLED: '1' }, files: CACHE_FILES },
  { id: 'hook-env-reentrant', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.tsx'), env: { IMPECCINO_HOOK_DEPTH: '1' }, files: CACHE_FILES },
  { id: 'hook-env-quiet', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.tsx'), env: { IMPECCINO_HOOK_QUIET: '1' }, files: CACHE_FILES },
  { id: 'hook-harness-github-edit', verb: 'hook', workspace: 'hook-project', stdin: { sessionId: 'g1', cwd: WS, toolName: 'edit', toolArgs: JSON.stringify({ path: 'src/components/Card.module.css', old_str: 'a', new_str: 'b' }) }, files: CACHE_FILES },
  { id: 'hook-harness-github-apply-patch', verb: 'hook', workspace: 'hook-project', stdin: { sessionId: 'g1', cwd: WS, toolName: 'apply_patch', toolArgs: '*** Begin Patch\n*** Update File: src/components/Card.module.css\n@@\n-x\n+y\n*** End Patch' }, files: CACHE_FILES },
  { id: 'hook-harness-codex-apply-patch', verb: 'hook', workspace: 'hook-project', stdin: { session_id: 'c1', cwd: WS, hook_event_name: 'PostToolUse', tool_name: 'apply_patch', tool_input: { command: '*** Begin Patch\n*** Update File: src/components/Card.module.css\n@@\n-x\n+y\n*** End Patch' } }, files: CACHE_FILES },
  { id: 'hook-harness-cursor-shaped', verb: 'hook', workspace: 'hook-project', stdin: { conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/components/Card.module.css' } }, files: CACHE_FILES },
  { id: 'hook-harness-forced-github', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.module.css'), env: { IMPECCINO_HOOK_HARNESS: 'github' }, files: CACHE_FILES },
  { id: 'hook-audit-log', verb: 'hook', workspace: 'hook-project', stdin: claudeEdit('src/components/Card.module.css'), env: { IMPECCINO_HOOK_LOG: `${WS}/logs/audit.ndjson` }, files: [...CACHE_FILES, 'logs/**'] },
  {
    id: 'hook-native-platform-skip', verb: 'hook', workspace: 'hook-project',
    setup: (ws) => fs.writeFileSync(`${ws}/PRODUCT.md`, '# P\n\n## Platform\nios\n'),
    stdin: claudeEdit('src/components/Card.module.css'), files: CACHE_FILES,
  },
  // No config file (docs/adr/0020): DESIGN.md records project-wide waivers
  // and declared fonts, and git metadata keeps files out.
  {
    id: 'hook-design-md-waiver', verb: 'hook', workspace: 'hook-project',
    setup: (ws) => write(ws, 'DESIGN.md', '# Design\n\n**The Signal Rule.** The title gradient is the brand mark. <!-- impeccino-disable gradient-text -- the brand mark -->\n'),
    stdin: claudeEdit('src/components/Card.module.css'), files: CACHE_FILES,
  },
  {
    id: 'hook-design-md-declared-font', verb: 'hook', workspace: 'hook-project',
    setup: (ws) => write(ws, 'DESIGN.md', '---\ntypography:\n  heading:\n    fontFamily: Inter\n---\n# Design\n'),
    stdin: claudeEdit('src/components/Card.tsx'), env: { IMPECCINO_HOOK_HARNESS: 'github' }, files: CACHE_FILES,
  },
  {
    id: 'hook-gitignored-file', verb: 'hook', workspace: 'hook-project',
    setup: (ws) => { gitRepo(ws); write(ws, '.gitignore', 'src/components/\n'); },
    stdin: claudeEdit('src/components/Card.module.css'), files: CACHE_FILES,
  },
  {
    id: 'hook-linguist-generated-file', verb: 'hook', workspace: 'hook-project',
    setup: (ws) => { gitRepo(ws); write(ws, '.gitattributes', 'src/components/*.css linguist-generated\n'); },
    stdin: claudeEdit('src/components/Card.module.css'), files: CACHE_FILES,
  },
  {
    id: 'hbe-gitignored-file', verb: 'hook-before-edit', workspace: 'hook-project',
    setup: (ws) => { gitRepo(ws); write(ws, '.gitignore', 'src/generated/\n'); },
    stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/generated/x.css', content: '.t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n' } }, files: CACHE_FILES,
  },
  // Session flows
  {
    id: 'hook-session-fresh-then-pending-then-stop', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    steps: [
      { stdin: claudeEdit('src/components/Card.module.css') },
      { stdin: claudeEdit('src/components/Card.module.css') },
      { stdin: claudeEdit('src/components/Clean.tsx') },
      { stdin: claudeEdit('src/components/Clean.tsx') },
      { stdin: stop() },
      { stdin: stop() },
    ],
  },
  {
    id: 'hook-session-stop-active', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    steps: [{ stdin: claudeEdit('src/components/Card.module.css') }, { stdin: stop({ stop_hook_active: true }) }],
  },
  { id: 'hook-stop-no-touched', verb: 'hook', workspace: 'hook-project', stdin: stop(), files: CACHE_FILES },
  {
    id: 'hook-session-suppression-after-6', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    steps: Array.from({ length: 9 }, () => ({ stdin: claudeEdit('src/components/Card.module.css') })),
  },
  {
    id: 'hook-session-two-sessions', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    steps: [
      { stdin: claudeEdit('src/components/Card.module.css') },
      { stdin: claudeEdit('src/components/Card.module.css', { session_id: 's2' }) },
      { stdin: stop({ session_id: 's2' }) },
    ],
  },
  // Grok Build camelCase envelope (#646): the per-edit pass scans and warms
  // the session cache without remembering findings (Grok drops PostToolUse
  // stdout), the end_turn Stop reports the full set, the observe-only
  // shutdown fire and a stopHookActive re-entry stay silent.
  {
    id: 'hook-session-grok-edit-then-stop', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    steps: [
      { stdin: { sessionId: 'g1', cwd: WS, hookEventName: 'post_tool_use', toolName: 'str_replace', toolInput: { file_path: `${WS}/src/components/Card.module.css` } } },
      { stdin: { sessionId: 'g1', cwd: WS, hookEventName: 'stop', reason: 'end_turn' } },
      { stdin: { sessionId: 'g1', cwd: WS, hookEventName: 'stop', reason: 'shutdown' } },
      { stdin: { sessionId: 'g1', cwd: WS, hookEventName: 'stop', reason: 'end_turn', stopHookActive: true } },
    ],
  },
  // Codex Stop contract (#603): turn_id identifies Codex, whose Stop channel
  // is a top-level decision/block instead of hookSpecificOutput.
  {
    id: 'hook-session-codex-stop-decision', verb: 'hook', workspace: 'hook-project', files: CACHE_FILES,
    steps: [
      { stdin: claudeEdit('src/components/Card.module.css', { session_id: 'cx1', turn_id: 't-1' }) },
      { stdin: stop({ session_id: 'cx1', turn_id: 't-1' }) },
    ],
  },

  // --- hook-before-edit.mjs (Cursor) ---
  { id: 'hbe-write-with-findings', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/new.css', content: '.t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n' } }, files: CACHE_FILES },
  { id: 'hbe-write-clean', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/new.css', content: '.t { color: #111; }\n' } }, files: CACHE_FILES },
  { id: 'hbe-write-empty-content', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/new.css', content: '' } }, files: CACHE_FILES },
  { id: 'hbe-write-non-ui', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/data.json', content: '{}' } }, files: CACHE_FILES },
  { id: 'hbe-edit-projection', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'StrReplace', tool_input: { path: 'src/components/Card.module.css', old_string: '.card {', new_string: '.card-v2 {' } }, files: CACHE_FILES },
  { id: 'hbe-edit-fragment-only', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Edit', tool_input: { path: 'src/components/Clean.tsx', new_string: 'x' } }, files: CACHE_FILES },
  { id: 'hbe-edit-old-missing', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Edit', tool_input: { path: 'src/components/Clean.tsx', old_string: 'NOPE', new_string: 'x' } }, files: CACHE_FILES },
  { id: 'hbe-shell-heredoc', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Shell', tool_input: { command: 'cat > src/x.css <<\'EOF\'\n.t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\nEOF\n' } }, files: CACHE_FILES },
  { id: 'hbe-shell-redirect-no-content', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Shell', tool_input: { command: 'echo hi > src/x.css' } }, files: CACHE_FILES },
  { id: 'hbe-shell-cp', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Shell', tool_input: { command: 'cp src/components/Card.module.css src/copy.css' } }, files: CACHE_FILES },
  { id: 'hbe-html-engine', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/new.html', content: '<!doctype html><html><head><style>.k{color:#777;background:#666}</style></head><body><p class="k">low contrast</p></body></html>' } }, files: CACHE_FILES },
  { id: 'hbe-no-file', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Shell', tool_input: { command: 'ls' } }, files: CACHE_FILES },
  { id: 'hbe-stdin-empty', verb: 'hook-before-edit', workspace: 'hook-project', stdin: '', files: CACHE_FILES },
  { id: 'hbe-stdin-malformed', verb: 'hook-before-edit', workspace: 'hook-project', stdin: '{', files: CACHE_FILES },
  { id: 'hbe-env-disabled', verb: 'hook-before-edit', workspace: 'hook-project', stdin: '{', env: { IMPECCINO_HOOK_DISABLED: 'true' }, files: CACHE_FILES },
  { id: 'hbe-outside-project', verb: 'hook-before-edit', workspace: 'hook-project', stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: '<REPO>/tests/x.css', content: '.a{}' } }, files: CACHE_FILES },
  {
    id: 'hbe-denial-downgrade-after-6', verb: 'hook-before-edit', workspace: 'hook-project', files: CACHE_FILES,
    steps: Array.from({ length: 8 }, () => ({ stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/new.css', content: '.t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n' } } })),
  },
  {
    id: 'hbe-native-platform', verb: 'hook-before-edit', workspace: 'hook-project',
    setup: (ws) => fs.writeFileSync(`${ws}/PRODUCT.md`, '# P\n\n## Platform\nandroid\n'),
    stdin: { hook_event_name: 'preToolUse', conversation_id: 'cv1', workspace_roots: [WS], tool_name: 'Write', tool_input: { path: 'src/new.css', content: '.t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n' } }, files: CACHE_FILES,
  },

  // --- hook-admin.mjs ---
  { id: 'hadmin-status-default', verb: 'hook-admin', workspace: 'hook-project', args: ['status'], files: CACHE_FILES },
  { id: 'hadmin-status-noargs', verb: 'hook-admin', workspace: 'hook-project', args: [], files: CACHE_FILES },
  { id: 'hadmin-unknown-action', verb: 'hook-admin', workspace: 'hook-project', args: ['bogus'], files: CACHE_FILES },
  { id: 'hadmin-off', verb: 'hook-admin', workspace: 'hook-project', args: ['off'], files: CACHE_FILES },
  { id: 'hadmin-on', verb: 'hook-admin', workspace: 'hook-project', args: ['on'], files: CACHE_FILES },
  { id: 'hadmin-on-twice', verb: 'hook-admin', workspace: 'hook-project', files: CACHE_FILES, steps: [{ args: ['on'] }, { args: ['on'] }, { args: ['status'] }] },
  { id: 'hadmin-off-then-status', verb: 'hook-admin', workspace: 'hook-project', files: CACHE_FILES, steps: [{ args: ['off'] }, { args: ['status'] }, { args: ['on'] }, { args: ['status'] }] },
  // ignore-rule / ignore-file / ignore-value wrote the retired config file.
  { id: 'hadmin-ignore-rule-removed', verb: 'hook-admin', workspace: 'hook-project', args: ['ignore-rule', 'side-tab'], files: CACHE_FILES },
  { id: 'hadmin-ignore-file-removed', verb: 'hook-admin', workspace: 'hook-project', args: ['ignore-file', 'src/legacy/**'], files: CACHE_FILES },
  { id: 'hadmin-ignore-value-removed', verb: 'hook-admin', workspace: 'hook-project', args: ['ignore-value', 'overused-font', 'Inter'], files: CACHE_FILES },
  {
    id: 'hadmin-status-design-waivers', verb: 'hook-admin', workspace: 'hook-project', args: ['status'], files: CACHE_FILES,
    setup: (ws) => write(ws, 'DESIGN.md', '---\ntypography:\n  body:\n    fontFamily: Inter\n---\n# Design\n\n<!-- impeccino-disable side-tab, glow -- ledger rails and the signal halo -->\n'),
  },
  { id: 'hadmin-reset-empty', verb: 'hook-admin', workspace: 'hook-project', args: ['reset'], files: CACHE_FILES },
  // `reset` removes the hook entries `on` wrote and the session cache.
  {
    id: 'hadmin-on-hook-reset', verb: 'hook-admin', workspace: 'hook-project', files: CACHE_FILES,
    steps: [
      { args: ['on'] },
      { verb: 'hook', stdin: claudeEdit('src/components/Card.module.css') },
      { args: ['status'] },
      { args: ['reset'] },
      { args: ['status'] },
    ],
  },
  // A hook in Claude Code's team-shared settings.json is named, never edited.
  {
    id: 'hadmin-off-shared-settings', verb: 'hook-admin', workspace: 'hook-project', files: [...CACHE_FILES, '.claude/settings.json'],
    setup: (ws) => write(ws, '.claude/settings.json', JSON.stringify({ hooks: { Stop: [{ hooks: [{ type: 'command', command: '"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino" hook' }] }] } }, null, 2) + '\n'),
    steps: [{ args: ['status'] }, { args: ['off'] }],
  },
  {
    id: 'hadmin-on-repairs-existing-manifest', verb: 'hook-admin', workspace: 'hook-project', files: CACHE_FILES,
    setup: (ws) => { fs.mkdirSync(`${ws}/.claude`, { recursive: true }); fs.writeFileSync(`${ws}/.claude/settings.local.json`, JSON.stringify({ permissions: { allow: ['Bash(ls)'] }, hooks: { PostToolUse: [{ matcher: 'Edit', hooks: [{ type: 'command', command: 'node old/skills/impeccino/scripts/hook.mjs' }] }, { matcher: 'Write', hooks: [{ type: 'command', command: 'echo other' }] }] } }, null, 2) + '\n'); },
    args: ['on'],
  },
  {
    id: 'hadmin-on-malformed-manifest-backup', verb: 'hook-admin', workspace: 'hook-project', files: [...CACHE_FILES, '.cursor/hooks.json.bak'],
    setup: (ws) => { fs.mkdirSync(`${ws}/.cursor`, { recursive: true }); fs.writeFileSync(`${ws}/.cursor/hooks.json`, '{ broken'); },
    args: ['on'],
  },
];
