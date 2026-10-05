//! `impeccino detect <url>`: the detector's rendered-page rules, measured in
//! a page that agent-browser holds (docs/adr/0016).
//!
//! Impeccino ships no browser. It drives `agent-browser`, the headless
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
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use impeccino_core::browser::driver::normalize_browser_font_name;
use impeccino_core::browser::{BrowserConfig, DesignSystemConfig};
use impeccino_core::findings::{derive_advisory_flag, try_finding, Finding};
use impeccino_detect::design_system::DesignSystem;
use impeccino_detect::engines::{EngineError, ScanOptions, SharedBrowser, UrlEngine};
use serde_json::{json, Value};

mod cdp;
mod scan;

/// The read-only page measurement: DOM, computed styles, rects, viewport.
const SNAPSHOT_JS: &str = include_str!("../../assets/page-snapshot.js");

/// The page operations the scan asks for, installed next to the measurement
/// as `window.__impeccinoProbe`. Page errors and screenshots come from
/// agent-browser itself, not from here.
const PROBE_JS: &str = r#"
const S = __impeccinoSnapshot;
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
  '[data-impeccino-visual-contrast-target] {',
  '  color: transparent !important;',
  '  -webkit-text-fill-color: transparent !important;',
  '  text-shadow: none !important;',
  '}',
  '[data-impeccino-visual-contrast-target][data-impeccino-bgclip-text="true"] {',
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
      let style = document.getElementById('impeccino-visual-contrast-hide-style');
      if (!style) { style = document.createElement('style'); style.id = 'impeccino-visual-contrast-hide-style'; style.textContent = HIDE_STYLE; document.head.appendChild(style); }
      el.setAttribute('data-impeccino-visual-contrast-target', '1');
      if (op.backgroundClipText) el.setAttribute('data-impeccino-bgclip-text', 'true');
      return true;
    }
    case 'showText': {
      const el = find(op.selector);
      if (el) { el.removeAttribute('data-impeccino-visual-contrast-target'); el.removeAttribute('data-impeccino-bgclip-text'); }
      const style = document.getElementById('impeccino-visual-contrast-hide-style');
      if (style) style.remove();
      return true;
    }
    default: return { error: 'unknown op ' + op.op };
  }
};
window.__impeccinoProbe = { run };
"#;

/// The viewport of a fresh session (the former URL engine's default).
const DEFAULT_VIEWPORT: (u32, u32) = (1280, 800);

const MISSING_BROWSER: &str = "Rendered-page scans need agent-browser (https://github.com/vercel-labs/agent-browser). Install it with `npm install -g agent-browser && agent-browser install`, or scan the source files instead.";
const AGENT_BROWSER_TIMEOUT: Duration = Duration::from_secs(60);
const AGENT_BROWSER_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const AGENT_BROWSER_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_AGENT_BROWSER_STDIN: usize = 4 * 1024 * 1024;
const MAX_AGENT_BROWSER_STDOUT: u64 = 64 * 1024 * 1024;
const MAX_AGENT_BROWSER_STDERR: u64 = 2 * 1024 * 1024;
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// [`UrlEngine`] over agent-browser.
pub struct AgentBrowserEngine;

impl UrlEngine for AgentBrowserEngine {
    fn detect_url(&self, url: &str, options: &ScanOptions) -> Result<Vec<Finding>, EngineError> {
        let _interrupt = interrupt_guard()?;
        Session::start(options.viewport)?.measure(url, options)
    }

    fn open_shared(&self) -> Option<Box<dyn SharedBrowser + '_>> {
        Some(Box::new(SharedSession {
            session: RefCell::new(None),
            _interrupt: interrupt_guard().ok()?,
        }))
    }
}

/// One session for a multi-URL scan, started on first use.
struct SharedSession {
    session: RefCell<Option<Rc<Session>>>,
    _interrupt: impeccino_common::proc::InterruptGuard,
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
    browser: impeccino_common::agent_browser::AgentBrowser,
    name: String,
    owned: bool,
    closed: std::cell::Cell<bool>,
}

impl Session {
    fn browser() -> Option<impeccino_common::agent_browser::AgentBrowser> {
        let search_paths = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .unwrap_or_default();
        impeccino_common::agent_browser::resolve_agent_browser(
            std::env::var_os("IMPECCINO_AGENT_BROWSER").as_deref(),
            &search_paths,
            std::env::var_os("PATHEXT").as_deref(),
        )
    }

