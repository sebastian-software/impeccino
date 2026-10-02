//! `impeccable detect <url>`: the detector's rendered-page rules, measured in
//! a page that agent-browser holds (docs/adr/0016).
//!
//! Impeccable ships no browser. It drives `agent-browser`, the headless
//! browser CLI agents already use: `open` the URL, install the read-only
//! measurement (`assets/page-snapshot.js`) plus a small set of page
//! operations with `eval`, and run the scan (`scan.rs`) here, one operation
//! per `eval`. Page errors come from agent-browser's own error log (from the
//! first script on), and the pixel fallback from its screenshots. Snapshots
//! (often megabytes) go from agent-browser's stdout to the engine and never
//! through the model's context.
//!
//! Each scan uses its own agent-browser session and closes it afterwards.
//! When `AGENT_BROWSER_SESSION` is set, the scan uses that session instead
//! (for example one the agent signed in with) and leaves it open.

use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;

use impeccable_core::browser::BrowserConfig;
use impeccable_core::findings::{derive_advisory_flag, try_finding, Finding};
use impeccable_detect::design_system::DesignSystem;
use impeccable_detect::engines::{EngineError, ScanOptions, SharedBrowser, UrlEngine};
use serde_json::{json, Value};

mod cdp;
mod scan;

/// The read-only page measurement: DOM, computed styles, rects, viewport.
const SNAPSHOT_JS: &str = include_str!("../../assets/page-snapshot.js");

/// The page operations the scan asks for, installed next to the measurement
/// as `window.__impeccableProbe`. Page errors and screenshots come from
/// agent-browser itself, not from here.
const PROBE_JS: &str = r#"
const S = __impeccableSnapshot;
let cap = null, io = null;
const paint = () => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(0))));
const find = (selector) => { try { return document.querySelector(selector); } catch (e) { return null; } };
const overlayErrors = () => {
  const out = [];
  const text = (el) => ((el && (el.shadowRoot ? el.shadowRoot.textContent : el.textContent)) || '').replace(/\s+/g, ' ').trim().slice(0, 300);
  const vite = document.querySelector('vite-error-overlay');
  if (vite) out.push('Dev server error overlay: ' + text(vite));
  const next = document.querySelector('nextjs-portal');
  if (next && next.shadowRoot && next.shadowRoot.querySelector('[data-nextjs-dialog], [data-nextjs-dialog-overlay]')) out.push('Dev server error overlay: ' + text(next));
  const webpack = document.getElementById('webpack-dev-server-client-overlay');
  if (webpack) { try { out.push('Dev server error overlay: ' + text(webpack.contentDocument && webpack.contentDocument.body)); } catch (e) { out.push('Dev server error overlay'); } }
  return out;
};
const directTextRect = (node) => {
  const rects = [];
  for (const child of node.childNodes) {
    if (child.nodeType !== 3 || !(child.textContent || '').trim()) continue;
    const range = document.createRange();
    range.selectNodeContents(child);
    for (const rect of range.getClientRects()) if (rect.width >= 1 && rect.height >= 1) rects.push(rect);
    if (range.detach) range.detach();
  }
  if (!rects.length) return null;
  const left = Math.min(...rects.map(r => r.left)), top = Math.min(...rects.map(r => r.top));
  return [left, top, Math.max(...rects.map(r => r.right)) - left, Math.max(...rects.map(r => r.bottom)) - top];
};
const HIDE_STYLE = [
  '[data-impeccable-visual-contrast-target] {',
  '  color: transparent !important;',
  '  -webkit-text-fill-color: transparent !important;',
  '  text-shadow: none !important;',
  '}',
  '[data-impeccable-visual-contrast-target][data-impeccable-bgclip-text="true"] {',
  '  background-image: none !important;',
  '}',
].join('\n');
const run = async (op) => {
  switch (op.op) {
    case 'capture': { const c = S.capture(); if (c.error) return { error: String(c.error) }; cap = c; io = S.visualIO(c); return { json: c.json }; }
    case 'answer': return S.answer({ hitTests: op.hitTests }, cap);
    case 'geometry': {
      const els = (cap && cap.elements) || [];
      const rects = [], dtrs = [];
      for (let id = 1; id < els.length; id++) {
        const el = els[id];
        rects.push(el && el.getBoundingClientRect ? (r => [r.x, r.y, r.width, r.height])(el.getBoundingClientRect()) : null);
        dtrs.push(el ? directTextRect(el) : null);
      }
      return { scrollX, scrollY, rects, dtrs };
    }
    case 'reveal': {
      const step = Math.max(200, Math.floor(innerHeight * 0.7));
      const max = Math.max(document.documentElement.scrollHeight || 0, (document.body && document.body.scrollHeight) || 0);
      for (let y = 0; y <= max; y += step) { scrollTo({ top: y, left: 0, behavior: 'instant' }); await new Promise(r => requestAnimationFrame(() => setTimeout(r, 40))); }
      scrollTo({ top: 0, left: 0, behavior: 'instant' });
      await new Promise(r => setTimeout(r, 700));
      return true;
    }
    case 'overlayErrors': return overlayErrors();
    case 'scroll': return { x: scrollX, y: scrollY };
    case 'scrollTo': scrollTo(op.x, op.y); return true;
    case 'scrollIntoView': { const el = find(op.selector); if (!el || !el.scrollIntoView) return false; el.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'instant' }); return true; }
    case 'paint': await paint(); return true;
    case 'loadImage': return await io.loadImage(op.src);
    case 'readPixel': return await io.readPixel(op.ref, op.plan, op.x, op.y);
    case 'rect': { const el = find(op.selector); if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height, vw: innerWidth, vh: innerHeight }; }
    case 'hideText': {
      const el = find(op.selector);
      if (!el) return false;
      let style = document.getElementById('impeccable-visual-contrast-hide-style');
      if (!style) { style = document.createElement('style'); style.id = 'impeccable-visual-contrast-hide-style'; style.textContent = HIDE_STYLE; document.head.appendChild(style); }
      el.setAttribute('data-impeccable-visual-contrast-target', '1');
      if (op.backgroundClipText) el.setAttribute('data-impeccable-bgclip-text', 'true');
      return true;
    }
    case 'showText': {
      const el = find(op.selector);
      if (el) { el.removeAttribute('data-impeccable-visual-contrast-target'); el.removeAttribute('data-impeccable-bgclip-text'); }
      const style = document.getElementById('impeccable-visual-contrast-hide-style');
      if (style) style.remove();
      return true;
    }
    default: return { error: 'unknown op ' + op.op };
  }
};
window.__impeccableProbe = { run };
"#;

