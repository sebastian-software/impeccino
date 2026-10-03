//! impeccino-detect: `impeccino detect` orchestration and the non-DOM
//! engines, ported from `cli/bin/cli.js`, `cli/engine/cli/main.mjs`,
//! `cli/engine/node/file-system.mjs`, `cli/lib/impeccino-config.mjs`,
//! `cli/engine/engines/regex/detect-text.mjs`, `cli/engine/design-system.mjs`,
//! `cli/engine/profile/profiler.mjs`, and `cli/bin/commands/ignores.mjs`.
//!
//! Engine seams: [`engines::HtmlEngine`] (static HTML, crates/html) and
//! [`engines::UrlEngine`] (browser, crates/browser). This crate never depends
//! on those crates; the `cli` binary wires them in through
//! [`engines::Engines`]. crates/html depends on this crate for the design
//! system types and helpers ([`design_system`]).

pub mod cli;
pub mod config;
pub mod design_system;
pub mod detect_text;
pub mod engines;
pub mod file_system;
pub mod ignores;
pub mod jsp;
pub mod profiler;
pub mod regex_matchers;
pub mod util;

use impeccino_common::Io;

pub use engines::{
    Engines, HtmlEngine, MissingHtmlEngine, MissingUrlEngine, ScanOptions, UrlEngine,
};

/// `impeccino detect [args]` (`detectCli`). Returns the exit code.
pub fn run_detect(args: &[String], io: &mut Io, engines: &Engines) -> i32 {
    cli::run_detect(args, io, engines)
}

/// `impeccino ignores [args]`.
pub fn run_ignores(args: &[String], io: &mut Io) -> i32 {
    ignores::run(args, io)
}

/// JS: cli.js#looksLikeDetectTarget
pub fn looks_like_detect_target(arg: &str, cwd: &str) -> bool {
    let is_flag = arg.starts_with('-');
    let is_url = {
        let lower = arg.to_ascii_lowercase();
        lower.starts_with("http://") || lower.starts_with("https://")
    };
    let is_path_shaped = arg.contains('/') || arg.contains('\\') || arg.contains('.');
    let is_existing = util::exists(&jsp::resolve(cwd, &[arg]));
    is_flag || is_url || is_path_shaped || is_existing
}

/// The `impeccino init` mistake message (`cli.js`).
pub const INIT_MESSAGE: &str = "\"init\" is not a CLI command. Type /impeccino init in your AI coding agent's chat (Claude Code, Cursor, Codex, ...), not in this terminal.\n";
