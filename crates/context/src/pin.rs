//! JS: pin.mjs -> `impeccino pin <pin|unpin> <command>`

use crate::jsp;
use crate::util::{exists, read_json};
use impeccino_common::Io;

/// `skill/scripts/command-metadata.json`, compiled in as the fallback when the
/// skill directory is not known at run time.
pub const COMMAND_METADATA_JSON: &str = include_str!("../../../skill/scripts/command-metadata.json");

const HARNESS_DIRS: [&str; 18] = [
    ".claude", ".cursor", ".dsh", ".gemini", ".codex", ".agents", ".agent", ".github", ".grok", ".hermes", ".trae", ".trae-cn",
    ".pi", ".opencode", ".kiro", ".rovodev", ".vibe", ".qoder",
];
const CODEX_HARNESSES: [&str; 2] = [".codex", ".agents"];
pub const VALID_COMMANDS: [&str; 22] = [
    "craft", "init", "extract", "document", "shape", "critique", "audit", "polish", "bolder", "quieter", "distill",
    "harden", "onboard", "animate", "colorize", "typeset", "layout", "delight", "overdrive", "clarify",
    "adapt", "optimize",
];
const PIN_MARKER: &str = "<!-- impeccino-pinned-skill -->";
const OPENCODE_PIN_MARKER: &str = "<!-- impeccino-pinned-command -->";

fn find_project_root(start: &str) -> String {
    let mut dir = jsp::resolve(start, &[]);
    while dir != "/" {
        if exists(&jsp::join(&[&dir, "package.json"]))
            || exists(&jsp::join(&[&dir, ".git"]))
            || exists(&jsp::join(&[&dir, "skills-lock.json"]))
        {
            return dir;
        }
        let parent = jsp::resolve(&dir, &[".."]);
        if parent == dir {
            break;
        }
        dir = parent;
    }
    jsp::resolve(start, &[])
}

fn find_harness_dirs(project_root: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    for h in HARNESS_DIRS {
        let skills = jsp::join(&[project_root, h, "skills"]);
        if exists(&jsp::join(&[&skills, "impeccino"])) || exists(&jsp::join(&[&skills, "i-impeccino"])) {
            dirs.push(skills);
        }
    }
    dirs
}

fn command_prefix_for(skills_dir: &str) -> &'static str {
    let harness = jsp::basename(&jsp::dirname(skills_dir));
    if CODEX_HARNESSES.contains(&harness.as_str()) {
        "$"
    } else {
        "/"
    }
}

