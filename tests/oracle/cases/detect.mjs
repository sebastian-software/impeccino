/**
 * `impeccino detect` corpus.
 *
 * Every antipattern fixture is scanned individually in JSON and text mode with
 * --no-config, plus directory scans, the project decisions (DESIGN.md
 * waivers and declared fonts, .gitignore and .gitattributes) and
 * inline-ignore behaviour from the detect-config workspace, and the flag
 * surface (help, scope, quiet, no-advisory, errors).
 *
 * detect-config is staged as a git repository (setup writes `.git/` and the
 * `.gitattributes` that marks src/vendor vendored), so git's own rules apply.
 */
import fs from 'node:fs';
import path from 'node:path';
import { REPO_ROOT } from '../lib.mjs';

// detect-config as a git repository whose .gitattributes marks src/vendor
// vendored. Written at stage time so the fixture tree carries no nested git
// metadata of its own.
const detectConfigRepo = (ws) => {
  fs.mkdirSync(path.join(ws, '.git'), { recursive: true });
  fs.writeFileSync(path.join(ws, '.gitattributes'), 'src/vendor/** linguist-vendored\n');
};

const FIXTURES = path.join(REPO_ROOT, 'tests', 'fixtures', 'antipatterns');

export default function cases() {
  const out = [];
  const entries = fs.readdirSync(FIXTURES, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name));

  for (const ent of entries) {
    const rel = `tests/fixtures/antipatterns/${ent.name}`;
    const id = ent.name.replace(/[^a-z0-9]+/gi, '-').toLowerCase();
    out.push({
      id: `detect-fixture-json-${id}`,
      verb: 'detect',
      args: ['--no-config', '--json', `<REPO>/${rel}`],
      isolateHome: false,
    });
    out.push({
      id: `detect-fixture-text-${id}`,
      verb: 'detect',
      args: ['--no-config', `<REPO>/${rel}`],
      isolateHome: false,
      ...(ent.name.startsWith('framework-') ? { windowsPowerShellGuidance: true } : {}),
    });
  }

  out.push(
    { id: 'detect-dom-quality-parity', verb: 'detect', args: ['--no-config', '--json', '<REPO>/tests/fixtures/dom-parity.html'], isolateHome: false },
    { id: 'detect-dir-json-all-fixtures', verb: 'detect', args: ['--no-config', '--json', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-dir-text-all-fixtures', verb: 'detect', args: ['--no-config', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-dir-quiet-all-fixtures', verb: 'detect', args: ['--no-config', '--quiet', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-scope-type', verb: 'detect', args: ['--no-config', '--json', '--scope', 'type', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-scope-layout-text', verb: 'detect', args: ['--no-config', '--scope', 'layout', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-scope-both', verb: 'detect', args: ['--no-config', '--json', '--scope', 'type,layout', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-scope-unknown', verb: 'detect', args: ['--no-config', '--json', '--scope', 'nope', `<REPO>/tests/fixtures/antipatterns/blinking-cursor.html`], isolateHome: false },
    { id: 'detect-no-advisory-json', verb: 'detect', args: ['--no-config', '--no-advisory', '--json', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-no-advisory-text', verb: 'detect', args: ['--no-config', '--no-advisory', `<REPO>/tests/fixtures/antipatterns`], isolateHome: false, timeoutMs: 180_000 },
    { id: 'detect-multifile-json', verb: 'detect', args: ['--no-config', '--json', `<REPO>/tests/fixtures/antipatterns/multifile`], isolateHome: false },
    { id: 'detect-multifile-text', verb: 'detect', args: ['--no-config', `<REPO>/tests/fixtures/antipatterns/multifile`], isolateHome: false },
    { id: 'detect-framework-vite-json', verb: 'detect', args: ['--no-config', '--json', `<REPO>/tests/fixtures/antipatterns/framework-vite`], isolateHome: false },
    { id: 'detect-framework-next-tailwind-json', verb: 'detect', args: ['--no-config', '--json', `<REPO>/tests/fixtures/antipatterns/framework-next-tailwind`], isolateHome: false },
    { id: 'detect-framework-next-modules-text', verb: 'detect', args: ['--no-config', `<REPO>/tests/fixtures/antipatterns/framework-next-modules`], isolateHome: false, windowsPowerShellGuidance: true },
    { id: 'detect-framework-next-cssinjs-json', verb: 'detect', args: ['--no-config', '--json', `<REPO>/tests/fixtures/antipatterns/framework-next-cssinjs`], isolateHome: false },
    {
      id: 'detect-jsx-commented-img', verb: 'detect',
      setup: (ws) => fs.writeFileSync(path.join(ws, 'repro.tsx'), `function Row({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}

function Thumb({ url }: { url?: string }) {
  return (
    <div>
      {url ? (
        // Plain <img>: presigned thumbnail URL
        <img src={url} alt="" />
      ) : null}
    </div>
  );
}
`),
      args: ['--no-config', '--json', 'repro.tsx'],
    },

    // Flag surface and errors
    { id: 'detect-help', verb: 'detect', args: ['--help'] },
    { id: 'detect-no-args', verb: 'detect', args: [] },
    {
      id: 'detect-stdin-dash', verb: 'detect',
      args: ['--no-config', '--json', '-'],
      stdin: '<div style="border-left: 4px solid #ff0000">x</div>\n',
    },
    { id: 'detect-missing-file', verb: 'detect', args: ['--no-config', 'does-not-exist.html'] },
    { id: 'detect-missing-file-json', verb: 'detect', args: ['--no-config', '--json', 'does-not-exist.html'] },
    // #711: a target that cannot be scanned forces exit 1, and that takes
    // precedence over findings from the targets that did scan.
    {
      id: 'detect-missing-file-with-findings', verb: 'detect',
      args: ['--no-config', '--json', `<REPO>/tests/fixtures/antipatterns/layout.html`, 'does-not-exist.html'],
      isolateHome: false,
    },
    {
      id: 'detect-unreadable-file-json', verb: 'detect',
      platforms: ['linux', 'darwin'],
      platformSkipReason: 'This case uses POSIX chmod(0) to deny reads, which Windows does not enforce.',
      setup: (ws) => {
        const p = path.join(ws, 'locked.html');
        fs.writeFileSync(p, '<div style="border-left: 4px solid #ff0000">x</div>\n');
        fs.chmodSync(p, 0o000);
      },
      args: ['--no-config', '--json', 'locked.html'],
    },
    {
      id: 'detect-latin1-css-json', verb: 'detect',
      setup: (ws) => fs.writeFileSync(
        path.join(ws, 'latin1.css'),
        Buffer.from([0x61, 0x7b, 0x63, 0x6f, 0x6c, 0x6f, 0x72, 0x3a, 0x72, 0x65, 0x64, 0x7d, 0x2f, 0x2a, 0x20, 0x63, 0x61, 0x66, 0xe9, 0x20, 0x2a, 0x2f, 0x0a]),
      ),
      args: ['--no-config', '--json', 'latin1.css'],
    },
    {
      id: 'detect-unreadable-file-in-dir', verb: 'detect',
      platforms: ['linux', 'darwin'],
      platformSkipReason: 'This case uses POSIX chmod(0) to deny reads, which Windows does not enforce.',
      setup: (ws) => {
        fs.writeFileSync(path.join(ws, 'a.html'), '<div style="border-left: 4px solid #ff0000">x</div>\n');
        const p = path.join(ws, 'b.html');
        fs.writeFileSync(p, '<div style="border-left: 4px solid #ff0000">x</div>\n');
        fs.chmodSync(p, 0o000);
      },
      args: ['--no-config', '--json', '.'],
    },
    { id: 'detect-unknown-flag', verb: 'detect', args: ['--bogus', `<REPO>/tests/fixtures/antipatterns/blinking-cursor.html`], isolateHome: false },
    { id: 'detect-bad-viewport', verb: 'detect', args: ['--viewport', 'wide', `<REPO>/tests/fixtures/antipatterns/blinking-cursor.html`], isolateHome: false },
    { id: 'cli-help', verb: 'cli-help', args: [] },
    { id: 'cli-version', verb: 'cli-version', args: [] },

    // Project decisions, DESIGN.md, inline ignores (detect-config workspace)
    {
      id: 'detect-config-independent-package-waivers', verb: 'detect', workspace: 'detect-config',
      setup: (ws) => {
        detectConfigRepo(ws);
        fs.appendFileSync(path.join(ws, 'DESIGN.md'), '\n<!-- impeccino-disable side-tab: root brand rule -->\n');
        const child = path.join(ws, 'independent');
        fs.mkdirSync(child, { recursive: true });
        fs.writeFileSync(path.join(child, 'package.json'), '{"name":"independent"}\n');
        fs.writeFileSync(path.join(child, 'page.tsx'), '<div style="border-left: 4px solid #ff0000">x</div>\n');
      },
      args: ['--json', 'independent/page.tsx'],
    },
    { id: 'detect-config-page-json', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', 'src/page.html'] },
    { id: 'detect-config-page-text', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['src/page.html'] },
    { id: 'detect-config-page-no-config', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--no-config', '--json', 'src/page.html'] },
    { id: 'detect-config-page-no-design-system', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--no-design-system', '--json', 'src/page.html'] },
    { id: 'detect-config-dir-json', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', 'src'] },
    { id: 'detect-config-dir-text', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['src'] },
    { id: 'detect-config-dir-dot', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', '.'] },
    { id: 'detect-config-inline-json', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', 'src/inline.html'] },
    { id: 'detect-config-inline-disabled', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--no-inline-ignores', '--json', 'src/inline.html'] },
    { id: 'detect-config-css-json', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', 'src/styles.css'] },
    { id: 'detect-config-css-text', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['src/styles.css'] },
    { id: 'detect-config-vendor-ignored', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', 'src/vendor/ignored.html'] },
    { id: 'detect-config-from-subdir', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, cwd: 'src', args: ['--json', 'page.html'] },
    // .gitignore keeps generated output out of a directory scan.
    {
      id: 'detect-config-gitignored-dir', verb: 'detect', workspace: 'detect-config', args: ['--json', '.'],
      setup: (ws) => {
        detectConfigRepo(ws);
        fs.writeFileSync(path.join(ws, '.gitignore'), 'src/generated/\n');
        fs.mkdirSync(path.join(ws, 'src/generated'), { recursive: true });
        fs.copyFileSync(path.join(ws, 'src/page.html'), path.join(ws, 'src/generated/page.html'));
      },
    },
    // Outside a git repository, .gitattributes does not apply.
    { id: 'detect-config-no-repo-vendor-scanned', verb: 'detect', workspace: 'detect-config', args: ['--json', 'src/vendor/ignored.html'],
      setup: (ws) => fs.writeFileSync(path.join(ws, '.gitattributes'), 'src/vendor/** linguist-vendored\n') },
    // The `ignores` verb wrote the retired config file.
    { id: 'ignores-removed', verb: 'ignores', args: ['add-rule', 'side-tab'] },
    // A file in one project must not pick up another project's DESIGN.md
    { id: 'detect-config-cross-project', verb: 'detect', workspace: 'detect-config', setup: detectConfigRepo, args: ['--json', `<REPO>/tests/fixtures/antipatterns/blinking-cursor.html`], isolateHome: false },
  );

  return out;
}
