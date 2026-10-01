import fs from 'node:fs';
import path from 'node:path';

/** Stage a declarative skill extension, independently of repo-install sidecars. */
export function stageVSCodeExtension(rootDir, distDir) {
  const source = path.join(distDir, 'github', '.github', 'skills', 'impeccable');
  const version = JSON.parse(fs.readFileSync(path.join(rootDir, '.claude-plugin/plugin.json'), 'utf8')).version;
  const extensionRoot = path.join(distDir, 'vscode');
  // Only this generated artifact is replaced. Never touch the user's editor.
  fs.rmSync(extensionRoot, { recursive: true, force: true });
  fs.mkdirSync(extensionRoot, { recursive: true });
  const skillDir = path.join(extensionRoot, 'skills', 'impeccable');
  fs.cpSync(source, skillDir, { recursive: true });

  const manifest = {
    name: 'impeccable',
    displayName: 'Impeccable',
    description: 'Design skills for GitHub Copilot: build, critique, audit, and refine interfaces.',
    version,
    publisher: 'renaissance-geek',
    license: 'Apache-2.0',
    homepage: 'https://impeccable.style',
    repository: { type: 'git', url: 'https://github.com/pbakaus/impeccable.git' },
    bugs: { url: 'https://github.com/pbakaus/impeccable/issues' },
    icon: 'icon.png',
    categories: ['AI'],
    keywords: ['copilot', 'design', 'skills', 'frontend', 'accessibility'],
    engines: { vscode: '^1.109.3' },
    extensionKind: ['workspace'],
    extensionDependencies: ['GitHub.copilot-chat'],
    capabilities: { untrustedWorkspaces: { supported: false }, virtualWorkspaces: false },
    contributes: { chatSkills: [{ path: './skills/impeccable/SKILL.md' }] },
  };
  fs.writeFileSync(path.join(extensionRoot, 'package.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  fs.copyFileSync(path.join(rootDir, 'scripts/lib/assets/plugin-icon.png'), path.join(extensionRoot, 'icon.png'));
  fs.copyFileSync(path.join(rootDir, 'LICENSE'), path.join(extensionRoot, 'LICENSE'));
  fs.copyFileSync(path.join(rootDir, 'vscode/README.md'), path.join(extensionRoot, 'README.md'));
  fs.copyFileSync(path.join(rootDir, 'vscode/.vscodeignore'), path.join(extensionRoot, '.vscodeignore'));
  return extensionRoot;
}