fn generate_pinned_skill(command: &str, metadata: &serde_json::Value, prefix: &str, is_codex: bool) -> String {
    let entry = metadata.get(command);
    let desc = entry
        .and_then(|e| e.get("description"))
        .and_then(|d| d.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Shortcut for {}impeccino {}.", prefix, command));
    let hint = entry
        .and_then(|e| e.get("argumentHint"))
        .and_then(|d| d.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("[target]");
    let provider_fm = if is_codex {
        format!("metadata:\n  argument-hint: \"{}\"", hint)
    } else {
        format!("argument-hint: \"{}\"\nuser-invocable: true", hint)
    };
    format!(
        "---\nname: {command}\ndescription: \"{desc}\"\n{provider_fm}\n---\n\n{marker}\n\nThis is a pinned shortcut for `{prefix}impeccino {command}`.\n\nInvoke {prefix}impeccino {command}, passing along any arguments provided here, and follow its instructions.\n",
        command = command,
        desc = desc,
        provider_fm = provider_fm,
        marker = PIN_MARKER,
        prefix = prefix
    )
}

// OpenCode 1.18.10 does not honor `user-invocable: true` on SKILL.md
// frontmatter (see docs/HARNESSES.md), so a pinned skill there shows up in
// `opencode debug skill` but never in the slash menu. The fix is a sibling
// `commands/impeccino-<cmd>.md` on the OpenCode command schema. The body
// loads the skill, runs the context verb, then reads the sub-command's
// reference file, so `/impeccino-<cmd>` runs the same workflow
// `/impeccino <cmd>` routes to.
//
// JS: pin.mjs#generatePinnedOpencodeCommand. The JS body says
// `node <skill-base-dir>/scripts/context.mjs`; the engine names its own
// command, as everywhere else the launcher replaced a script path.
fn generate_pinned_opencode_command(command: &str, metadata: &serde_json::Value) -> String {
    let desc = metadata
        .get(command)
        .and_then(|e| e.get("description"))
        .and_then(|d| d.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!("Impeccino sub-command shortcut; runs the {command} workflow via /impeccino.")
        });
    format!(
        "---\ndescription: \"{desc}\"\nagent: build\nsubtask: true\n---\n\n{marker}\n\nLoad the `impeccino` skill via the skill tool (name: \"impeccino\"), then run `<skill-base-dir>/scripts/impeccino context`, then load `<skill-base-dir>/reference/{command}.md` and follow it. `<skill-base-dir>` is the skill's base directory as reported by the skill tool response; substitute the actual absolute path before running or reading anything.\n\n$ARGUMENTS\n",
        desc = desc,
        marker = OPENCODE_PIN_MARKER,
        command = command
    )
}

/// JS: pin.mjs#opencodeUserConfigDir. Mirrors the CLI's precedence
/// (`OPENCODE_CONFIG_DIR` -> `XDG_CONFIG_HOME/opencode` -> `~/.config/opencode`).
fn opencode_user_config_dir(io: &Io) -> String {
    if let Some(v) = io.env.get("OPENCODE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return v.clone();
    }
    if let Some(v) = io.env.get("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return jsp::join(&[v, "opencode"]);
    }
    jsp::join(&[&crate::util::homedir(&io.env), ".config", "opencode"])
}

/// JS: pin.mjs#findOpencodeCommandsDirs. The project-local dir when the
/// project has the skill, plus the user config dir when Impeccino is
/// installed globally. With `for_cleanup`, both are included even when the
/// skill is gone, so unpin can still reach a pin a removed install left
/// behind; removal stays safe because it is marker-guarded.
fn find_opencode_commands_dirs(project_root: &str, io: &Io, for_cleanup: bool) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    let mut push = |dir: String| {
        let key = jsp::resolve(&dir, &[]);
        if !dirs.iter().any(|d| jsp::resolve(d, &[]) == key) {
            dirs.push(dir);
        }
    };
    if for_cleanup || exists(&jsp::join(&[project_root, ".opencode", "skills", "impeccino"])) {
        push(jsp::join(&[project_root, ".opencode", "commands"]));
    }
    let user_config = opencode_user_config_dir(io);
    if for_cleanup || exists(&jsp::join(&[&user_config, "skills", "impeccino"])) {
        push(jsp::join(&[&user_config, "commands"]));
    }
    dirs
}

/// JS: pin.mjs#writePinnedOpencodeCommand
fn write_pinned_opencode_command(
    commands_dir: &str,
    command: &str,
    metadata: &serde_json::Value,
    io: &mut Io,
) -> Result<bool, String> {
    let command_file = jsp::join(&[commands_dir, &format!("impeccino-{command}.md")]);
    if exists(&command_file) {
        let existing = std::fs::read(&command_file)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|err| format!("Could not read {}: {}", command_file, err))?;
        if !existing.contains(OPENCODE_PIN_MARKER) {
            io.out(&format!(
                "  SKIP: {} (non-pinned command already exists)\n",
                command_file
            ));
            return Ok(false);
        }
    } else {
        std::fs::create_dir_all(commands_dir).map_err(|err| format!("Could not create {}: {}", commands_dir, err))?;
    }
    let content = generate_pinned_opencode_command(command, metadata);
    impeccino_common::atomic_file::write(std::path::Path::new(&command_file), content.as_bytes())
        .map_err(|err| format!("Could not write {}: {}", command_file, err))?;
    io.out(&format!("  + {}\n", command_file));
    Ok(true)
}

