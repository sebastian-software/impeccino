//! `impeccable page-probe`: the detector's rendered-page rules, measured by the
//! agent's own browser (docs/adr/0016).
//!
//! Impeccable drives no browser (docs/adr/0011). The harness does: it runs a
//! one-line loader in the page with its own JavaScript tool. The loader pulls
//! the read-only measurement script (`assets/page-snapshot.js`) from a
//! short-lived receiver on 127.0.0.1, which takes the page snapshot, answers
//! the hit tests the rules ask for, and hands the findings to `--result`. The
//! snapshot (often megabytes) never passes through the model's context.
//!
//!   page-probe [--timeout <s>] [--inline]   start the receiver, print the loader
//!   page-probe --result <key> [--json] [--no-config] [--wait <s>]
//!   page-probe --serve --key <key> --timeout <s>   (internal: the receiver)

use std::hash::BuildHasher;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use impeccable_common::Io;
use impeccable_core::browser::driver::{collect_browser_findings, serialize_findings};
use impeccable_core::browser::snapshot::{Facts, SnapshotDom};
use impeccable_core::browser::BrowserConfig;
use impeccable_core::findings::{derive_advisory_flag, try_finding, Finding};
use impeccable_detect::design_system::{load_design_system_for_cwd, DesignSystem};
use serde_json::{json, Value};

/// The read-only page measurement: DOM, computed styles, rects, viewport.
const SNAPSHOT_JS: &str = include_str!("../assets/page-snapshot.js");

const DEFAULT_TIMEOUT_S: u64 = 120;
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
const MAX_HIT_TEST_ROUNDS: usize = 12;

const USAGE: &str = "Usage:
  impeccable page-probe [--timeout <seconds>] [--inline]
      Start a one-shot receiver on 127.0.0.1 and print the line to run in the
      rendered page with your browser tool's JavaScript evaluation.
  impeccable page-probe --result <key> [--json] [--no-config] [--wait <seconds>]
      Print the findings once the page has been measured.
";

pub fn run(args: &[String], io: &mut Io) -> i32 {
    if has(args, "--help") || has(args, "-h") {
        io.out(USAGE);
        return 0;
    }
    if has(args, "--serve") {
        return serve(args, io);
    }
    if let Some(key) = value(args, "--result") {
        return result(&key, args, io);
    }
    start(args, io)
}

// ─── start ─────────────────────────────────────────────────────────────────

fn start(args: &[String], io: &mut Io) -> i32 {
    let timeout = value(args, "--timeout").and_then(|v| v.parse::<u64>().ok()).unwrap_or(DEFAULT_TIMEOUT_S);
    let dir = state_dir(&io.cwd);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        io.err(&format!("page-probe: cannot create {}: {e}\n", dir.display()));
        return 1;
    }
    let key = format!("{:08x}", random_u64() as u32);
    let Ok(exe) = std::env::current_exe() else {
        io.err("page-probe: cannot locate the engine binary\n");
        return 1;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(["page-probe", "--serve", "--key", &key, "--timeout", &timeout.to_string()])
        .current_dir(&io.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    detach(&mut cmd);
    if let Err(e) = cmd.spawn() {
        io.err(&format!("page-probe: cannot start the receiver: {e}\n"));
        return 1;
    }
    let state_path = dir.join(format!("{key}.json"));
    let deadline = Instant::now() + Duration::from_secs(5);
    let state = loop {
        if let Some(s) = std::fs::read_to_string(&state_path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            break s;
        }
        if Instant::now() > deadline {
            io.err("page-probe: the receiver did not start\n");
            return 1;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let port = state.get("port").and_then(Value::as_u64).unwrap_or(0);
    let token = state.get("token").and_then(Value::as_str).unwrap_or("");
    let base = format!("http://127.0.0.1:{port}");
    let script = if has(args, "--inline") {
        probe_script(&base, token)
    } else {
        format!("fetch(\"{base}/probe.js?t={token}\").then(r => r.text()).then(s => (0, eval)(s))")
    };
    io.out(&format!(
        "PAGE_PROBE_KEY: {key}\n\nRun this in the rendered page with your browser tool's JavaScript evaluation within {timeout}s, and wait for the promise:\n\n{script}\n\nThen print the findings:\n\n{} page-probe --result {key}\n\nIf the page's Content-Security-Policy blocks the loader, start again with --inline and run the full script instead.\n",
        self_cmd(io)
    ));
    0
}

/// The script the loader evaluates: the measurement plus the round trip.
fn probe_script(base: &str, token: &str) -> String {
    format!(
        r#"(async () => {{
{SNAPSHOT_JS}
const base = {base:?}, t = {token:?};
const post = async (path, body) => (await fetch(`${{base}}${{path}}?t=${{t}}&u=${{encodeURIComponent(location.href)}}`, {{ method: 'POST', body }})).json();
const capture = __impeccableSnapshot.capture();
if (capture.error) {{ await post('/fail', String(capture.error)); return `impeccable: page capture failed: ${{capture.error}}`; }}
let res = await post('/snapshot', capture.json);
for (let round = 0; res.needs && round < {MAX_HIT_TEST_ROUNDS}; round++) {{
  res = await post('/facts', JSON.stringify(__impeccableSnapshot.answer(res.needs, capture)));
}}
return res.done ? `impeccable: page measured, ${{res.findings}} finding(s). Now run: ${{res.next}}` : `impeccable: ${{res.error || 'no result'}}`;
}})()"#
    )
}

