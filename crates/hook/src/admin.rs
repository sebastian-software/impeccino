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
const CLAUDE_HOOK_COMMAND: &str = "\"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook";
const AGENTS_HOOK_COMMAND: &str = "\".agents/skills/impeccino/scripts/impeccino\" hook";
const AGENTS_HOOK_COMMAND_WINDOWS: &str = "\".agents/skills/impeccino/scripts/impeccino.cmd\" hook";
const CURSOR_HOOK_COMMAND: &str = "\".cursor/skills/impeccino/scripts/impeccino\" hook-before-edit";
const GITHUB_HOOK_COMMAND: &str = "\"$(git rev-parse --show-toplevel)/.github/skills/impeccino/scripts/impeccino\" hook";

struct ManifestTarget {
    provider: &'static str,
    skill_rel: &'static str,
    dest_rel: &'static str,
    shared_dest_rel: Option<&'static str>,
    manifest: fn() -> Value,
    /// The manifest is the user's whole settings file (model, auth, MCP), so
    /// an unreadable one is skipped instead of backed up and replaced.
    user_settings: bool,
}

fn claude_manifest() -> Value {
    let cmd = CLAUDE_HOOK_COMMAND;
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
                        ("hooks", Value::Array(vec![command_hook(cmd, TIMEOUT_SECONDS, STATUS_MESSAGE)])),
                    ])]),
                ),
                ("Stop", Value::Array(vec![stop_manifest_entry(cmd)])),
            ]),
        ),
    ])
}

fn agents_manifest() -> Value {
    let cmd = AGENTS_HOOK_COMMAND;
    let win = AGENTS_HOOK_COMMAND_WINDOWS;
    obj(vec![(
        "hooks",
        obj(vec![
            (
                "PostToolUse",
                Value::Array(vec![obj(vec![
                    ("matcher", Value::from("Edit|Write|apply_patch")),
                    (
                        "hooks",
                        Value::Array(vec![command_hook_with_windows(cmd, win, TIMEOUT_SECONDS, STATUS_MESSAGE)]),
                    ),
                ])]),
            ),
            ("Stop", Value::Array(vec![stop_manifest_entry_with_windows(cmd, win)])),
        ]),
    )])
}

fn cursor_manifest() -> Value {
    obj(vec![
        ("version", Value::from(1)),
        (
            "hooks",
            obj(vec![(
                "preToolUse",
                Value::Array(vec![obj(vec![
                    (
                        "command",
                        Value::from(CURSOR_HOOK_COMMAND),
                    ),
                    ("timeout", Value::from(TIMEOUT_SECONDS)),
                ])]),
            )]),
        ),
    ])
}

