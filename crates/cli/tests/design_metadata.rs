//! Exercise the single-file design record and legacy migration through the shipped binary.
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "impeccino-design-record-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

const DESIGN: &str = "---\ncolors:\n  ink: '#111111'\nrounded:\n  card: '4px'\n---\n# Design\n\n<!-- impeccino-waiver: deliberate fixture -->\nKeep this prose.\n";
const META: &str = r##"{"schemaVersion":2,"extensions":{"colorMeta":{"accent":{"canonical":"#abcdef","tonalRamp":["#fedcba"]}},"roundedMeta":{"card":{"canonical":"8px","aliases":["12px"]}},"shadows":[{"value":"0 2px 4px #123456"}]},"custom":{"preserve":[1,true,"extra"]}}"##;

#[test]
fn embedded_metadata_drives_detection_without_a_sidecar() {
    let p = Project::new();
    fs::write(p.0.join("DESIGN.md"), DESIGN).unwrap();
    let css = ".card { color:#abcdef; border-radius:8px; box-shadow:0 2px 4px #123456; } .step { color:#fedcba; border-radius:12px; }";
    fs::write(p.0.join("card.css"), css).unwrap();
    let before = p.run(&["detect", "--json", "card.css"], "");
    let hits: Vec<Value> = serde_json::from_slice(&before.stdout).unwrap();
    assert!(hits
        .iter()
        .any(|h| h["antipattern"] == "design-system-color"));
    assert!(hits
        .iter()
        .any(|h| h["antipattern"] == "design-system-radius"));
    fs::write(
        p.0.join("DESIGN.md"),
        format!("{DESIGN}\n<!-- impeccino:design-metadata -->\n```json\n{META}\n```\n"),
    )
    .unwrap();
    let after = p.run(&["detect", "--json", "card.css"], "");
    let hits: Vec<Value> = serde_json::from_slice(&after.stdout).unwrap();
    assert!(
        !hits
            .iter()
            .any(|h| h["antipattern"] == "design-system-color"
                || h["antipattern"] == "design-system-radius"),
        "{hits:?}"
    );
}

#[test]
fn static_html_reads_the_same_embedded_tokens_and_preserves_project_waivers() {
    let p = Project::new();
    fs::write(p.0.join("DESIGN.md"), format!("{DESIGN}\n<!-- impeccino-disable side-tab: incumbent rail -->\n<!-- impeccino:design-metadata -->\n```json\n{META}\n```\n")).unwrap();
    fs::write(p.0.join("page.html"), "<style>.card{color:#abcdef;border-radius:8px;box-shadow:0 2px 4px #123456;border-left:4px solid #fedcba}</style><div class=card>Content</div>").unwrap();
    let out = p.run(&["detect", "--json", "page.html"], "");
    let hits: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        !hits.iter().any(|h| matches!(
            h["antipattern"].as_str(),
            Some("design-system-color" | "design-system-radius" | "side-tab")
        )),
        "{hits:?}"
    );
}

#[test]
fn legacy_fallback_stops_when_a_marked_block_is_present_even_if_invalid() {
    let p = Project::new();
    fs::write(p.0.join("DESIGN.md"), DESIGN).unwrap();
    fs::write(p.0.join("DESIGN.json"), META).unwrap();
    fs::write(
        p.0.join("card.css"),
        ".card { color:#abcdef; border-radius:8px; }",
    )
    .unwrap();
    let before = p.run(&["detect", "--json", "card.css"], "");
    let hits: Vec<Value> = serde_json::from_slice(&before.stdout).unwrap();
    assert!(!hits
        .iter()
        .any(|h| h["antipattern"] == "design-system-color"
            || h["antipattern"] == "design-system-radius"));
    for metadata in ["{}", "broken"] {
        fs::write(
            p.0.join("DESIGN.md"),
            format!("{DESIGN}\n<!-- impeccino:design-metadata -->\n```json\n{metadata}\n```\n"),
        )
        .unwrap();
        let out = p.run(&["detect", "--json", "card.css"], "");
        let hits: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
        assert!(hits
            .iter()
            .any(|h| h["antipattern"] == "design-system-color"));
        assert!(hits
            .iter()
            .any(|h| h["antipattern"] == "design-system-radius"));
    }
}

#[test]
fn doctor_migrates_legacy_metadata_without_losing_prose_or_unknown_fields() {
    let p = Project::new();
    fs::create_dir(p.0.join("docs")).unwrap();
    fs::write(p.0.join("docs/DESIGN.md"), DESIGN).unwrap();
    fs::write(p.0.join("docs/DESIGN.json"), META).unwrap();
    let out = p.run(&["doctor", "--fix", "--json"], "");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let md = fs::read_to_string(p.0.join("docs/DESIGN.md")).unwrap();
    assert!(md.starts_with(DESIGN));
    assert!(md.contains("<!-- impeccino:design-metadata -->"));
    let embedded = md
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(embedded).unwrap(),
        serde_json::from_str::<Value>(META).unwrap()
    );
    assert!(!p.0.join("docs/DESIGN.json").exists());
    let again = p.run(&["doctor", "--fix", "--json"], "");
    assert!(again.status.success());
    assert_eq!(fs::read_to_string(p.0.join("docs/DESIGN.md")).unwrap(), md);
}

#[test]
fn conflicting_or_invalid_embedded_metadata_preserves_both_artifacts() {
    for (block, id) in [
        ("{}", "design-metadata-conflict"),
        ("invalid", "design-metadata-invalid"),
    ] {
        let p = Project::new();
        let md = format!("{DESIGN}\n<!-- impeccino:design-metadata -->\n```json\n{block}\n```\n");
        fs::write(p.0.join("DESIGN.md"), &md).unwrap();
        fs::write(p.0.join("DESIGN.json"), META).unwrap();
        let out = p.run(&["doctor", "--fix", "--json"], "");
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            result["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["id"] == id),
            "{result}"
        );
        assert_eq!(fs::read_to_string(p.0.join("DESIGN.md")).unwrap(), md);
        assert_eq!(fs::read_to_string(p.0.join("DESIGN.json")).unwrap(), META);
    }
}

#[test]
fn doctor_reports_failed_migration_and_retains_the_legacy_file() {
    let p = Project::new();
    let md = format!("{DESIGN}\n```markdown\nUnclosed example\n");
    fs::write(p.0.join("DESIGN.md"), &md).unwrap();
    fs::write(p.0.join("DESIGN.json"), META).unwrap();
    let out = p.run(&["doctor", "--fix", "--json"], "");
    assert_eq!(out.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        report["fixes"]["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["id"] == "design-sidecar-legacy"),
        "{report}"
    );
    assert_eq!(fs::read_to_string(p.0.join("DESIGN.md")).unwrap(), md);
    assert_eq!(fs::read_to_string(p.0.join("DESIGN.json")).unwrap(), META);
}
