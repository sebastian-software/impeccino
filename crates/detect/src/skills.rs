//! `impeccino help|install|link|update|check` and the legacy
//! `impeccino skills <verb>` namespace (`cli/bin/commands/skills.mjs`).
//!
//! Only the dispatch surface is ported for now: the contract's exact
//! unknown-command text, and a clear not-implemented message for the verbs
//! that need the network or a bundle (help fetches the command list from
//! impeccable.style; install/update/link/check read or download bundles).

use impeccino_common::Io;

/// JS: skills.mjs#run
pub fn run(args: &[String], io: &mut Io) -> i32 {
    let sub = args.first().map(String::as_str).unwrap_or("");
    match sub {
        "" | "help" | "--help" | "-h" => not_implemented("help", io),
        "install" | "link" | "update" | "check" => not_implemented(sub, io),
        other => {
            io.err(&format!("Unknown skills command: {other}\n"));
            io.err("Run 'impeccino --help' for available commands.\n");
            1
        }
    }
}

fn not_implemented(verb: &str, io: &mut Io) -> i32 {
    io.err(&format!(
        "impeccino {verb}: not implemented yet in this build. Use `npx impeccino {verb}` (the Node CLI) for now.\n"
    ));
    1
}
