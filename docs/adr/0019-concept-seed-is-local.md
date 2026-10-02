# 0019: concept-seed is local

**Status:** Accepted · **Date:** 2026-10-02

## Context

`concept-seed` decides which direction a new-work round builds. Inherited from Impeccable, it did that in two layers. The local layer hashed a seed key into an index on the model's own ordered list of grounded directions, so the model's first, most predictable pick was not the one that shipped. The remote layer fetched "challengers" from Impeccable's roll API (`impeccable.style/api/roll`): reviewed visual worlds from a catalog, each with a system grammar and QUALITY BAR board and hero images. After the user chose, the skill pinged `/api/chosen` with the outcome. Without the service, the roll printed a "degraded" notice and told the agent to retry with network access.

That catalog is Impeccable's. Its repository calls the catalog the moat of a paid service and keeps the data out of the open source tree, and Impeccino has no agreement to use it. Keeping the dependency meant building every new-work round on someone else's product and sending them telemetry from a project that is not theirs.

## Decision

`concept-seed` runs entirely on the user's machine. The model writes its ordered list; the script hashes the seed key into the assigned index (a direction round) or three dealt indices (a surface round), with re-roll and the safer and bolder registers. That is the whole roll, not a fallback.

Removed: the roll API fetch, the `/chosen` ping and its `--chosen` and `--kind` flags, the local catalog loader and challenger and composition dealing (`catalog.rs`, `roll_selection.rs`), the `--grain` and `--platform` flags that only steered compositions, the engine's HTTP client (`ureq`, `http.rs`), the catalog fixtures, and the variables `IMPECCINO_API_URL`, `IMPECCINO_API_TIMEOUT`, `IMPECCINO_CARD_BASE`, `IMPECCINO_CATALOG_DIR`, `IMPECCINO_COMPOSITIONS`, and `IMPECCINO_NO_TELEMETRY` (`DO_NOT_TRACK` is no longer read either).

## Consequences

- A direction round presents the assigned direction, at most one pick card, re-roll with its registers, and the standing exit. There are no challengers, verdicts, or donations, and the bolder register asks the model for candidates from the unfamiliar end of the audience's world instead of dealing foreign catalog forms.
- The finish reviewer judges ceiling against the full range of the world the direction contract names; there is no QUALITY BAR card in its input.
- `--chosen` and `--kind` print a "was removed" note and exit 1, so an older skill copy that still sends a ping gets a clear answer. Unknown flags, `--grain` and `--platform` among them, are ignored as before.
- The engine binary carries no HTTP client any more. Its only connection is to a local agent-browser during `detect <url>`; the launcher's one-time engine download is the skill's only required network access.
- The roll's diversity now rests on the model's own list. new-work.md's derivation rules (seven candidates across at least three material families, the category rut kept out) carry the weight the catalog used to share.
