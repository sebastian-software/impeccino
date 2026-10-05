//! `impeccino hooks` / `hook-admin`: status, on, off, reset. There is no
//! config file (docs/adr/0020): the hook is on where its entries sit in the
//! harness settings (docs/adr/0007), so `on` installs or repairs them, `off`
//! removes them, and `reset` also clears the session cache in the user cache.
//! Manifests are written as JSON with 2-space indentation and a trailing
//! newline, in the JS key order.
//!
//! Where the JS wrote `node "<skill>/scripts/hook.mjs"`, the binary writes
//! `"<skill>/scripts/impeccino" hook`: the launcher shipped next to the
//! skill picks the platform binary, so no Node is needed. Old `.mjs`
//! manifests are still recognized (`impeccino_context::hook_markers`) so
//! `on` repairs them to the new form and `off` prunes either.

use impeccino_core::js;
use impeccino_detect::design_decisions::DesignDecisions;
use serde_json::{Map, Value};
use std::io::Write;
use std::path::Path;

use crate::hook_lib::*;
use crate::util::{exists, json_pretty, jsp, obj_field, safe_read};

const ACTIONS: &[&str] = &["status", "on", "off", "reset"];
/// The suppression actions that wrote the retired config file.
const RETIRED_ACTIONS: &[&str] = &["ignore-rule", "ignore-file", "ignore-value"];
const TIMEOUT_SECONDS: i64 = 5;
const STATUS_MESSAGE: &str = "Checking UI changes";
const STOP_TIMEOUT_SECONDS: i64 = 30;
const STOP_STATUS_MESSAGE: &str = "Design deep pass";
fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

fn command_hook(command: &str, timeout: i64, status: &str) -> Value {
    obj(vec![
        ("type", Value::from("command")),
        ("command", Value::from(command)),
        ("timeout", Value::from(timeout)),
        ("statusMessage", Value::from(status)),
    ])
}

/// A command hook with the `commandWindows` sibling Codex 0.146.0+ selects
/// on Windows (`command_windows.unwrap_or(command)`), pointing at the
/// launcher's `.cmd` shim so the same `.codex/hooks.json` runs on every OS.
fn command_hook_with_windows(command: &str, windows: &str, timeout: i64, status: &str) -> Value {
    obj(vec![
        ("type", Value::from("command")),
        ("command", Value::from(command)),
        ("commandWindows", Value::from(windows)),
        ("timeout", Value::from(timeout)),
        ("statusMessage", Value::from(status)),
    ])
}

/// JS: stopManifestEntry(command)
fn stop_manifest_entry(command: &str) -> Value {
    obj(vec![(
        "hooks",
        Value::Array(vec![command_hook(
            command,
            STOP_TIMEOUT_SECONDS,
            STOP_STATUS_MESSAGE,
        )]),
    )])
}

fn stop_manifest_entry_with_windows(command: &str, windows: &str) -> Value {
    obj(vec![(
        "hooks",
        Value::Array(vec![command_hook_with_windows(
            command,
            windows,
            STOP_TIMEOUT_SECONDS,
            STOP_STATUS_MESSAGE,
        )]),
    )])
}

/// The launcher paths the manifests invoke, per harness. Project-relative
/// (or `${CLAUDE_PROJECT_DIR}` / repo-root anchored) so a committed manifest
/// resolves on every teammate's checkout.
const CLAUDE_HOOK_COMMAND: &str = "if [ -x \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" ]; then \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook; fi";
const AGENTS_HOOK_COMMAND: &str = "if [ -x \"$(git rev-parse --show-toplevel)/.agents/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.agents/skills/impeccino/scripts/impeccino\" hook; fi";
const AGENTS_HOOK_COMMAND_WINDOWS: &str = "for /f \"delims=\" %i in ('git rev-parse --show-toplevel') do if exist \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" hook";
const CURSOR_HOOK_COMMAND: &str = "if [ -x \"$(git rev-parse --show-toplevel)/.cursor/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.cursor/skills/impeccino/scripts/impeccino\" hook-before-edit; fi";
const GITHUB_HOOK_COMMAND: &str = "if [ -x \"$(git rev-parse --show-toplevel)/.github/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.github/skills/impeccino/scripts/impeccino\" hook; fi";

struct ManifestTarget {
    provider: &'static str,
    skill_rel: &'static str,
    dest_rel: &'static str,
    shared_dest_rel: Option<&'static str>,
    manifest: fn(&str, Option<&str>, Option<&[String]>) -> Value,
    command: &'static str,
    command_windows: Option<&'static str>,
    /// The manifest is the user's whole settings file (model, auth, MCP), so
    /// an unreadable one is skipped instead of backed up and replaced.
    user_settings: bool,
}

