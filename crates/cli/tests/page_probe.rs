//! `impeccable page-probe` end to end without a browser: start the receiver,
//! deliver a recorded page snapshot the way the in-page script does, answer
//! the hit tests with nothing, and read the findings back through `--result`.
//! The fixture is `tests/fixtures/antipatterns/quality.html`, captured with
//! `assets/page-snapshot.js` at 1280x800.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::Value;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_impeccable")
}

fn temp_project(tag: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "impeccable-page-probe-{tag}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn wait_for_state(project: &Path, key: &str) -> (u16, String) {
    let path = project.join(".impeccable/page-probe").join(format!("{key}.json"));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(v) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            return (v["port"].as_u64().unwrap() as u16, v["token"].as_str().unwrap().to_string());
        }
        assert!(Instant::now() < deadline, "receiver never wrote its state file");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Answer every asked point with "nothing there", the way a page with no
/// element at those coordinates would.
fn empty_facts(reply: &str) -> String {
    let v: Value = serde_json::from_str(reply).unwrap();
    let hits: Vec<Value> = v["needs"]["hitTests"]
        .as_array()
        .map(|points| points.iter().map(|p| serde_json::json!({ "x": p[0], "y": p[1], "top": 0, "stack": [] })).collect())
        .unwrap_or_default();
    serde_json::json!({ "hits": hits }).to_string()
}

fn request(port: u16, method: &str, target: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
    (status, body)
}

#[test]
fn measures_a_delivered_snapshot_and_reports_rendered_page_rules() {
    let project = temp_project("ok");
    let key = "testkey1";
    let mut serve = Command::new(bin())
        .args(["page-probe", "--serve", "--key", key, "--timeout", "30"])
        .current_dir(&project)
        .spawn()
        .unwrap();
    let (port, token) = wait_for_state(&project, key);

    // A request without the token is refused.
    assert_eq!(request(port, "GET", "/probe.js?t=wrong", "").0, 403);

    // The loader's target serves the measurement script bound to this receiver.
    let (status, script) = request(port, "GET", &format!("/probe.js?t={token}"), "");
    assert_eq!(status, 200);
    assert!(script.contains("__impeccableSnapshot"));
    assert!(script.contains(&format!("127.0.0.1:{port}")));

    let snapshot = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quality.snapshot.json")).unwrap();
    let (status, mut reply) = request(port, "POST", &format!("/snapshot?t={token}&u=http%3A%2F%2Flocalhost%3A3000%2F"), &snapshot);
    assert_eq!(status, 200);
    let mut rounds = 0;
    while serde_json::from_str::<Value>(&reply).unwrap().get("needs").is_some() {
        rounds += 1;
        assert!(rounds < 12, "hit-test rounds did not converge");
        reply = request(port, "POST", &format!("/facts?t={token}"), &empty_facts(&reply)).1;
    }
    let done: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(done["done"], true, "{reply}");
    assert!(done["next"].as_str().unwrap().contains(&format!("page-probe --result {key}")));
    serve.wait().unwrap();

    let out = Command::new(bin())
        .args(["page-probe", "--result", key, "--json"])
        .current_dir(&project)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    let findings: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = findings.iter().map(|f| f["antipattern"].as_str().unwrap()).collect();
    // `line-length` needs rendered line boxes; no source-file engine can see it.
    assert!(ids.contains(&"line-length"), "{ids:?}");
    assert!(findings.iter().all(|f| f["file"] == "http://localhost:3000/"));
    // The result is consumed, and the receiver cleaned up its state.
    assert!(!project.join(".impeccable/page-probe").join(format!("{key}.json")).exists());
    assert!(!project.join(".impeccable/page-probe").join(format!("{key}.result.json")).exists());
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn project_ignores_apply_to_page_findings() {
    let project = temp_project("ignore");
    std::fs::create_dir_all(project.join(".impeccable")).unwrap();
    std::fs::write(project.join(".impeccable/config.json"), r#"{"detector":{"ignoreRules":["line-length"]}}"#).unwrap();
    let key = "testkey2";
    let mut serve = Command::new(bin())
        .args(["page-probe", "--serve", "--key", key, "--timeout", "30"])
        .current_dir(&project)
        .spawn()
        .unwrap();
    let (port, token) = wait_for_state(&project, key);
    let snapshot = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quality.snapshot.json")).unwrap();
    let mut reply = request(port, "POST", &format!("/snapshot?t={token}"), &snapshot).1;
    while serde_json::from_str::<Value>(&reply).unwrap().get("needs").is_some() {
        reply = request(port, "POST", &format!("/facts?t={token}"), &empty_facts(&reply)).1;
    }
    serve.wait().unwrap();
    let out = Command::new(bin())
        .args(["page-probe", "--result", key, "--json"])
        .current_dir(&project)
        .output()
        .unwrap();
    let findings: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
    assert!(findings.iter().all(|f| f["antipattern"] != "line-length"));
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn start_prints_a_loader_and_a_result_command() {
    let project = temp_project("start");
    let out = Command::new(bin())
        .args(["page-probe", "--timeout", "2"])
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).unwrap();
    let key = text.lines().find_map(|l| l.strip_prefix("PAGE_PROBE_KEY: ")).unwrap().to_string();
    assert!(text.contains("fetch(\"http://127.0.0.1:"));
    assert!(text.contains(&format!("page-probe --result {key}")));
    // Nothing arrives, so the receiver gives up and says why.
    let out = Command::new(bin())
        .args(["page-probe", "--result", &key, "--wait", "10"])
        .current_dir(&project)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no page measurement arrived"));
    let _ = std::fs::remove_dir_all(&project);
}
