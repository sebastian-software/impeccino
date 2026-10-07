//! Where hook state lives: the user cache by default (docs/adr/0020), or
//! IMPECCINO_CACHE_ROOT (pbakaus/impeccable#422), which mirrors the scenarios main's
//! tests/hook.test.mjs added in 77a2eae8 / 5c82d58b / 30b3628f / cbd78701.
//!
//! These tests mutate the process environment, so they live in their own
//! test binary (its own process) and serialize on ENV_LOCK.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use impeccino_detect::MissingHtmlEngine;
use impeccino_hook::hook;
use impeccino_hook::hook_lib::{get_cache_path, Runtime};
use serde_json::json;

static TMP_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

static HTML: MissingHtmlEngine = MissingHtmlEngine;
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}
impl EnvGuard {
    fn set(pairs: &[(&'static str, Option<&str>)]) -> EnvGuard {
        let mut saved = Vec::new();
        for (k, v) in pairs {
            saved.push((*k, std::env::var(k).ok()));
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        EnvGuard { saved }
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            match v {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Tmp {
        let base = std::env::temp_dir().join(format!(
            "impeccino-cache-root-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            // A per-process counter: Windows' clock is coarse enough that two
            // parallel tests can share a nanosecond stamp and then delete each
            // other's directories.
            TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&base).unwrap();
        // Like Node's `realpathSync`: no `\\?\` verbatim prefix on Windows, so the
        // paths the hook joins under this root resolve (the kernel takes a
        // verbatim path literally and rejects a forward slash).
        let real = std::fs::canonicalize(&base)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        Tmp(PathBuf::from(real.strip_prefix(r"\\?\").unwrap_or(&real)))
    }
    fn path(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
    fn write(&self, rel: &str, body: &str) -> String {
        let abs = self.0.join(rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, body).unwrap();
        abs.to_string_lossy().into_owned()
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rt(cwd: &str) -> Runtime<'static> {
    Runtime::new(
        cwd.to_string(),
        HashMap::new(),
        "/impeccino".to_string(),
        "/opt/bin/impeccino",
        &HTML,
    )
}

fn edit_event(cwd: &str, file: &str, session: &str) -> String {
    json!({
        "session_id": session, "cwd": cwd, "hook_event_name": "PostToolUse",
        "tool_name": "Edit", "tool_input": { "file_path": file },
    })
    .to_string()
}

const GRADIENT_CSS: &str = ".title { background: linear-gradient(90deg, #f472b6, #a78bfa); -webkit-background-clip: text; color: transparent; }\n";
const CLEAN_CSS: &str = ".card { color: #333; }\n";

#[test]
fn state_relocates_and_slug_normalizes() {
    let _l = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = Tmp::new();
    let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", Some(&root.path()))]);

    let cache = get_cache_path("/x/my.app");
    assert!(cache.starts_with(&root.path()), "{}", cache);
    assert!(cache.ends_with("hook.cache.json"));
    // Trailing separators and relative segments slug to the same dir.
    assert_eq!(get_cache_path("/x/my.app"), get_cache_path("/x/my.app/"));
    assert_eq!(
        get_cache_path("/x/my.app"),
        get_cache_path("/x/other/../my.app")
    );
    // The readable part stays human-scannable and the 8-hex digest keeps
    // colliding readable slugs apart (`/x/my.app` vs `/x/my-app`).
    let dir = std::path::Path::new(&cache)
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    // The readable part is the RESOLVED project path with `:`, `\`, `/` and `.`
    // mapped to `-`, so on Windows it carries the current drive
    // (`D:\x\my.app` -> `D--x-my-app`). Derive it rather than pinning the
    // POSIX spelling.
    let resolved = impeccino_common::jsp::resolve(
        &std::env::current_dir().unwrap().to_string_lossy(),
        &["/x/my.app"],
    );
    let readable: String = resolved
        .chars()
        .map(|c| {
            if matches!(c, ':' | '\\' | '/' | '.') {
                '-'
            } else {
                c
            }
        })
        .collect();
    assert!(dir.starts_with(&format!("{readable}-")), "{}", dir);
    let digest = dir.rsplit('-').next().unwrap();
    assert_eq!(digest.len(), 8);
    assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(get_cache_path("/x/my.app"), get_cache_path("/x/my-app"));
}

/// The default cache path for `/x/app`: a per-project dir under
/// `<user cache>/impeccino/projects`.
fn stock_cache_path() -> String {
    let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", None)]);
    get_cache_path("/x/app")
}

#[cfg(not(windows))]
#[test]
fn default_state_lives_in_the_xdg_user_cache() {
    let _l = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let xdg = Tmp::new();
    let home = Tmp::new();
    let with_xdg = {
        let _g = EnvGuard::set(&[
            ("IMPECCINO_CACHE_ROOT", None),
            ("XDG_CACHE_HOME", Some(&xdg.path())),
            ("HOME", Some(&home.path())),
        ]);
        get_cache_path("/x/app")
    };
    assert!(
        with_xdg.starts_with(&format!("{}/impeccino/projects/", xdg.path())),
        "{with_xdg}"
    );
    assert!(with_xdg.ends_with("/hook.cache.json"));
    let without = {
        let _g = EnvGuard::set(&[
            ("IMPECCINO_CACHE_ROOT", None),
            ("XDG_CACHE_HOME", None),
            ("HOME", Some(&home.path())),
        ]);
        get_cache_path("/x/app")
    };
    assert!(
        without.starts_with(&format!("{}/.cache/impeccino/projects/", home.path())),
        "{without}"
    );
    assert!(!without.contains("/x/app/"), "never project-local");
}

#[test]
fn root_value_normalization_and_opt_out() {
    let _l = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = Tmp::new();
    // Stray whitespace in env files trims away.
    let padded = format!("  {}  ", root.path());
    let trimmed = {
        let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", Some(&root.path()))]);
        get_cache_path("/x/app")
    };
    let with_ws = {
        let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", Some(&padded))]);
        get_cache_path("/x/app")
    };
    assert_eq!(trimmed, with_ws);
    // Unset or blank keeps the user-cache default.
    let stock = stock_cache_path();
    {
        let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", Some("   "))]);
        assert_eq!(get_cache_path("/x/app"), stock);
    }
}

#[cfg(unix)]
#[test]
fn tilde_expands_against_homedir_or_rejects() {
    let _l = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = Tmp::new();
    let explicit = {
        let joined = format!("{}/caches", home.path());
        let _g = EnvGuard::set(&[
            ("HOME", Some(&home.path())),
            ("IMPECCINO_CACHE_ROOT", Some(&joined)),
        ]);
        get_cache_path("/x/app")
    };
    let tilde = {
        let _g = EnvGuard::set(&[
            ("HOME", Some(&home.path())),
            ("IMPECCINO_CACHE_ROOT", Some("~/caches")),
        ]);
        get_cache_path("/x/app")
    };
    assert_eq!(explicit, tilde);
    // No determinable home dir: expansion is rejected and state falls back
    // to the default, which without a home is the system temp dir (never
    // the process cwd, never the project).
    let no_home = {
        let _g = EnvGuard::set(&[
            ("HOME", None),
            ("USERPROFILE", None),
            ("XDG_CACHE_HOME", None),
            ("IMPECCINO_CACHE_ROOT", Some("~/caches")),
        ]);
        get_cache_path("/x/app")
    };
    let tmp = std::env::temp_dir().to_string_lossy().into_owned();
    assert!(
        no_home.starts_with(&impeccino_common::jsp::join(&[
            &tmp,
            "impeccino",
            "projects"
        ])),
        "{no_home}"
    );
}

#[test]
fn run_hook_persists_and_dedupes_through_the_redirect() {
    let _l = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = Tmp::new();
    let project = Tmp::new();
    let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", Some(&root.path()))]);
    let cwd = project.path();
    let r = rt(&cwd);
    let css = project.write("src/a.css", GRADIENT_CSS);

    let one = hook::run_hook(&r, &edit_event(&cwd, &css, "s1"));
    assert!(one.stdout.contains("gradient-text"), "{}", one.stdout);
    // State lands under the redirect root; the project stays footprint-free.
    assert!(std::path::Path::new(&get_cache_path(&cwd)).exists());
    assert!(!project.0.join(".impeccino").exists());

    // The remembered finding dedupes the second identical edit into pending.
    let two = hook::run_hook(&r, &edit_event(&cwd, &css, "s1"));
    assert!(
        two.stdout.contains("flagged earlier this session"),
        "{}",
        two.stdout
    );

    // A clean edit still persists its editCount bump once a session cache
    // exists for the project.
    let clean = project.write("src/b.css", CLEAN_CSS);
    let before = std::fs::read_to_string(get_cache_path(&cwd)).unwrap();
    let three = hook::run_hook(&r, &edit_event(&cwd, &clean, "s1"));
    assert_eq!(
        three.audit.get("kind").and_then(|v| v.as_str()),
        Some("clean")
    );
    let after = std::fs::read_to_string(get_cache_path(&cwd)).unwrap();
    assert_ne!(
        before, after,
        "clean-edit editCount bump persisted through the redirect"
    );
}

#[test]
fn clean_edit_without_a_session_cache_writes_nothing() {
    let _l = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = Tmp::new();
    let project = Tmp::new();
    let _g = EnvGuard::set(&[("IMPECCINO_CACHE_ROOT", Some(&root.path()))]);
    let cwd = project.path();
    let r = rt(&cwd);
    // A clean UI edit before the project has any session state writes
    // nothing at all (issues pbakaus/impeccable#344, pbakaus/impeccable#305).
    let clean = project.write("src/b.css", CLEAN_CSS);
    let res = hook::run_hook(&r, &edit_event(&cwd, &clean, "s1"));
    assert_eq!(
        res.audit.get("kind").and_then(|v| v.as_str()),
        Some("clean")
    );
    assert!(!std::path::Path::new(&get_cache_path(&cwd)).exists());
    assert!(!project.0.join(".impeccino").exists());
}