    fn check_installed() -> Result<(), EngineError> {
        let browser = Self::browser().ok_or_else(|| EngineError::new(MISSING_BROWSER))?;
        Self::check_installed_with(&browser)
    }

    fn start(viewport: Option<(u32, u32)>) -> Result<Session, EngineError> {
        let browser = Self::browser().ok_or_else(|| EngineError::new(MISSING_BROWSER))?;
        Self::check_installed_with(&browser)?;
        let (name, owned) = match std::env::var("AGENT_BROWSER_SESSION") {
            Ok(name) if !name.is_empty() => (name, false),
            _ => (
                format!("impeccino-{}-{:x}", std::process::id(), random_u32()),
                true,
            ),
        };
        Self::start_with(browser, name, owned, viewport)
    }

    fn start_with(
        browser: impeccino_common::agent_browser::AgentBrowser,
        name: String,
        owned: bool,
        viewport: Option<(u32, u32)>,
    ) -> Result<Session, EngineError> {
        let session = Session {
            browser,
            name,
            owned,
            closed: std::cell::Cell::new(false),
        };
        if let Some((w, h)) = viewport.or(owned.then_some(DEFAULT_VIEWPORT)) {
            session.run(&["set", "viewport", &w.to_string(), &h.to_string()], None)?;
        }
        Ok(session)
    }

    fn check_installed_with(
        browser: &impeccino_common::agent_browser::AgentBrowser,
    ) -> Result<(), EngineError> {
        let cmd = browser.command(&[OsString::from("--version")]);
        match run_process(cmd, "--version", None, AGENT_BROWSER_PROBE_TIMEOUT, true) {
            Ok(out) if out.status.success() => Ok(()),
            Ok(_) => Err(EngineError::new(MISSING_BROWSER)),
            Err(error) => Err(error),
        }
    }

    fn close(&self) {
        if self.owned && !self.closed.replace(true) {
            let _ = self.run_with_timeout(&["close"], None, AGENT_BROWSER_CLOSE_TIMEOUT, false);
        }
    }

    /// Run one agent-browser command with `--json` and return its `data`.
    fn run(&self, args: &[&str], stdin: Option<&str>) -> Result<Value, EngineError> {
        self.run_with_timeout(args, stdin, AGENT_BROWSER_TIMEOUT, true)
    }

