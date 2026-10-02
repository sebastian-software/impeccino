//! `impeccino` binary: verb router.
//!
//! Every skill script and CLI subcommand is a verb here. Verb crates expose
//! `run(args: &[String], io: &mut Io) -> i32` (exit code) and never call
//! `std::process::exit` themselves, so this file is the single place exit codes
//! and stream flushing are decided (contract: docs/CLI-CONTRACT.md in the
//! public repo). Verb names are the JS script basenames; a few carry aliases
//! (`signals` for context-signals, `hooks` for hook-admin).

use std::io::Write;

use impeccino_common::Io;

mod page_scan;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut io = Io::stdio();
    let code = run(&args, &mut io);
    let _ = io.stdout.flush();
    let _ = io.stderr.flush();
    std::process::exit(code);
}

fn run(args: &[String], io: &mut Io) -> i32 {
    let Some(verb) = args.first().map(String::as_str) else {
        io.out(impeccino_detect::ROOT_USAGE);
        return 0;
    };
    let rest = &args[1..];
    match verb {
        "--help" | "-h" => {
            io.out(impeccino_detect::ROOT_USAGE);
            0
        }
        "--version" | "-v" => {
            io.out(&format!("{VERSION}\n"));
            0
        }
        // Launcher handshake: a cheap discriminator so the launchers can tell
        // this engine apart from the retired 3.x npm CLI (which answers any
        // unknown verb with `Unknown command`, exit 1) before exec'ing a
        // candidate found on PATH or in the unversioned user cache. Kept out
        // of --help on purpose; not part of the user-facing contract.
        "engine-probe" => {
            io.out(&format!("impeccino-engine {VERSION}\n"));
            0
        }
        "detect" => impeccino_detect::run_detect(rest, io, &engines()),
        "ignores" | "ignore" => impeccino_detect::run_ignores(rest, io),
        "help" => {
            io.out(impeccino_detect::ROOT_USAGE);
            0
        }
        // Impeccino no longer installs itself (docs/adr/0003): Dalo or
        // skills.sh places skill/ in each harness.
        "skills" | "install" | "link" | "update" | "check" => {
            io.err(SELF_INSTALL_RETIRED);
            1
        }
        // skill scripts
        "context" => impeccino_context::run_context(rest, io),
        "pin" => impeccino_context::run_pin(rest, io),
        "palette" => impeccino_context::run_palette(rest, io),
        "surface-brief" => impeccino_context::run_surface_brief(rest, io),
        "critique-storage" => impeccino_context::run_critique_storage(rest, io),
        "signals" | "context-signals" => impeccino_context::run_signals(rest, io),
        "doctor" => impeccino_context::run_doctor(rest, io),
        "concept-seed" => impeccino_context::run_concept_seed(rest, io),
        "hook" => impeccino_hook::run_hook(rest, io, engines().html),
        "hook-before-edit" => impeccino_hook::run_hook_before_edit(rest, io, engines().html),
        "hooks" | "hook-admin" => impeccino_hook::run_hook_admin(rest, io),
        // Browser-run and image-comp verbs are gone (docs/adr/0011, 0012).
        v if v.starts_with("live") || RETIRED_VERBS.contains(&v) => {
            io.err(&format!("\"{v}\" was removed: Impeccino no longer runs anything in the browser or builds image comps.\n"));
            1
        }
        // `npx impeccino src/` shorthand: a path-shaped, flag, URL, or existing
        // first arg is a detect target (cli.js looksLikeDetectTarget).
        v if impeccino_detect::looks_like_detect_target(v, &io.cwd.to_string_lossy()) => {
            impeccino_detect::run_detect(args, io, &engines())
        }
        "init" => {
            io.err(impeccino_detect::INIT_MESSAGE);
            1
        }
        other => {
            io.err(&format!(
                "Unknown command: \"{other}\"\n\nTo see a list of supported commands, run:\n  impeccino --help\n"
            ));
            1
        }
    }
}

const RETIRED_VERBS: &[&str] = &[
    "detect-csp", "embed-prompt", "generate-image", "serve-question", "component-review",
    "comp-spec", "comp-diff", "font-match", "capture-server", "build-phase",
];

const SELF_INSTALL_RETIRED: &str = "Impeccino no longer installs or updates itself.\n\nInstall the skill with Dalo:\n  dalo source add impeccino https://github.com/sebastian-software/impeccino.git --subpath skill\n  dalo sync\nor with skills.sh:\n  npx skills add sebastian-software/impeccino\n";

/// The engines wired into `impeccino detect`: the static HTML engine
/// (crates/html) and URL scans through agent-browser (docs/adr/0016).
fn engines() -> impeccino_detect::Engines<'static> {
    static HTML: impeccino_html::StaticHtmlEngine = impeccino_html::StaticHtmlEngine {
        // The shipped binary carries the built-in rules only.
        static_rule_pack: None,
    };
    impeccino_detect::Engines { html: &HTML, url: Some(&page_scan::AgentBrowserEngine) }
}
