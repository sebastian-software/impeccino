import fs from 'node:fs';
import path from 'node:path';
import { buildCursorHooksManifest } from './transformers/hooks.js';

export function stageCursorPlugin(rootDir, distDir) {
  const provider = path.join(distDir, 'cursor', '.cursor');
  const icon = path.join(rootDir, 'scripts/lib/assets/plugin-icon.png');
  const readme = path.join(rootDir, 'docs/CURSOR-PLUGIN.md');
  for (const input of [path.join(provider, 'skills/impeccable/SKILL.md'), path.join(provider, 'agents'), icon, readme]) {
    if (!fs.existsSync(input)) throw new Error(`Cannot build Cursor plugin: missing ${input}`);
  }
  const source = JSON.parse(fs.readFileSync(path.join(rootDir, '.claude-plugin/plugin.json'), 'utf8'));
  const output = path.join(distDir, 'cursor-plugin');
  fs.rmSync(output, { recursive: true, force: true });
  fs.mkdirSync(path.join(output, '.cursor-plugin'), { recursive: true });
  fs.cpSync(path.join(provider, 'skills'), path.join(output, 'skills'), { recursive: true });
  fs.cpSync(path.join(provider, 'agents'), path.join(output, 'agents'), { recursive: true });
  fs.mkdirSync(path.join(output, 'assets'));
  fs.copyFileSync(icon, path.join(output, 'assets/icon.png'));
  fs.copyFileSync(readme, path.join(output, 'README.md'));
  fs.copyFileSync(path.join(rootDir, 'LICENSE'), path.join(output, 'LICENSE'));
  fs.writeFileSync(path.join(output, '.cursor-plugin/plugin.json'), `${JSON.stringify({
    name: 'impeccable', version: source.version,
    description: 'Design and refine interfaces with Impeccable skills, specialist agents, and design checks.',
    author: { name: 'Renaissance Geek, Inc.' },
    homepage: source.homepage, repository: source.repository,
    license: 'Apache-2.0', keywords: ['design', 'frontend', 'accessibility', 'ui', 'ux'],
    logo: 'assets/icon.png', skills: './skills/', agents: './agents/', hooks: './hooks/hooks.json',
  }, null, 2)}\n`);
  fs.mkdirSync(path.join(output, 'hooks'));
  fs.writeFileSync(path.join(output, 'hooks/hooks.json'), `${JSON.stringify(
    buildCursorHooksManifest('${CURSOR_PLUGIN_ROOT}/skills/impeccable/scripts'), null, 2,
  )}\n`);
  return output;
}
