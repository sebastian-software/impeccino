# Impeccable CLI

Detect UI anti-patterns and design quality issues in source files from the command line. The detector scans HTML, CSS, JSX, TSX, Vue, and Svelte files for 61 deterministic rules, including AI-generated UI tells, accessibility violations, and general design quality problems.

The npm package is a small launcher. It runs the `impeccable` engine binary for your platform, installed alongside it as an optional dependency (`@impeccable/cli-<os>-<arch>`), and falls back to a per-user cache or a one-time download when that package is missing.

## Quick Start

```bash
# Scan files or directories for anti-patterns
npx impeccable detect src/

# JSON output for CI/tooling
npx impeccable detect --json src/

# List all available commands
npx impeccable help
```

The CLI does not install the Impeccable skill. The skill is the `skill/` folder of [pbakaus/impeccable](https://github.com/pbakaus/impeccable); add it with a skill manager such as [Dalo](https://dalo.sh) or copy it into your harness, then run `/impeccable init` there.

## What It Detects

**AI Slop Tells**: patterns that scream "AI generated this":
- Side-tab accent borders, gradient text on headings
- Purple/violet gradients and cyan-on-dark palettes
- Dark mode with glowing accents, border + border-radius clashes

**Typography Issues**: overused fonts (Inter, Roboto), flat type hierarchy, single font families

**Color & Contrast**: WCAG AA violations, gray text on colored backgrounds, pure black/white

**Layout & Composition**: nested cards, monotonous spacing, everything-centered layouts

**Motion**: bounce/elastic easing, layout property transitions

**Quality**: tiny body text, cramped padding, tight leading, skipped headings, justified text

61 deterministic detector rules in total. See the full catalog at [impeccable.style/slop](https://impeccable.style/slop).

`detect` reads files, directories, and URLs. A URL is loaded headlessly through [agent-browser](https://github.com/vercel-labs/agent-browser), which adds the rules that need a rendered page: `impeccable detect http://localhost:3000/`.

## Exit Codes

- `0`: scan completed with no primary findings (advisories may still be listed)
- `1`: at least one requested target could not be scanned
- `2`: scan completed with primary findings

Operational failure takes precedence when a multi-target scan is partial. In JSON mode, stdout remains a findings array and diagnostics are written to stderr.

## Options

```
impeccable detect [options] [file-or-dir...]

  --json      Output findings as JSON
  --scope     Only report rules in a design domain (type, layout)
  --help      Show help
```

## Requirements

- Node.js 22.18+ to run `npx impeccable`. The engine itself is a self-contained binary and needs no runtime; the skill installed into your harness calls it directly.
- Behind a TLS-inspecting proxy, downloads trust your OS certificate store as well as the bundled Mozilla roots. Set `SSL_CERT_FILE` or `SSL_CERT_DIR` to use a specific CA bundle instead.

Binary lookup order: `IMPECCABLE_BIN`, the platform package, `~/.impeccable/bin/<version>/`, then a download of the pinned version into that cache. Set `IMPECCABLE_BIN` to a local build to skip all of that.

## Part of Impeccable

This CLI is part of [Impeccable](https://impeccable.style), a cross-provider design skill pack for AI-powered development tools. The full suite includes 22 commands for Claude, Cursor, GitHub Copilot, Gemini, Codex, Hermes Agent, Veto, and more.

## License

[Apache 2.0](https://github.com/pbakaus/impeccable/blob/main/LICENSE)
