# Light ADRs

Short architecture decision records for the decisions that shape how
Impeccable is built and delivered. One file per decision, one screen each:
the context that forced it, the decision, and what it costs. Supersede a
record with a new one instead of rewriting history.

| ADR | Decision |
| --- | --- |
| [0001](0001-one-universal-skill-folder.md) | One universal skill folder instead of per-harness variants |
| [0002](0002-no-generated-output-in-git.md) | Nothing generated is tracked |
| [0003](0003-no-self-installer.md) | No self-installer; skill managers place, update, and remove the skill |
| [0004](0004-no-update-check.md) | No update check |
| [0005](0005-no-marketplace-packages.md) | No marketplace or editor packages |
| [0006](0006-agents-as-claude-code-files.md) | Agents ship as Claude Code files with a generic-subagent fallback |
| [0007](0007-hooks-are-a-project-opt-in.md) | Hooks are a per-project opt-in |
| [0008](0008-spec-only-skill-frontmatter.md) | Skill frontmatter stays within the Agent Skills spec |
| [0009](0009-engine-version-in-one-file.md) | The engine version lives in `skill/scripts/VERSION` |
| [0010](0010-launcher-fetches-the-engine.md) | The launcher still fetches the pinned engine |
| [0011](0011-nothing-runs-in-the-browser.md) | Nothing runs in the browser; harnesses bring their own |
| [0012](0012-no-image-comps.md) | No image-generated comps; code-led builds only |
| [0013](0013-no-wasm-or-browser-extension.md) | No WebAssembly build and no browser extension |
| [0014](0014-releases-are-tags.md) | Releases are tags with generated notes |
| [0015](0015-history-lives-in-git.md) | History lives in git |
| [0016](0016-rendered-page-rules-via-the-harness-browser.md) | Rendered-page rules run through the harness's browser |

Format: **Status**, **Date**, **Context**, **Decision**, **Consequences**,
and, where it applies, **Revisit when**.
