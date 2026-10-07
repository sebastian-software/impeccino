//! Shared hook state, harness envelopes and scan helpers.

use std::collections::HashMap;
use std::rc::Rc;

use impeccino_core::findings::Finding;
use impeccino_core::js;
use impeccino_detect::config::{
    extract_finding_ignore_value, normalize_ignore_rule, normalize_ignore_value,
};
use impeccino_detect::design_decisions::DesignDecisions;
use impeccino_detect::design_system::{
    load_design_system_for_cwd, resolve_design_md_path, DesignSystem,
};
use impeccino_detect::detect_text::{detect_text, TextOptions};
use impeccino_detect::engines::{HtmlEngine, ScanOptions};
use impeccino_detect::project_ignores::ProjectIgnores;
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock as Lazy;

use crate::util::{
    exists, iso_now, js_string, jsp, now_value, obj_field, safe_read, safe_read_json, slice_prefix,
    str_field, truthy_value, utf16_len,
};
use impeccino_common::project_paths::has_project_marker;
pub use impeccino_common::scan_scope::{
    is_generated_path, HOOK_SOURCE_EXTENSIONS as ALLOWED_EXTS, TEMPLATE_EXTENSIONS,
};

pub const ENVELOPE_PREFIX: &str = "[impeccino@1]";

pub const ACK_EXTS: &[&str] = &[
    ".tsx", ".jsx", ".html", ".htm", ".vue", ".svelte", ".astro", ".css", ".scss", ".sass", ".less",
];

const WS: &str = impeccino_core::js::WS;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: Lazy<Regex> = Lazy::new(|| Regex::new(&$pat).expect(stringify!($name)));
    };
}

// JS: SENSITIVE_PATH (case-insensitive). The lookahead `(?=[._-])` in the
// secret/credential branch is rewritten as an optional consuming
// `[._-][^/\\]*` before the final `\.ext$`, which accepts the same strings.
re!(
    SENSITIVE_PATH_RE,
    [
        r"(?:^|[/\\])\.env(?:\.|$)",
        r"(?:^|[/\\])\.git(?:[/\\]|$)",
        r"(?:^|[/\\])id_rsa(?:$|[._-])[^/\\]*$",
        r"(?:^|[/\\])[^/\\]*\.pem$",
        r"(?:^|[/\\])(?:[^/\\]*[._-])?(?:secret|secrets|credential|credentials)(?:[._-][^/\\]*)?\.(?:json|ya?ml|toml|ini|conf|config|env|txt|key|cert|crt|pem|js|ts)$",
    ]
    .iter()
    .map(|p| format!("(?i:{p})"))
    .collect::<Vec<_>>()
    .join("|")
);

pub fn is_sensitive_path(p: &str) -> bool {
    SENSITIVE_PATH_RE.is_match(p)
}

/// JS: truthy(value) — `/^(1|true|yes|on)$/i` on a string (no trim).
pub fn truthy(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.to_ascii_lowercase()).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

/// The immediate tier defined by the shared registry.
pub use impeccino_core::registry::IMMEDIATE_TIER_RULES;

/// A legacy id fallback that keeps older detector findings recognizable when
/// they carry neither the runtime flag nor the canonical advisory severity.
/// Current findings are classified by their serialized metadata (pbakaus/impeccable#709).
pub const ADVISORY_RULES: &[&str] = &["em-dash-overuse"];

/// JS: isAdvisoryFinding(finding)
pub fn is_advisory_finding(f: &Finding) -> bool {
    let id = normalize_ignore_rule(&f.antipattern);
    !id.is_empty()
        && (ADVISORY_RULES.contains(&id.as_str())
            || f.advisory == Some(true)
            || f.severity == "advisory")
}

const CACHE_MAX_SESSIONS: usize = 8;
const CACHE_MAX_PROJECT_ROOTS: usize = 16;
pub const EDIT_COUNT_THRESHOLD: u64 = 6;
pub const MAX_SCAN_TARGETS: usize = 6;
pub const STOP_MAX_FILES: usize = 20;
const STEER_LINE: &str = "That does not mean the design is good: keep following the project design system and the impeccino skill guidance.";

// ── paths ─────────────────────────────────────────────────────────────────

/// `hookStateDir`: (issue pbakaus/impeccable#422) — where the mutable hook cache
/// lives: a per-project directory under the user cache,
/// `<cache>/impeccino/projects/<slug>-<hash>` (docs/adr/0020), so the project
/// itself carries no tool state. IMPECCINO_CACHE_ROOT replaces
/// `<cache>/impeccino/projects` with a root of its own.
///
/// Read from the process env (the JS read process.env, not runHook's
/// injected env): the cache root is a machine-scoped setting like
/// CURSOR_PROJECT_DIR, not a per-invocation switch. Trim guards against
/// stray whitespace in env files; `~/` (or the Windows `~\` spelling)
/// expands via os.homedir(), and when no home dir can be determined the
/// expansion is rejected — state falls back to the user-cache default
/// rather than anchoring under the hook process's cwd. Resolving both sides
/// makes the slug deterministic when callers hand in a trailing separator or
/// unnormalized cwd. The slug is the readable separator-mapped path PLUS an
/// 8-hex sha256 of the resolved path: the readable part alone is lossy
/// (`/x/my.app` and `/x/my-app` would both map to `-x-my-app` and share
/// state), so the digest disambiguates while keeping the dir name
/// human-scannable.
fn hook_state_dir(cwd: &str) -> String {
    let raw = std::env::var("IMPECCINO_CACHE_ROOT").unwrap_or_default();
    let mut root = impeccino_core::js::trim(&raw).to_string();
    if root.starts_with("~/") || root.starts_with("~\\") || root == "~" {
        // JS: os.homedir() || '' — HOME on unix, USERPROFILE on Windows.
        let home = impeccino_common::project_files::home_dir(|key| std::env::var(key).ok())
            .unwrap_or_default();
        root = if home.is_empty() {
            String::new()
        } else {
            let rest = if root.len() > 2 { &root[2..] } else { "" };
            jsp::join(&[&home, rest])
        };
    }
    let proc_cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| jsp::SEP.to_string());
    if root.is_empty() {
        root = user_projects_cache_root();
    }
    let resolved = jsp::resolve(&proc_cwd, &[cwd]);
    let slug: String = resolved
        .chars()
        .map(|c| {
            if matches!(c, ':' | '\\' | '/' | '.') {
                '-'
            } else {
                c
            }
        })
        .collect();
    let digest = {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(resolved.as_bytes());
        format!("{:x}", h.finalize())[..8].to_string()
    };
    jsp::join(&[
        &jsp::resolve(&proc_cwd, &[&root]),
        &format!("{}-{}", slug, digest),
    ])
}

/// `<user cache>/impeccino/projects`, read from the process environment like
/// IMPECCINO_CACHE_ROOT. Without any home directory the system temp dir
/// stands in, so state never lands in the project.
fn user_projects_cache_root() -> String {
    let base = impeccino_common::project_files::user_cache_dir(|k| std::env::var(k).ok())
        .unwrap_or_else(|| jsp::join(&[&std::env::temp_dir().to_string_lossy(), "impeccino"]));
    jsp::join(&[&base, "projects"])
}

pub fn get_cache_path(cwd: &str) -> String {
    jsp::join(&[&hook_state_dir(cwd), "hook.cache.json"])
}
// ── runtime handle ────────────────────────────────────────────────────────

/// What the JS reached through `process.*`, `import.meta.url` and
/// `lib/provider.mjs`: the process cwd, the environment, the command names
/// printed in messages, and the detector engines.
pub struct Runtime<'a> {
    pub proc_cwd: String,
    pub env: HashMap<String, String>,
    /// `IMPECCINO_COMMAND` (`/impeccino` or `$impeccino`).
    pub impeccino_command: String,
    /// The `HOOK_ADMIN_COMMAND` printed in the full footer
    /// Printed as `'<self>' hooks`, using the configured launcher.
    pub hook_admin_command: String,
    pub html: &'a dyn HtmlEngine,
    /// `process.platform === 'win32'` (command-arg quoting).
    pub win32: bool,
    canonical_cache: std::cell::RefCell<HashMap<String, String>>,
}

