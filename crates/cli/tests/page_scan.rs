//! `impeccino detect <url>` through agent-browser. The browser tests need
//! `agent-browser` on PATH (or `IMPECCINO_AGENT_BROWSER`) and skip without
//! it; the scan logic itself is covered without a browser in
//! `src/page_scan` unit tests.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_impeccino")
}

fn fixture_url(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/antipatterns")
        .join(name);
    format!("file://{}", path.canonicalize().unwrap().display())
}

fn temp_project(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("impeccino-page-scan-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn has_agent_browser() -> bool {
    let bin = std::env::var("IMPECCINO_AGENT_BROWSER").unwrap_or_else(|_| "agent-browser".into());
    let found = Command::new(bin)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !found {
        eprintln!("skipped: agent-browser is not installed");
    }
    found
}

fn detect(cwd: &Path, url: &str) -> (Output, Vec<Value>) {
    let out = Command::new(bin())
        .args(["detect", "--json", url])
        .current_dir(cwd)
        .env_remove("AGENT_BROWSER_SESSION")
        .output()
        .unwrap();
    let findings = serde_json::from_slice(&out.stdout).unwrap_or_default();
    (out, findings)
}

fn ids(findings: &[Value]) -> Vec<&str> {
    findings
        .iter()
        .map(|f| f["antipattern"].as_str().unwrap())
        .collect()
}

#[test]
fn a_missing_agent_browser_is_reported_with_how_to_install_it() {
    let project = temp_project("missing");
    let out = Command::new(bin())
        .args(["detect", "http://localhost:3000/"])
        .current_dir(&project)
        .env(
            "IMPECCINO_AGENT_BROWSER",
            project.join("no-such-agent-browser"),
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Rendered-page scans need agent-browser"),
        "{stderr}"
    );
    assert!(stderr.contains("npm install -g agent-browser"), "{stderr}");
}

#[test]
fn rendered_page_rules_need_the_rendered_page() {
    if !has_agent_browser() {
        return;
    }
    let project = temp_project("rules");
    let (out, findings) = detect(&project, &fixture_url("text-occlusion.html"));
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let ids = ids(&findings);
    // Layout rules no source-file engine can see.
    assert!(ids.contains(&"text-occlusion"), "{ids:?}");
    assert!(ids.contains(&"text-overflow"), "{ids:?}");
    assert!(findings
        .iter()
        .all(|f| f["file"].as_str().unwrap().ends_with("text-occlusion.html")));
}

#[test]
fn design_md_waivers_apply_to_page_findings() {
    if !has_agent_browser() {
        return;
    }
    let project = temp_project("ignore");
    std::fs::write(project.join("DESIGN.md"), "# Design\n\n<!-- impeccino-disable line-length -- long legal copy is set wide on purpose -->\n").unwrap();
    let (_, findings) = detect(&project, &fixture_url("quality.html"));
    assert!(!findings.is_empty());
    assert!(!ids(&findings).contains(&"line-length"));
}

#[test]
fn errors_from_page_load_on_become_findings() {
    if !has_agent_browser() {
        return;
    }
    let project = temp_project("errors");
    let page = project.join("errors.html");
    std::fs::write(&page, "<!doctype html><title>x</title><h1>Hello</h1><script>throw new Error('boom at load')</script>").unwrap();
    let (_, findings) = detect(&project, &format!("file://{}", page.display()));
    let errors: Vec<&Value> = findings
        .iter()
        .filter(|f| f["antipattern"] == "script-error")
        .collect();
    assert_eq!(errors.len(), 1, "{findings:?}");
    assert!(errors[0]["snippet"]
        .as_str()
        .unwrap()
        .contains("boom at load"));
}

#[test]
fn screenshot_pixels_decide_what_the_analyses_cannot() {
    if !has_agent_browser() {
        return;
    }
    let project = temp_project("pixels");
    let (_, findings) = detect(&project, &fixture_url("screenshot-contrast.html"));
    let low: Vec<&str> = findings
        .iter()
        .filter(|f| f["antipattern"] == "low-contrast")
        .map(|f| f["snippet"].as_str().unwrap())
        .collect();
    assert_eq!(low.len(), 1, "{low:?}");
    assert!(low[0].starts_with("pixel contrast "), "{}", low[0]);
    assert!(low[0].contains("Pale text"), "{}", low[0]);
}
