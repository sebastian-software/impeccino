//! `impeccino` binary: verb router.
//!
//! Every skill script and CLI subcommand is a verb here. Verb crates expose
//! `run(args: &[String], io: &mut Io) -> i32` (exit code) and never call
//! `std::process::exit` themselves, so this file is the single place exit codes
//! and stream flushing are decided. A few verbs carry aliases
//! (`signals` for context-signals, `hooks` for hook-admin).

use std::io::Write;

use impeccino_common::Io;

mod page_scan;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const ROOT_COMMANDS: &[(&str, &str)] = &[
    ("context", "Resolve project and target context"),
    ("doctor", "Diagnose or repair project state"),
    ("pin", "Manage standalone command shortcuts"),
    ("surface-brief", "Read or update a saved surface brief"),
    ("palette", "Generate a design color palette"),
    ("signals, context-signals", "Read project context signals"),
    ("concept-seed", "Explore visual concept directions"),
    (
        "detect",
        "Scan source files and rendered pages for UI quality issues",
    ),
    ("hook", "Run the design hook"),
    ("hook-before-edit", "Inspect a proposed file edit"),
    ("hooks, hook-admin", "Manage project hook integration"),
    ("help", "Show this help message"),
];

fn root_usage() -> String {
    let mut usage = String::from("Usage: impeccino <command> [options]\n\nCommands:\n");
    for (names, description) in ROOT_COMMANDS {
        usage.push_str(&format!("  {names:<31}{description}\n"));
    }
    usage.push_str(
        "\nOptions:\n  --help       Show this help message\n  --version    Show version number\n\nThe skill itself lives in skill/ of https://github.com/sebastian-software/impeccino;\ninstall it with Dalo or skills.sh (npx skills add sebastian-software/impeccino).\n",
    );
    usage
}

fn main() {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let mut io = Io::stdio();
    let code = run(&args, &mut io);
    let _ = io.stdout.flush();
    let _ = io.stderr.flush();
    std::process::exit(code);
}

fn run(args: &[String], io: &mut Io) -> i32 {
    let Some(verb) = args.first().map(String::as_str) else {
        io.out(&root_usage());
        return 0;
    };
    let rest = &args[1..];
    match verb {
        "--help" | "-h" => {
            io.out(&root_usage());
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
        // There is no config file to hold ignores any more (docs/adr/0020).
        "ignores" | "ignore" => {
            io.err(IGNORES_RETIRED);
            1
        }
        "help" => {
            io.out(&root_usage());
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
        // Critiques are no longer archived (docs/adr/0020): the report lives in
        // the chat, and polish runs its own pass.
        "critique-storage" => {
            io.err("\"critique-storage\" was removed: Impeccino no longer archives critiques. The critique report lives in the chat; polish runs its own pass and takes findings you hand it.\n");
            1
        }
        "signals" | "context-signals" => impeccino_context::run_signals(rest, io),
        "doctor" => impeccino_context::run_doctor(rest, io),
        "concept-seed" => impeccino_context::run_concept_seed(rest, io),
        "hook" => impeccino_hook::run_hook(rest, io, engines().html),
        "hook-before-edit" => impeccino_hook::run_hook_before_edit(rest, io, engines().html),
        "hooks" | "hook-admin" => impeccino_hook::run_hook_admin(rest, io),
        // Browser-run and image-comp verbs are gone (docs/adr/0011, 0012).
        v if RETIRED_VERBS.contains(&v)
            || (v.starts_with("live")
                && !impeccino_detect::looks_like_detect_target(v, &io.cwd.to_string_lossy())) =>
        {
            io.err(&format!("\"{v}\" was removed: Impeccino no longer runs anything in the browser or builds image comps.\n"));
            1
        }
        // `<launcher> src/` shorthand: a path-shaped, flag, URL, or existing
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
    "live",
    "detect-csp",
    "embed-prompt",
    "generate-image",
    "serve-question",
    "component-review",
    "comp-spec",
    "comp-diff",
    "font-match",
    "capture-server",
    "build-phase",
];

const IGNORES_RETIRED: &str = "\"ignores\" was removed: Impeccino keeps no config file.\n\nRecord deliberate choices where the project keeps its design decisions:\n  DESIGN.md      <!-- impeccino-disable <rule-id>: reason --> waives a rule for the whole project;\n                 a font declared under typography never counts as an overused font\n  .gitignore     files git ignores are not scanned; .gitattributes linguist-generated\n                 or linguist-vendored does the same for checked-in files\n  in the file    an impeccino-disable comment waives one file, line, or next line\n";

const SELF_INSTALL_RETIRED: &str = "Impeccino no longer installs or updates itself.\n\nInstall the skill with Dalo:\n  dalo source add impeccino https://github.com/sebastian-software/impeccino.git --subpath skill\n  dalo sync\nor with skills.sh:\n  npx skills add sebastian-software/impeccino\n";

/// The engines wired into `impeccino detect`: the static HTML engine
/// (crates/html) and URL scans through agent-browser (docs/adr/0016).
fn engines() -> impeccino_detect::Engines<'static> {
    static HTML: impeccino_html::StaticHtmlEngine = impeccino_html::StaticHtmlEngine {
        // The shipped binary carries the built-in rules only.
        static_rule_pack: None,
    };
    impeccino_detect::Engines {
        html: &HTML,
        url: Some(&page_scan::AgentBrowserEngine),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn root_help_lists_active_internal_verbs_and_aliases() {
        let (mut io, captured) = Io::captured("", std::env::temp_dir(), HashMap::new());
        let status = run(&["--help".to_string()], &mut io);
        let stdout = String::from_utf8(captured.stdout.borrow().clone()).unwrap();

        assert_eq!(status, 0);
        for command in [
            "context",
            "doctor",
            "pin",
            "surface-brief",
            "palette",
            "signals",
            "context-signals",
            "concept-seed",
            "detect",
            "hook",
            "hook-before-edit",
            "hooks",
            "hook-admin",
        ] {
            assert!(
                stdout.contains(command),
                "root help is missing {command}: {stdout}"
            );
        }
    }

    #[test]
    fn live_prefixed_paths_still_use_detect_shorthand() {
        let root =
            std::env::temp_dir().join(format!("impeccino-cli-live-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("live-demo")).unwrap();
        std::fs::write(root.join("liveblog.html"), "").unwrap();
        std::fs::write(root.join("live"), "").unwrap();

        for target in ["liveblog.html", "live-demo", "live-demo/"] {
            let (mut io, captured) = Io::captured("", root.clone(), HashMap::new());
            let status = run(&[target.to_string(), "--no-config".to_string()], &mut io);
            let stderr = String::from_utf8(captured.stderr.borrow().clone()).unwrap();
            assert_eq!(status, 0, "{target}: {stderr}");
            assert!(!stderr.contains("was removed"), "{target}: {stderr}");
        }

        for retired in ["live", "detect-csp"] {
            let (mut io, captured) = Io::captured("", root.clone(), HashMap::new());
            let status = run(&[retired.to_string()], &mut io);
            let stderr = String::from_utf8(captured.stderr.borrow().clone()).unwrap();
            assert_eq!(status, 1, "{retired}: {stderr}");
            assert!(stderr.contains("was removed"), "{retired}: {stderr}");
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
