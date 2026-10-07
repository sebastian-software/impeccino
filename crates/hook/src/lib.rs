//! Hook execution and administration for supported host harnesses.

pub mod admin;
pub mod before_edit;
pub mod hook;
pub mod hook_lib;
mod stop_baseline;
pub mod util;

use impeccino_common::Io;
use impeccino_detect::engines::HtmlEngine;

pub use hook_lib::Runtime;

fn runtime<'a>(io: &Io, html: &'a dyn HtmlEngine) -> Runtime<'a> {
    let cwd = io.cwd.to_string_lossy().into_owned();
    let provider = impeccino_context::provider::detect(&io.env, &cwd);
    Runtime::new(
        cwd,
        io.env.clone(),
        provider.command,
        &provider.self_cmd,
        html,
    )
}

/// `impeccino hook` (PostToolUse per-edit pass + Stop deep pass). Exit 0.
pub fn run_hook(_args: &[String], io: &mut Io, html: &dyn HtmlEngine) -> i32 {
    let stdin = io.stdin().to_string();
    let rt = runtime(io, html);
    hook::run(&rt, &stdin, io)
}

/// `impeccino hook-before-edit` (Cursor preToolUse gate). Exit 0.
pub fn run_hook_before_edit(_args: &[String], io: &mut Io, html: &dyn HtmlEngine) -> i32 {
    let stdin = io.stdin().to_string();
    let rt = runtime(io, html);
    before_edit::run(&rt, &stdin, io)
}

/// `impeccino hooks <action> [args]` (hook-admin.mjs).
pub fn run_hook_admin(args: &[String], io: &mut Io) -> i32 {
    static NONE: impeccino_detect::MissingHtmlEngine = impeccino_detect::MissingHtmlEngine;
    let rt = runtime(io, &NONE);
    admin::run(&rt, args, io)
}