impl<'a> Runtime<'a> {
    pub fn new(
        proc_cwd: String,
        env: HashMap<String, String>,
        impeccino_command: String,
        self_cmd: &str,
        html: &'a dyn HtmlEngine,
    ) -> Self {
        let win32 = cfg!(windows);
        Runtime {
            proc_cwd,
            env,
            impeccino_command,
            hook_admin_command: format!("{} hooks", quote_command_arg(self_cmd, win32)),
            html,
            win32,
            canonical_cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn env(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(String::as_str)
    }

    /// `path.resolve(...)` against the process cwd.
    pub fn resolve(&self, parts: &[&str]) -> String {
        jsp::resolve(&self.proc_cwd, parts)
    }

    /// `path.relative(from, to)`.
    pub fn relative(&self, from: &str, to: &str) -> String {
        jsp::relative(&self.proc_cwd, from, to)
    }

    /// `os.homedir()`.
    pub fn homedir(&self) -> String {
        impeccino_context::util::homedir(&self.env)
    }

    /// JS: envProjectDir(fallback) — `$CURSOR_PROJECT_DIR` when non-empty.
    fn env_project_dir(&self) -> Option<&str> {
        self.env("CURSOR_PROJECT_DIR").filter(|v| !v.is_empty())
    }
}

/// JS: resolveProjectCwd(event, fallback)
pub fn resolve_project_cwd(
    rt: &Runtime,
    event: Option<&Map<String, Value>>,
    fallback: &str,
) -> String {
    if let Some(ev) = event {
        if let Some(c) = str_field(ev, "cwd") {
            return c.to_string();
        }
        if let Some(Value::Array(roots)) = ev.get("workspace_roots") {
            if let Some(Value::String(r)) = roots.first() {
                if !r.is_empty() {
                    return r.clone();
                }
            }
        }
    }
    if let Some(d) = rt.env_project_dir() {
        return d.to_string();
    }
    fallback.to_string()
}

/// JS: looksLikeProjectRoot(dir)
fn looks_like_project_root(dir: &str) -> bool {
    has_project_marker(dir)
}

/// Resolve the project that owns the edited file. A workspace root may itself
/// be a project while an app below it is a separate project with its own
/// PRODUCT.md and hook state, so prefer the nearest marker on the path from
/// the file up to that workspace root.
pub fn resolve_cache_cwd(rt: &Runtime, primary_file: Option<&str>, session_cwd: &str) -> String {
    let base = rt.resolve(&[if session_cwd.is_empty() {
        &rt.proc_cwd
    } else {
        session_cwd
    }]);
    let Some(primary) = primary_file.filter(|p| !p.is_empty()) else {
        return base;
    };
    if has_path_traversal(primary) {
        return base;
    }
    let base_is_project = looks_like_project_root(&base);
    if base_is_project && !is_scan_target_inside_project(rt, primary, &base) {
        return base;
    }
    let mut dir = jsp::dirname(&rt.resolve(&[primary]));
    let home = rt.resolve(&[&rt.homedir()]);
    let canonical_base = canonical_path(rt, &base);
    loop {
        if looks_like_project_root(&dir) {
            return dir;
        }
        if base_is_project && canonical_path(rt, &dir) == canonical_base {
            return base;
        }
        if dir == home {
            return base;
        }
        let parent = jsp::dirname(&dir);
        if parent == dir {
            return base;
        }
        dir = parent;
    }
}

/// JS: resolveProjectPlatform(cwd) — `extractPlatform(loadContext(cwd).product)`.
/// Only the platform is observable, so this reads PRODUCT.md through the same
/// resolution and skips loadContext's surface-brief and visual-implementation
/// work.
pub fn resolve_project_platform(rt: &Runtime, cwd: &str) -> Option<String> {
    let options = impeccino_context::target_args::TargetOptions::default();
    let resolved = impeccino_context::context::resolve_context(cwd, &options, &rt.env);
    let product = resolved.product_path.as_deref().and_then(safe_read);
    impeccino_context::context::extract_platform(product.as_deref())
}

/// Resolve the platform for the project that owns one touched file. The
/// target matters in workspaces where native and web apps share a hook-cache
/// root and the native app is identified by PRODUCT.md rather than package.json.
pub fn resolve_project_platform_for_target(
    rt: &Runtime,
    cwd: &str,
    target_path: &str,
) -> Option<String> {
    let options = impeccino_context::target_args::TargetOptions {
        target_path: Some(target_path.to_string()),
    };
    let resolved = impeccino_context::context::resolve_context(cwd, &options, &rt.env);
    let product = resolved.product_path.as_deref().and_then(safe_read);
    impeccino_context::context::extract_platform(product.as_deref())
}

/// JS: isNativePlatform(platform)
pub fn is_native_platform(platform: Option<&str>) -> bool {
    matches!(platform, Some("ios" | "android" | "adaptive"))
}

// ── config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionEntry {
    pub ext: String,
    pub engine: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    pub max_findings: f64,
    pub max_chars: f64,
    pub max_file_bytes: f64,
}

/// The hook's settings. There is no config file (docs/adr/0020): whether the
/// hook runs is whether it is installed in the harness settings, and the
/// knobs below are fixed. The only per-project input is DESIGN.md (waivers
/// and declared fonts), which `HookScanOptions` carries per edited file. The
/// env vars IMPECCINO_HOOK_DISABLED, IMPECCINO_HOOK_QUIET, and
/// IMPECCINO_HOOK_LOG remain the switches.
#[derive(Debug, Clone, PartialEq)]
pub struct HookConfig {
    pub design_system_enabled: bool,
    pub extensions: Vec<ExtensionEntry>,
    pub per_edit_rules: String,
    pub advisory_rules: String,
    pub limits: Limits,
}

impl Default for HookConfig {
    fn default() -> Self {
        HookConfig {
            design_system_enabled: true,
            extensions: default_template_extensions(),
            per_edit_rules: "immediate".to_string(),
            advisory_rules: "exclude".to_string(),
            limits: Limits {
                max_findings: DEFAULT_MAX_FINDINGS,
                max_chars: DEFAULT_MAX_CHARS,
                max_file_bytes: 131072.0,
            },
        }
    }
}

pub const DEFAULT_MAX_FINDINGS: f64 = 5.0;
pub const DEFAULT_MAX_CHARS: f64 = 8000.0;

/// Server-side template extensions the hook scans with the static HTML
/// engine by default.
fn default_template_extensions() -> Vec<ExtensionEntry> {
    TEMPLATE_EXTENSIONS
        .iter()
        .map(|ext| ExtensionEntry {
            ext: ext.to_string(),
            engine: "html".to_string(),
        })
        .collect()
}

/// JS: readConfig(cwd). The settings are fixed; the project's own input
/// arrives per file through `design_system_options_for_file`.
pub fn read_config(_cwd: &str) -> HookConfig {
    HookConfig::default()
}

/// True when the project's git metadata says to leave `file_path` alone:
/// git ignores it, or .gitattributes marks it generated or vendored.
pub fn is_project_skipped(rt: &Runtime, file_path: &str) -> bool {
    let abs = rt.resolve(&[file_path]);
    ProjectIgnores::new().is_skipped(&abs, false)
}

pub fn match_configured_extension<'a>(
    file_path: &str,
    extensions: &'a [ExtensionEntry],
) -> Option<&'a ExtensionEntry> {
    if extensions.is_empty() {
        return None;
    }
    let name = js::to_lower_case(&jsp::basename(file_path));
    if name.is_empty() {
        return None;
    }
    let mut best: Option<&ExtensionEntry> = None;
    for entry in extensions {
        if utf16_len(&name) > utf16_len(&entry.ext)
            && name.ends_with(entry.ext.as_str())
            && best
                .map(|b| utf16_len(&entry.ext) > utf16_len(&b.ext))
                .unwrap_or(true)
        {
            best = Some(entry);
        }
    }
    best
}

// ── cache ─────────────────────────────────────────────────────────────────

/// The user-cache `hook.cache.json` document for a normalized project root,
/// kept as ordered JSON so insertion order and foreign keys round-trip.
pub type Cache = Map<String, Value>;

/// JS: readCache(cwd)
pub fn read_cache(cwd: &str) -> Cache {
    let raw = safe_read_json(&get_cache_path(cwd));
    let mut cache = Map::new();
    cache.insert("version".into(), Value::from(1));
    let sessions = match raw {
        Some(Value::Object(o)) if o.get("version").and_then(Value::as_f64) == Some(1.0) => {
            match o.get("sessions") {
                Some(Value::Object(s)) => s.clone(),
                _ => Map::new(),
            }
        }
        _ => Map::new(),
    };
    cache.insert("sessions".into(), Value::Object(sessions));
    cache
}

fn sessions_mut(cache: &mut Cache) -> &mut Map<String, Value> {
    if !matches!(cache.get("sessions"), Some(Value::Object(_))) {
        cache.insert("sessions".into(), Value::Object(Map::new()));
    }
    cache.get_mut("sessions").unwrap().as_object_mut().unwrap()
}

pub fn sessions(cache: &Cache) -> Option<&Map<String, Value>> {
    obj_field(cache, "sessions")
}

/// JS: persistCache(cwd, cache)
pub fn persist_cache(rt: &Runtime, cwd: &str, cache: &Cache) -> bool {
    let mut cache = cache.clone();
    let ids: Vec<String> = sessions(&cache)
        .map(|s| s.keys().cloned().collect())
        .unwrap_or_default();
    if ids.len() > CACHE_MAX_SESSIONS {
        let sess = sessions(&cache).cloned().unwrap_or_default();
        let mut ordered: Vec<(String, f64)> = ids
            .iter()
            .map(|id| {
                let updated = sess
                    .get(id)
                    .and_then(|s| s.as_object())
                    .and_then(|s| s.get("updatedAt"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                (id.clone(), updated)
            })
            .collect();
        // JS: `.sort((a, b) => b[1] - a[1])` — stable, descending.
        ordered.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut next = Map::new();
        for (id, _) in ordered.into_iter().take(CACHE_MAX_SESSIONS) {
            if let Some(v) = sess.get(&id) {
                next.insert(id, v.clone());
            }
        }
        cache.insert("sessions".into(), Value::Object(next));
    }
    let _ = rt;
    let target = get_cache_path(cwd);
    if std::fs::create_dir_all(jsp::dirname(&target)).is_err() {
        return false;
    }
    let bytes = serde_json::to_string(&Value::Object(cache)).unwrap_or_default();
    impeccino_common::atomic_file::write(std::path::Path::new(&target), bytes.as_bytes()).is_ok()
}

/// Hold the per-cache project-root lock while a cache is read/modified/written.
/// Atomic replacement prevents partial JSON, but without this advisory lock a
/// stale session-cwd cache can still erase a root another hook just registered.
struct ProjectRootsLock {
    _file: std::fs::File,
}

impl ProjectRootsLock {
    fn acquire(cache_path: &str) -> std::io::Result<Self> {
        let mut lock_name = std::ffi::OsString::from(cache_path);
        lock_name.push(".project-roots.lock");
        let lock_path = std::path::PathBuf::from(lock_name);
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::symlink_metadata(&lock_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "project-roots lock path is not a regular file",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&lock_path)?;
        let metadata = std::fs::symlink_metadata(&lock_path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "project-roots lock path is not a regular file",
            ));
        }
        file.lock()?;
        Ok(Self { _file: file })
    }
}