fn command_hook_with_args(command: &str, args: &[String], timeout: i64, status: &str) -> Value {
    obj(vec![
        ("type", Value::from("command")),
        ("command", Value::from(command)),
        (
            "args",
            Value::Array(args.iter().cloned().map(Value::from).collect()),
        ),
        ("timeout", Value::from(timeout)),
        ("statusMessage", Value::from(status)),
    ])
}

fn claude_hook(command: &str, args: Option<&[String]>, timeout: i64, status: &str) -> Value {
    match args {
        Some(args) => command_hook_with_args(command, args, timeout, status),
        None => command_hook(command, timeout, status),
    }
}

fn claude_manifest(cmd: &str, _windows: Option<&str>, args: Option<&[String]>) -> Value {
    let stop_entry = match args {
        Some(args) => obj(vec![(
            "hooks",
            Value::Array(vec![claude_hook(
                cmd,
                Some(args),
                STOP_TIMEOUT_SECONDS,
                STOP_STATUS_MESSAGE,
            )]),
        )]),
        None => stop_manifest_entry(cmd),
    };
    obj(vec![
        (
            "description",
            // JS: Claude Code folded multi-edit behavior into Edit; the manifest
            // tracks the current Edit and Write tools (upstream 7d5c60d2).
            Value::from("Impeccino design detector: immediate-tier checks after Edit/Write on UI files, full-rule deep pass on Stop."),
        ),
        (
            "hooks",
            obj(vec![
                (
                    "PostToolUse",
                    Value::Array(vec![obj(vec![
                        ("matcher", Value::from("Edit|Write")),
                        ("hooks", Value::Array(vec![claude_hook(cmd, args, TIMEOUT_SECONDS, STATUS_MESSAGE)])),
                    ])]),
                ),
                ("Stop", Value::Array(vec![stop_entry])),
            ]),
        ),
    ])
}

fn agents_manifest(cmd: &str, windows: Option<&str>, _args: Option<&[String]>) -> Value {
    let win = windows.unwrap_or(cmd);
    obj(vec![(
        "hooks",
        obj(vec![
            (
                "PostToolUse",
                Value::Array(vec![obj(vec![
                    ("matcher", Value::from("Edit|Write|apply_patch")),
                    (
                        "hooks",
                        Value::Array(vec![command_hook_with_windows(
                            cmd,
                            win,
                            TIMEOUT_SECONDS,
                            STATUS_MESSAGE,
                        )]),
                    ),
                ])]),
            ),
            (
                "Stop",
                Value::Array(vec![stop_manifest_entry_with_windows(cmd, win)]),
            ),
        ]),
    )])
}

fn cursor_manifest(command: &str, _windows: Option<&str>, _args: Option<&[String]>) -> Value {
    obj(vec![
        ("version", Value::from(1)),
        (
            "hooks",
            obj(vec![(
                "preToolUse",
                Value::Array(vec![obj(vec![
                    ("command", Value::from(command)),
                    ("timeout", Value::from(TIMEOUT_SECONDS)),
                ])]),
            )]),
        ),
    ])
}

fn github_manifest(command: &str, _windows: Option<&str>, _args: Option<&[String]>) -> Value {
    obj(vec![
        ("version", Value::from(1)),
        (
            "hooks",
            obj(vec![(
                "postToolUse",
                Value::Array(vec![obj(vec![
                    ("type", Value::from("command")),
                    ("matcher", Value::from("edit|create|apply_patch")),
                    ("bash", Value::from(command)),
                    ("timeoutSec", Value::from(TIMEOUT_SECONDS)),
                ])]),
            )]),
        ),
    ])
}

const HOOK_MANIFEST_TARGETS: &[ManifestTarget] = &[
    ManifestTarget {
        provider: ".claude",
        skill_rel: ".claude/skills/impeccino",
        dest_rel: ".claude/settings.local.json",
        shared_dest_rel: Some(".claude/settings.json"),
        manifest: claude_manifest,
        command: CLAUDE_HOOK_COMMAND,
        command_windows: None,
        user_settings: true,
    },
    ManifestTarget {
        provider: ".agents",
        skill_rel: ".agents/skills/impeccino",
        dest_rel: ".codex/hooks.json",
        shared_dest_rel: None,
        manifest: agents_manifest,
        command: AGENTS_HOOK_COMMAND,
        command_windows: Some(AGENTS_HOOK_COMMAND_WINDOWS),
        user_settings: false,
    },
    ManifestTarget {
        provider: ".cursor",
        skill_rel: ".cursor/skills/impeccino",
        dest_rel: ".cursor/hooks.json",
        shared_dest_rel: None,
        manifest: cursor_manifest,
        command: CURSOR_HOOK_COMMAND,
        command_windows: None,
        user_settings: false,
    },
    ManifestTarget {
        provider: ".github",
        skill_rel: ".github/skills/impeccino",
        dest_rel: ".github/hooks/impeccino.json",
        shared_dest_rel: None,
        manifest: github_manifest,
        command: GITHUB_HOOK_COMMAND,
        command_windows: None,
        user_settings: false,
    },
];