/// JS: pin.mjs#removePinnedOpencodeCommand
fn remove_pinned_opencode_command(commands_dir: &str, command: &str, io: &mut Io) -> Result<bool, String> {
    let command_file = jsp::join(&[commands_dir, &format!("impeccino-{command}.md")]);
    if !exists(&command_file) {
        return Ok(false);
    }
    let content = std::fs::read(&command_file)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|err| format!("Could not read {}: {}", command_file, err))?;
    if !content.contains(OPENCODE_PIN_MARKER) {
        io.out(&format!("  SKIP: {} (not a pinned command)\n", command_file));
        return Ok(false);
    }
    std::fs::remove_file(&command_file).map_err(|err| format!("Could not remove {}: {}", command_file, err))?;
    io.out(&format!("  - {}\n", command_file));
    Ok(true)
}

fn remove_pinned_skill_with(
    skill_dir: &str,
    io: &mut Io,
    remove: impl FnOnce(&str) -> std::io::Result<()>,
) -> Result<bool, String> {
    if !exists(skill_dir) {
        return Ok(false);
    }
    let md = jsp::join(&[skill_dir, "SKILL.md"]);
    if !exists(&md) {
        return Ok(false);
    }
    let content = std::fs::read(&md)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|err| format!("Could not read {}: {}", md, err))?;
    if !content.contains(PIN_MARKER) {
        io.out(&format!("  SKIP: {} (not a pinned skill)\n", skill_dir));
        return Ok(false);
    }
    remove(skill_dir).map_err(|err| format!("Could not remove {}: {}", skill_dir, err))?;
    io.out(&format!("  - {}\n", skill_dir));
    Ok(true)
}

fn remove_pinned_skill(skill_dir: &str, io: &mut Io) -> Result<bool, String> {
    remove_pinned_skill_with(skill_dir, io, |path| std::fs::remove_dir_all(path))
}

/// JS `skillsDir.includes(`${sep}.opencode${sep}`)`.
fn is_opencode_skills_dir(skills_dir: &str) -> bool {
    skills_dir.contains(&format!("{sep}.opencode{sep}", sep = jsp::SEP))
}

fn load_metadata(io: &Io) -> serde_json::Value {
    // Prefer a sibling command-metadata.json in the skill dir when present
    // (an installed skill may be newer than the embedded copy).
    let cwd = io.cwd.to_string_lossy().into_owned();
    let provider = crate::provider::detect(&io.env, &cwd);
    if let Some(dir) = &provider.skill_dir {
        let p = jsp::join(&[dir, "scripts", "command-metadata.json"]);
        if let Some(v) = read_json(&p) {
            return v;
        }
    }
    serde_json::from_str(COMMAND_METADATA_JSON).unwrap_or(serde_json::Value::Object(Default::default()))
}