/// Carry only the current project-root registry into a potentially stale
/// cache snapshot. Session file state keeps its existing write semantics.
fn preserve_current_project_roots(destination: &mut Cache, current: &Cache) {
    let current_sessions = sessions(current).cloned().unwrap_or_default();
    let destination_sessions = sessions_mut(destination);

    for (session_id, session) in destination_sessions.iter_mut() {
        let roots = current_sessions
            .get(session_id)
            .and_then(Value::as_object)
            .and_then(|value| value.get("projectRoots"))
            .and_then(Value::as_array);
        match roots {
            Some(roots) => {
                if !session.is_object() {
                    let mut repaired = Map::new();
                    repaired.insert("updatedAt".into(), now_value());
                    repaired.insert("files".into(), Value::Object(Map::new()));
                    *session = Value::Object(repaired);
                }
                if let Some(session) = session.as_object_mut() {
                    session.insert(
                        "projectRoots".into(),
                        Value::Array(bounded_project_roots(roots)),
                    );
                }
            }
            None => {
                if let Some(session) = session.as_object_mut() {
                    session.remove("projectRoots");
                }
            }
        }
    }

    // A different session may have registered a child root after this cache
    // snapshot was read. Preserve just its bounded index record; the child
    // cache contains that session's touched files.
    for (session_id, current_session) in &current_sessions {
        let Some(roots) = current_session
            .as_object()
            .and_then(|value| value.get("projectRoots"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        let destination_session = destination_sessions
            .entry(session_id.clone())
            .or_insert_with(|| {
                let mut value = Map::new();
                value.insert(
                    "updatedAt".into(),
                    current_session
                        .get("updatedAt")
                        .cloned()
                        .unwrap_or_else(now_value),
                );
                value.insert("files".into(), Value::Object(Map::new()));
                Value::Object(value)
            });
        if !destination_session.is_object() {
            let mut repaired = Map::new();
            repaired.insert(
                "updatedAt".into(),
                current_session
                    .get("updatedAt")
                    .cloned()
                    .unwrap_or_else(now_value),
            );
            repaired.insert("files".into(), Value::Object(Map::new()));
            *destination_session = Value::Object(repaired);
        }
        if let Some(session) = destination_session.as_object_mut() {
            session.insert(
                "projectRoots".into(),
                Value::Array(bounded_project_roots(roots)),
            );
        }
    }
}

fn bounded_project_roots(roots: &[Value]) -> Vec<Value> {
    let mut bounded = Vec::new();
    for root in roots.iter().filter_map(Value::as_str) {
        if bounded
            .iter()
            .any(|seen: &Value| seen.as_str() == Some(root))
        {
            continue;
        }
        bounded.push(Value::String(root.to_string()));
        if bounded.len() == CACHE_MAX_PROJECT_ROOTS {
            break;
        }
    }
    bounded
}

/// Rotate Stop's project order while retaining roots registered concurrently.
pub fn advance_stop_project_offset(
    rt: &Runtime,
    cwd: &str,
    session_id: &str,
    count: usize,
) -> usize {
    let target = get_cache_path(cwd);
    let Ok(_lock) = ProjectRootsLock::acquire(&target) else {
        return 0;
    };
    let mut cache = read_cache(cwd);
    let session = ensure_session(&mut cache, session_id);
    let offset = session
        .get("stopProjectOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize
        % count;
    session.insert(
        "stopProjectOffset".into(),
        Value::from((offset + 1) % count),
    );
    persist_cache(rt, cwd, &cache);
    offset
}

fn persist_cache_preserving_project_roots(rt: &Runtime, cwd: &str, cache: &Cache) -> bool {
    let target = get_cache_path(cwd);
    let _lock = match ProjectRootsLock::acquire(&target) {
        Ok(lock) => lock,
        Err(_) => return false,
    };
    let current = read_cache(cwd);
    let mut cache = cache.clone();
    preserve_current_project_roots(&mut cache, &current);
    persist_cache(rt, cwd, &cache)
}

fn persist_project_root_index(
    rt: &Runtime,
    session_cwd: &str,
    session_id: &str,
    project_root: &str,
) -> bool {
    let target = get_cache_path(session_cwd);
    let _lock = match ProjectRootsLock::acquire(&target) {
        Ok(lock) => lock,
        Err(_) => return false,
    };
    let mut index = read_cache(session_cwd);
    register_project_root(&mut index, session_id, project_root);
    persist_cache(rt, session_cwd, &index)
}

/// Remember which per-project cache roots a hook session touched. The index
/// lives at the normalized session cwd so Stop can find child projects
/// without searching the entire user cache. `projectRoots` is session-local
/// and bounded independently of the existing bounded session map.
pub fn register_project_root(cache: &mut Cache, session_id: &str, project_root: &str) {
    let session = ensure_session(cache, session_id);
    let roots = session
        .entry("projectRoots")
        .or_insert_with(|| Value::Array(vec![]));
    if !matches!(roots, Value::Array(_)) {
        *roots = Value::Array(vec![]);
    }
    let roots = roots.as_array_mut().unwrap();
    let root = project_root.to_string();
    let already_registered = roots.iter().any(|value| value.as_str() == Some(&root));
    if !already_registered {
        roots.push(Value::String(root));
        if roots.len() > CACHE_MAX_PROJECT_ROOTS {
            let excess = roots.len() - CACHE_MAX_PROJECT_ROOTS;
            roots.drain(..excess);
        }
    }
    session.insert("updatedAt".into(), now_value());
}

/// Registered per-project caches for one session; malformed index values are
/// ignored so a damaged cache keeps the hook fail-open.
pub fn registered_project_roots(cache: &Cache, session_id: &str) -> Vec<String> {
    sessions(cache)
        .and_then(|all| all.get(session_id))
        .and_then(Value::as_object)
        .and_then(|session| session.get("projectRoots"))
        .and_then(Value::as_array)
        .map(|roots| {
            let mut unique = Vec::new();
            for root in roots.iter().filter_map(Value::as_str) {
                if !unique.iter().any(|seen| seen == root) {
                    unique.push(root.to_string());
                    if unique.len() == CACHE_MAX_PROJECT_ROOTS {
                        break;
                    }
                }
            }
            unique
        })
        .unwrap_or_default()
}

/// Persist a project's session state and register its project root at the
/// session cwd. When both roots are the same, keep the index in the project
/// cache itself to avoid a second file and preserve the project-scoped cache
/// behavior for ordinary single-root sessions.
pub fn persist_project_cache(
    rt: &Runtime,
    project_root: &str,
    session_cwd: &str,
    cache: &mut Cache,
    session_id: &str,
) -> bool {
    let project_root = rt.resolve(&[project_root]);
    let session_cwd = rt.resolve(&[session_cwd]);
    if project_root == session_cwd {
        return persist_cache_preserving_project_roots(rt, &project_root, cache);
    }
    let saved_project = persist_cache_preserving_project_roots(rt, &project_root, cache);
    let saved_index = persist_project_root_index(rt, &session_cwd, session_id, &project_root);
    saved_project && saved_index
}

/// JS: ensureSession(cache, sessionId)
pub fn ensure_session<'c>(cache: &'c mut Cache, session_id: &str) -> &'c mut Map<String, Value> {
    let sessions = sessions_mut(cache);
    if !matches!(sessions.get(session_id), Some(Value::Object(_))) {
        let mut s = Map::new();
        s.insert("updatedAt".into(), now_value());
        s.insert("files".into(), Value::Object(Map::new()));
        sessions.insert(session_id.to_string(), Value::Object(s));
    }
    sessions
        .get_mut(session_id)
        .unwrap()
        .as_object_mut()
        .unwrap()
}

fn files_mut(session: &mut Map<String, Value>) -> &mut Map<String, Value> {
    if !matches!(session.get("files"), Some(Value::Object(_))) {
        session.insert("files".into(), Value::Object(Map::new()));
    }
    session.get_mut("files").unwrap().as_object_mut().unwrap()
}

/// JS: ensureFile(cache, sessionId, filePath)
pub fn ensure_file<'c>(
    cache: &'c mut Cache,
    session_id: &str,
    file_path: &str,
) -> &'c mut Map<String, Value> {
    let session = ensure_session(cache, session_id);
    let files = files_mut(session);
    if !matches!(files.get(file_path), Some(Value::Object(_))) {
        let mut f = Map::new();
        f.insert("editCount".into(), Value::from(0));
        f.insert("findings".into(), Value::Array(vec![]));
        files.insert(file_path.to_string(), Value::Object(f));
    }
    files.get_mut(file_path).unwrap().as_object_mut().unwrap()
}

/// The touched-file list of a session (JS `Object.keys(cache.sessions[sid].files)`).
pub fn touched_files(cache: &Cache, session_id: &str) -> Vec<String> {
    sessions(cache)
        .and_then(|s| s.get(session_id))
        .and_then(Value::as_object)
        .and_then(|s| s.get("files"))
        .and_then(Value::as_object)
        .map(|f| f.keys().cloned().collect())
        .unwrap_or_default()
}

/// JS: bumpEditCount(cache, sessionId, filePath)
pub fn bump_edit_count(cache: &mut Cache, session_id: &str, file_path: &str) -> f64 {
    let entry = ensure_file(cache, session_id, file_path);
    let count = entry
        .get("editCount")
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        + 1.0;
    entry.insert("editCount".into(), num_value(count));
    ensure_session(cache, session_id).insert("updatedAt".into(), now_value());
    count
}

