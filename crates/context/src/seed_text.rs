//! The text `impeccino concept-seed` prints. `@@NAME@@` marks a value
//! concept_seed.rs fills in.

pub const HEADER: &str = "@@SCOPE_UPPER@@ CONCEPT SEED (key: @@KEY@@; mode: @@MODE_OR_UNSCOPED@@; rerun with --scope @@SCOPE@@@@MODE_FLAG@@ --from @@KEY@@@@REROLL_FLAG@@@@REGISTER_FLAG@@ --candidate-count @@CANDIDATECOUNT@@ to reproduce this roll)";

pub const PROMOTED_DIRECTION: &str = "After ordering the grounded directions by resonance, build candidate\n  @@BUILDINDEX@@ of your own grounded list. The assignment is the roll, not a\n  suggestion: your top-ranked direction is what every run would ship, so the\n  script decides which grounded direction gets built. Each direction joins a\n  durable visual system to a concrete expression for the requested first\n  surface, decided as one. It must survive the current task plus navigation,\n  quiet and dense content, interaction and state, and a substantially\n  different future surface. In an attended run, present the assigned\n  direction fully committed and offer re-roll. You may add ONE card for your\n  top-ranked grounded candidate when it is not the assigned direction, kicker\n  IMPECCINO’S PICK, with an honest risk line naming its familiarity; one pick\n  card, never a ranked lineup, and the pick never takes the lead position.\n  When the assignment IS your top candidate, there is no pick card. Re-roll\n  yourself only on named factual grounds, when the assignment cannot carry\n  the product's truth or task; taste is never grounds.";

pub const PROMOTED_SURFACE: &str = "After ordering the task's grounded structural candidates by resonance,\n  deal candidates @@DEALT_INDICES@@ of your own grounded list to the\n  table; index @@BUILDINDEX@@ leads. The deal is the roll, not a suggestion:\n  the dice decide which structures reach the user, so the ranking rut stays\n  broken while the user still gets a real choice, and the full ranked list\n  stays yours. In an attended run, present the three dealt structures as full\n  cards of equal salience, the lead carrying kicker THE ROLL, with steer and\n  re-roll, and let the user lock one in; the world is already settled, so this\n  choice is composition. Present the cards through the structured question\n  tool: each card names its structure in one line and describes its first\n  viewport in words. Re-roll yourself only when every dealt structure fails\n  audience identification or product clarity on named factual grounds.";

pub const ASSIGNED_BLOCK: &str = "@@ASSIGNED_OR_DEALT@@\n  @@PROMOTEDINSTRUCTION@@\n  The assignment exists to refuse the model's ranking rut, never to outrank\n  the user or the brief. Never expose assignment metadata in user-facing labels.\n";

pub const CHANNEL_DIRECTION: &str = "Present the round through the structured question tool, every option in one\nshared anatomy: the assigned direction first, then the pick card when there is\none, then re-roll with its three registers (plain, safer, bolder), and the\nstanding exit last: the category standard, played straight, offered as the\nuser's door and never recommended.\n";

pub const AUTHORITY_DIRECTION: &str = "PRODUCT.md and explicit incumbent brand commitments constrain every direction.\nThe seed never chooses exact colors, fonts, tokens, or a user preference, and\nit never permits the world and first surface to be selected independently.";

pub const AUTHORITY_SURFACE: &str = "PRODUCT.md and DESIGN.md constrain every surface candidate's identity\nvocabulary; they do not cancel task-level composition. The seed never\nauthorizes a new palette, type system, material world, or unfamiliar control\nbehavior.";

pub const REROLL_BLOCK: &str = "RE-ROLL ROUND @@REROLL@@: every candidate presented in earlier rounds is\n  eliminated and may not return reworded. Derive genuinely new grounded\n  candidates from unexplored angles and order them again before the\n  roll below applies.\n";

pub const BOLDER_BLOCK: &str = "BOLDER REGISTER (user-requested): derive this round's candidates from the\n  unfamiliar end of the audience's world: forms the category never ships and\n  your first ranking would place last, each still grounded in the product's\n  truth and committed in full. The conventional end of your list sits out this\n  round, and so does the pick card; the standing exit stays. The assignment\n  below applies to the new list.\n";

pub const SAFER_BLOCK: &str = "SAFER REGISTER (user-requested): the assigned index is suspended this\n  round; the user picks, and no candidate is mandated. Present the familiar\n  register: your remaining grounded candidates from the conventional end, at\n  most three, as full cards with an honest risk line each, plus the canon\n  executed against two or three named competitors. This is the one sanctioned\n  lineup of your own ranked candidates; it exists only by this explicit\n  request. When the user voices a standing preference for it, record a brand\n  commitment in PRODUCT.md.\n";

pub const PINNED: &str = "A user- or brief-pinned decision beats the roll, always.\n";

pub const RESTATED_DIRECTION: &str = "ASSIGNED INDEX (restated for truncated readers): @@BUILDINDEX@@. Build candidate\n@@BUILDINDEX@@ of your own grounded list; seed key @@KEY@@.\n";

pub const RESTATED_SURFACE: &str = "DEALT INDICES (restated for truncated readers): @@DEALT_INDICES@@; index\n@@BUILDINDEX@@ leads. Present all three dealt structures; seed key @@KEY@@.\n";

pub const RESTATED_SAFER: &str = "REGISTER (restated for truncated readers): safer, user-requested; the\nassigned index is suspended this round and the user picks; seed key @@KEY@@.\n";

pub const NO_PRODUCT: &str = "NO_PRODUCT_MD: the dice stay in the cup until product truth exists. Complete the init ask round and write PRODUCT.md first (reference/init.md), then re-run this exact command. Every candidate on your list takes its facts from PRODUCT.md; without it every direction is ungrounded.\n";

pub const REMOVED_FLAG: &str = "concept-seed: @@FLAG@@ was removed: Impeccino sends no choice pings and concept-seed makes no network calls (ADR 0019). Nothing needs recording; continue with the chosen direction.\n";
