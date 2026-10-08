# Changelog

## [0.3.0](https://github.com/sebastian-software/impeccino/compare/engine-v0.2.0...engine-v0.3.0) (2026-10-08)


### ⚠ BREAKING CHANGES

* **skill:** record decisions in PRODUCT.md and DESIGN.md, not tool state
* **engine:** keep project state in top-level files, drop the config file
* **engine:** concept-seed rolls locally, with no catalog service or choice ping

### Features

* **engine:** concept-seed rolls locally, with no catalog service or choice ping ([cdbc88a](https://github.com/sebastian-software/impeccino/commit/cdbc88a806a3f45cffceacdac9f2d0588aa38893)), closes [#7](https://github.com/sebastian-software/impeccino/issues/7)
* **engine:** keep project state in top-level files, drop the config file ([5d111fe](https://github.com/sebastian-software/impeccino/commit/5d111fef146120ad8ec21b50aa7fb8a780021679)), closes [#6](https://github.com/sebastian-software/impeccino/issues/6)
* unify skill and engine releases with Release Please ([#90](https://github.com/sebastian-software/impeccino/issues/90)) ([d7ae767](https://github.com/sebastian-software/impeccino/commit/d7ae767a051ef0f7736ae1159e82d6ca2a6dad43))


### Bug Fixes

* Align contradictory skill guidance ([ae25743](https://github.com/sebastian-software/impeccino/commit/ae257432c42a90051c9920865ba6d9e6a459e4c7))
* Align static and rendered DOM checks ([b604f82](https://github.com/sebastian-software/impeccino/commit/b604f82cd4d4de902f5e1ee2f14f8cc16fdfa996))
* Align Windows oracle replays across platforms ([aee7e30](https://github.com/sebastian-software/impeccino/commit/aee7e301c3b3a81361ffe7eb6449aa0e79635e2b))
* Avoid PowerShell provider loading in cooldown ([631e70f](https://github.com/sebastian-software/impeccino/commit/631e70f4b3c5665d474b2f00ef4682bacb9149ba))
* Bound local dev-server probe requests ([#47](https://github.com/sebastian-software/impeccino/issues/47)) ([122c7c1](https://github.com/sebastian-software/impeccino/commit/122c7c1996efed6a23568de7046dd9a12e2bc8e5))
* clarify CLI help and stdin contract (AI-assisted) ([#50](https://github.com/sebastian-software/impeccino/issues/50)) ([0140375](https://github.com/sebastian-software/impeccino/commit/01403758e3b2c1ec88056d1bb10c7cb196067652))
* Close stdin for Windows cooldown checks ([eb57c37](https://github.com/sebastian-software/impeccino/commit/eb57c3726ecaaa02870bd0a9c8979d94eda5bb3c))
* Close Windows oracle parity gaps (AI-assisted) ([f1315f2](https://github.com/sebastian-software/impeccino/commit/f1315f2c8a9aef3de2cb63d3c99dfb84a679d714))
* Dispatch shipped roles through generic subagents ([3be36f0](https://github.com/sebastian-software/impeccino/commit/3be36f06b8732a94cc95b7593baddf0076181e62))
* Format and lint engine cleanup ([e83ff7a](https://github.com/sebastian-software/impeccino/commit/e83ff7a90b62fc6d99e9d445778945f1c6af8a9e))
* Harden CI release and oracle checks ([#29](https://github.com/sebastian-software/impeccino/issues/29)) ([5ea6c45](https://github.com/sebastian-software/impeccino/commit/5ea6c4512c3610a18a44e6527fe380b32174d903))
* keep published engine pins during release preparation ([#92](https://github.com/sebastian-software/impeccino/issues/92)) ([9f69f1d](https://github.com/sebastian-software/impeccino/commit/9f69f1d16e42f58ad782f3500bbffb03731c715f))
* Match static rules to browser behavior ([#51](https://github.com/sebastian-software/impeccino/issues/51)) ([8a1e373](https://github.com/sebastian-software/impeccino/commit/8a1e373fdf5f0aefadc317a465497e0bc2a25155))
* **release:** retry creating the GitHub release right after pushing its tag ([5ff2461](https://github.com/sebastian-software/impeccino/commit/5ff246172088c42eaf274986e1474e016dfcda9c))
* **release:** start release notes from the previous tag on origin ([d8bd6c0](https://github.com/sebastian-software/impeccino/commit/d8bd6c0c34deb42d195606cd1a0b9eb8e57d8288))
* Report PRODUCT schema stamps before doctor applies them ([765271b](https://github.com/sebastian-software/impeccino/commit/765271b2fc7a5c250198e03704a7411362bd5c8a)), closes [#23](https://github.com/sebastian-software/impeccino/issues/23)
* scan catch-all route files ([9fec735](https://github.com/sebastian-software/impeccino/commit/9fec735e16468d52b3f9ec3af3b17320c2c8403d))
* Share repeated-text predicates and assert parity ([ce47a57](https://github.com/sebastian-software/impeccino/commit/ce47a57f2c52a67e6657a4f9694ef57fe83e6ec4))
* Share static and rendered DOM rule semantics ([1f877d4](https://github.com/sebastian-software/impeccino/commit/1f877d4b9b99c5e87ab4edd4c9e9dd9e56400606))
* Use the pinned engine launcher consistently ([6f4b3f4](https://github.com/sebastian-software/impeccino/commit/6f4b3f4d24e1f7040c83d43dfe2c927977604c92))


### Documentation

* **skill:** record decisions in PRODUCT.md and DESIGN.md, not tool state ([bb9c020](https://github.com/sebastian-software/impeccino/commit/bb9c0206c17c5759a33b9f4b510e8e14726cc48a)), closes [#6](https://github.com/sebastian-software/impeccino/issues/6)

## Changelog

Release Please records product changes here. Releases before the shared version
flow are recorded in the [GitHub release history](https://github.com/sebastian-software/impeccino/releases).