/// The viewport of a fresh session (the former URL engine's default).
const DEFAULT_VIEWPORT: (u32, u32) = (1280, 800);

const MISSING_BROWSER: &str = "Rendered-page scans need agent-browser (https://github.com/vercel-labs/agent-browser). Install it with `npm install -g agent-browser && agent-browser install`, or scan the source files instead.";

/// [`UrlEngine`] over agent-browser.
pub struct AgentBrowserEngine;

impl UrlEngine for AgentBrowserEngine {
    fn detect_url(&self, url: &str, options: &ScanOptions) -> Result<Vec<Finding>, EngineError> {
        let session = Session::start(options.viewport)?;
        let result = session.measure(url, options);
        session.close();
        result
    }

    fn open_shared(&self) -> Option<Box<dyn SharedBrowser + '_>> {
        Some(Box::new(SharedSession { session: RefCell::new(None) }))
    }
}

/// One session for a multi-URL scan, started on first use.
struct SharedSession {
    session: RefCell<Option<Rc<Session>>>,
}

impl SharedBrowser for SharedSession {
    fn detect_url(&self, url: &str, options: &ScanOptions) -> Result<Vec<Finding>, EngineError> {
        let existing = self.session.borrow().clone();
        let session = match existing {
            Some(s) => s,
            None => {
                let s = Rc::new(Session::start(options.viewport)?);
                *self.session.borrow_mut() = Some(Rc::clone(&s));
                s
            }
        };
        session.measure(url, options)
    }

    fn close(&self) {
        if let Some(s) = self.session.borrow_mut().take() {
            s.close();
        }
    }

    fn ensure_launched(&self) -> Result<(), EngineError> {
        Session::check_installed()
    }
}

/// An agent-browser session: ours (closed at the end) or the caller's.
struct Session {
    bin: PathBuf,
    name: String,
    owned: bool,
}

