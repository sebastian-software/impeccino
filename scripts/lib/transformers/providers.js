/**
 * Provider configurations for the transformer factory.
 *
 * Every provider receives the same verbatim copy of skill/. A config only
 * describes where that copy goes and which harness wiring surrounds it:
 * - provider: dist/ subdirectory and install id (e.g. 'claude-code')
 * - configDir: dot-directory name (e.g. '.claude')
 * - displayName: human-readable name for log output (e.g. 'Claude Code')
 * - agentFormat: native subagent file format emitted to <configDir>/agents/
 * - emitHooks / hooksManifestRel: design-hook manifest for the harness
 */
export const PROVIDERS = {
  cursor: {
    provider: 'cursor',
    configDir: '.cursor',
    displayName: 'Cursor',
    // Cursor subagents: `.cursor/agents/<name>.md` at repo level,
    // `~/.cursor/agents/` at user level. Project agents take precedence over
    // user ones, so installs simply overwrite on update.
    agentFormat: 'cursor-md',
    emitHooks: 'cursor',
    // Cursor reads `.cursor/hooks.json`, not `.cursor/hooks/hooks.json`.
    hooksManifestRel: 'hooks.json',
  },
  'claude-code': {
    provider: 'claude-code',
    configDir: '.claude',
    displayName: 'Claude Code',
    agentFormat: 'claude-md',
    emitHooks: 'claude',
    // Project-local Claude Code hooks live in `.claude/settings.json`.
    hooksManifestRel: 'settings.json',
  },
  gemini: {
    provider: 'gemini',
    configDir: '.gemini',
    displayName: 'Gemini',
    emitHooks: 'gemini',
    hooksManifestRel: 'settings.json',
  },
  dsh: {
    provider: 'dsh',
    configDir: '.dsh',
    displayName: 'DeepSeek Harness',
    // DeepSeek Harness reads the Agent Skills spec subset (`name`,
    // `description`, `license`, `compatibility`, `metadata`) plus
    // `user-invocable` and `disable-model-invocation`; unknown keys are
    // silently ignored. No hook surface (hooks are in-process plugins, not
    // on-disk manifests) and no native subagent file format, so no
    // emitHooks / agentFormat. Global skills live at ~/.dsh/skills
    // ($DSH_HOME/skills when set), matching the engine's home override.
  },
  codex: {
    provider: 'codex',
    configDir: '.codex',
    displayName: 'Codex',
    // No agentFormat: the Codex subagents ship inside skill/agents/ as committed
    // .toml files (scripts/lib/skill-artifacts.js), which Codex auto-discovers
    // in an installed skill. No top-level .codex/agents/ sidecar is emitted.
    emitHooks: 'codex',
    // Codex discovers project-local hooks at `.codex/hooks.json`.
    hooksManifestRel: 'hooks.json',
  },
  agents: {
    provider: 'agents',
    configDir: '.agents',
    displayName: 'Codex Repo Skills',
  },
  github: {
    provider: 'github',
    configDir: '.github',
    displayName: 'GitHub Copilot',
    // Copilot custom agents: `.github/agents/<name>.agent.md` at repo level,
    // `~/.copilot/agents/` at user level (the CLI installer handles placement).
    agentFormat: 'copilot-agent-md',
    emitHooks: 'github',
    // GitHub Copilot discovers repo-level hooks under `.github/hooks/*.json`.
    hooksManifestRel: 'hooks/impeccable.json',
  },
  kiro: {
    provider: 'kiro',
    configDir: '.kiro',
    displayName: 'Kiro',
  },
  opencode: {
    provider: 'opencode',
    configDir: '.opencode',
    displayName: 'OpenCode',
  },
  pi: {
    provider: 'pi',
    configDir: '.pi',
    displayName: 'Pi',
  },
  qoder: {
    provider: 'qoder',
    configDir: '.qoder',
    displayName: 'Qoder',
  },
  'trae-cn': {
    provider: 'trae-cn',
    configDir: '.trae-cn',
    displayName: 'Trae China',
  },
  trae: {
    provider: 'trae',
    configDir: '.trae',
    displayName: 'Trae',
  },
  'rovo-dev': {
    provider: 'rovo-dev',
    configDir: '.rovodev',
    displayName: 'Rovo Dev',
  },
  vibe: {
    provider: 'vibe',
    configDir: '.vibe',
    displayName: 'Mistral Vibe',
  },
  veto: {
    provider: 'veto',
    configDir: '.veto',
    displayName: 'Veto',
  },
  grok: {
    provider: 'grok',
    configDir: '.grok',
    displayName: 'Grok Build',
    // Grok's skill frontmatter matches the Agent Skills spec plus Claude-style
    // extensions (user-invocable, argument-hint, allowed-tools, model, effort).
    // See https://docs.x.ai/build/features/skills-plugins-marketplaces and
    // ~/.grok/docs/user-guide/08-skills.md.
    // Project/user agents are markdown with YAML frontmatter (Claude-compatible).
    agentFormat: 'claude-md',
    emitHooks: 'grok',
    // Grok discovers project hooks from `.grok/hooks/*.json` (not a single
    // settings.json). Claude tool-name matchers alias to Grok tools.
    hooksManifestRel: 'hooks/impeccable.json',
  },
  antigravity: {
    provider: 'antigravity',
    configDir: '.agent',
    displayName: 'Antigravity',
  },
  hermes: {
    provider: 'hermes',
    configDir: '.hermes',
    displayName: 'Hermes Agent',
    // Hermes ships the Agent Skills spec as-is. The optional fields below
    // (license, compatibility, metadata) are spec-defined; harness-specific
    // extensions (user-invocable, argument-hint, allowed-tools) are NOT
    // recognized by the Hermes skill loader and would be silently ignored.
    // Hermes also has no hook surface, no equivalent of Claude's slash
    // commands, and no per-skill tool ACL -- so no emitHooks and no agentFormat.
    // See hermes-agent/SKILL.md "Skills" section.
  },
};