fn num_value(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9e15 {
        Value::from(v as i64)
    } else {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// JS: touchFile(cache, sessionId, filePath)
pub fn touch_file(cache: &mut Cache, session_id: &str, file_path: &str) {
    ensure_file(cache, session_id, file_path);
    ensure_session(cache, session_id).insert("updatedAt".into(), now_value());
}

/// JS: suppressionNotice(filePath)
pub fn suppression_notice(rt: &Runtime, file_path: &str) -> String {
    format!(
        "{ENVELOPE_PREFIX} Suppressing further design hints on {file_path}. More than {EDIT_COUNT_THRESHOLD} edits in this session reached. Run {} audit to revisit.",
        rt.impeccino_command
    )
}

// ── findings ──────────────────────────────────────────────────────────────

/// JS: filterFindings(findings, content, ext, config): advisory findings
/// stay out unless the config includes them.
pub fn filter_findings(findings: Vec<Finding>, config: &HookConfig) -> Vec<Finding> {
    if findings.is_empty() {
        return vec![];
    }
    let include_advisory = config.advisory_rules == "include";
    findings
        .into_iter()
        .filter(|f| include_advisory || !is_advisory_finding(f))
        .collect()
}

/// `filter_findings`, then the decisions the file's DESIGN.md records
/// (project-wide waivers and declared fonts).
pub fn filter_findings_for(
    findings: Vec<Finding>,
    config: &HookConfig,
    scan: &HookScanOptions,
) -> Vec<Finding> {
    scan.decisions.apply(filter_findings(findings, config))
}

/// JS: splitFindingsByTier(findings) -> (immediate, deferred)
pub fn split_findings_by_tier(findings: Vec<Finding>) -> (Vec<Finding>, Vec<Finding>) {
    let mut immediate = Vec::new();
    let mut deferred = Vec::new();
    for f in findings {
        if IMMEDIATE_TIER_RULES.contains(&normalize_ignore_rule(&f.antipattern).as_str()) {
            immediate.push(f);
        } else {
            deferred.push(f);
        }
    }
    (immediate, deferred)
}

/// JS: perEditTieringActive(config, harness) — Claude Code, Codex, and
/// Grok Build dispatch our Stop hook; Cursor and GitHub Copilot have no
/// deep pass wired, so deferring for them would silently drop the
/// non-immediate rules entirely.
pub fn per_edit_tiering_active(config: &HookConfig, harness: &str) -> bool {
    if harness == "cursor" || harness == "github" {
        return false;
    }
    config.per_edit_rules != "all"
}

/// JS: findingCacheKey(finding)
pub fn finding_cache_key(f: &Finding) -> String {
    let line = if f.line.is_nan() { 0.0 } else { f.line };
    let value = extract_finding_ignore_value(f);
    if line > 0.0 && !value.is_empty() {
        return format!("{}:{}:{}", f.antipattern, js::number_to_string(line), value);
    }
    if line > 0.0 {
        return format!("{}:{}", f.antipattern, js::number_to_string(line));
    }
    if !value.is_empty() {
        return format!("{}:0:{}", f.antipattern, value);
    }
    let snippet = slice_prefix(js::trim(&f.snippet), 80);
    if snippet.is_empty() {
        format!("{}:0", f.antipattern)
    } else {
        format!("{}:0:{}", f.antipattern, snippet)
    }
}

fn known_findings(entry: &Map<String, Value>) -> Vec<String> {
    match entry.get("findings") {
        Some(Value::Array(list)) => list.iter().map(js_string).collect(),
        _ => vec![],
    }
}

/// JS: dedupeAgainstCache(findings, cache, sessionId, filePath)
pub fn dedupe_against_cache(
    findings: &[Finding],
    cache: &mut Cache,
    session_id: &str,
    file_path: &str,
) -> Vec<Finding> {
    if findings.is_empty() {
        return vec![];
    }
    let entry = ensure_file(cache, session_id, file_path);
    let mut known = known_findings(entry);
    let mut fresh = Vec::new();
    for f in findings {
        let key = finding_cache_key(f);
        if known.contains(&key) {
            continue;
        }
        known.push(key);
        fresh.push(f.clone());
    }
    fresh
}

/// JS: rememberFindings(cache, sessionId, filePath, findings) — replaces.
pub fn remember_findings(
    cache: &mut Cache,
    session_id: &str,
    file_path: &str,
    findings: &[Finding],
) {
    let entry = ensure_file(cache, session_id, file_path);
    let mut keys: Vec<String> = Vec::new();
    for f in findings {
        let k = finding_cache_key(f);
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    entry.insert(
        "findings".into(),
        Value::Array(keys.into_iter().map(Value::String).collect()),
    );
    ensure_session(cache, session_id).insert("updatedAt".into(), now_value());
}

// ── rendering ─────────────────────────────────────────────────────────────

/// JS: quoteCommandArg(value)
pub fn quote_command_arg(value: &str, win32: bool) -> String {
    let text = js::trim(value);
    if !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
    {
        return text.to_string();
    }
    if win32 {
        return format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
    }
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// JS: relativize(filePath, cwd)
pub fn relativize(rt: &Runtime, file_path: &str, cwd: &str) -> String {
    let rel = rt.relative(cwd, file_path);
    if rel.is_empty() || rel.starts_with("..") {
        return file_path.to_string();
    }
    jsp::to_posix(&rel)
}

re!(TRAILING_DOTS_RE, format!(r"\.+{WS}*$"));
re!(WS_RUN_RE, format!("{WS}+"));

/// JS: `.replace(/\s+/g, ' ').trim()`
fn collapse_ws(s: &str) -> String {
    js::trim(&WS_RUN_RE.replace_all(s, " ")).to_string()
}

/// JS: extractFindingIgnoreValueRaw(finding, rule) — the display (un-lowercased)
/// value the ignore hint quotes. Mirrors the detect crate's private helper.
fn extract_finding_ignore_value_raw(f: &Finding, rule: &str) -> String {
    let extra = |k: &str| match f.extras.get(k) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(v)
            if !matches!(v, Value::Null | Value::Bool(false)) && !matches!(v, Value::String(_)) =>
        {
            Some(js_string(v))
        }
        _ => None,
    };
    let direct = clean_ignore_value_display(
        &extra("ignoreValue")
            .or_else(|| extra("value"))
            .unwrap_or_default(),
    );
    if !direct.is_empty() {
        return direct;
    }
    let mut candidates: Vec<String> = Vec::new();
    if let Some(Value::String(d)) = f.extras.get("detail") {
        if !d.is_empty() {
            candidates.push(d.clone());
        }
    }
    if !f.snippet.is_empty() {
        candidates.push(f.snippet.clone());
    }
    for text in &candidates {
        if rule == "bounce-easing" {
            let motion = extract_motion_ignore_value(text);
            if !motion.is_empty() {
                return motion;
            }
            continue;
        }
        if let Some(m) = PRIMARY_FONT_RE.captures(text) {
            return clean_ignore_value_display(&m[1]);
        }
        if let Some(m) = GOOGLE_LABEL_RE.captures(text) {
            return clean_ignore_value_display(&m[1]);
        }
        if let Some(m) = FAMILY_RE.captures(text) {
            return clean_ignore_value_display(&m[1]);
        }
        if let Some(m) = GOOGLE_PARAM_RE.captures(text) {
            return clean_ignore_value_display(&impeccino_detect::config::decode_uri_component(
                &m[1],
            ));
        }
    }
    String::new()
}

re!(
    PRIMARY_FONT_RE,
    format!("(?i:Primary font):{WS}*([^()\n;]+)")
);
re!(
    GOOGLE_LABEL_RE,
    format!("(?i:Google Fonts):{WS}*([^()\n;]+)")
);
re!(
    FAMILY_RE,
    format!(r#"(?i:font-family){WS}*:{WS}*["']?([^'",;\n]+)"#)
);
re!(GOOGLE_PARAM_RE, "[?&](?i:family)=([^&:;\n]+)");
re!(ANIMATE_BOUNCE_RE, r"(?-u:\b)(?i:animate-bounce)(?-u:\b)");
re!(BEZIER_RE, r"(?i:cubic-bezier)\([^)]+\)");
re!(
    ANIMATION_RE,
    format!("(?i:animation)(?:-(?i:name))?{WS}*:{WS}*([^;\n]+)")
);
re!(MOTION_TOKEN_RE, "(?i:bounce|elastic|wobble|jiggle|spring)");
re!(
    COMMA_WS_SPLIT_RE,
    format!("[,{}]+", impeccino_core::js::WS_CHARS)
);
re!(EDGE_QUOTE_RE, r#"^["']|["']$"#);

fn extract_motion_ignore_value(text: &str) -> String {
    if let Some(m) = ANIMATE_BOUNCE_RE.find(text) {
        return clean_ignore_value_display(m.as_str());
    }
    if let Some(m) = BEZIER_RE.find(text) {
        return clean_ignore_value_display(m.as_str());
    }
    if let Some(m) = ANIMATION_RE.captures(text) {
        if let Some(t) = COMMA_WS_SPLIT_RE
            .split(&m[1])
            .find(|part| MOTION_TOKEN_RE.is_match(part))
        {
            return clean_ignore_value_display(t);
        }
    }
    String::new()
}

/// JS: cleanIgnoreValueDisplay(value)
fn clean_ignore_value_display(value: &str) -> String {
    let t = js::trim(value);
    let t = EDGE_QUOTE_RE.replace_all(t, "");
    let t = t.replace('+', " ");
    WS_RUN_RE.replace_all(&t, " ").into_owned()
}

/// What to do when a finding with an extractable value was deliberate
/// (docs/adr/0020): a token value belongs in DESIGN.md, anything else is
/// waived on its line.
fn format_finding_ignore_hint(f: &Finding) -> String {
    let rule = normalize_ignore_rule(&f.antipattern);
    if rule.is_empty() {
        return String::new();
    }
    let normalized = extract_finding_ignore_value(f);
    if normalized.is_empty() {
        return String::new();
    }
    if rule == "overused-font" || rule.starts_with("design-system-") {
        let value = extract_finding_ignore_value_raw(f, &rule);
        return format!("If deliberate, declare `{value}` in DESIGN.md.");
    }
    format!("If deliberate, waive the line: `impeccino-disable-line {rule}: <reason>`.")
}

/// JS: formatFindingLine(f, { compact })
fn format_finding_line(rt: &Runtime, f: &Finding, compact: bool) -> String {
    let prefix = if f.line > 0.0 {
        format!("- L{}", js::number_to_string(f.line))
    } else {
        "-".to_string()
    };
    let desc = if compact {
        ""
    } else {
        js::trim(&f.description)
    };
    let name = js::trim(&f.name);
    let name_segment = if name.is_empty() {
        String::new()
    } else {
        format!("{}.", TRAILING_DOTS_RE.replacen(name, 1, ""))
    };
    let _ = rt;
    let hint = format_finding_ignore_hint(f);
    let ignore_segment = if hint.is_empty() {
        String::new()
    } else {
        format!(" {hint}")
    };
    collapse_ws(&format!(
        "{prefix} [{}] {name_segment} {desc}{ignore_segment}",
        f.antipattern
    ))
}

/// JS: formatDedupedFindingLine(finding, seenRules)
fn format_deduped_finding_line(rt: &Runtime, f: &Finding, seen_rules: &mut Vec<String>) -> String {
    let rule = normalize_ignore_rule(&f.antipattern);
    let compact = !rule.is_empty() && seen_rules.contains(&rule);
    if !rule.is_empty() && !compact {
        seen_rules.push(rule);
    }
    format_finding_line(rt, f, compact)
}

/// JS: directiveFooter({ mode })
pub fn directive_footer(rt: &Runtime, short: bool) -> String {
    if short {
        return "Triage per the session policy: fix real problems; waive a confident false positive or sanctioned exception where it lives with an `impeccino-disable-line <rule>: <reason>` comment and disclose it in your reply; unsure, ask in one line.".to_string();
    }
    let _ = rt;
    [
        "Triage each finding, then state in your reply what you fixed, what you waived, and what you left standing:".to_string(),
        "- Real design problem: fix it. Keep intentional design as designed.".to_string(),
        "- Confident false positive or sanctioned exception in one place (an intentional demo or fixture, documentation of bad design, literal or domain-appropriate motion, a choice the user confirmed): waive it where it lives with an `impeccino-disable-line <rule>: <who decided, and the evidence>` comment (`-next-line` for the line below, `impeccino-disable` for the whole file) and disclose it. Write \"user confirmed\" in a reason only when the user did.".to_string(),
        "- Unsure: leave it as is and ask the user in one line.".to_string(),
        "Self-serve ends at the in-file waiver. A project-wide decision (a rule waived in DESIGN.md with `<!-- impeccino-disable <rule>: reason -->`, a font or color declared as a DESIGN.md token, a file kept out through .gitignore or .gitattributes) needs the user's explicit approval, and never waive a finding to push a blocked write through.".to_string(),
    ]
    .join("\n")
}

/// Render options (JS `opts`).
#[derive(Clone, Default)]
pub struct RenderOpts {
    pub cwd: Option<String>,
    /// `opts.footer === 'short'`
    pub short_footer: bool,
    pub reserve_chars: f64,
}

pub(crate) fn cap_of(config: &HookConfig) -> usize {
    let mf = config.limits.max_findings;
    let mf = if mf == 0.0 || mf.is_nan() {
        DEFAULT_MAX_FINDINGS
    } else {
        mf
    };
    // JS `slice(0, cap)` truncates a fractional cap.
    js::math_max(1.0, mf).floor() as usize
}

fn max_chars_of(config: &HookConfig, opts: &RenderOpts) -> f64 {
    let mc = config.limits.max_chars;
    let mc = if mc == 0.0 || mc.is_nan() {
        DEFAULT_MAX_CHARS
    } else {
        mc
    };
    js::math_max(500.0, mc) - opts.reserve_chars
}

fn is_finding_line(line: &str) -> bool {
    line.starts_with("- ")
}

fn footer_fallbacks(rt: &Runtime, footer: &str) -> Vec<String> {
    let short = directive_footer(rt, true);
    if footer == short {
        vec![footer.to_string()]
    } else {
        vec![footer.to_string(), short]
    }
}

/// JS: renderTemplate(findings, filePath, config, opts)
pub fn render_template(
    rt: &Runtime,
    findings: &[Finding],
    file_path: &str,
    config: &HookConfig,
    opts: &RenderOpts,
) -> String {
    if findings.is_empty() {
        return String::new();
    }
    let cap = cap_of(config);
    let max_chars = max_chars_of(config, opts);
    let cwd = opts.cwd.clone().unwrap_or_else(|| rt.proc_cwd.clone());
    let display = relativize(rt, file_path, &cwd);
    let total = findings.len();
    let shown = &findings[..findings.len().min(cap)];
    let remaining = total - shown.len();

    let header = format!(
        "{ENVELOPE_PREFIX} Design hook findings requiring review in {display} ({total} issue(s)):"
    );
    let mut seen_rules: Vec<String> = Vec::new();
    let lines: Vec<String> = shown
        .iter()
        .map(|f| format_deduped_finding_line(rt, f, &mut seen_rules))
        .collect();
    let more = if remaining > 0 {
        Some(format!(
            "... and {remaining} more (see {} audit).",
            rt.impeccino_command
        ))
    } else {
        None
    };
    let footer = directive_footer(rt, opts.short_footer);

    let mut blocks: Vec<String> = vec![header.clone()];
    blocks.extend(lines.iter().cloned());
    if let Some(m) = &more {
        blocks.push(m.clone());
    }
    blocks.push(String::new());
    blocks.push(footer.clone());
    let text = blocks.join("\n");
    if utf16_len(&text) as f64 > max_chars {
        return clamp_to_budget(rt, &header, &lines, more.as_deref(), &footer, max_chars);
    }
    text
}

fn assemble_single(header: &str, lines: &[String], more: Option<&str>, footer: &str) -> String {
    let mut blocks: Vec<&str> = vec![header];
    blocks.extend(lines.iter().map(String::as_str));
    if let Some(m) = more {
        blocks.push(m);
    }
    blocks.push("");
    blocks.push(footer);
    blocks.join("\n")
}

/// JS: clampToBudget(header, lines, more, footer, maxChars)
fn clamp_to_budget(
    rt: &Runtime,
    header: &str,
    lines: &[String],
    more: Option<&str>,
    footer: &str,
    max_chars: f64,
) -> String {
    let more_generic = format!("... and more (see {} audit).", rt.impeccino_command);
    let mut last_more: Option<String> = more.map(str::to_string);
    for footer_text in footer_fallbacks(rt, footer) {
        let mut working: Vec<String> = lines.to_vec();
        let mut more_text: Option<String> = more.map(str::to_string);
        let mut assembled = assemble_single(header, &working, more_text.as_deref(), &footer_text);
        while utf16_len(&assembled) as f64 > max_chars && working.len() > 1 {
            working.pop();
            more_text = Some(more_generic.clone());
            assembled = assemble_single(header, &working, more_text.as_deref(), &footer_text);
        }
        last_more = more_text;
        if utf16_len(&assembled) as f64 <= max_chars {
            return assembled;
        }
    }
    let line = lines
        .iter()
        .find(|l| is_finding_line(l))
        .or(lines.first())
        .cloned()
        .unwrap_or_default();
    clamp_last_line(
        rt,
        &|l: &[String], f: &str| assemble_single(header, l, last_more.as_deref(), f),
        &line,
        max_chars,
    )
}

/// JS: clampLastLine(build, line, maxChars)
fn clamp_last_line(
    rt: &Runtime,
    build: &dyn Fn(&[String], &str) -> String,
    line: &str,
    max_chars: f64,
) -> String {
    let footer_text = directive_footer(rt, true);
    let bare = build(&[], &footer_text);
    let room = max_chars - utf16_len(&bare) as f64 - 1.0;
    if room >= 24.0 {
        let clipped = if utf16_len(line) as f64 > room {
            format!("{}…", slice_prefix(line, (room - 1.0) as usize))
        } else {
            line.to_string()
        };
        return build(&[clipped], &footer_text);
    }
    if utf16_len(&bare) as f64 <= max_chars {
        return bare;
    }
    let head_len = js::math_max(0.0, max_chars - utf16_len(&footer_text) as f64 - 4.0);
    let head = slice_prefix(&bare, head_len as usize);
    format!("{head}…\n\n{footer_text}")
}

/// One file's fresh findings (JS `{ filePath, findings }`).
pub struct Group {
    pub file_path: String,
    pub findings: Vec<Finding>,
}

/// JS: renderGroupedTemplate(groups, config, opts)
pub fn render_grouped_template(
    rt: &Runtime,
    groups: &[Group],
    config: &HookConfig,
    opts: &RenderOpts,
) -> String {
    let real: Vec<&Group> = groups.iter().filter(|g| !g.findings.is_empty()).collect();
    if real.is_empty() {
        return String::new();
    }
    if real.len() == 1 {
        return render_template(rt, &real[0].findings, &real[0].file_path, config, opts);
    }
    let cap = cap_of(config);
    let max_chars = max_chars_of(config, opts);
    let cwd = opts.cwd.clone().unwrap_or_else(|| rt.proc_cwd.clone());
    let total: usize = real.iter().map(|g| g.findings.len()).sum();
    let header = format!(
        "{ENVELOPE_PREFIX} Design hook findings requiring review across {} files ({total} issue(s)):",
        real.len()
    );
    let mut lines: Vec<String> = Vec::new();
    let mut shown_count = 0usize;
    let mut seen_rules: Vec<String> = Vec::new();
    for group in &real {
        let display = relativize(rt, &group.file_path, &cwd);
        lines.push(format!("{display} ({} issue(s)):", group.findings.len()));
        let remaining_cap = cap.saturating_sub(shown_count);
        let shown = &group.findings[..group.findings.len().min(remaining_cap)];
        for f in shown {
            lines.push(format_deduped_finding_line(rt, f, &mut seen_rules));
        }
        shown_count += shown.len();
        let hidden = group.findings.len() - shown.len();
        if hidden > 0 {
            lines.push(format!(
                "- ... {hidden} more in {display} (see {} audit).",
                rt.impeccino_command
            ));
        }
    }
    let footer = directive_footer(rt, opts.short_footer);
    let mut blocks: Vec<String> = vec![header.clone()];
    blocks.extend(lines.iter().cloned());
    blocks.push(String::new());
    blocks.push(footer.clone());
    let text = blocks.join("\n");
    if utf16_len(&text) as f64 > max_chars {
        return clamp_grouped_to_budget(rt, &header, &lines, &footer, max_chars);
    }
    text
}

/// JS: clampGroupedToBudget(header, lines, footer, maxChars)
fn clamp_grouped_to_budget(
    rt: &Runtime,
    header: &str,
    lines: &[String],
    footer: &str,
    max_chars: f64,
) -> String {
    let more_generic = format!("... and more (see {} audit).", rt.impeccino_command);
    let assemble = |l: &[String], omitted: bool, f: &str| -> String {
        let mut blocks: Vec<&str> = vec![header];
        blocks.extend(l.iter().map(String::as_str));
        if omitted {
            blocks.push(&more_generic);
        }
        blocks.push("");
        blocks.push(f);
        blocks.join("\n")
    };
    for footer_text in footer_fallbacks(rt, footer) {
        let mut working: Vec<String> = lines.to_vec();
        let mut omitted = false;
        let mut assembled = assemble(&working, omitted, &footer_text);
        while utf16_len(&assembled) as f64 > max_chars && working.len() > 1 {
            working.pop();
            omitted = true;
            assembled = assemble(&working, omitted, &footer_text);
        }
        if utf16_len(&assembled) as f64 <= max_chars && working.iter().any(|l| is_finding_line(l)) {
            return assembled;
        }
    }
    let line = lines
        .iter()
        .find(|l| is_finding_line(l))
        .or(lines.first())
        .cloned()
        .unwrap_or_default();
    clamp_last_line(
        rt,
        &|l: &[String], f: &str| assemble(l, true, f),
        &line,
        max_chars,
    )
}

/// JS: renderCleanAck(filePath, opts)
pub fn render_clean_ack(rt: &Runtime, file_path: &str, cwd: &str) -> String {
    let display = relativize(rt, file_path, cwd);
    format!("{ENVELOPE_PREFIX} Design hook scanned {display}. No deterministic design-quality issues found. {STEER_LINE}")
}

/// JS: renderPendingAck(filePath, knownFindings, opts)
pub fn render_pending_ack(rt: &Runtime, file_path: &str, known: &[String], cwd: &str) -> String {
    let display = relativize(rt, file_path, cwd);
    let count = known.len();
    let sample = known.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    let more = if count > 3 {
        format!(", +{} more", count - 3)
    } else {
        String::new()
    };
    format!("{ENVELOPE_PREFIX} Design hook scanned {display}. Still has {count} finding(s) flagged earlier this session ({sample}{more}). Handle them before finalizing — the previous reminder still applies.")
}

/// JS: shouldEmitAckForFile(filePath, config)
pub fn should_emit_ack_for_file(file_path: &str, config: &HookConfig) -> bool {
    let ext = js::to_lower_case(&jsp::extname(file_path));
    if ACK_EXTS.contains(&ext.as_str()) {
        return true;
    }
    match_configured_extension(file_path, &config.extensions)
        .map(|c| c.engine == "html")
        .unwrap_or(false)
}

/// The detector option object the hook builds (`{ designSystem? }`), plus
/// the decisions of the DESIGN.md that governs the file.
#[derive(Default, Clone)]
pub struct HookScanOptions {
    pub design_system: Option<Rc<DesignSystem>>,
    pub decisions: Rc<DesignDecisions>,
}

impl HookScanOptions {
    pub fn md_newer_than_json(&self) -> bool {
        self.design_system
            .as_ref()
            .map(|d| d.md_newer_than_json)
            .unwrap_or(false)
    }
    pub fn to_scan_options(&self) -> ScanOptions {
        ScanOptions {
            inline_ignores: true,
            design_system: self.design_system.clone(),
            viewport: None,
            rule_pack: None,
        }
    }
}

/// JS: designSystemOptions(config, detector, projectCwd)
pub fn design_system_options(config: &HookConfig, project_cwd: &str) -> HookScanOptions {
    if !config.design_system_enabled {
        return HookScanOptions {
            decisions: Rc::new(DesignDecisions::load_for_dir(project_cwd)),
            ..HookScanOptions::default()
        };
    }
    HookScanOptions {
        design_system: load_design_system_for_cwd(project_cwd).map(Rc::new),
        decisions: Rc::new(DesignDecisions::load_for_dir(project_cwd)),
    }
}

/// Resolve design rules for the edited workspace without moving hook state.
pub fn design_system_options_for_file(
    rt: &Runtime,
    config: &HookConfig,
    project_cwd: &str,
    file_path: &str,
) -> HookScanOptions {
    let project = impeccino_context::context::resolve_project(
        project_cwd,
        &impeccino_context::target_args::TargetOptions {
            target_path: Some(file_path.to_string()),
        },
        &rt.env,
    );
    // A local DESIGN.md owns the scope even if it has no usable frontmatter.
    // Fall back only when the app has no document, never to a sibling app.
    let root = if resolve_design_md_path(&project.project_root).is_some() {
        &project.project_root
    } else {
        &project.repo_root
    };
    let decisions = Rc::new(DesignDecisions::load_for_dir(root));
    HookScanOptions {
        decisions,
        ..design_system_options(config, root)
    }
}

/// The detector the hook drives: the regex engine from `impeccino-detect`
/// and the static HTML engine through the `HtmlEngine` seam.
pub fn detector_detect_text(
    content: &str,
    file_path: &str,
    scan: &HookScanOptions,
) -> Vec<Finding> {
    let opts = TextOptions {
        design_system: scan.design_system.as_deref(),
        inline_ignores: true,
        rule_pack: None,
    };
    detect_text(content, file_path, &opts)
}

pub fn detector_detect_html(
    rt: &Runtime,
    file_path: &str,
    scan: &HookScanOptions,
) -> Result<Vec<Finding>, String> {
    let mut sink = std::io::sink();
    rt.html
        .detect_html(file_path, &scan.to_scan_options(), &mut sink)
        .map_err(|e| e.message)
}

pub fn design_stale_note(rt: &Runtime) -> String {
    format!(
        "{ENVELOPE_PREFIX} DESIGN.md is newer than DESIGN.json. Run {} document to refresh the design-system sidecar.",
        rt.impeccino_command
    )
}

/// JS: appendDesignSystemNote(text, scanOptions)
pub fn append_design_system_note(rt: &Runtime, text: &str, scan: &HookScanOptions) -> String {
    if text.is_empty() || !scan.md_newer_than_json() {
        return text.to_string();
    }
    format!("{text}\n\n{}", design_stale_note(rt))
}

/// JS: consumeSessionNoticeFlag(cache, sessionId, flag)
fn consume_session_notice_flag(cache: &mut Cache, session_id: &str, flag: &str) -> bool {
    let session = ensure_session(cache, session_id);
    if truthy_value(session.get(flag)) {
        return false;
    }
    session.insert(flag.to_string(), Value::Bool(true));
    session.insert("updatedAt".into(), now_value());
    true
}

/// JS: appendDesignSystemNoteOnce(text, scanOptions, cache, sessionId, config)
pub fn append_design_system_note_once(
    rt: &Runtime,
    text: &str,
    scan: &HookScanOptions,
    cache: &mut Cache,
    session_id: &str,
    config: &HookConfig,
) -> String {
    if text.is_empty() || !scan.md_newer_than_json() {
        return text.to_string();
    }
    let mc = config.limits.max_chars;
    let mc = if mc == 0.0 || mc.is_nan() {
        DEFAULT_MAX_CHARS
    } else {
        mc
    };
    let max_chars = js::math_max(500.0, mc);
    let note = design_stale_note(rt);
    if (utf16_len(text) + utf16_len(&note) + 2) as f64 > max_chars {
        return text.to_string();
    }
    if !consume_session_notice_flag(cache, session_id, "designNoteShown") {
        return text.to_string();
    }
    append_design_system_note(rt, text, scan)
}

/// JS: designNoteReserve(scanOptions, cache, sessionId)
pub fn design_note_reserve(
    rt: &Runtime,
    scan: &HookScanOptions,
    cache: &mut Cache,
    session_id: &str,
) -> f64 {
    if !scan.md_newer_than_json() {
        return 0.0;
    }
    if truthy_value(ensure_session(cache, session_id).get("designNoteShown")) {
        return 0.0;
    }
    (utf16_len(&design_stale_note(rt)) + 2) as f64
}

/// JS: footerModeForSession(cache, sessionId) — true when the short footer applies.
pub fn footer_mode_short(cache: &mut Cache, session_id: &str) -> bool {
    truthy_value(ensure_session(cache, session_id).get("footerShown"))
}

/// JS: commitFooterShown(cache, sessionId, text)
pub fn commit_footer_shown(rt: &Runtime, cache: &mut Cache, session_id: &str, text: &str) {
    if text.is_empty() || !text.contains(&directive_footer(rt, false)) {
        return;
    }
    let session = ensure_session(cache, session_id);
    if truthy_value(session.get("footerShown")) {
        return;
    }
    session.insert("footerShown".into(), Value::Bool(true));
    session.insert("updatedAt".into(), now_value());
}

/// JS: payload(text, eventName, harness)
pub fn payload(text: &str, event_name: &str, harness: &str) -> String {
    let mut out = Map::new();
    if harness == "cursor" {
        out.insert("additional_context".into(), Value::String(text.to_string()));
    } else if harness == "github" {
        out.insert("additionalContext".into(), Value::String(text.to_string()));
    } else if harness == "codex" && event_name == "Stop" {
        // Codex's Stop schema rejects unknown fields, so findings that should
        // continue the turn use a top-level blocking decision (pbakaus/impeccable#603).
        // https://developers.openai.com/codex/hooks#stop
        if js::trim(text).is_empty() {
            return String::new();
        }
        out.insert("decision".into(), Value::String("block".to_string()));
        out.insert("reason".into(), Value::String(text.to_string()));
    } else {
        let mut inner = Map::new();
        inner.insert(
            "hookEventName".into(),
            Value::String(event_name.to_string()),
        );
        inner.insert("additionalContext".into(), Value::String(text.to_string()));
        out.insert("hookSpecificOutput".into(), Value::Object(inner));
    }
    serde_json::to_string(&Value::Object(out)).unwrap_or_default()
}

// ── events / harness ──────────────────────────────────────────────────────

re!(
    APPLY_PATCH_FILE_RE,
    r"(?m)^\*\*\* (?:Update|Add) File: ([^\n\r\x{2028}\x{2029}]+)(?:\r\n|\n|\r|\x{2028}|\x{2029}|\z)"
);

/// JS: parseApplyPatchPaths(command, projectCwd)
pub fn parse_apply_patch_paths(rt: &Runtime, command: &str, project_cwd: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in APPLY_PATCH_FILE_RE.captures_iter(command) {
        let p = js::trim(&m[1]);
        if p.is_empty() {
            continue;
        }
        out.push(if jsp::is_absolute(p) {
            p.to_string()
        } else {
            rt.resolve(&[project_cwd, p])
        });
    }
    out
}

/// JS: resolveTargetFiles(event, projectCwd)
pub fn resolve_target_files(
    rt: &Runtime,
    event: &Map<String, Value>,
    project_cwd: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |p: &str| {
        if !p.is_empty() && !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
    };
    let ti = obj_field(event, "tool_input");
    if event.get("tool_name") == Some(&Value::String("apply_patch".to_string())) {
        if let Some(cmd) = ti.and_then(|t| str_field_any(t, "command")) {
            for p in parse_apply_patch_paths(rt, cmd, project_cwd) {
                add(&p);
            }
        }
    }
    if let Some(p) = ti.and_then(|t| str_field(t, "file_path")) {
        add(p);
    }
    if let Some(p) = ti.and_then(|t| str_field(t, "path")) {
        add(p);
    }
    if let Some(p) = str_field(event, "file_path") {
        add(p);
    }
    out
}

fn str_field_any<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    crate::util::str_field_any(map, key)
}

/// JS: resolveHarness(env, event)
pub fn resolve_harness(rt: &Runtime, event: Option<&Map<String, Value>>) -> &'static str {
    match rt.env("IMPECCINO_HOOK_HARNESS") {
        Some("cursor") => return "cursor",
        Some("github") => return "github",
        Some("grok") => return "grok",
        Some("claude") => return "claude",
        Some("codex") => return "codex",
        _ => {}
    }
    if let Some(ev) = event {
        // Grok Build sends camelCase `toolName`/`toolInput`/`hookEventName`
        // and no snake_case pair. GitHub Copilot sends camelCase
        // `toolName`/`toolArgs`. Check Grok first: the old GitHub heuristic
        // (`toolName` and no `tool_input`) also matches Grok, which is how
        // live PostToolUse was classified as Copilot and then skipped with
        // no-file-path (pbakaus/impeccable#646).
        if looks_like_grok_envelope(ev) {
            return "grok";
        }
        let has_tool_name = matches!(ev.get("toolName"), Some(Value::String(_)));
        let has_tool_args = ev.contains_key("toolArgs");
        if (has_tool_name || has_tool_args)
            && !ev.contains_key("tool_name")
            && !ev.contains_key("tool_input")
        {
            return "github";
        }
        if str_field(ev, "conversation_id").is_some() {
            return "cursor";
        }
        // Codex turn-scoped events carry `turn_id`. Claude Code does not.
        // Detecting it here means an already-installed Codex hook emits the
        // Codex Stop contract without rewriting the hook command to set
        // IMPECCINO_HOOK_HARNESS (pbakaus/impeccable#603).
        if str_field(ev, "turn_id").is_some() {
            return "codex";
        }
    }
    "claude"
}

fn looks_like_grok_envelope(ev: &Map<String, Value>) -> bool {
    if ev.contains_key("hook_event_name")
        || ev.contains_key("tool_name")
        || ev.contains_key("tool_input")
    {
        return false;
    }
    if ev.contains_key("toolArgs") {
        return false;
    }
    if matches!(ev.get("hookEventName"), Some(Value::String(_))) {
        return true;
    }
    matches!(ev.get("toolName"), Some(Value::String(_))) && ev.contains_key("toolInput")
}

/// `isStopEvent`: Stop arrives as Claude's
/// `hook_event_name: "Stop"` or Grok Build's `hookEventName: "stop"`.
/// hook.mjs routes on the raw stdin, before any normalize, so both casings
/// must match here.
pub fn is_stop_event(ev: &Map<String, Value>) -> bool {
    let name = match ev.get("hook_event_name").filter(|v| truthy_value(Some(v))) {
        Some(v) => Some(v),
        None => ev.get("hookEventName"),
    };
    matches!(name, Some(Value::String(s)) if matches!(js::to_lower_case(s).as_str(), "stop" | "afteragent"))
}

/// JS: parseGitHubToolArgs(toolArgs)
pub fn parse_github_tool_args(tool_args: Option<&Value>) -> Map<String, Value> {
    match tool_args {
        Some(Value::Object(o)) => o.clone(),
        Some(Value::String(s)) if !js::trim(s).is_empty() => match serde_json::from_str::<Value>(s)
        {
            Ok(Value::Object(o)) => o,
            _ => Map::new(),
        },
        _ => Map::new(),
    }
}

re!(
    APPLY_PATCH_MARKER_RE,
    r"\*\*\* (?:Begin Patch|Add File:|Update File:|Delete File:)"
);

/// JS: looksLikeApplyPatch(rawArgs)
fn looks_like_apply_patch(raw: Option<&Value>) -> bool {
    let Some(Value::String(s)) = raw else {
        return false;
    };
    if !APPLY_PATCH_MARKER_RE.is_match(s) {
        return false;
    }
    !matches!(
        serde_json::from_str::<Value>(s),
        Ok(Value::Object(_)) | Ok(Value::Array(_))
    )
}

/// JS: applyPatchText(rawArgs)
fn apply_patch_text(raw: Option<&Value>) -> String {
    let pick = |o: &Map<String, Value>| -> String {
        for k in ["patch", "input", "command"] {
            if truthy_value(o.get(k)) {
                return js_string(o.get(k).unwrap());
            }
        }
        String::new()
    };
    match raw {
        Some(Value::String(s)) => {
            if APPLY_PATCH_MARKER_RE.is_match(s) {
                return s.clone();
            }
            pick(&parse_github_tool_args(raw))
        }
        Some(Value::Object(o)) => pick(o),
        _ => String::new(),
    }
}

/// JS: normalizeGitHubEvent(event, projectCwd)
fn normalize_github_event(
    rt: &Runtime,
    event: &Map<String, Value>,
    project_cwd: &str,
) -> Map<String, Value> {
    let cwd = str_field(event, "cwd")
        .map(str::to_string)
        .or_else(|| rt.env_project_dir().map(str::to_string))
        .unwrap_or_else(|| project_cwd.to_string());
    let session_id = event
        .get("sessionId")
        .filter(|v| truthy_value(Some(v)))
        .or_else(|| event.get("session_id").filter(|v| truthy_value(Some(v))))
        .cloned()
        .unwrap_or_else(|| Value::String("unknown".to_string()));
    let tool_name = event
        .get("toolName")
        .filter(|v| truthy_value(Some(v)))
        .or_else(|| event.get("tool_name").filter(|v| truthy_value(Some(v))))
        .cloned()
        .unwrap_or(Value::Null);
    let mut tool_input = obj_field(event, "tool_input").cloned().unwrap_or_default();
    let raw_args = event.get("toolArgs");
    let mut normalized_tool_name = tool_name.clone();
    if tool_name == Value::String("apply_patch".to_string()) || looks_like_apply_patch(raw_args) {
        let patch = apply_patch_text(raw_args);
        if !patch.is_empty() {
            tool_input.insert("command".into(), Value::String(patch));
            normalized_tool_name = Value::String("apply_patch".to_string());
        }
    } else {
        let args = parse_github_tool_args(raw_args);
        let fp = ["path", "file_path", "filePath", "target_file"]
            .iter()
            .find_map(|k| args.get(*k).filter(|v| truthy_value(Some(v))));
        if let Some(Value::String(p)) = fp {
            tool_input.insert("file_path".into(), Value::String(p.clone()));
        }
    }
    let mut out = event.clone();
    out.insert("cwd".into(), Value::String(cwd));
    out.insert("session_id".into(), session_id);
    out.insert("tool_name".into(), normalized_tool_name);
    out.insert("tool_input".into(), Value::Object(tool_input));
    out
}

/// `normalizeGrokEvent`: Grok Build 1.0.5 (captured
/// 2026-08-24) sends camelCase `toolName` / `toolInput` / `sessionId` /
/// `stopHookActive`, plus `cwd` alongside a trailing-slashed
/// `workspaceRoot` (every consumer path.resolve()s, so no stripping here).
/// Only the fields the hook reads are copied; the event name stays
/// camelCase because routing already happened on the raw stdin
/// (is_stop_event) and nothing downstream reads `hook_event_name`.
fn normalize_grok_event(
    rt: &Runtime,
    event: &Map<String, Value>,
    project_cwd: &str,
) -> Map<String, Value> {
    // JS: event.cwd || event.workspaceRoot || envProjectDir(projectCwd) || projectCwd
    let cwd = event
        .get("cwd")
        .filter(|v| truthy_value(Some(v)))
        .cloned()
        .or_else(|| {
            event
                .get("workspaceRoot")
                .filter(|v| truthy_value(Some(v)))
                .cloned()
        })
        .or_else(|| {
            rt.env("CURSOR_PROJECT_DIR")
                .filter(|v| !v.is_empty())
                .map(|v| Value::String(v.to_string()))
        })
        .unwrap_or_else(|| Value::String(project_cwd.to_string()));
    let session_id = event
        .get("sessionId")
        .filter(|v| truthy_value(Some(v)))
        .or_else(|| event.get("session_id").filter(|v| truthy_value(Some(v))))
        .cloned()
        .unwrap_or_else(|| Value::String("unknown".to_string()));
    // JS: event.toolInput ?? event.tool_input (nullish coalescing)
    let raw_input = match event.get("toolInput") {
        Some(Value::Null) | None => event.get("tool_input"),
        v => v,
    };
    let tool_input = match raw_input {
        Some(Value::Object(o)) => o.clone(),
        _ => Map::new(),
    };
    let tool_name = event
        .get("toolName")
        .filter(|v| truthy_value(Some(v)))
        .or_else(|| event.get("tool_name").filter(|v| truthy_value(Some(v))))
        .cloned()
        .unwrap_or(Value::Null);
    let mut out = event.clone();
    out.insert("cwd".into(), cwd);
    out.insert("session_id".into(), session_id);
    out.insert("tool_name".into(), tool_name);
    out.insert("tool_input".into(), Value::Object(tool_input));
    if event.contains_key("stopHookActive") && !event.contains_key("stop_hook_active") {
        out.insert(
            "stop_hook_active".into(),
            event.get("stopHookActive").cloned().unwrap_or(Value::Null),
        );
    }
    out
}

/// JS: normalizeHookEvent(event, projectCwd, harness)
pub fn normalize_hook_event(
    rt: &Runtime,
    event: &Map<String, Value>,
    project_cwd: &str,
    harness: &str,
) -> Map<String, Value> {
    if harness == "github" {
        return normalize_github_event(rt, event, project_cwd);
    }
    if harness == "grok" {
        return normalize_grok_event(rt, event, project_cwd);
    }
    if harness != "cursor" {
        return event.clone();
    }
    let cwd = str_field(event, "cwd")
        .map(str::to_string)
        .or_else(|| match event.get("workspace_roots") {
            Some(Value::Array(roots)) => match roots.first() {
                Some(Value::String(r)) if !r.is_empty() => Some(r.clone()),
                _ => None,
            },
            _ => None,
        })
        .or_else(|| rt.env_project_dir().map(str::to_string))
        .unwrap_or_else(|| project_cwd.to_string());
    let session_id = event
        .get("session_id")
        .filter(|v| truthy_value(Some(v)))
        .or_else(|| {
            event
                .get("conversation_id")
                .filter(|v| truthy_value(Some(v)))
        })
        .cloned()
        .unwrap_or_else(|| Value::String("unknown".to_string()));
    let ti = obj_field(event, "tool_input").cloned().unwrap_or_default();
    let file_path = ti
        .get("file_path")
        .filter(|v| truthy_value(Some(v)))
        .or_else(|| ti.get("path").filter(|v| truthy_value(Some(v))))
        .or_else(|| event.get("file_path").filter(|v| truthy_value(Some(v))))
        .cloned();
    let mut out = event.clone();
    out.insert("cwd".into(), Value::String(cwd));
    out.insert("session_id".into(), session_id);
    if let Some(fp) = file_path {
        let mut ti = ti;
        ti.insert("file_path".into(), fp);
        out.insert("tool_input".into(), Value::Object(ti));
    }
    out
}

// ── targets ───────────────────────────────────────────────────────────────

const UI_CODE_EXTS: &[&str] = &[".jsx", ".tsx", ".vue", ".svelte", ".astro"];
const STYLE_EXTS: &[&str] = &[".css", ".scss", ".sass", ".less"];
const CO_SCAN_STYLE_NAMES: &[&str] = &[
    "styles.css",
    "styles.scss",
    "styles.sass",
    "styles.less",
    "index.css",
    "index.scss",
    "index.sass",
    "index.less",
    "global.css",
    "global.scss",
    "global.sass",
    "global.less",
    "globals.css",
    "globals.scss",
    "globals.sass",
    "globals.less",
];

re!(
    STATIC_STYLE_IMPORT_RE,
    format!(
        r#"(?i)import{WS}+(?:[A-Za-z0-9_*{{}}{},$]+{WS}+from{WS}+)?['"]([^'"]+\.(?:css|scss|sass|less))['"]"#,
        impeccino_core::js::WS_CHARS
    )
);

/// JS: hasPathTraversal(filePath)
pub fn has_path_traversal(p: &str) -> bool {
    p.split(['/', '\\']).any(|segment| segment == "..")
}

/// JS: isInsideProject(filePath, projectCwd)
pub fn is_inside_project(rt: &Runtime, file_path: &str, project_cwd: &str) -> bool {
    if file_path.is_empty() || project_cwd.is_empty() || has_path_traversal(file_path) {
        return false;
    }
    let rel = rt.relative(project_cwd, file_path);
    rel.is_empty() || (!rel.starts_with("..") && !jsp::is_absolute(&rel))
}

/// JS: canonicalPath(p) — realpath of the nearest existing ancestor plus the
/// remainder; memoized per process.
fn canonical_path(rt: &Runtime, p: &str) -> String {
    let resolved = rt.resolve(&[p]);
    if let Some(c) = rt.canonical_cache.borrow().get(&resolved) {
        return c.clone();
    }
    let mut canonical = resolved.clone();
    let mut dir = resolved.clone();
    let mut tail: Vec<String> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&dir) {
            let real = real.to_string_lossy().into_owned();
            canonical = if tail.is_empty() {
                real
            } else {
                let mut parts: Vec<&str> = vec![real.as_str()];
                parts.extend(tail.iter().map(String::as_str));
                jsp::join(&parts)
            };
            break;
        }
        let parent = jsp::dirname(&dir);
        if parent == dir {
            break;
        }
        tail.insert(0, jsp::basename(&dir));
        dir = parent;
    }
    let mut cache = rt.canonical_cache.borrow_mut();
    if cache.len() >= 1024 {
        cache.clear();
    }
    cache.insert(resolved, canonical.clone());
    canonical
}

/// JS: isScanTargetInsideProject(filePath, projectCwd)
pub fn is_scan_target_inside_project(rt: &Runtime, file_path: &str, project_cwd: &str) -> bool {
    if file_path.is_empty() || project_cwd.is_empty() {
        return false;
    }
    is_inside_project(
        rt,
        &canonical_path(rt, file_path),
        &canonical_path(rt, project_cwd),
    )
}

/// JS: parseStaticStyleImports(content, fromFile, projectCwd)
pub fn parse_static_style_imports(
    rt: &Runtime,
    content: &str,
    from_file: &str,
    project_cwd: &str,
) -> Vec<String> {
    if content.is_empty() {
        return vec![];
    }
    let dir = jsp::dirname(from_file);
    let mut out = Vec::new();
    for m in STATIC_STYLE_IMPORT_RE.captures_iter(content) {
        let p = js::trim(&m[1]);
        if p.is_empty() {
            continue;
        }
        let p = if p.starts_with('.') {
            rt.resolve(&[&dir, p])
        } else if !jsp::is_absolute(p) {
            rt.resolve(&[project_cwd, p])
        } else {
            p.to_string()
        };
        if !is_inside_project(rt, &p, project_cwd) {
            continue;
        }
        out.push(p);
    }
    out
}

/// JS: coLocatedStylesheets(filePath)
pub fn co_located_stylesheets(file_path: &str) -> Vec<String> {
    let dir = jsp::dirname(file_path);
    let base = jsp::basename_ext(file_path, &jsp::extname(file_path));
    let mut candidates: Vec<String> = Vec::new();
    for suffix in [
        ".css",
        ".module.css",
        ".scss",
        ".module.scss",
        ".sass",
        ".module.sass",
        ".less",
        ".module.less",
    ] {
        let p = jsp::join(&[&dir, &format!("{base}{suffix}")]);
        if !candidates.contains(&p) {
            candidates.push(p);
        }
    }
    for name in CO_SCAN_STYLE_NAMES {
        let p = jsp::join(&[&dir, name]);
        if !candidates.contains(&p) {
            candidates.push(p);
        }
    }
    candidates.into_iter().filter(|p| exists(p)).collect()
}

/// JS: normalizeScanTargets(primaryTargets, projectCwd)
pub fn normalize_scan_targets(
    rt: &Runtime,
    primaries: &[String],
    project_cwd: &str,
) -> Vec<String> {
    let base_cwd = if project_cwd.is_empty() {
        rt.proc_cwd.as_str()
    } else {
        project_cwd
    };
    let mut ordered: Vec<String> = Vec::new();
    for p in primaries {
        if ordered.len() >= MAX_SCAN_TARGETS {
            break;
        }
        let abs = if has_path_traversal(p) || jsp::is_absolute(p) {
            p.clone()
        } else {
            rt.resolve(&[base_cwd, p])
        };
        if !ordered.contains(&abs) {
            ordered.push(abs);
        }
    }
    ordered
}

/// JS: expandScanTargets(primaryTargets, projectCwd)
pub fn expand_scan_targets(rt: &Runtime, primaries: &[String], project_cwd: &str) -> Vec<String> {
    expand_scan_targets_with_limit(rt, primaries, project_cwd, MAX_SCAN_TARGETS)
}

/// Expand targets with the remaining allowance for one invocation. Recursive
/// per-project hook passes share one event-wide cap instead of each getting a
/// fresh `MAX_SCAN_TARGETS` budget.
pub fn expand_scan_targets_with_limit(
    rt: &Runtime,
    primaries: &[String],
    project_cwd: &str,
    max_targets: usize,
) -> Vec<String> {
    let max_targets = max_targets.min(MAX_SCAN_TARGETS);
    let mut ordered = normalize_scan_targets(rt, primaries, project_cwd);
    ordered.truncate(max_targets);
    if ordered.is_empty() {
        return vec![];
    }
    let base_cwd = if project_cwd.is_empty() {
        rt.proc_cwd.clone()
    } else {
        project_cwd.to_string()
    };
    let normalized_primaries = ordered.clone();
    let add = |ordered: &mut Vec<String>, p: &str| {
        if ordered.len() >= max_targets {
            return;
        }
        let abs = if has_path_traversal(p) || jsp::is_absolute(p) {
            p.to_string()
        } else {
            rt.resolve(&[&base_cwd, p])
        };
        if !ordered.contains(&abs) {
            ordered.push(abs);
        }
    };
    for p in &normalized_primaries {
        if ordered.len() >= max_targets {
            break;
        }
        if !is_inside_project(rt, p, &base_cwd) {
            continue;
        }
        let ext = js::to_lower_case(&jsp::extname(p));
        if STYLE_EXTS.contains(&ext.as_str()) || !UI_CODE_EXTS.contains(&ext.as_str()) {
            continue;
        }
        let content = safe_read(p).unwrap_or_default();
        for imp in parse_static_style_imports(rt, &content, p, project_cwd) {
            add(&mut ordered, &imp);
            if ordered.len() >= max_targets {
                break;
            }
        }
        for col in co_located_stylesheets(p) {
            add(&mut ordered, &col);
            if ordered.len() >= max_targets {
                break;
            }
        }
    }
    ordered
}

// ── audit log ─────────────────────────────────────────────────────────────

/// JS: writeAuditLog(env, entry, cwd)
pub fn write_audit_log(rt: &Runtime, entry: &Map<String, Value>, cwd: &str) -> bool {
    let base_cwd = str_field(entry, "cwd").unwrap_or(cwd).to_string();
    let Some(target) = rt
        .env("IMPECCINO_HOOK_LOG")
        .filter(|v| !v.is_empty())
        .map(str::to_string)
    else {
        return false;
    };
    let expanded = if let Some(rest) = target.strip_prefix("~/") {
        let home = rt
            .env("HOME")
            .filter(|v| !v.is_empty())
            .or_else(|| rt.env("USERPROFILE").filter(|v| !v.is_empty()))
            .unwrap_or(".");
        jsp::join(&[home, rest])
    } else if jsp::is_absolute(&target) {
        target.clone()
    } else {
        rt.resolve(&[&base_cwd, &target])
    };
    if std::fs::create_dir_all(jsp::dirname(&expanded)).is_err() {
        return false;
    }
    let mut record = Map::new();
    record.insert("ts".into(), Value::String(iso_now()));
    for (k, v) in entry {
        record.insert(k.clone(), v.clone());
    }
    let line = format!(
        "{}\n",
        serde_json::to_string(&Value::Object(record)).unwrap_or_default()
    );
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&expanded)
    {
        Ok(mut f) => f.write_all(line.as_bytes()).is_ok(),
        Err(_) => false,
    }
}

