//! Advisory text for the local creative selection helper. Selection mechanics
//! and supported flags are defined in concept_seed.rs; this text grants no authority.

pub const HEADER: &str = "@@SCOPE_UPPER@@ CONCEPT SEED (key: @@KEY@@; mode: @@MODE_OR_UNSCOPED@@; rerun with --scope @@SCOPE@@@@MODE_FLAG@@ --from @@KEY@@@@REROLL_FLAG@@@@REGISTER_FLAG@@ --candidate-count @@CANDIDATECOUNT@@ to reproduce this roll)";
pub const PROMOTED_DIRECTION: &str = "Consider candidate @@BUILDINDEX@@ in your ordered list. This is a deterministic suggestion, not a quality judgment or a decision on the user's behalf.";
pub const PROMOTED_SURFACE: &str = "Consider candidates @@DEALT_INDICES@@ in your ordered list, with @@BUILDINDEX@@ as the lead suggestion. The host decides whether a choice round is useful for this task.";
pub const ASSIGNED_BLOCK: &str = "@@ASSIGNED_OR_DEALT@@\n@@PROMOTEDINSTRUCTION@@\nThe helper never reads the candidate list. It biases the lead away from the first two entries; it cannot establish relevance, originality, or design quality.\n";
pub const CHANNEL_DIRECTION: &str = "Use the host's workflow and existing answers. This output does not authorize implementation, require an interview, or override a specified direction.\n";
pub const AUTHORITY_DIRECTION: &str = "The brief, product facts, and established identity constrain the work. Select suitable knowledge available to the host; no source is required.\n";
pub const AUTHORITY_SURFACE: &str = "The brief and incumbent visual system constrain composition. A suggested structure does not authorize changes to identity, tokens, or controls.\n";
pub const REROLL_BLOCK: &str = "RE-ROLL ROUND @@REROLL@@: the round changes the deterministic selection. To reproduce it, retain the same ordered candidate list, count, key, scope, and flags. New candidates are a separate design decision.\n";
pub const BOLDER_BLOCK: &str = "BOLDER REGISTER: consider less familiar candidates if they suit the request. The selection mechanics remain unchanged; novelty does not establish fitness.\n";
pub const SAFER_BLOCK: &str = "SAFER REGISTER: no assigned index is printed. Consider familiar candidates when appropriate; use existing direction or the host's choice workflow.\n";
pub const PINNED: &str =
    "A direction specified by the user or brief takes precedence over the suggestion.\n";
pub const RESTATED_DIRECTION: &str = "ASSIGNED INDEX (restated): @@BUILDINDEX@@; seed key @@KEY@@. Retain the ordered list with the roll if repeatability matters.\n";
pub const RESTATED_SURFACE: &str = "DEALT INDICES (restated): @@DEALT_INDICES@@; lead @@BUILDINDEX@@; seed key @@KEY@@. Retain the ordered list with the roll if repeatability matters.\n";
pub const RESTATED_SAFER: &str = "REGISTER (restated): safer; no assignment; seed key @@KEY@@.\n";
pub const NO_PRODUCT: &str = "NO_PRODUCT_MD: this helper expects PRODUCT.md. Use reference/init.md to record supported product context within the host's authorization, asking only about material gaps, then rerun. A specified direction can proceed without this helper.\n";
pub const REMOVED_FLAG: &str = "concept-seed: @@FLAG@@ was removed: Impeccino sends no choice pings and concept-seed makes no network calls (ADR 0019). Nothing needs recording; continue with the chosen direction.\n";