    fn run_with_timeout(
        &self,
        args: &[&str],
        stdin: Option<&str>,
        timeout: Duration,
        check_interrupt: bool,
    ) -> Result<Value, EngineError> {
        let verb = args.first().copied().unwrap_or("");
        let mut command_args: Vec<OsString> = vec![
            "--session".into(),
            self.name.clone().into(),
            "--json".into(),
        ];
        command_args.extend(args.iter().map(OsString::from));
        let cmd = self.browser.command(&command_args);
        let out = run_process(cmd, verb, stdin, timeout, check_interrupt)?;
        let reply: Value = serde_json::from_slice(&out.stdout).map_err(|_| {
            EngineError::new(format!(
                "agent-browser {verb}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        })?;
        if reply.get("success").and_then(Value::as_bool) != Some(true) {
            let error = reply
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(EngineError::new(format!("agent-browser {verb}: {error}")));
        }
        Ok(reply.get("data").cloned().unwrap_or(Value::Null))
    }

    fn eval(&self, script: &str) -> Result<Value, String> {
        let data = self
            .run(&["eval", "--stdin"], Some(script))
            .map_err(|e| e.message)?;
        Ok(data.get("result").cloned().unwrap_or(Value::Null))
    }

    /// The fast lane to the open page, when the browser exposes DevTools.
    fn channel(&self) -> Option<cdp::PageChannel> {
        let endpoint = self.run(&["get", "cdp-url"], None).ok()?;
        let page = self.run(&["get", "url"], None).ok()?;
        let endpoint = endpoint
            .get("cdpUrl")
            .or_else(|| endpoint.get("url"))
            .or(Some(&endpoint))
            .and_then(Value::as_str)?
            .to_string();
        let page = page
            .get("url")
            .or(Some(&page))
            .and_then(Value::as_str)?
            .to_string();
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
        let mut page = AgentBrowserPage {
            session: self,
            channel: self.channel(),
            scratch: scratch_dir(),
        };
        let raw = scan::scan(&mut page, &config);
        let _ = std::fs::remove_dir_all(&page.scratch);
        Ok(to_findings(raw.map_err(EngineError::new)?, url))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

fn interrupt_guard() -> Result<impeccino_common::proc::InterruptGuard, EngineError> {
    impeccino_common::proc::InterruptGuard::install(&INTERRUPTED).map_err(|e| {
        EngineError::new(format!(
            "could not install browser scan signal handler: {e}"
        ))
    })
}

/// Private per-command files keep subprocess I/O independent of descendants
/// that inherit handles. The directory is unique and owner-only on Unix.
struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn create() -> std::io::Result<TempWorkspace> {
        for _ in 0..32 {
            let path = std::env::temp_dir().join(format!(
                "impeccino-agent-browser-{}-{:x}",
                std::process::id(),
                random_u32()
            ));
            #[cfg(unix)]
            let created = {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = fs::DirBuilder::new();
                builder.mode(0o700).create(&path)
            };
            #[cfg(not(unix))]
            let created = fs::create_dir(&path);
            match created {
                Ok(()) => {
                    return Ok(TempWorkspace(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique browser temp directory",
        ))
    }

    fn file(&self, name: &str) -> std::io::Result<File> {
        OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(self.0.join(name))
    }

    fn size(&self, name: &str) -> std::io::Result<u64> {
        fs::metadata(self.0.join(name)).map(|metadata| metadata.len())
    }

    fn read_limited(&self, name: &str, limit: u64) -> std::io::Result<Vec<u8>> {
        let mut file = File::open(self.0.join(name))?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "agent-browser output exceeded its size limit",
            ));
        }
        Ok(bytes)
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        // Cleanup is best-effort and nonblocking. On Windows, a descendant
        // that inherited a standard-handle file can keep this private
        // directory busy after the direct CLI exits.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_process(
    cmd: Command,
    verb: &str,
    stdin: Option<&str>,
    timeout: Duration,
    check_interrupt: bool,
) -> Result<Output, EngineError> {
    let deadline = Instant::now() + timeout;
    if stdin.is_some_and(|input| input.len() > MAX_AGENT_BROWSER_STDIN) {
        return Err(EngineError::new(format!(
            "agent-browser {verb}: stdin exceeds the {} MiB limit",
            MAX_AGENT_BROWSER_STDIN / (1024 * 1024)
        )));
    }
    if Instant::now() >= deadline {
        return Err(EngineError::new(format!(
            "agent-browser {verb} timed out before launch"
        )));
    }
    let workspace = TempWorkspace::create()
        .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
    run_process_in_workspace(
        cmd,
        verb,
        stdin,
        timeout,
        deadline,
        check_interrupt,
        workspace,
    )
}

fn run_process_in_workspace(
    mut cmd: Command,
    verb: &str,
    stdin: Option<&str>,
    timeout: Duration,
    deadline: Instant,
    check_interrupt: bool,
    workspace: TempWorkspace,
) -> Result<Output, EngineError> {
    let stdin_file = if let Some(input) = stdin {
        let mut file = workspace
            .file("stdin")
            .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
        file.write_all(input.as_bytes())
            .and_then(|_| file.flush())
            .and_then(|_| file.seek(SeekFrom::Start(0)))
            .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
        Some(file)
    } else {
        None
    };
    let stdout_file = workspace
        .file("stdout")
        .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
    let stderr_file = workspace
        .file("stderr")
        .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
    if Instant::now() >= deadline {
        return Err(EngineError::new(format!(
            "agent-browser {verb} timed out before launch"
        )));
    }
    cmd.stdin(stdin_file.map(Stdio::from).unwrap_or_else(Stdio::null))
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    impeccino_common::proc::hide_window(&mut cmd);

    let spawn_result = cmd.spawn();
    // `Command` retains the configured stdio files on Windows. Drop it before
    // any early return or TempWorkspace cleanup attempts to remove them.
    drop(cmd);
    let mut child = spawn_result.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            EngineError::new(MISSING_BROWSER)
        } else {
            EngineError::new(format!("agent-browser {verb}: {e}"))
        }
    })?;
    let status = loop {
        if check_interrupt && INTERRUPTED.load(Ordering::SeqCst) {
            stop_child(&mut child);
            return Err(EngineError::new(format!(
                "agent-browser {verb}: interrupted"
            )));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                stop_child(&mut child);
                return Err(EngineError::new(format!("agent-browser {verb}: {error}")));
            }
        }
        let output_too_large = workspace
            .size("stdout")
            .is_ok_and(|size| size > MAX_AGENT_BROWSER_STDOUT)
            || workspace
                .size("stderr")
                .is_ok_and(|size| size > MAX_AGENT_BROWSER_STDERR);
        if output_too_large {
            stop_child(&mut child);
            return Err(EngineError::new(format!(
                "agent-browser {verb}: output exceeded its size limit"
            )));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            stop_child(&mut child);
            return Err(EngineError::new(format!(
                "agent-browser {verb} timed out after {} ms",
                timeout.as_millis()
            )));
        }
        thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
    };
    let stdout = workspace
        .read_limited("stdout", MAX_AGENT_BROWSER_STDOUT)
        .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
    let stderr = workspace
        .read_limited("stderr", MAX_AGENT_BROWSER_STDERR)
        .map_err(|e| EngineError::new(format!("agent-browser {verb}: {e}")))?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn stop_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    #[cfg(windows)]
    stop_child_tree(child.id());
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    let _ = child.kill();
    // Reap promptly after the direct child is killed, but keep error cleanup
    // bounded if the platform refuses termination for an unexpected reason.
    let deadline = Instant::now() + Duration::from_millis(250);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// A `.cmd` browser shim runs under `cmd.exe`, which may have started the
/// actual CLI as a descendant. On failure, terminate only the process tree
/// rooted at this still-owned command PID. Successful commands never reach
/// this path, so a detached agent-browser daemon can keep serving sessions.
#[cfg(windows)]
fn stop_child_tree(pid: u32) {
    // Tree termination can take longer on a loaded Windows host; still cap it
    // so error cleanup never waits on taskkill indefinitely.
    const TASKKILL_TIMEOUT: Duration = Duration::from_secs(2);
    let system_taskkill = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| root.join("System32").join("taskkill.exe"))
        .filter(|path| path.is_file());
    let mut helper = Command::new(system_taskkill.unwrap_or_else(|| PathBuf::from("taskkill.exe")));
    let pid = pid.to_string();
    helper
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    impeccino_common::proc::hide_window(&mut helper);
    let Ok(mut helper) = helper.spawn() else {
        return;
    };
    let deadline = Instant::now() + TASKKILL_TIMEOUT;
    loop {
        match helper.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            _ => break,
        }
    }
    let _ = helper.kill();
    // Keep even the cleanup helper bounded if taskkill unexpectedly stalls.
    let reap_deadline = Instant::now() + Duration::from_millis(100);
    while Instant::now() < reap_deadline {
        if helper.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
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
        self.eval(&format!(
            "(() => {{\n{SNAPSHOT_JS}\n{PROBE_JS}\nreturn true;\n}})()"
        ))
        .map(|_| ())
    }

    fn page_op(&mut self, op: &Value) -> Result<Value, String> {
        let script = format!(
            "(async () => window.__impeccinoProbe ? await window.__impeccinoProbe.run({op}) : {{ __impeccinoMissing: true }})()"
        );
        let out = self.eval(&script)?;
        if out.get("__impeccinoMissing").is_none() {
            return Ok(out);
        }
        // First use, or the page navigated and dropped the probe.
        self.install()?;
        self.eval(&script)
    }

    fn errors(&mut self) -> Result<Value, String> {
        let mut messages: Vec<Value> = self
            .page_op(&json!({ "op": "overlayErrors" }))?
            .as_array()
            .cloned()
            .unwrap_or_default();
        // The page's uncaught errors and rejections since it started loading.
        if let Ok(data) = self.session.run(&["errors"], None) {
            for e in data
                .get("errors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(text) = e.get("text").and_then(Value::as_str) else {
                    continue;
                };
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
        let path = self
            .scratch
            .join(format!("{:x}.png", random_u32()))
            .to_string_lossy()
            .into_owned();
        self.session
            .run(&["screenshot", &path], None)
            .map_err(|e| e.message)?;
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
        let Some(mut item) = try_finding(&r.id, url, &r.snippet, 0.0) else {
            continue;
        };
        if !r.ignore_value.is_empty() {
            item.extras
                .insert("ignoreValue".into(), Value::String(r.ignore_value));
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
        let mut allowed_fonts = Vec::new();
        for font in &ds.allowed_fonts {
            let font = normalize_browser_font_name(font);
            if !font.is_empty() && !allowed_fonts.contains(&font) {
                allowed_fonts.push(font);
            }
        }
        let allowed_colors: Vec<impeccino_core::color::Rgba> = ds
            .allowed_color_keys
            .iter()
            .map(|(_, entry)| &entry.color)
            .filter(|c| c.r.is_finite() && c.g.is_finite() && c.b.is_finite())
            .map(|c| impeccino_core::color::Rgba {
                r: c.r,
                g: c.g,
                b: c.b,
                a: None,
            })
            .collect();
        let allowed_radii = ds
            .allowed_radii
            .iter()
            .map(|r| r.px)
            .filter(|px| px.is_finite())
            .collect::<Vec<_>>();
        let declared_selectors = ds
            .declared_selectors
            .iter()
            .map(|selector| impeccino_core::js::trim(selector).to_string())
            .filter(|selector| !selector.is_empty())
            .collect();
        DesignSystemConfig {
            declared_selectors,
            has_fonts: ds.has_fonts && !allowed_fonts.is_empty(),
            allowed_fonts,
            has_colors: ds.has_colors && !allowed_colors.is_empty(),
            allowed_colors,
            has_radii: ds.has_radii && !allowed_radii.is_empty(),
            allowed_radii,
            has_pill_radius: ds.has_pill_radius,
        }
    });
    BrowserConfig { design_system }
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "impeccino-page-scan-{}-{:x}",
        std::process::id(),
        random_u32()
    ))
}