pub fn run(args: &[String], io: &mut Io) -> i32 {
    let action = args.first().cloned();
    let command = args.get(1).cloned();
    let (Some(action), Some(command)) = (action.filter(|a| !a.is_empty()), command.filter(|c| !c.is_empty())) else {
        io.out("Usage: impeccino pin <pin|unpin> <command>\n");
        io.out(&format!("\nAvailable commands: {}\n", VALID_COMMANDS.join(", ")));
        return 1;
    };
    if action != "pin" && action != "unpin" {
        io.err(&format!("Unknown action: {}. Use 'pin' or 'unpin'.\n", action));
        return 1;
    }
    if !VALID_COMMANDS.contains(&command.as_str()) {
        io.err(&format!("Unknown command: {}\n", command));
        io.err(&format!("Available commands: {}\n", VALID_COMMANDS.join(", ")));
        return 1;
    }
    let cwd = io.cwd.to_string_lossy().into_owned();
    let root = find_project_root(&cwd);
    if action == "pin" {
        let metadata = load_metadata(io);
        let harness_dirs = find_harness_dirs(&root);
        let opencode_commands_dirs = find_opencode_commands_dirs(&root, io, false);
        if harness_dirs.is_empty() && opencode_commands_dirs.is_empty() {
            io.err("No project harness directories with Impeccino installed were found; install the skill in a project harness before pinning a command.\n");
            return 1;
        }
        let mut created = 0;
        let mut failed = false;
        // OpenCode is handled separately below because its shortcut format is
        // a slash command, not a SKILL.md. Excluding it from the skill loop
        // prevents a duplicate `.opencode/skills/<cmd>/SKILL.md` that OpenCode
        // would never surface as `/<cmd>`.
        for skills_dir in &harness_dirs {
            if is_opencode_skills_dir(skills_dir) {
                continue;
            }
            let prefix = command_prefix_for(skills_dir);
            let content = generate_pinned_skill(&command, &metadata, prefix, prefix == "$");
            let skill_dir = jsp::join(&[skills_dir, &command]);
            if exists(&skill_dir) {
                let md = jsp::join(&[&skill_dir, "SKILL.md"]);
                if exists(&md) {
                    let existing = match std::fs::read(&md) {
                        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                        Err(err) => {
                            io.err(&format!("  ERROR: Could not read {}: {}\n", md, err));
                            failed = true;
                            continue;
                        }
                    };
                    if !existing.contains(PIN_MARKER) {
                        io.out(&format!("  SKIP: {} (non-pinned skill already exists)\n", skill_dir));
                        continue;
                    }
                }
            }
            let skill_file = jsp::join(&[&skill_dir, "SKILL.md"]);
            if let Err(err) = std::fs::create_dir_all(&skill_dir) {
                io.err(&format!("  ERROR: Could not create {}: {}\n", skill_dir, err));
                failed = true;
                continue;
            }
            if let Err(err) = impeccino_common::atomic_file::write(std::path::Path::new(&skill_file), content.as_bytes()) {
                io.err(&format!("  ERROR: Could not write {}: {}\n", skill_file, err));
                failed = true;
                continue;
            }
            io.out(&format!("  + {}\n", skill_dir));
            created += 1;
        }
        // OpenCode: a slash command bridge, not a skill shortcut. Covers both
        // project installs and user-scope (global config) installs.
        for commands_dir in &opencode_commands_dirs {
            match write_pinned_opencode_command(commands_dir, &command, &metadata, io) {
                Ok(true) => created += 1,
                Ok(false) => {}
                Err(err) => {
                    io.err(&format!("  ERROR: {}\n", err));
                    failed = true;
                }
            }
        }
        if created > 0 {
            io.out(&format!("\nPinned '{}' as a standalone shortcut in {} location(s).\n", command, created));
            io.out("Use the pinned command directly in each harness.\n");
        }
        if failed { 1 } else { 0 }
    } else {
        let harness_dirs = find_harness_dirs(&root);
        let mut removed = 0;
        let mut failed = false;
        // OpenCode has its own cleanup path below; skip the skill loop here so
        // a stray `.opencode/skills/<cmd>/SKILL.md` written by an older
        // Impeccino version is never silently dropped.
        for skills_dir in &harness_dirs {
            if is_opencode_skills_dir(skills_dir) {
                continue;
            }
            let skill_dir = jsp::join(&[skills_dir, &command]);
            match remove_pinned_skill(&skill_dir, io) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(err) => {
                    io.err(&format!("  ERROR: {}\n", err));
                    failed = true;
                }
            }
        }
        // OpenCode: remove the pinned command file if it is one of ours, in
        // every scope it could have been written to, even when the skill
        // itself is gone, since removal is marker-guarded.
        for commands_dir in find_opencode_commands_dirs(&root, io, true) {
            match remove_pinned_opencode_command(&commands_dir, &command, io) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(err) => {
                    io.err(&format!("  ERROR: {}\n", err));
                    failed = true;
                }
            }
        }
        if removed > 0 {
            io.out(&format!("\nUnpinned '{}' from {} location(s).\n", command, removed));
            io.out(&format!("Use Impeccino's '{}' workflow directly to access it.\n", command));
        } else {
            io.out(&format!("No pinned '{}' shortcut found.\n", command));
        }
        if failed { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::{remove_pinned_skill_with, run, PIN_MARKER};
    use impeccino_common::Io;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "impeccino pin write {} {} {}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
                suffix
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn invoke(root: &Path, action: &str) -> (i32, String, String) {
        let home = root.join("isolated-home").to_string_lossy().into_owned();
        let opencode = root.join("isolated-opencode-config").to_string_lossy().into_owned();
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), home.clone());
        env.insert("USERPROFILE".to_string(), home);
        env.insert("OPENCODE_CONFIG_DIR".to_string(), opencode);
        let (mut io, captured) = Io::captured("", root.to_path_buf(), env);
        let code = run(&[action.to_string(), "polish".to_string()], &mut io);
        let stdout = String::from_utf8_lossy(&captured.stdout.borrow()).into_owned();
        let stderr = String::from_utf8_lossy(&captured.stderr.borrow()).into_owned();
        (code, stdout, stderr)
    }

    #[test]
    fn pin_without_an_installed_project_harness_fails_actionably() {
        let root = TempDir::new();
        let (code, stdout, stderr) = invoke(&root.0, "pin");
        assert_eq!(code, 1);
        assert!(stdout.is_empty());
        assert!(stderr.contains("No project harness directories with Impeccino installed"));
    }

    #[test]
    fn pin_reports_path_conflict_and_counts_only_successful_harness_writes() {
        let root = TempDir::new();
        write(&root.0, ".claude/skills/impeccino/SKILL.md", "---\nname: impeccino\n---\n");
        write(&root.0, ".cursor/skills/impeccino/SKILL.md", "---\nname: impeccino\n---\n");
        write(&root.0, ".claude/skills/polish", "a regular file blocks the skill directory\n");

        let (code, stdout, stderr) = invoke(&root.0, "pin");
        assert_eq!(code, 1);
        assert!(stderr.contains("ERROR: Could not create"), "{stderr}");
        assert!(stdout.contains("Pinned 'polish' as a standalone shortcut in 1 location(s)."), "{stdout}");
        assert!(!stdout.contains("  + ") || stdout.matches("  + ").count() == 1, "{stdout}");
        assert!(root.0.join(".cursor/skills/polish/SKILL.md").is_file());
        assert!(root.0.join(".claude/skills/polish").is_file());
    }

    #[test]
    fn opencode_pin_reports_a_commands_directory_conflict() {
        let root = TempDir::new();
        write(&root.0, ".opencode/skills/impeccino/SKILL.md", "---\nname: impeccino\n---\n");
        write(&root.0, ".opencode/commands", "a regular file blocks the commands directory\n");

        let (code, stdout, stderr) = invoke(&root.0, "pin");
        assert_eq!(code, 1);
        assert!(stdout.is_empty(), "{stdout}");
        assert!(stderr.contains("ERROR: Could not create"), "{stderr}");
        assert!(!stdout.contains("Pinned 'polish'"));
    }

    #[test]
    fn unpin_remove_failure_is_not_reported_as_removed() {
        let root = TempDir::new();
        write(&root.0, ".claude/skills/polish/SKILL.md", PIN_MARKER);
        let skill_dir = root.0.join(".claude/skills/polish").to_string_lossy().into_owned();
        let (mut io, captured) = Io::captured("", root.0.clone(), HashMap::new());
        let result = remove_pinned_skill_with(&skill_dir, &mut io, |_| {
            Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected remove failure"))
        });
        assert!(result.unwrap_err().contains("Could not remove"));
        assert!(root.0.join(".claude/skills/polish/SKILL.md").is_file());
        assert!(captured.stdout.borrow().is_empty());
    }
}
