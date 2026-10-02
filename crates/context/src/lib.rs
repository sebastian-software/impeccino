//! Context and utility verbs: context, doctor, pin, surface-brief,
//! critique-storage, palette, signals, concept-seed. Each verb is `run(args, io) -> exit code`.

pub mod jsp;
pub mod util;
pub mod url;
pub mod provider;
pub mod hook_markers;
pub mod target_args;
pub mod target_slug;
pub mod surface_briefs;
pub mod artifact_schema;
pub mod context;
pub mod staleness;
pub mod staleness_notice;
pub mod context_cli;
pub mod pin;

pub use context_cli::run as run_context;
pub use pin::run as run_pin;
pub mod palette_data;
pub mod palette;
pub use palette::run as run_palette;
pub mod critique_storage;
pub mod surface_brief_cli;
pub use critique_storage::run as run_critique_storage;
pub use surface_brief_cli::run as run_surface_brief;
pub mod signals;
pub use signals::run as run_signals;
pub mod design_parser;
pub mod staleness_deep;
pub mod doctor;
pub use doctor::run as run_doctor;
pub mod seed_text;
pub mod concept_seed;
pub use concept_seed::run as run_concept_seed;