fn github_manifest() -> Value {
    obj(vec![
        ("version", Value::from(1)),
        (
            "hooks",
            obj(vec![(
                "postToolUse",
                Value::Array(vec![obj(vec![
                    ("type", Value::from("command")),
                    ("matcher", Value::from("edit|create|apply_patch")),
                    (
                        "bash",
                        Value::from(GITHUB_HOOK_COMMAND),
                    ),
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
        user_settings: false,
    },
    ManifestTarget {
        provider: ".agents",
        skill_rel: ".agents/skills/impeccino",
        dest_rel: ".codex/hooks.json",
        shared_dest_rel: None,
        manifest: agents_manifest,
        user_settings: false,
    },
    ManifestTarget {
        provider: ".cursor",
        skill_rel: ".cursor/skills/impeccino",
        dest_rel: ".cursor/hooks.json",
        shared_dest_rel: None,
        manifest: cursor_manifest,
        user_settings: false,
    },
    ManifestTarget {
        provider: ".github",
        skill_rel: ".github/skills/impeccino",
        dest_rel: ".github/hooks/impeccino.json",
        shared_dest_rel: None,
        manifest: github_manifest,
        user_settings: false,
    },
];

/// JS: statusReport(cwd)
fn status_report(rt: &Runtime, cwd: &str) -> String {
    let env_state = match rt.env("IMPECCINO_HOOK_DISABLED").filter(|v| !v.is_empty()) {
        Some(v) => format!("IMPECCINO_HOOK_DISABLED={v}"),
        None => "unset".to_string(),
    };
    let mut installed: Vec<String> = Vec::new();
    for target in HOOK_MANIFEST_TARGETS {
        for rel in std::iter::once(target.dest_rel).chain(target.shared_dest_rel) {
            if file_has_impeccino_hook_marker(&jsp::join(&[cwd, rel])) {
                installed.push(rel.to_string());
            }
        }
    }
    let decisions = DesignDecisions::load_for_dir(cwd);
    let design = match &decisions.source {
        Some(path) => {
            let list = |v: &[String]| if v.is_empty() { "none".to_string() } else { v.join(", ") };
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
            if installed.is_empty() {
                format!("no (run {} hooks on to install)", rt.impeccino_command)
            } else {
                installed.join(", ")
            }
        ),
        format!("  env override: {env_state}"),
        format!("  DESIGN.md:    {design}"),
        format!(
            "  cache file:   {}",
            if exists(&cache_path) { cache_path } else { format!("{cache_path} (not present)") }
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
fn install(rt: &Runtime, cwd: &str) -> Result<String, String> {
    let repaired = repair_hook_manifests(cwd)?;
    let mut parts = Vec::new();
    if !repaired.written.is_empty() {
        parts.push(format!("Installed or repaired hook manifests for: {}.", repaired.written.join(", ")));
    } else if !repaired.already.is_empty() {
        parts.push(format!("Hook manifests already installed for: {}.", repaired.already.join(", ")));
    } else {
        parts.push("No installed provider skill folders found to install into.".to_string());
    }
    if !repaired.skipped.is_empty() {
        parts.push(format!(
            "Skipped {}: not valid JSON, and it holds your other settings too; fix it and re-run.",
            repaired.skipped.join(", ")
        ));
    }
    if !repaired.backups.is_empty() {
        let names: Vec<String> = repaired.backups.iter().map(|b| rel_or(rt, cwd, b)).collect();
        parts.push(format!("Backed up malformed manifest(s): {}.", names.join(", ")));
    }
    Ok(parts.join(" "))
}

/// Remove the hook entries from the local manifests `on` writes. A team-shared
/// manifest (Claude Code's `settings.json`) is never edited; the returned
/// note names it when it still installs the hook.
fn uninstall(cwd: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let mut pruned: Vec<String> = Vec::new();
    let mut shared_left: Vec<String> = Vec::new();
    for target in HOOK_MANIFEST_TARGETS {
        let dest = jsp::join(&[cwd, target.dest_rel]);
        if prune_impeccino_hook_from_manifest(&dest)? {
            pruned.push(target.provider.to_string());
        }
        if let Some(shared) = target.shared_dest_rel {
            if file_has_impeccino_hook_marker(&jsp::join(&[cwd, shared])) {
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
fn turn_off(cwd: &str) -> Result<String, String> {
    let (pruned, shared_left) = uninstall(cwd)?;
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
fn repair_hook_manifests(cwd: &str) -> Result<Repaired, String> {
    let mut result = Repaired {
        written: vec![],
        already: vec![],
        backups: vec![],
        skipped: vec![],
    };
    for target in HOOK_MANIFEST_TARGETS {
        if !exists(&jsp::join(&[cwd, target.skill_rel])) {
            continue;
        }
        let dest = jsp::join(&[cwd, target.dest_rel]);
        let shared_dest = target.shared_dest_rel.map(|s| jsp::join(&[cwd, s]));
        if let Some(sd) = &shared_dest {
            if file_has_impeccino_hook_marker(sd) {
                prune_impeccino_hook_from_manifest(&dest)?;
                result.already.push(target.provider.to_string());
                continue;
            }
        }
        let fresh = (target.manifest)();
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
                    let backup = format!("{dest}.bak");
                    std::fs::copy(&dest, &backup).map_err(|e| e.to_string())?;
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
        backup_commented(&dest, had_comments)?;
        std::fs::create_dir_all(jsp::dirname(&dest)).map_err(|e| e.to_string())?;
        std::fs::write(&dest, serialized).map_err(|e| e.to_string())?;
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

/// A manifest parsed with comments tolerated (Gemini's `settings.json`
/// allows them); the flag says a rewrite would drop comments.
fn read_manifest(path: &str) -> Option<(Value, bool)> {
    impeccino_context::hook_markers::parse_manifest_jsonc(&safe_read(path)?)
}

/// Keep the original of a commented manifest before the JSON writer drops
/// its comments.
fn backup_commented(path: &str, had_comments: bool) -> Result<(), String> {
    if had_comments {
        std::fs::copy(path, format!("{path}.bak")).map_err(|e| e.to_string())?;
    }
    Ok(())
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
        .filter(|v| truthy_json(v))
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
            .filter(|v| truthy_json(v))
            .collect(),
        _ => vec![],
    }
}

/// JS: pruneImpeccinoHookFromManifest(manifestPath)
fn prune_impeccino_hook_from_manifest(path: &str) -> Result<bool, String> {
    if !file_has_impeccino_hook_marker(path) {
        return Ok(false);
    }
    let Some((parsed, had_comments)) = read_manifest(path) else {
        return Ok(false);
    };
    backup_commented(path, had_comments)?;
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
        let _ = std::fs::remove_file(path);
    } else {
        std::fs::write(path, format!("{}\n", json_pretty(&Value::Object(next))))
            .map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// `hooks reset`: remove the hook entries (issue #512: a leftover entry
/// keeps invoking the hook) and the session cache in the user cache.
fn reset(rt: &Runtime, cwd: &str) -> Result<String, String> {
    let (pruned, shared_left) = uninstall(cwd)?;
    let mut removed: Vec<String> = Vec::new();
    for file_path in [get_cache_path(cwd), get_pending_path(cwd)] {
        if exists(&file_path) && std::fs::remove_file(&file_path).is_ok() {
            removed.push(file_path);
        }
    }
    // The per-project cache dir is ours alone; drop it once it is empty.
    let _ = std::fs::remove_dir(jsp::dirname(&get_cache_path(cwd)));
    let mut parts: Vec<String> = Vec::new();
    if !pruned.is_empty() {
        parts.push(format!("Removed hook entries from: {}.", pruned.join(", ")));
    }
    if !removed.is_empty() {
        parts.push(format!("Cleared the hook's session cache ({}).", removed.join(", ")));
    }
    parts.extend(shared_note(&shared_left));
    let _ = rt;
    Ok(if parts.is_empty() { "No hook entries or cache to remove.".to_string() } else { parts.join(" ") })
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
        "status" => Ok(status_report(rt, &cwd)),
        "on" => install(rt, &cwd),
        "off" => turn_off(&cwd),
        "reset" => reset(rt, &cwd),
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
