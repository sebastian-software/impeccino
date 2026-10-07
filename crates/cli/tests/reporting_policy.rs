//! Exercise signal policy through the shipped binary, not raw compatibility vectors.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "impeccino-policy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("package.json"), "{}").unwrap();
        Self(p)
    }
    fn run(&self, args: &[&str], input: &str) -> std::process::Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_impeccino"))
            .args(args)
            .current_dir(&self.0)
            .env("HOME", self.0.join("home"))
            .env("USERPROFILE", self.0.join("home"))
            .env("LOCALAPPDATA", self.0.join("cache"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env("IMPECCINO_CACHE_ROOT", self.0.join("cache"))
            .env_remove("IMPECCINO_HOOK_DISABLED")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const PATTERN: &str =
    ".title {background:linear-gradient(red,blue);background-clip:text;color:transparent;}";

#[test]
fn style_only_source_is_reported_without_primary_failure() {
    let p = Project::new();
    let out = p.run(&["detect", "--no-config", "--json", "-"], PATTERN);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let hits: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(hits
        .iter()
        .any(|h| h["antipattern"] == "gradient-text" && h["advisory"] == true));
}

#[test]
fn static_measured_defects_remain_primary_among_style_signals() {
    let p = Project::new();
    fs::write(
        p.0.join("page.html"),
        format!("<style>{PATTERN}p{{color:#777;background:#666}}</style><h1 class=title>Title</h1><p>Unreadable text</p>"),
    )
    .unwrap();
    let out = p.run(&["detect", "--no-config", "--json", "page.html"], "");
    assert_eq!(out.status.code(), Some(2));
    let hits: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(hits
        .iter()
        .any(|h| h["antipattern"] == "low-contrast" && h["advisory"] != true));
    assert!(hits
        .iter()
        .any(|h| h["antipattern"] == "gradient-text" && h["advisory"] == true));
}

#[test]
fn cursor_does_not_deny_a_style_only_write() {
    let p = Project::new();
    let event = json!({"hook_event_name":"preToolUse", "conversation_id":"policy", "workspace_roots":[p.0], "tool_name":"Write", "tool_input":{"path":"card.css", "content":PATTERN}});
    let out = p.run(&["hook-before-edit"], &event.to_string());
    let payload: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(payload["permission"], "allow", "{payload}");
}

#[test]
fn no_advisory_hides_context_signals_without_hiding_defects() {
    let p = Project::new();
    let out = p.run(
        &["detect", "--no-config", "--no-advisory", "--json", "-"],
        PATTERN,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!([])
    );
}

#[test]
fn creative_tools_preserve_repeatability_without_granting_authority() {
    let p = Project::new();
    fs::write(p.0.join("PRODUCT.md"), "# Product\n\n## Users\nReaders\n").unwrap();
    let args = [
        "concept-seed",
        "--scope",
        "direction",
        "--from",
        "policy",
        "--candidate-count",
        "5",
    ];
    let first = p.run(&args, "");
    let second = p.run(&args, "");
    assert_eq!(first.status.code(), Some(0));
    assert_eq!(first.stdout, second.stdout);
    let text = String::from_utf8(first.stdout).unwrap();
    assert!(text.contains("ASSIGNED INDEX:"));
    assert!(!text.contains("script decides which grounded direction gets built"));
    assert!(!text.contains("never recommended"));
}

#[test]
fn mixed_cursor_write_denies_the_defect_and_honors_its_inline_waiver() {
    let p = Project::new();
    let html = format!("<style>{PATTERN}</style><h1 class=title>Title</h1><p style=\"color:#777;background:#666\">Unreadable text</p>");
    let event = |content: &str| {
        json!({"hook_event_name":"preToolUse", "conversation_id":"mixed", "workspace_roots":[p.0], "tool_name":"Write", "tool_input":{"path":"page.html", "content":content}}).to_string()
    };
    let out = p.run(&["hook-before-edit"], &event(&html));
    let payload: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(payload["permission"], "deny", "{payload}");
    let reason = payload["user_message"].as_str().unwrap();
    assert!(reason.contains("low-contrast"));
    assert!(!reason.contains("[gradient-text]"));
    let waived = format!(
        "<!-- impeccino-disable low-contrast: intentional contrast regression fixture -->\n{html}"
    );
    let out = p.run(&["hook-before-edit"], &event(&waived));
    let payload: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(payload["permission"], "allow", "{payload}");
}

#[test]
fn codex_stop_does_not_block_for_contextual_style_advice() {
    let p = Project::new();
    fs::write(
        p.0.join("card.css"),
        ".card{border-left:4px solid red;border-radius:8px}",
    )
    .unwrap();
    let post = json!({"hook_event_name":"PostToolUse", "session_id":"advice-stop", "cwd":p.0, "tool_name":"Edit", "tool_input":{"file_path":p.0.join("card.css")}});
    assert_eq!(p.run(&["hook"], &post.to_string()).status.code(), Some(0));
    let stop = json!({"hook_event_name":"Stop", "session_id":"advice-stop", "cwd":p.0, "turn_id":"codex-turn"});
    let out = p.run(&["hook"], &stop.to_string());
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn boot_does_not_override_host_autonomy_or_invent_subagent_authorization() {
    let p = Project::new();
    let out = p.run(&["context"], "");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("AUTONOMY_DIRECTIVE_CHECK"));
    assert!(!text.contains("SUBAGENT_AUTHORIZATION"));
    assert!(!text.contains("invocation of this skill is that request"));
}
