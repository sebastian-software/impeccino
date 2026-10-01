# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Designers, product managers, and engineers who use AI coding tools (Cursor, Claude Code, GitHub Copilot, Gemini CLI, Codex CLI, and others) and want better design output from their AI. They land on the site from GitHub, social media, or word of mouth, already aware that AI-generated UIs have quality problems. They're looking for a practical solution, not education about the problem.

## Product Purpose

Impeccable gives builders a shared design vocabulary with their AI, delivered as a plug-and-play skill that works in every major AI coding harness. Success is measured in two ways: (1) the user can steer AI output with design precision instead of vague prose, and (2) the AI produces interfaces that pass professional design review, not "looks like an AI made it" output.

## Positioning

Impeccable combines an opinionated design skill and deterministic anti-pattern detection in one folder that installs unchanged in every supported AI coding harness. The harness supplies the browser, screenshots, and image tools; Impeccable supplies the judgment and the detector.

## Operating Context

Builders add `skill/` with a skill manager or a copy, run `/impeccable init` once to record product truth, then direct design work through the shared command vocabulary. The agent checks rendered pages with its own browser tool and source files with the detector. Maintainers author the skill under `skill/`; there is no build step.

## Capabilities and Constraints

- The skill exposes 22 design commands covering new work, critique, technical audit, refinement, hardening, and adaptation.
- The CLI runs deterministic detector rules on source files without an LLM or API key; LLM critique remains a separate judgment layer.
- Nothing generated is tracked. Source changes belong in `skill/`, `crates/`, `scripts/`, `cli/`, or `tests/`; the decisions behind the layout are in `docs/adr/`.
- Product claims, testimonials, customers, benchmarks, pricing, licensing, and deployment facts must not be invented when evidence is absent.

## Brand Commitments

Expert, opinionated, refined. Impeccable speaks with an authoritative design voice: confident taste, editorial quality, zero hedging. It's the design director in the room who knows exactly what's wrong and how to fix it. The tone is **direct** (no "maybe consider"), **specific** (no "improve the vibe"), and **rooted in craft** (no hype, no hedging).

Three-word personality: **expert, decisive, editorial**.

The site and brand must be the antithesis of everything Impeccable critiques. Specifically, avoid:

- **Generic AI tool marketing**: dark mode with purple gradients, neon accents, glassmorphism, glowing particles, cyan-on-black.
- **SaaS landing-page clichés**: hero-metric layouts, identical-card feature grids, sparkline decorations, "boost your productivity" copy.
- **Hedging language**: "might", "could", "consider", "perhaps". Impeccable is opinionated — it picks a direction and commits.
- **Educational framing**: this product is for people who already know they have a problem; we solve it, we don't teach it.
- **Over-decoration**: every visual element must earn its place. No ornament for ornament's sake.

## Evidence on Hand

- `README.md` documents the public command set, supported installation paths, and deterministic detector behavior.
- `skill/` contains the guidance and command references; `tests/` contains regression coverage for the skill rules and the engine.
- The public site lives in a separate repository.
- `cli/` is a working detector implementation, not a roadmap claim.
- The existing product record names no customer testimonials or quantified outcome studies. Future surfaces must not fabricate them.

## Product Principles

1. **Practice what you preach.** The site must pass its own anti-pattern tests with flying colors. If we ship anything we'd flag in an audit, we've lost.
2. **Show, don't tell.** Demonstrate design quality through execution, not through words about design quality. The site IS the demo.
3. **Expert confidence.** Direct, opinionated, decisive. No hedging. Every assertion should sound like it comes from someone who has seen a thousand interfaces and knows what works.
4. **Editorial over marketing.** Feels like a design publication (Eye Magazine, It's Nice That, A List Apart) rather than a SaaS landing page. Long-form reading, considered typography, breathing room.
5. **Purposeful restraint.** Every element earns its place. Nothing decorative without function. Nothing included because "landing pages usually have that".

## Accessibility & Inclusion

Baseline: WCAG 2.1 AA on all pages. Key commitments:
- Color contrast ratios verified with actual contrast checks, not eyeballing.
- All interactive elements keyboard-navigable with visible focus states.
- `prefers-reduced-motion` respected for every animation.
- Semantic HTML first, ARIA as a supplement, never as a workaround.
- Copy readable at an 8th-grade level; jargon only when precisely needed (design terms like "OKLCH", "chroma" are acceptable because they're the actual vocabulary users are here to learn).