// ─── serve ─────────────────────────────────────────────────────────────────

struct Session {
    url: String,
    dom: Option<SnapshotDom>,
    config: BrowserConfig,
}

fn serve(args: &[String], io: &mut Io) -> i32 {
    let Some(key) = value(args, "--key") else { return 1 };
    let timeout = value(args, "--timeout").and_then(|v| v.parse::<u64>().ok()).unwrap_or(DEFAULT_TIMEOUT_S);
    let dir = state_dir(&io.cwd);
    if std::fs::create_dir_all(&dir).is_err() {
        return 1;
    }
    let Ok(listener) = TcpListener::bind("127.0.0.1:0") else { return 1 };
    let Ok(port) = listener.local_addr().map(|a| a.port()) else { return 1 };
    let token = format!("{:016x}", random_u64());
    let state = json!({ "port": port, "token": token, "pid": std::process::id() });
    if std::fs::write(dir.join(format!("{key}.json")), state.to_string()).is_err() {
        return 1;
    }
    let _ = listener.set_nonblocking(true);
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let cwd = io.cwd.to_string_lossy().into_owned();
    let mut session = Session {
        url: String::new(),
        dom: None,
        config: browser_config(load_design_system_for_cwd(&cwd).as_ref()),
    };
    let result_path = dir.join(format!("{key}.result.json"));
    loop {
        if Instant::now() > deadline {
            write_result(&result_path, &json!({ "error": format!("no page measurement arrived within {timeout}s") }));
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                if let Some(done) = handle(stream, &token, &key, &mut session, &result_path, io) {
                    if done {
                        break;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let _ = std::fs::remove_file(dir.join(format!("{key}.json")));
    0
}

/// Answer one request. `Some(true)` once the findings are written.
fn handle(mut stream: TcpStream, token: &str, key: &str, s: &mut Session, result_path: &Path, io: &Io) -> Option<bool> {
    let (method, target, body) = read_request(&mut stream)?;
    if method == "OPTIONS" {
        respond(&mut stream, 204, "text/plain", "");
        return Some(false);
    }
    let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
    if query_param(query, "t").as_deref() != Some(token) {
        respond(&mut stream, 403, "text/plain", "forbidden");
        return Some(false);
    }
    if let Some(u) = query_param(query, "u") {
        s.url = u;
    }
    let reply = |stream: &mut TcpStream, v: Value| respond(stream, 200, "application/json", &v.to_string());
    match (method.as_str(), path) {
        ("GET", "/probe.js") => {
            let base = format!("http://127.0.0.1:{}", stream.local_addr().map(|a| a.port()).unwrap_or(0));
            respond(&mut stream, 200, "text/javascript", &probe_script(&base, token));
            Some(false)
        }
        ("POST", "/fail") => {
            let message = format!("page capture failed: {}", String::from_utf8_lossy(&body));
            write_result(result_path, &json!({ "error": message }));
            reply(&mut stream, json!({ "error": message }));
            Some(true)
        }
        ("POST", "/snapshot") => match SnapshotDom::from_json(&String::from_utf8_lossy(&body)) {
            Ok(dom) => {
                s.dom = Some(dom);
                Some(step(&mut stream, s, key, result_path, io))
            }
            Err(e) => {
                let message = format!("unreadable page snapshot: {e}");
                write_result(result_path, &json!({ "error": message }));
                reply(&mut stream, json!({ "error": message }));
                Some(true)
            }
        },
        ("POST", "/facts") => {
            if let (Some(dom), Ok(facts)) = (&s.dom, serde_json::from_slice::<Facts>(&body)) {
                dom.add_facts(&facts);
            }
            Some(step(&mut stream, s, key, result_path, io))
        }
        _ => {
            respond(&mut stream, 404, "text/plain", "not found");
            Some(false)
        }
    }
}

/// Run the rules over the snapshot. While they asked hit tests the snapshot
/// cannot answer, send those to the page; otherwise write the findings.
fn step(stream: &mut TcpStream, s: &mut Session, key: &str, result_path: &Path, io: &Io) -> bool {
    let Some(dom) = &s.dom else { return false };
    let collected = collect_browser_findings(dom, &s.config);
    if dom.has_needs() {
        let needs = dom.take_needs();
        respond(stream, 200, "application/json", &json!({ "needs": needs }).to_string());
        return false;
    }
    let groups = serialize_findings(dom, &collected.groups);
    let url = if s.url.is_empty() { "page".to_string() } else { s.url.clone() };
    let mut findings: Vec<Finding> = Vec::new();
    for group in groups.as_array().into_iter().flatten() {
        for f in group.get("findings").and_then(Value::as_array).into_iter().flatten() {
            let id = f.get("type").and_then(Value::as_str).unwrap_or("");
            let snippet = f.get("detail").and_then(Value::as_str).unwrap_or("");
            let Some(mut item) = try_finding(id, &url, snippet, 0.0) else { continue };
            if let Some(v) = f.get("ignoreValue").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                item.extras.insert("ignoreValue".into(), Value::String(v.to_string()));
            }
            if let Some(sev) = f.get("severity").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                item.severity = sev.to_string();
            }
            derive_advisory_flag(&mut item);
            findings.push(item);
        }
    }
    write_result(result_path, &json!({ "url": url, "findings": findings }));
    let next = format!("{} page-probe --result {key}", self_cmd(io));
    respond(stream, 200, "application/json", &json!({ "done": true, "findings": findings.len(), "next": next }).to_string());
    true
}

fn browser_config(ds: Option<&DesignSystem>) -> BrowserConfig {
    let design_system = ds.filter(|d| d.present).map(|ds| {
        let colors: Vec<Value> = ds
            .allowed_color_keys
            .iter()
            .map(|(_, entry)| &entry.color)
            .filter(|c| c.r.is_finite() && c.g.is_finite() && c.b.is_finite())
            .map(|c| json!({ "r": c.r, "g": c.g, "b": c.b }))
            .collect();
        let radii: Vec<Value> = ds.allowed_radii.iter().map(|r| r.px).filter(|px| px.is_finite()).map(|px| json!(px)).collect();
        json!({
            "present": true,
            "hasFonts": ds.has_fonts,
            "allowedFonts": ds.allowed_fonts,
            "hasColors": ds.has_colors,
            "allowedColors": colors,
            "hasRadii": ds.has_radii,
            "allowedRadii": radii,
            "hasPillRadius": ds.has_pill_radius,
            "declaredSelectors": ds.declared_selectors,
        })
    });
    BrowserConfig {
        extension_mode: false,
        disabled_rules: Vec::new(),
        disabled_values: Vec::new(),
        skip_scan: false,
        design_system,
        line_length_max: None,
        rule_pack: None,
    }
}

// ─── result ────────────────────────────────────────────────────────────────

fn result(key: &str, args: &[String], io: &mut Io) -> i32 {
    let wait = value(args, "--wait").and_then(|v| v.parse::<u64>().ok()).unwrap_or(DEFAULT_TIMEOUT_S);
    let json_mode = has(args, "--json");
    let dir = state_dir(&io.cwd);
    let path = dir.join(format!("{key}.result.json"));
    let deadline = Instant::now() + Duration::from_secs(wait);
    let data = loop {
        if let Some(v) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            break v;
        }
        if Instant::now() > deadline || !dir.join(format!("{key}.json")).exists() && !path.exists() {
            io.err(&format!("page-probe: no result for {key}. Start again with `{} page-probe`.\n", self_cmd(io)));
            return 1;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = std::fs::remove_file(&path);
    if let Some(error) = data.get("error").and_then(Value::as_str) {
        io.err(&format!("page-probe: {error}\n"));
        return 1;
    }
    let mut findings: Vec<Finding> = data
        .get("findings")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    if !has(args, "--no-config") {
        let cwd = io.cwd.to_string_lossy().into_owned();
        let config = impeccable_detect::config::read_detection_config(&cwd);
        findings = impeccable_detect::config::filter_detection_findings(findings, &config);
    }
    let primary = findings.iter().filter(|f| f.advisory != Some(true)).count();
    let stderr_tty = impeccable_detect::cli::stderr_is_tty();
    if json_mode {
        io.out(&format!("{}\n", impeccable_detect::cli::format_findings(&findings, true, stderr_tty)));
    } else if !findings.is_empty() {
        io.err(&format!("{}\n", impeccable_detect::cli::format_findings(&findings, false, stderr_tty)));
    }
    if primary > 0 { 2 } else { 0 }
}

// ─── helpers ───────────────────────────────────────────────────────────────

fn state_dir(cwd: &Path) -> PathBuf {
    cwd.join(".impeccable").join("page-probe")
}

fn write_result(path: &Path, v: &Value) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, v.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn self_cmd(io: &Io) -> String {
    io.env.get("IMPECCABLE_SELF").cloned().unwrap_or_else(|| "impeccable".to_string())
}

fn random_u64() -> u64 {
    std::collections::hash_map::RandomState::new().hash_one(Instant::now())
}

fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn value(args: &[String], flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    args.iter().enumerate().find_map(|(i, a)| {
        if a == flag {
            args.get(i + 1).cloned()
        } else {
            a.strip_prefix(&prefix).map(str::to_string)
        }
    })
}

fn query_param(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == name).then(|| impeccable_detect::config::decode_uri_component(v))
    })
}