fn random_u32() -> u32 {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(std::time::Instant::now()) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::time::{Duration, Instant};

    fn fake_browser(block_eval: bool, fail_set: bool) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "impeccino fake browser & {}-{:x}",
            std::process::id(),
            random_u32()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let log = root.join("calls.log");
        #[cfg(unix)]
        let script = {
            use std::os::unix::fs::PermissionsExt;
            let log = log.to_string_lossy().replace('\'', "'\\''");
            let wait = if block_eval {
                "case \" $* \" in *\" eval --stdin \"*) while :; do :; done;; esac\n"
            } else {
                ""
            };
            let fail = if fail_set {
                "case \" $* \" in *\" set viewport \"*) printf '%s\\n' '{\"success\":false,\"error\":\"fake viewport error\"}'; exit 0;; esac\n"
            } else {
                ""
            };
            let body = format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log}'\n{fail}{wait}printf '%s\\n' '{{\"success\":true,\"data\":{{}}}}'\n"
            );
            let path = root.join("agent-browser");
            std::fs::write(&path, body).unwrap();
            let mut permissions = std::fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&path, permissions).unwrap();
            path
        };
        #[cfg(windows)]
        let script = {
            let log = log.to_string_lossy();
            let wait = if block_eval {
                "echo %* | findstr /C:\"eval --stdin\" >nul\r\nif not errorlevel 1 goto wait_forever\r\n"
            } else {
                ""
            };
            let fail = if fail_set {
                "echo %* | findstr /C:\"set viewport\" >nul\r\nif not errorlevel 1 goto fail_set\r\n"
            } else {
                ""
            };
            let wait_loop = if block_eval {
                ":wait_forever\r\ngoto wait_forever\r\n"
            } else {
                ""
            };
            let fail_label = if fail_set {
                ":fail_set\r\necho {\"success\":false,\"error\":\"fake viewport error\"}\r\nexit /b 0\r\n"
            } else {
                ""
            };
            let body = format!(
                "@echo off\r\n>>\"{log}\" echo %*\r\n{fail}{wait}echo {{\"success\":true,\"data\":{{}}}}\r\nexit /b 0\r\n{fail_label}{wait_loop}"
            );
            let path = root.join("agent-browser.cmd");
            std::fs::write(&path, body).unwrap();
            path
        };
        (script, log)
    }

    #[cfg(unix)]
    fn kill_isolated_test_group(child: &mut Child) {
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn agent_browser_deadline_covers_an_unresponsive_command_with_large_stdin() {
        let (bin, _) = fake_browser(true, false);
        let browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(bin.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        let session = Session {
            browser,
            name: "impeccino-test".into(),
            owned: false,
            closed: std::cell::Cell::new(false),
        };
        let script = "x".repeat(2 * 1024 * 1024);
        let workspace = TempWorkspace::create().unwrap();
        let workspace_path = workspace.0.clone();
        let start = Instant::now();
        let mut args: Vec<OsString> = vec![
            "--session".into(),
            session.name.clone().into(),
            "--json".into(),
        ];
        args.extend([OsString::from("eval"), OsString::from("--stdin")]);
        let result = run_process_in_workspace(
            session.browser.command(&args),
            "eval",
            Some(&script),
            Duration::from_millis(150),
            start + Duration::from_millis(150),
            true,
            workspace,
        );
        assert!(
            result.is_err(),
            "a blocked agent-browser process must time out"
        );
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "agent-browser exceeded its injected deadline"
        );
        assert!(
            !workspace_path.exists(),
            "timeout cleanup left its private directory behind"
        );
    }

    #[test]
    fn process_workspace_is_removed_after_success_and_nonzero_exit() {
        let (bin, _) = fake_browser(false, false);
        let browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(bin.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        let workspace = TempWorkspace::create().unwrap();
        let workspace_path = workspace.0.clone();
        let output = run_process_in_workspace(
            browser.command(&[OsString::from("--json")]),
            "test",
            None,
            Duration::from_secs(2),
            Instant::now() + Duration::from_secs(2),
            false,
            workspace,
        )
        .unwrap();
        assert!(output.status.success());
        assert!(
            !workspace_path.exists(),
            "successful command cleanup left its private directory behind"
        );

        let workspace = TempWorkspace::create().unwrap();
        let workspace_path = workspace.0.clone();
        let mut command = Command::new(if cfg!(windows) { "cmd.exe" } else { "sh" });
        if cfg!(windows) {
            command.args(["/d", "/c", "exit", "/b", "7"]);
        } else {
            command.args(["-c", "exit 7"]);
        }
        let output = run_process_in_workspace(
            command,
            "test",
            None,
            Duration::from_secs(2),
            Instant::now() + Duration::from_secs(2),
            false,
            workspace,
        )
        .unwrap();
        assert!(!output.status.success());
        assert!(
            !workspace_path.exists(),
            "failed command cleanup left its private directory behind"
        );
    }

    #[test]
    fn dropping_an_owned_session_closes_it() {
        let (bin, log) = fake_browser(false, false);
        let browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(bin.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        drop(Session {
            browser,
            name: "impeccino-test".into(),
            owned: true,
            closed: std::cell::Cell::new(false),
        });
        let calls = std::fs::read_to_string(log).unwrap_or_default();
        assert!(
            calls.lines().any(|line| line.ends_with(" close")),
            "owned session was not closed: {calls:?}"
        );
    }

    #[test]
    fn dropping_a_borrowed_session_never_closes_it() {
        let (bin, log) = fake_browser(false, false);
        let browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(bin.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        drop(Session {
            browser,
            name: "borrowed-test".into(),
            owned: false,
            closed: std::cell::Cell::new(false),
        });
        assert!(std::fs::read_to_string(log).unwrap_or_default().is_empty());
    }

    #[test]
    fn a_viewport_setup_error_closes_the_new_owned_session() {
        let (bin, log) = fake_browser(false, true);
        let browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(bin.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        let result = Session::start_with(browser, "owned-test".into(), true, Some((1280, 800)));
        assert!(result.is_err());
        let calls = std::fs::read_to_string(log).unwrap_or_default();
        assert!(
            calls.lines().any(|line| line.contains("set viewport")),
            "viewport setup was not attempted: {calls:?}"
        );
        assert!(
            calls.lines().any(|line| line.ends_with(" close")),
            "owned session was not closed after setup failed: {calls:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn sigterm_interrupts_a_browser_command_and_restores_the_child_handler() {
        const CHILD_MODE: &str = "IMPECCINO_PAGE_SCAN_SIGTERM_CHILD";
        if let Ok(bin) = std::env::var(CHILD_MODE) {
            let browser = impeccino_common::agent_browser::resolve_agent_browser(
                Some(OsStr::new(&bin)),
                &[],
                None,
            )
            .unwrap();
            let _guard = interrupt_guard().unwrap();
            let session = Session {
                browser,
                name: "sigterm-test".into(),
                owned: false,
                closed: std::cell::Cell::new(false),
            };
            let result = session.run_with_timeout(
                &["eval", "--stdin"],
                Some("probe"),
                Duration::from_secs(5),
                true,
            );
            assert!(result.unwrap_err().message.contains("interrupted"));
            return;
        }

        use std::os::unix::process::CommandExt;
        let (bin, log) = fake_browser(true, false);
        let executable = std::env::current_exe().unwrap();
        let test_name =
            "page_scan::tests::sigterm_interrupts_a_browser_command_and_restores_the_child_handler";
        let mut command = Command::new(executable);
        command
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_MODE, &bin)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // If the regression returns, only this test process and its fake CLI
        // are killed; no browser owned by the developer is involved.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command.spawn().expect("spawn isolated signal-test process");
        let ready_deadline = Instant::now() + Duration::from_secs(5);
        let ready = loop {
            if std::fs::read_to_string(&log)
                .is_ok_and(|calls| calls.lines().any(|line| line.contains(" eval --stdin")))
            {
                break true;
            }
            if child.try_wait().unwrap().is_some() || Instant::now() >= ready_deadline {
                break false;
            }
            thread::sleep(Duration::from_millis(10));
        };
        if !ready {
            kill_isolated_test_group(&mut child);
            panic!("child test never started the fake browser command");
        }
        assert_eq!(
            unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) },
            0
        );
        let exit_deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= exit_deadline {
                kill_isolated_test_group(&mut child);
                panic!("SIGTERM did not unwind the browser command within its deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "the signal-observing test process failed: {status}"
        );
    }

    #[cfg(windows)]
    struct TestChildProcess {
        pid_file: PathBuf,
        pid: Option<u32>,
    }

    #[cfg(windows)]
    impl TestChildProcess {
        fn set_pid(&mut self, pid: u32) {
            self.pid = Some(pid);
        }
    }

    #[cfg(windows)]
    impl Drop for TestChildProcess {
        fn drop(&mut self) {
            let pid = self
                .pid
                .or_else(|| test_child_pid(&self.pid_file, Duration::from_secs(2)));
            let Some(pid) = pid else { return };
            impeccino_common::proc::terminate(pid as i64);
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline && impeccino_common::proc::pid_reachable(pid as i64) {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    #[cfg(windows)]
    fn test_child_pid(path: &std::path::Path, timeout: Duration) -> Option<u32> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(pid) = std::fs::read_to_string(path).map(|value| value.trim().parse()) {
                if let Ok(pid) = pid {
                    return Some(pid);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_cmd_timeout_kills_its_owned_child_tree_but_success_keeps_a_detached_process() {
        const CHILD_PID_FILE: &str = "IMPECCINO_PAGE_SCAN_TREE_CHILD_PID_FILE";
        const CHILD_EXE: &str = "IMPECCINO_PAGE_SCAN_TREE_CHILD_EXE";
        if let Ok(pid_file) = std::env::var(CHILD_PID_FILE) {
            std::fs::write(pid_file, std::process::id().to_string()).unwrap();
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }

        let root = std::env::temp_dir().join(format!(
            "impeccino cmd tree & {}-{:x}",
            std::process::id(),
            random_u32()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let executable = std::env::current_exe().unwrap();
        let test_name = "page_scan::tests::windows_cmd_timeout_kills_its_owned_child_tree_but_success_keeps_a_detached_process";
        let start_child = format!(
            "start \"\" /B \"%{CHILD_EXE}%\" --exact {test_name} --nocapture <nul >nul 2>&1\r\n"
        );

        let timeout_pid_file = root.join("timeout-child.pid");
        let mut timeout_child_guard = TestChildProcess {
            pid_file: timeout_pid_file.clone(),
            pid: None,
        };
        let timeout_shim = root.join("agent-browser-timeout.cmd");
        std::fs::write(
            &timeout_shim,
            format!("@echo off\r\n{start_child}:wait\r\nping -n 2 127.0.0.1 >nul\r\ngoto wait\r\n"),
        )
        .unwrap();
        let timeout_browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(timeout_shim.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        let mut timeout_command = timeout_browser.command(&[]);
        timeout_command
            .current_dir(&root)
            .env(CHILD_EXE, &executable)
            .env(CHILD_PID_FILE, &timeout_pid_file);
        let workspace = TempWorkspace::create().unwrap();
        let workspace_path = workspace.0.clone();
        let start = Instant::now();
        let result = run_process_in_workspace(
            timeout_command,
            "tree-test",
            None,
            Duration::from_secs(1),
            start + Duration::from_secs(1),
            false,
            workspace,
        );
        assert!(result.unwrap_err().message.contains("timed out"));
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Windows process-tree cleanup exceeded its bound"
        );
        let timeout_pid = test_child_pid(&timeout_pid_file, Duration::from_secs(2))
            .expect("the .cmd shim spawned its child");
        timeout_child_guard.set_pid(timeout_pid);
        let exit_deadline = Instant::now() + Duration::from_secs(2);
        while impeccino_common::proc::pid_reachable(timeout_pid as i64)
            && Instant::now() < exit_deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !impeccino_common::proc::pid_reachable(timeout_pid as i64),
            "taskkill left the .cmd descendant alive"
        );
        assert!(
            !workspace_path.exists(),
            "tree timeout left its private stdio directory behind"
        );

        let success_pid_file = root.join("success-child.pid");
        let mut success_child_guard = TestChildProcess {
            pid_file: success_pid_file.clone(),
            pid: None,
        };
        let success_shim = root.join("agent-browser-success.cmd");
        std::fs::write(
            &success_shim,
            format!("@echo off\r\n{start_child}echo {{\"success\":true,\"data\":{{}}}}\r\n"),
        )
        .unwrap();
        let success_browser = impeccino_common::agent_browser::resolve_agent_browser(
            Some(success_shim.as_os_str()),
            &[],
            None,
        )
        .unwrap();
        let mut success_command = success_browser.command(&[]);
        success_command
            .current_dir(&root)
            .env(CHILD_EXE, &executable)
            .env(CHILD_PID_FILE, &success_pid_file);
        let output = run_process(
            success_command,
            "tree-success",
            None,
            Duration::from_secs(2),
            false,
        )
        .unwrap();
        assert!(output.status.success());
        assert!(serde_json::from_slice::<Value>(&output.stdout).is_ok());
        let success_pid = test_child_pid(&success_pid_file, Duration::from_secs(2))
            .expect("successful shim detached its child");
        success_child_guard.set_pid(success_pid);
        assert!(
            impeccino_common::proc::pid_reachable(success_pid as i64),
            "successful command cleanup killed a detached child"
        );
        drop(success_child_guard);
        let _ = std::fs::remove_dir_all(root);
    }

    /// The JS array literal `const <name> = [ ... ];` from the measurement script.
    fn js_list(name: &str) -> Vec<String> {
        let start = SNAPSHOT_JS.find(&format!("const {name} = [")).expect(name);
        let body = &SNAPSHOT_JS[start..];
        let body = &body[body.find('[').unwrap() + 1..body.find("];").unwrap()];
        body.split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    #[test]
    fn measurement_script_captures_exactly_the_properties_the_rules_read() {
        use impeccino_core::browser::snapshot::{PSEUDO_PROPS, STYLE_PROPS};
        assert_eq!(
            js_list("__SNAP_STYLE_PROPS"),
            STYLE_PROPS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            js_list("__SNAP_PSEUDO_PROPS"),
            PSEUDO_PROPS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
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
                        .map(|points| {
                            points
                                .iter()
                                .map(|p| json!({ "x": p[0], "y": p[1], "top": 0, "stack": [] }))
                                .collect()
                        })
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
        let snapshot = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/quality.snapshot.json"
        ))
        .unwrap();
        let raw = scan::scan(&mut FakePage { snapshot }, &browser_config(None)).unwrap();
        let findings = to_findings(raw, "http://localhost:3000/");
        let ids: Vec<&str> = findings.iter().map(|f| f.antipattern.as_str()).collect();
        // `line-length` needs rendered line boxes; no source-file engine sees it.
        assert!(ids.contains(&"line-length"), "{ids:?}");
        assert!(ids.contains(&"script-error"), "{ids:?}");
        assert!(findings.iter().all(|f| f.file == "http://localhost:3000/"));
    }
}