/// The value normalizer, re-exported for hook-admin.
pub fn normalize_ignore_value_str(v: &str) -> String {
    normalize_ignore_value(v)
}

#[cfg(test)]
mod project_roots_lock_tests {
    use super::{preserve_current_project_roots, Cache, ProjectRootsLock};
    use serde_json::{json, Value};
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn project_root_index_lock_serializes_concurrent_writers() {
        let dir = std::env::temp_dir().join(format!(
            "impeccino-root-index-lock-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cache_path = dir.join("hook.cache.json").to_string_lossy().into_owned();
        let first = ProjectRootsLock::acquire(&cache_path).unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let second_path = cache_path.clone();
        let second = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let acquired = ProjectRootsLock::acquire(&second_path).is_ok();
            done_tx.send(acquired).unwrap();
        });

        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(
            done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "a second writer entered while the index lock was held"
        );
        drop(first);
        assert!(done_rx.recv_timeout(Duration::from_secs(1)).unwrap());
        second.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stale_malformed_session_value_does_not_panic_or_drop_roots() {
        let mut destination: Cache = serde_json::from_value(json!({
            "version": 1,
            "sessions": {"s": "malformed-old-value"}
        }))
        .unwrap();
        let current: Cache = serde_json::from_value(json!({
            "version": 1,
            "sessions": {"s": {"projectRoots": ["/apps/web"]}}
        }))
        .unwrap();

        preserve_current_project_roots(&mut destination, &current);
        assert_eq!(
            destination["sessions"]["s"]["projectRoots"],
            Value::Array(vec![Value::String("/apps/web".into())])
        );
        assert!(destination["sessions"]["s"]["files"].is_object());
    }
}