struct SelectedTarget {
    target: &'static ManifestTarget,
    command: String,
    command_windows: Option<String>,
    command_args: Option<Vec<String>>,
}

fn powershell_launcher_args(launcher: &Path) -> Vec<String> {
    let path = launcher
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "''");
    let script =
        format!("if (Test-Path -LiteralPath '{path}' -PathType Leaf) {{ & '{path}' hook }}");
    vec!["-NoProfile".into(), "-Command".into(), script]
}

fn launcher_files_present(skill_dir: &str) -> bool {
    let unix = jsp::join(&[skill_dir, "scripts/impeccino"]);
    let windows = jsp::join(&[skill_dir, "scripts/impeccino.cmd"]);
    let unix_ok = std::fs::metadata(&unix)
        .map(|m| m.is_file())
        .unwrap_or(false);
    let windows_ok = std::fs::metadata(&windows)
        .map(|m| m.is_file())
        .unwrap_or(false);
    #[cfg(unix)]
    let unix_ok = {
        use std::os::unix::fs::PermissionsExt;
        unix_ok
            && std::fs::metadata(&unix)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    };
    unix_ok && windows_ok
}

fn global_claude_target(rt: &Runtime, cwd: &str) -> Option<SelectedTarget> {
    let declared_skill = rt.env("IMPECCINO_SKILL_DIR")?;
    let provider = impeccino_context::provider::detect(&rt.env, cwd);
    if provider.id != "claude-code" {
        return None;
    }
    let installed_skill = jsp::resolve(cwd, &[declared_skill]);
    let global_skill = jsp::join(&[&rt.homedir(), ".claude/skills/impeccino"]);
    let installed_canonical = std::fs::canonicalize(&installed_skill).ok()?;
    let global_canonical = std::fs::canonicalize(&global_skill).ok()?;
    if installed_canonical != global_canonical || !launcher_files_present(&installed_skill) {
        return None;
    }
    let launcher_name = if cfg!(windows) {
        "impeccino.cmd"
    } else {
        "impeccino"
    };
    // `canonicalize` is only for identity comparison. On Windows it returns
    // extended-length `\\?\` paths that PowerShell and Git Bash may not accept
    // from a command line, so invoke the normalized installed-skill path.
    let launcher = Path::new(&installed_skill)
        .join("scripts")
        .join(launcher_name);
    let (command, command_args) = if cfg!(windows) {
        (
            "powershell.exe".to_string(),
            Some(powershell_launcher_args(&launcher)),
        )
    } else {
        let executable =
            impeccino_common::quote_executable_path(&launcher.to_string_lossy(), false);
        (
            format!("if [ -x {executable} ]; then {executable} hook; fi"),
            None,
        )
    };
    Some(SelectedTarget {
        target: &HOOK_MANIFEST_TARGETS[0],
        command,
        command_windows: None,
        command_args,
    })
}

fn git_root_from(start: &str) -> Option<String> {
    let mut dir = jsp::resolve(start, &[]);
    loop {
        if impeccino_context::context::has_git_boundary(&dir) {
            return Some(dir);
        }
        let parent = jsp::dirname(&dir);
        if parent == dir {
            return None;
        }
        dir = parent;
    }
}