impl Session {
    fn bin() -> PathBuf {
        std::env::var_os("IMPECCABLE_AGENT_BROWSER").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("agent-browser"))
    }

    fn check_installed() -> Result<(), EngineError> {
        let mut cmd = Command::new(Self::bin());
        cmd.arg("--version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        impeccable_common::proc::hide_window(&mut cmd);
        if cmd.status().is_ok_and(|s| s.success()) {
            Ok(())
        } else {
            Err(EngineError::new(MISSING_BROWSER))
        }
    }

    fn start(viewport: Option<(u32, u32)>) -> Result<Session, EngineError> {
        Self::check_installed()?;
        let (name, owned) = match std::env::var("AGENT_BROWSER_SESSION") {
            Ok(name) if !name.is_empty() => (name, false),
            _ => (format!("impeccable-{}-{:x}", std::process::id(), random_u32()), true),
        };
        let session = Session { bin: Self::bin(), name, owned };
        if let Some((w, h)) = viewport.or(owned.then_some(DEFAULT_VIEWPORT)) {
            session.run(&["set", "viewport", &w.to_string(), &h.to_string()], None)?;
        }
        Ok(session)
    }

    fn close(&self) {
        if self.owned {
            let _ = self.run(&["close"], None);
        }
    }

    /// Run one agent-browser command with `--json` and return its `data`.
    fn run(&self, args: &[&str], stdin: Option<&str>) -> Result<Value, EngineError> {
        let verb = args.first().copied().unwrap_or("");
        let mut cmd = Command::new(&self.bin);
        cmd.args(["--session", &self.name, "--json"]).args(args);
        cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
        impeccable_common::proc::hide_window(&mut cmd);
        let mut child = cmd.spawn().map_err(|_| EngineError::new(MISSING_BROWSER))?;
        if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
            let _ = pipe.write_all(text.as_bytes());
        }
        let out = child.wait_with_output().map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
        let reply: Value = serde_json::from_slice(&out.stdout).map_err(|_| {
            EngineError::new(format!("agent-browser {verb}: {}", String::from_utf8_lossy(&out.stderr).trim()))
        })?;
        if reply.get("success").and_then(Value::as_bool) != Some(true) {
            let error = reply.get("error").and_then(Value::as_str).unwrap_or("unknown error");
            return Err(EngineError::new(format!("agent-browser {verb}: {error}")));
        }
        Ok(reply.get("data").cloned().unwrap_or(Value::Null))
    }

    fn eval(&self, script: &str) -> Result<Value, String> {
        let data = self.run(&["eval", "--stdin"], Some(script)).map_err(|e| e.message)?;
        Ok(data.get("result").cloned().unwrap_or(Value::Null))
    }

    /// The fast lane to the open page, when the browser exposes DevTools.
    fn channel(&self) -> Option<cdp::PageChannel> {
        let endpoint = self.run(&["get", "cdp-url"], None).ok()?;
        let page = self.run(&["get", "url"], None).ok()?;
        let endpoint = endpoint.get("cdpUrl").or_else(|| endpoint.get("url")).or(Some(&endpoint)).and_then(Value::as_str)?.to_string();
        let page = page.get("url").or(Some(&page)).and_then(Value::as_str)?.to_string();
        cdp::PageChannel::attach(&endpoint, &page).ok()
    }

    fn measure(&self, url: &str, options: &ScanOptions) -> Result<Vec<Finding>, EngineError> {
        // The error log is per session; a reused session carries old errors.
        let _ = self.run(&["errors", "--clear"], None);
        self.run(&["open", url], None)?;
        // A page that keeps a connection open never goes network-idle; the
        // load event has fired by then, so measure anyway.
        let _ = self.run(&["wait", "--load", "networkidle"], None);
        let config = browser_config(options.design_system.as_deref());
        let mut page = AgentBrowserPage { session: self, channel: self.channel(), scratch: scratch_dir() };
        let raw = scan::scan(&mut page, &config);
        let _ = std::fs::remove_dir_all(&page.scratch);
        Ok(to_findings(raw.map_err(EngineError::new)?, url))
    }
}

/// [`scan::PageIo`] over a session: page operations go through the DevTools
/// channel (or `eval` without one), errors and screenshots through
/// agent-browser's own commands.
struct AgentBrowserPage<'a> {
    session: &'a Session,
    channel: Option<cdp::PageChannel>,
    scratch: PathBuf,
}

