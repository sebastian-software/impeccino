import fs from 'fs';
import path from 'path';
import {
  cleanDir,
  ensureDir,
  writeFile,
  generateYamlFrontmatter,
  PER_PROJECT_SCRIPT_ARTIFACTS,
  SKILL_BINARY_DIR,
} from '../utils.js';
import { hooksJsonFor } from './hooks.js';

// skill/ is the universal, directly installable skill. Every provider gets a
// verbatim copy of it; only the harness wiring around the skill (agent files
// in the harness's own format, hook manifests, OpenCode's command bridge)
// differs per provider.

/**
 * Copy the source skill tree verbatim, skipping per-machine and per-project
 * artifacts: fetched engine binaries and legacy live-mode config.
 */
export function copySkillTree(srcDir, destDir) {
  const walk = (src, dest, depth) => {
    for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
      if (entry.name === '.DS_Store' || entry.name === '.impeccable') continue;
      const from = path.join(src, entry.name);
      const to = path.join(dest, entry.name);
      const inScripts = path.basename(src) === 'scripts' && depth === 1;
      if (entry.isDirectory()) {
        if (inScripts && entry.name === SKILL_BINARY_DIR) continue;
        walk(from, to, depth + 1);
        continue;
      }
      if (!entry.isFile()) continue;
      if (inScripts && PER_PROJECT_SCRIPT_ARTIFACTS.has(entry.name)) continue;
      ensureDir(dest);
      fs.copyFileSync(from, to);
      fs.chmodSync(to, fs.statSync(from).mode & 0o777);
    }
  };
  walk(srcDir, destDir, 0);
}

// GitHub Copilot custom agents are markdown files named `<name>.agent.md`
// (project scope: `.github/agents/`; user scope: `~/.copilot/agents/`). Only
// the portable frontmatter fields are emitted: `name` and `description`.
// `tools` is omitted deliberately -- omitting it grants access to all tools,
// and Copilot's tool vocabulary differs from ours -- and Copilot has no
// documented model/effort/max-turns equivalents.
function buildCopilotAgent(agent) {
  const frontmatter = {
    name: agent.name,
    description: agent.description,
  };
  return `${generateYamlFrontmatter(frontmatter)}\n${agent.body.trim()}\n`;
}

// Cursor subagents are plain markdown files with YAML frontmatter (project
// scope: `.cursor/agents/`; user scope: `~/.cursor/agents/`). Fields: name,
// description (drives auto-delegation), model (`inherit` maps directly to our
// value), readonly, is_background. `readonly` is derived from the agent's own
// tool list: a role that declares tools but neither Write nor Edit is a
// reader, and Cursor can enforce that.
function buildCursorAgent(agent) {
  const frontmatter = {
    name: agent.name,
    description: agent.description,
    model: agent.model || 'inherit',
  };
  const tools = String(agent.tools || '').split(',').map(t => t.trim()).filter(Boolean);
  if (tools.length > 0 && !tools.includes('Write') && !tools.includes('Edit')) {
    frontmatter.readonly = true;
  }
  // The parent thread waits on each role's return; none of these run detached.
  frontmatter.is_background = false;
  return `${generateYamlFrontmatter(frontmatter)}\n${agent.body.trim()}\n`;
}

function buildAgentFile(config, agent) {
  // skill/agents/*.md already are Claude Code agent files.
  if (config.agentFormat === 'claude-md') {
    return { filename: `${agent.name}.md`, content: fs.readFileSync(agent.filePath, 'utf-8') };
  }
  if (config.agentFormat === 'copilot-agent-md') {
    return { filename: `${agent.name}.agent.md`, content: buildCopilotAgent(agent) };
  }
  if (config.agentFormat === 'cursor-md') {
    return { filename: `${agent.name}.md`, content: buildCursorAgent(agent) };
  }
  return null;
}

/**
 * Create a transformer function for a given provider config.
 *
 * @param {Object} config - Provider configuration from providers.js
 * @returns {Function} transform(skills, distDir)
 */
export function createTransformer(config) {
  const { provider, configDir, displayName } = config;

  return function transform(skills, distDir) {
    const providerDir = path.join(distDir, provider);
    const skillsDir = path.join(providerDir, `${configDir}/skills`);

    cleanDir(providerDir);
    ensureDir(skillsDir);

    let agentCount = 0;

    for (const skill of skills) {
      copySkillTree(path.dirname(skill.filePath), path.join(skillsDir, skill.name));
    }

    // Ship an explicit slash-command surface for OpenCode. OpenCode registers
    // skill commands natively but its TUI autocomplete hides them by deliberate
    // design (anomalyco/opencode#25439); this file also pins execution policy
    // (agent: build, subtask: true) and routes through OpenCode's skill tool,
    // which resolves the skill base dir for any install scope.
    if (provider === 'opencode' && skills.length > 0) {
      const commandsDir = path.join(providerDir, `${configDir}/commands`);
      ensureDir(commandsDir);
      for (const skill of skills) {
        const bridgeBody = `Call skill({ name: "${skill.name}" }) and follow its \`Setup\` and \`Commands\` sections to handle $ARGUMENTS.\n`;
        const bridgeFrontmatter = generateYamlFrontmatter({
          description: skill.description,
          agent: 'build',
          subtask: true,
        });
        writeFile(path.join(commandsDir, `${skill.name}.md`), `${bridgeFrontmatter}\n${bridgeBody}`.replace(/\n+$/, '\n'));
      }
    }

    if (config.agentFormat) {
      const agentsDir = path.join(providerDir, `${configDir}/agents`);
      for (const skill of skills) {
        for (const agent of skill.agents || []) {
          const agentFile = buildAgentFile(config, agent);
          if (!agentFile) continue;
          writeFile(path.join(agentsDir, agentFile.filename), agentFile.content);
          agentCount++;
        }
      }
    }

    // Emit the provider hook manifest when the provider opts in.
    let hooksEmitted = false;
    if (config.emitHooks) {
      const manifest = hooksJsonFor(config.emitHooks, { configDir });
      if (manifest) {
        const hooksRel = config.hooksManifestRel || path.join('hooks', 'hooks.json');
        writeFile(path.join(providerDir, configDir, hooksRel), JSON.stringify(manifest, null, 2) + '\n');
        hooksEmitted = true;
      }
    }

    const skillWord = skills.length === 1 ? 'skill' : 'skills';
    const agentInfo = agentCount > 0 ? ` (${agentCount} agent files)` : '';
    const hooksInfo = hooksEmitted
      ? ` (${config.hooksManifestRel || path.join('hooks', 'hooks.json')})`
      : '';
    console.log(`✓ ${displayName}: ${skills.length} ${skillWord}${agentInfo}${hooksInfo}`);
  };
}
