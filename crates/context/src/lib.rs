//! Context and utility verbs: context, doctor, pin, surface-brief,
//! palette, signals, concept-seed. Each verb is `run(args, io) -> exit code`.

pub mod artifact_schema;
pub mod context;
pub mod context_cli;
pub mod hook_markers;
pub mod jsp;
pub mod pin;
pub mod provider;
pub mod staleness;
pub mod staleness_notice;
pub mod surface_briefs;
pub mod target_args;
pub mod url;
pub mod util;

pub use context_cli::run as run_context;
pub use pin::run as run_pin;
pub mod palette;
pub mod palette_data;
pub use palette::run as run_palette;
pub mod surface_brief_cli;
pub use surface_brief_cli::run as run_surface_brief;
pub mod signals;
pub use signals::run as run_signals;
pub mod design_parser;
pub mod doctor;
pub mod staleness_deep;
pub use doctor::run as run_doctor;
pub mod concept_seed;
pub mod seed_text;
pub use concept_seed::run as run_concept_seed;