impl AgentBrowserPage<'_> {
    fn eval(&mut self, script: &str) -> Result<Value, String> {
        if let Some(channel) = self.channel.as_mut() {
            match channel.eval(script) {
                Ok(v) => return Ok(v),
                // A broken channel falls back to agent-browser for good; a
                // page-side exception fails there too and is reported.
                Err(e) => {
                    let fallback = self.session.eval(script);
                    if fallback.is_ok() {
                        self.channel = None;
                    }
                    return fallback.map_err(|_| e);
                }
            }
        }
        self.session.eval(script)
    }

    fn install(&mut self) -> Result<(), String> {
        self.eval(&format!("(() => {{\n{SNAPSHOT_JS}\n{PROBE_JS}\nreturn true;\n}})()")).map(|_| ())
    }

    fn page_op(&mut self, op: &Value) -> Result<Value, String> {
        let script = format!(
            "(async () => window.__impeccableProbe ? await window.__impeccableProbe.run({op}) : {{ __impeccableMissing: true }})()"
        );
        let out = self.eval(&script)?;
        if out.get("__impeccableMissing").is_none() {
            return Ok(out);
        }
        // First use, or the page navigated and dropped the probe.
        self.install()?;
        self.eval(&script)
    }

    fn errors(&mut self) -> Result<Value, String> {
        let mut messages: Vec<Value> = self.page_op(&json!({ "op": "overlayErrors" }))?.as_array().cloned().unwrap_or_default();
        // The page's uncaught errors and rejections since it started loading.
        if let Ok(data) = self.session.run(&["errors"], None) {
            for e in data.get("errors").and_then(Value::as_array).into_iter().flatten() {
                let Some(text) = e.get("text").and_then(Value::as_str) else { continue };
                let message = Value::String(text.lines().next().unwrap_or(text).to_string());
                if !messages.contains(&message) {
                    messages.push(message);
                }
            }
        }
        Ok(Value::Array(messages))
    }

    fn screenshot(&self) -> Result<Value, String> {
        std::fs::create_dir_all(&self.scratch).map_err(|e| e.to_string())?;
        let path = self.scratch.join(format!("{:x}.png", random_u32())).to_string_lossy().into_owned();
        self.session.run(&["screenshot", &path], None).map_err(|e| e.message)?;
        Ok(json!({ "path": path }))
    }
}

impl scan::PageIo for AgentBrowserPage<'_> {
    fn call(&mut self, op: Value) -> Result<Value, String> {
        match op.get("op").and_then(Value::as_str) {
            Some("errors") => self.errors(),
            Some("screenshot") => self.screenshot(),
            _ => self.page_op(&op),
        }
    }
}

/// Resolve raw results against the registry, as `detect` reports them.
fn to_findings(raw: Vec<scan::RawResult>, url: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for r in raw {
        let Some(mut item) = try_finding(&r.id, url, &r.snippet, 0.0) else { continue };
        if !r.ignore_value.is_empty() {
            item.extras.insert("ignoreValue".into(), Value::String(r.ignore_value));
        }
        if !r.severity.is_empty() {
            item.severity = r.severity;
        }
        derive_advisory_flag(&mut item);
        findings.push(item);
    }
    findings
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

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!("impeccable-page-scan-{}-{:x}", std::process::id(), random_u32()))
}

fn random_u32() -> u32 {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(std::time::Instant::now()) as u32
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

    /// A page with `tests/fixtures/antipatterns/quality.html` loaded, without
    /// a browser: captures return the recorded snapshot, hit tests find
    /// nothing, images do not load, and one script error was seen.
    struct FakePage {
        snapshot: String,
    }

    impl scan::PageIo for FakePage {
        fn call(&mut self, op: Value) -> Result<Value, String> {
            Ok(match op["op"].as_str().unwrap() {
                "capture" => json!({ "json": self.snapshot }),
                "answer" => {
                    let hits: Vec<Value> = op["hitTests"]
                        .as_array()
                        .map(|points| points.iter().map(|p| json!({ "x": p[0], "y": p[1], "top": 0, "stack": [] })).collect())
                        .unwrap_or_default();
                    json!({ "hits": hits })
                }
                "errors" => json!(["TypeError: boom"]),
                "scroll" => json!({ "x": 0, "y": 0 }),
                "geometry" => json!({}),
                "scrollIntoView" | "hideText" => Value::Bool(false),
                "loadImage" | "rect" => Value::Null,
                "readPixel" => json!({ "noContext": true }),
                _ => Value::Bool(true),
            })
        }
    }

    #[test]
    fn scan_reports_rendered_page_rules_and_script_errors() {
        let snapshot = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/quality.snapshot.json")).unwrap();
        let raw = scan::scan(&mut FakePage { snapshot }, &browser_config(None)).unwrap();
        let findings = to_findings(raw, "http://localhost:3000/");
        let ids: Vec<&str> = findings.iter().map(|f| f.antipattern.as_str()).collect();
        // `line-length` needs rendered line boxes; no source-file engine sees it.
        assert!(ids.contains(&"line-length"), "{ids:?}");
        assert!(ids.contains(&"script-error"), "{ids:?}");
        assert!(findings.iter().all(|f| f.file == "http://localhost:3000/"));
    }
}