fn detach(cmd: &mut std::process::Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    impeccable_common::proc::hide_window(cmd);
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String, Vec<u8>)> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    if length > MAX_BODY_BYTES {
        return None;
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some((method, target, body))
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        403 => "Forbidden",
        _ => "Not Found",
    };
    // Any page may call in (the token is the gate). Private Network Access
    // lets an https page reach 127.0.0.1 after the preflight.
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Private-Network: true\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JS array literal `const <name> = [ ... ];` from the measurement script.
    fn js_list(name: &str) -> Vec<String> {
        let start = SNAPSHOT_JS.find(&format!("const {name} = [")).expect(name);
        let body = &SNAPSHOT_JS[start..];
        let body = &body[body.find('[').unwrap() + 1..body.find("];").unwrap()];
        body.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect()
    }

    #[test]
    fn measurement_script_captures_exactly_the_properties_the_rules_read() {
        use impeccable_core::browser::snapshot::{PSEUDO_PROPS, STYLE_PROPS};
        assert_eq!(js_list("__SNAP_STYLE_PROPS"), STYLE_PROPS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(js_list("__SNAP_PSEUDO_PROPS"), PSEUDO_PROPS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn query_values_are_decoded() {
        assert_eq!(query_param("t=abc&u=http%3A%2F%2Fx%2F", "u").as_deref(), Some("http://x/"));
        assert_eq!(query_param("t=abc", "u"), None);
    }
}