fn escape_posix_double_quoted(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

fn anchored_command(command: &str, repo_prefix: &str) -> String {
    if repo_prefix.is_empty() {
        return command.to_string();
    }
    let prefix = escape_posix_double_quoted(repo_prefix);
    command.replace(
        "$(git rev-parse --show-toplevel)/",
        &format!("$(git rev-parse --show-toplevel)/{prefix}/"),
    )
}

fn anchored_windows_command(command: &str, repo_prefix: &str) -> String {
    if repo_prefix.is_empty() {
        return command.to_string();
    }
    let prefix = repo_prefix.replace('/', "\\");
    command.replace("\"%i\\", &format!("\"%i\\{prefix}\\"))
}

fn select_targets(rt: &Runtime, cwd: &str, repo_root: &str) -> Result<Vec<SelectedTarget>, String> {
    let mut selected = Vec::new();
    for target in HOOK_MANIFEST_TARGETS {
        let skill_dir = jsp::join(&[repo_root, target.skill_rel]);
        if !exists(&skill_dir) {
            continue;
        }
        if !launcher_files_present(&skill_dir) {
            return Err(format!(
                "Found {} but {}/scripts/impeccino is missing or not executable (and the Windows launcher is required for portability); reinstall Impeccino into that project skill before enabling hooks.",
                target.skill_rel,
                target.skill_rel
            ));
        }
        let mut selected_target = SelectedTarget {
            target,
            command: target.command.to_string(),
            command_windows: target.command_windows.map(str::to_string),
            command_args: None,
        };
        if cfg!(windows) && target.provider == ".claude" {
            let launcher = Path::new(&skill_dir).join("scripts").join("impeccino.cmd");
            selected_target.command = "powershell.exe".to_string();
            selected_target.command_args = Some(powershell_launcher_args(&launcher));
        }
        selected.push(selected_target);
    }
    if let Some(global) = global_claude_target(rt, cwd) {
        if !selected
            .iter()
            .any(|entry| entry.target.provider == ".claude")
        {
            selected.push(global);
        }
    }
    if selected
        .iter()
        .any(|entry| entry.target.provider != ".claude")
    {
        let git_root = git_root_from(repo_root).ok_or_else(|| {
            "Codex, Cursor, and Copilot hook launchers need a Git repository root so their shared commands work from nested directories. Initialize or enter the project's Git repository, then run hooks on again.".to_string()
        })?;
        let repo_prefix = jsp::relative(&git_root, &git_root, repo_root);
        if selected
            .iter()
            .any(|entry| entry.target.command_windows.is_some() && repo_prefix.contains('%'))
        {
            return Err("Codex hook manifests need a Windows command that is portable across teammates. This project's workspace path contains `%`, which Windows command parsing expands as an environment variable; move the workspace to a path without `%` before enabling hooks.".to_string());
        }
        for entry in &mut selected {
            if entry.target.provider == ".claude" {
                continue;
            }
            entry.command = anchored_command(&entry.command, &repo_prefix);
            if let Some(windows) = &entry.command_windows {
                entry.command_windows = Some(anchored_windows_command(windows, &repo_prefix));
            }
        }
    }
    if selected.is_empty() {
        return Err(format!(
            "No supported project-local Impeccino launcher was found. Install the skill in this project (for example .claude/skills/impeccino, .agents/skills/impeccino, .cursor/skills/impeccino, or .github/skills/impeccino), then run {} hooks on. A global skill does not enable hooks for every project; only a detected global Claude skill can opt this project in through its local settings.",
            rt.impeccino_command
        ));
    }
    Ok(selected)
}

fn root_has_hook_scope(root: &str) -> bool {
    HOOK_MANIFEST_TARGETS.iter().any(|target| {
        exists(&jsp::join(&[root, target.skill_rel]))
            || std::iter::once(target.dest_rel)
                .chain(target.shared_dest_rel)
                .any(|rel| manifest_has_or_may_have_impeccino_hook(&jsp::join(&[root, rel])))
    })
}

fn project_manifest_root(rt: &Runtime) -> String {
    let resolved = impeccino_context::context::resolve_project(
        &rt.proc_cwd,
        &impeccino_context::target_args::TargetOptions::default(),
        &rt.env,
    );
    let cwd = jsp::resolve(&rt.proc_cwd, &[]);
    let manifest_boundary = git_root_from(&cwd).unwrap_or_else(|| resolved.repo_root.clone());
    let mut dir = cwd.clone();
    loop {
        if root_has_hook_scope(&dir) {
            return dir;
        }
        if dir == manifest_boundary {
            break;
        }
        let parent = jsp::dirname(&dir);
        if parent == dir
            || !impeccino_context::context::is_path_inside_or_equal(&parent, &manifest_boundary)
        {
            break;
        }
        dir = parent;
    }
    if root_has_hook_scope(&resolved.project_root) {
        return resolved.project_root;
    }
    if resolved.is_monorepo || resolved.repo_root != cwd {
        return resolved.project_root;
    }
    let mut dir = cwd;
    loop {
        if impeccino_context::context::has_git_boundary(&dir) {
            return dir;
        }
        let parent = jsp::dirname(&dir);
        if parent == dir {
            return resolved.repo_root;
        }
        dir = parent;
    }
}

fn reject_symlink(path: &str) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(format!("refusing to modify symlink manifest {}", path))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// JS: statusReport(cwd)
fn status_report(rt: &Runtime, cwd: &str, manifest_root: &str) -> String {
    let env_state = match rt.env("IMPECCINO_HOOK_DISABLED").filter(|v| !v.is_empty()) {
        Some(v) => format!("IMPECCINO_HOOK_DISABLED={v}"),
        None => "unset".to_string(),
    };
    let mut installed: Vec<String> = Vec::new();
    let mut uncertain: Vec<String> = Vec::new();
    for target in HOOK_MANIFEST_TARGETS {
        for rel in std::iter::once(target.dest_rel).chain(target.shared_dest_rel) {
            let path = jsp::join(&[manifest_root, rel]);
            if file_has_impeccino_hook_marker(&path) {
                installed.push(rel.to_string());
            } else if manifest_may_contain_impeccino_hook(&path) {
                uncertain.push(rel.to_string());
            }
        }
    }
    let decisions = DesignDecisions::load_for_dir(cwd);
    let design = match &decisions.source {
        Some(path) => {
            let list = |v: &[String]| {
                if v.is_empty() {
                    "none".to_string()
                } else {
                    v.join(", ")
                }
            };
            format!(
                "{} (waived rules: {}; declared fonts: {})",
                rel_or(rt, cwd, path),
                list(&decisions.waived_rules),
                list(&decisions.declared_fonts)
            )
        }
        None => "not present (no project waivers)".to_string(),
    };
    let cache_path = get_cache_path(cwd);
    [
        "Impeccino design hook".to_string(),
        format!(
            "  installed:    {}",
            if installed.is_empty() && uncertain.is_empty() {
                format!("no (run {} hooks on to install)", rt.impeccino_command)
            } else if installed.is_empty() {
                format!(
                    "unknown (could not safely inspect: {})",
                    uncertain.join(", ")
                )
            } else if !uncertain.is_empty() {
                format!(
                    "{} (could not safely inspect: {})",
                    installed.join(", "),
                    uncertain.join(", ")
                )
            } else {
                installed.join(", ")
            }
        ),
        format!("  env override: {env_state}"),
        format!("  DESIGN.md:    {design}"),
        format!(
            "  cache file:   {}",
            if exists(&cache_path) {
                cache_path
            } else {
                format!("{cache_path} (not present)")
            }
        ),
    ]
    .join("\n")
}

fn rel_or(rt: &Runtime, cwd: &str, target: &str) -> String {
    let r = rt.relative(cwd, target);
    if r.is_empty() {
        target.to_string()
    } else {
        r
    }
}

/// `hooks on`: install or repair the hook entries in every harness whose
/// skill folder is present.
fn install(rt: &Runtime, cwd: &str, manifest_root: &str) -> Result<String, String> {
    let repaired = repair_hook_manifests(rt, cwd, manifest_root)?;
    let mut parts = Vec::new();
    if !repaired.written.is_empty() {
        parts.push(format!(
            "Installed or repaired hook manifests for: {}.",
            repaired.written.join(", ")
        ));
    } else if !repaired.already.is_empty() {
        parts.push(format!(
            "Hook manifests already installed for: {}.",
            repaired.already.join(", ")
        ));
    }
    if !repaired.skipped.is_empty() {
        let partial = if repaired.written.is_empty() && repaired.already.is_empty() {
            String::new()
        } else {
            format!(
                " Other project manifests may already be installed for: {}.",
                repaired
                    .written
                    .iter()
                    .chain(&repaired.already)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        return Err(format!(
            "Hook setup is incomplete.{partial} Skipped {} because it is not a valid JSON object for hook settings; the file was preserved. Repair it and run {} hooks on again.",
            repaired.skipped.join(", "),
            rt.impeccino_command
        ));
    }
    if !repaired.backups.is_empty() {
        let names: Vec<String> = repaired
            .backups
            .iter()
            .map(|b| rel_or(rt, cwd, b))
            .collect();
        parts.push(format!(
            "Backed up malformed manifest(s): {}.",
            names.join(", ")
        ));
    }
    Ok(parts.join(" "))
}

/// Remove the hook entries from the local manifests `on` writes. A team-shared
/// manifest (Claude Code's `settings.json`) is never edited; the returned
/// note names it when it still installs the hook.
fn uninstall(rt: &Runtime, manifest_root: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut pruned: Vec<String> = Vec::new();
    let mut shared_left: Vec<String> = Vec::new();
    for target in HOOK_MANIFEST_TARGETS {
        let dest = jsp::join(&[manifest_root, target.dest_rel]);
        if manifest_may_contain_impeccino_hook(&dest) {
            return Err(format!(
                "Cannot safely turn off hooks because {} is unreadable or malformed and may contain an Impeccino entry. Repair the JSON, then run {} hooks off again.",
                target.dest_rel,
                rt.impeccino_command
            ));
        }
        if file_has_impeccino_hook_marker(&dest) {
            reject_symlink(&dest)?;
        }
        if prune_impeccino_hook_from_manifest(&dest)? {
            pruned.push(target.provider.to_string());
        }
        if let Some(shared) = target.shared_dest_rel {
            if file_has_impeccino_hook_marker(&jsp::join(&[manifest_root, shared])) {
                shared_left.push(shared.to_string());
            }
        }
    }
    Ok((pruned, shared_left))
}

fn shared_note(shared_left: &[String]) -> Option<String> {
    if shared_left.is_empty() {
        return None;
    }
    Some(format!(
        "Still installed in {}, which the team shares; remove the entry there to turn the hook off for everyone, or set IMPECCINO_HOOK_DISABLED=1 for yourself.",
        shared_left.join(", ")
    ))
}

/// `hooks off`: remove the hook entries.
fn turn_off(rt: &Runtime, manifest_root: &str) -> Result<String, String> {
    let (pruned, shared_left) = uninstall(rt, manifest_root)?;
    let mut parts = vec![if pruned.is_empty() {
        "No local hook entries to remove.".to_string()
    } else {
        format!("Removed hook entries from: {}.", pruned.join(", "))
    }];
    parts.extend(shared_note(&shared_left));
    Ok(parts.join(" "))
}

struct Repaired {
    written: Vec<String>,
    already: Vec<String>,
    backups: Vec<String>,
    skipped: Vec<String>,
}

/// JS: repairHookManifests(cwd)
fn repair_hook_manifests(rt: &Runtime, cwd: &str, manifest_root: &str) -> Result<Repaired, String> {
    let targets = select_targets(rt, cwd, manifest_root)?;
    let mut result = Repaired {
        written: vec![],
        already: vec![],
        backups: vec![],
        skipped: vec![],
    };
    for selected in targets {
        let target = selected.target;
        let dest = jsp::join(&[manifest_root, target.dest_rel]);
        reject_symlink(&dest)?;
        let shared_dest = target
            .shared_dest_rel
            .map(|s| jsp::join(&[manifest_root, s]));
        if let Some(sd) = &shared_dest {
            if file_has_impeccino_hook_marker(sd) {
                prune_impeccino_hook_from_manifest(&dest)?;
                result.already.push(target.provider.to_string());
                continue;
            }
        }
        let fresh = (target.manifest)(
            &selected.command,
            selected.command_windows.as_deref(),
            selected.command_args.as_deref(),
        );
        let mut next = fresh.clone();
        let mut had_comments = false;
        if exists(&dest) {
            match read_manifest(&dest) {
                Some((existing, commented)) => {
                    had_comments = commented;
                    next = merge_hook_manifests(&existing, &fresh);
                }
                // The user's whole settings file: never replace it.
                None if target.user_settings => {
                    result.skipped.push(target.dest_rel.to_string());
                    continue;
                }
                None => {
                    let backup = backup_without_overwrite(&dest)?;
                    result.backups.push(backup);
                }
            }
        }
        let serialized = format!("{}\n", json_pretty(&next));
        let current = if exists(&dest) {
            safe_read(&dest)
        } else {
            None
        };
        if current.as_deref() == Some(serialized.as_str()) {
            result.already.push(target.provider.to_string());
            continue;
        }
        if let Some(backup) = backup_commented(&dest, had_comments)? {
            result.backups.push(backup);
        }
        std::fs::create_dir_all(jsp::dirname(&dest)).map_err(|e| e.to_string())?;
        impeccino_common::atomic_file::write(Path::new(&dest), serialized.as_bytes())
            .map_err(|e| e.to_string())?;
        result.written.push(target.provider.to_string());
    }
    Ok(result)
}

fn as_object(v: &Value) -> Map<String, Value> {
    match v {
        Value::Object(o) => o.clone(),
        _ => Map::new(),
    }
}

/// JS: mergeHookManifests(existing, fresh)
fn merge_hook_manifests(existing: &Value, fresh: &Value) -> Value {
    let existing_object = as_object(existing);
    let fresh_object = as_object(fresh);
    let existing_hooks = obj_field(&existing_object, "hooks")
        .cloned()
        .unwrap_or_default();
    let fresh_hooks = obj_field(&fresh_object, "hooks")
        .cloned()
        .unwrap_or_default();
    let mut merged = existing_object.clone();
    merged.insert("hooks".into(), Value::Object(Map::new()));
    if let Some(v) = fresh_object.get("version") {
        merged.insert("version".into(), v.clone());
    }
    if let Some(d) = fresh_object.get("description") {
        merged.insert("description".into(), d.clone());
    }
    let mut events: Vec<String> = existing_hooks.keys().cloned().collect();
    for k in fresh_hooks.keys() {
        if !events.contains(k) {
            events.push(k.clone());
        }
    }
    let mut hooks = Map::new();
    for event in events {
        let preserved = strip_impeccino_hook_entries(existing_hooks.get(&event));
        let added: Vec<Value> = match fresh_hooks.get(&event) {
            Some(Value::Array(a)) => a.clone(),
            _ => vec![],
        };
        let mut merged_entries = preserved;
        merged_entries.extend(added);
        if !merged_entries.is_empty() {
            hooks.insert(event, Value::Array(merged_entries));
        }
    }
    merged.insert("hooks".into(), Value::Object(hooks));
    Value::Object(merged)
}

/// A manifest parsed with comments tolerated; the flag says a rewrite would
/// drop those comments.
fn read_manifest(path: &str) -> Option<(Value, bool)> {
    let (parsed, had_comments) =
        impeccino_context::hook_markers::parse_manifest_jsonc(&safe_read(path)?)?;
    parsed.as_object()?;
    Some((parsed, had_comments))
}

/// Keep the original of a commented manifest before the JSON writer drops
/// its comments.
fn backup_without_overwrite(path: &str) -> Result<String, String> {
    let contents = std::fs::read(path).map_err(|e| e.to_string())?;
    for suffix in 0..10_000 {
        let backup = if suffix == 0 {
            format!("{path}.bak")
        } else {
            format!("{path}.bak.{suffix}")
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&backup) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&contents).and_then(|_| file.sync_all()) {
                    drop(file);
                    let _ = std::fs::remove_file(&backup);
                    return Err(error.to_string());
                }
                return Ok(backup);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(format!("could not choose an unused backup path for {path}"))
}

fn backup_commented(path: &str, had_comments: bool) -> Result<Option<String>, String> {
    if had_comments {
        backup_without_overwrite(path).map(Some)
    } else {
        Ok(None)
    }
}

/// JS: fileHasImpeccinoHookMarker(filePath)
fn file_has_impeccino_hook_marker(path: &str) -> bool {
    if !exists(path) {
        return false;
    }
    let Some((parsed, _)) = read_manifest(path) else {
        return false;
    };
    let Value::Object(o) = parsed else {
        return false;
    };
    match o.get("hooks") {
        Some(h @ (Value::Object(_) | Value::Array(_))) => value_has_impeccino_hook_marker(h),
        _ => false,
    }
}

fn manifest_has_or_may_have_impeccino_hook(path: &str) -> bool {
    if !exists(path) {
        return false;
    }
    if file_has_impeccino_hook_marker(path) {
        return true;
    }
    safe_read(path).is_some_and(|text| {
        let valid_object = impeccino_context::hook_markers::parse_manifest_jsonc(&text)
            .map(|(value, _)| value.is_object())
            .unwrap_or(false);
        !valid_object && text.to_ascii_lowercase().contains("impeccino")
    })
}

fn manifest_may_contain_impeccino_hook(path: &str) -> bool {
    if !exists(path) {
        return false;
    }
    let Some(text) = safe_read(path) else {
        return true;
    };
    match impeccino_context::hook_markers::parse_manifest_jsonc(&text) {
        Some((value, _)) if value.is_object() => false,
        _ => text.to_ascii_lowercase().contains("impeccino"),
    }
}

/// JS: valueHasImpeccinoHookMarker(value)
fn value_has_impeccino_hook_marker(value: &Value) -> bool {
    match value {
        Value::String(s) => impeccino_context::hook_markers::is_impeccino_hook_command(s),
        Value::Array(a) => a.iter().any(value_has_impeccino_hook_marker),
        Value::Object(o) => o.values().any(value_has_impeccino_hook_marker),
        _ => false,
    }
}

fn marker_in(entry: &Map<String, Value>, key: &str) -> bool {
    entry
        .get(key)
        .map(value_has_impeccino_hook_marker)
        .unwrap_or(false)
}

/// JS: stripImpeccinoHookEntry(entry) — `None` drops the entry.
fn strip_impeccino_hook_entry(entry: &Value) -> Option<Value> {
    let Value::Object(e) = entry else {
        // JS: `!entry || typeof entry !== 'object'` returns the entry as-is
        // (a null/primitive survives until `.filter(Boolean)`; an array is
        // an object with none of the inspected keys and no `hooks` array).
        return Some(entry.clone());
    };
    if marker_in(e, "command")
        || marker_in(e, "commandWindows")
        || marker_in(e, "args")
        || marker_in(e, "bash")
        || marker_in(e, "powershell")
    {
        return None;
    }
    let Some(Value::Array(hooks)) = e.get("hooks") else {
        return Some(entry.clone());
    };
    let stripped: Vec<Value> = hooks
        .iter()
        .filter_map(strip_impeccino_hook_entry)
        .filter(truthy_json)
        .collect();
    if stripped.is_empty() && hooks.iter().any(value_has_impeccino_hook_marker) {
        return None;
    }
    let mut out = e.clone();
    out.insert("hooks".into(), Value::Array(stripped));
    Some(Value::Object(out))
}

/// JS `.filter(Boolean)` on the mapped entries.
fn truthy_json(v: &Value) -> bool {
    crate::util::truthy_value(Some(v))
}

/// JS: stripImpeccinoHookEntries(entries)
fn strip_impeccino_hook_entries(entries: Option<&Value>) -> Vec<Value> {
    match entries {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(strip_impeccino_hook_entry)
            .filter(truthy_json)
            .collect(),
        _ => vec![],
    }
}

fn remove_manifest_file_with(
    path: &str,
    remove: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<(), String> {
    remove(path).map_err(|error| format!("could not remove hook manifest {path}: {error}"))
}

fn remove_state_file_with(
    path: &str,
    remove: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<bool, String> {
    match remove(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not remove hook state file {path}: {error}")),
    }
}

/// JS: pruneImpeccinoHookFromManifest(manifestPath)
fn prune_impeccino_hook_from_manifest(path: &str) -> Result<bool, String> {
    if !file_has_impeccino_hook_marker(path) {
        return Ok(false);
    }
    reject_symlink(path)?;
    let Some((parsed, had_comments)) = read_manifest(path) else {
        return Ok(false);
    };
    let _ = backup_commented(path, had_comments)?;
    let parsed = as_object(&parsed);
    let existing_hooks = obj_field(&parsed, "hooks").cloned().unwrap_or_default();
    let mut cleaned = Map::new();
    for (event, entries) in &existing_hooks {
        let kept = strip_impeccino_hook_entries(Some(entries));
        if !kept.is_empty() {
            cleaned.insert(event.clone(), Value::Array(kept));
        }
    }
    let mut next = parsed.clone();
    if !cleaned.is_empty() {
        next.insert("hooks".into(), Value::Object(cleaned));
    } else {
        next.shift_remove("hooks");
        next.shift_remove("description");
        next.shift_remove("version");
    }
    if next.is_empty() {
        remove_manifest_file_with(path, |path| std::fs::remove_file(path))?;
    } else {
        impeccino_common::atomic_file::write(
            Path::new(path),
            format!("{}\n", json_pretty(&Value::Object(next))).as_bytes(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// `hooks reset`: remove the hook entries (issue #512: a leftover entry
/// keeps invoking the hook) and the session cache in the user cache.
fn reset(rt: &Runtime, cwd: &str, manifest_root: &str) -> Result<String, String> {
    let (pruned, shared_left) = uninstall(rt, manifest_root)?;
    let mut removed: Vec<String> = Vec::new();
    let file_path = get_cache_path(cwd);
    if remove_state_file_with(&file_path, |path| std::fs::remove_file(path))? {
        removed.push(file_path);
    }
    // The per-project cache dir is ours alone; drop it once it is empty.
    let _ = std::fs::remove_dir(jsp::dirname(&get_cache_path(cwd)));
    let mut parts: Vec<String> = Vec::new();
    if !pruned.is_empty() {
        parts.push(format!("Removed hook entries from: {}.", pruned.join(", ")));
    }
    if !removed.is_empty() {
        parts.push(format!(
            "Cleared the hook's session cache ({}).",
            removed.join(", ")
        ));
    }
    parts.extend(shared_note(&shared_left));
    let _ = rt;
    Ok(if parts.is_empty() {
        "No hook entries or cache to remove.".to_string()
    } else {
        parts.join(" ")
    })
}

/// `impeccino hooks [action] [args...]` (hook-admin.mjs main). Returns the exit code.
pub fn run(rt: &Runtime, args: &[String], io: &mut impeccino_common::Io) -> i32 {
    let action = js::to_lower_case(
        args.first()
            .map(String::as_str)
            .filter(|a| !a.is_empty())
            .unwrap_or("status"),
    );
    let cwd = rt.proc_cwd.clone();
    let manifest_root = project_manifest_root(rt);
    if RETIRED_ACTIONS.contains(&action.as_str()) {
        io.err(&format!(
            "\"{action}\" was removed: Impeccino keeps no config file. Waive a rule for the whole project with <!-- impeccino-disable <rule>: reason --> in DESIGN.md, declare a deliberate font or color as a DESIGN.md token, keep generated files out with .gitignore or .gitattributes (linguist-generated), or waive one spot with an in-file impeccino-disable comment.\n"
        ));
        return 1;
    }
    if !ACTIONS.contains(&action.as_str()) {
        io.err(&format!(
            "Unknown action: {action}\nValid: {}\n",
            ACTIONS.join(", ")
        ));
        return 1;
    }
    let out = match action.as_str() {
        "status" => Ok(status_report(rt, &cwd, &manifest_root)),
        "on" => install(rt, &cwd, &manifest_root),
        "off" => turn_off(rt, &manifest_root),
        "reset" => reset(rt, &cwd, &manifest_root),
        _ => Ok(String::new()),
    };
    match out {
        Ok(text) => {
            io.out(&format!("{text}\n"));
            0
        }
        Err(message) => {
            io.err(&format!("Error: {message}\n"));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{remove_manifest_file_with, remove_state_file_with};

    #[test]
    fn manifest_removal_failure_is_reported() {
        let error = remove_manifest_file_with(".claude/settings.local.json", |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected delete failure",
            ))
        })
        .unwrap_err();
        assert!(error.contains(".claude/settings.local.json"), "{error}");
        assert!(error.contains("injected delete failure"), "{error}");
    }

    #[test]
    fn reset_state_removal_failure_is_reported_with_path() {
        let error = remove_state_file_with("/cache/hook.cache.json", |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected delete failure",
            ))
        })
        .unwrap_err();
        assert!(error.contains("/cache/hook.cache.json"), "{error}");
        assert!(error.contains("injected delete failure"), "{error}");
    }
}
