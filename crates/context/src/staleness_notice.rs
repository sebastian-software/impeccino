//! JS: lib/staleness-notice.mjs

use crate::jsp;
use crate::staleness::Finding;
use crate::util::{json_compact, json_pretty, user_cache_dir, Env};
use serde_json::{Map, Value};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

const RENOTIFY_INTERVAL_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
static CACHE_THREAD_LOCK: Mutex<()> = Mutex::new(());

fn cache_path(env: &Env) -> PathBuf {
    match env
        .get("IMPECCINO_STALENESS_CACHE")
        .filter(|v| !v.is_empty())
    {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(jsp::join(&[&user_cache_dir(env), "staleness-check.json"])),
    }
}

fn empty_cache() -> Map<String, Value> {
    let mut cache = Map::new();
    cache.insert("projects".into(), Value::Object(Map::new()));
    cache
}

fn invalid_cache(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn read_cache(path: &Path) -> io::Result<Map<String, Value>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(empty_cache()),
        Err(error) => return Err(error),
    };
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        invalid_cache(format!(
            "invalid JSON at line {}, column {}",
            error.line(),
            error.column()
        ))
    })?;
    let Some(raw) = value.as_object().cloned() else {
        return Err(invalid_cache("cache root must be a JSON object"));
    };
    let Some(Value::Object(projects)) = raw.get("projects") else {
        return Err(invalid_cache("cache must contain a projects object"));
    };
    for entries in projects.values() {
        let Some(entries) = entries.as_object() else {
            return Err(invalid_cache("project entries must be objects"));
        };
        if entries
            .values()
            .any(|timestamp| as_number(timestamp).is_none())
        {
            return Err(invalid_cache("finding timestamps must be numbers"));
        }
    }
    Ok(raw)
}

fn as_number(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn prune_cache(cache: &Map<String, Value>, now: f64) -> Map<String, Value> {
    let mut projects = Map::new();
    if let Some(Value::Object(ps)) = cache.get("projects") {
        for (key, entries) in ps {
            let Some(obj) = entries.as_object() else {
                continue;
            };
            let stamps: Vec<f64> = obj.values().filter_map(as_number).collect();
            if !stamps.is_empty() {
                let max = stamps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if now - max < RENOTIFY_INTERVAL_MS {
                    projects.insert(key.clone(), entries.clone());
                }
            }
        }
    }
    let mut out = Map::new();
    out.insert("projects".into(), Value::Object(projects));
    out
}

fn lock_cache(path: &Path) -> io::Result<(File, MutexGuard<'static, ()>)> {
    // File locks serialize separate processes; this mutex also serializes
    // callers in the same process on platforms where advisory locks are
    // process-scoped.
    let thread_lock = CACHE_THREAD_LOCK
        .lock()
        .map_err(|_| io::Error::other("cache lock mutex was poisoned"))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut lock_path = path.as_os_str().to_os_string();
    lock_path.push(".lock");
    let lock_path = PathBuf::from(lock_path);
    match fs::symlink_metadata(&lock_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cache lock path is a symlink",
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cache lock path is not a file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(lock_path)?;
    lock.lock()?;
    Ok((lock, thread_lock))
}

fn write_cache(path: &Path, cache: &Map<String, Value>) -> io::Result<()> {
    impeccino_common::atomic_file::write(
        path,
        json_compact(&Value::Object(cache.clone())).as_bytes(),
    )
}

fn cache_failure(path: &Path, error: &io::Error) -> String {
    format!("{}: {}", path.display(), error)
}

#[derive(Debug)]
pub struct FreshFindings {
    pub findings: Vec<Finding>,
    pub cache_error: Option<String>,
}

/// The boot staleness check is off for this session. There is no config
/// file (docs/adr/0020), so the env var is the only switch.
pub fn staleness_check_disabled(env: &Env) -> bool {
    env.get("IMPECCINO_NO_STALENESS_CHECK")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// JS: filterFreshFindings(findings, { projectRoot, now })
pub fn filter_fresh_findings(
    env: &Env,
    findings: Vec<Finding>,
    project_root: &str,
    now: f64,
) -> FreshFindings {
    if findings.is_empty() {
        return FreshFindings {
            findings: vec![],
            cache_error: None,
        };
    }
    let auto: Vec<Finding> = findings
        .iter()
        .filter(|f| f.severity == "auto")
        .cloned()
        .collect();
    let notifiable: Vec<Finding> = findings
        .iter()
        .filter(|f| f.severity != "auto")
        .cloned()
        .collect();
    if notifiable.is_empty() {
        return FreshFindings {
            findings: auto,
            cache_error: None,
        };
    }
    let path = cache_path(env);
    let _lock = match lock_cache(&path) {
        Ok(lock) => lock,
        Err(error) => {
            return FreshFindings {
                findings: auto,
                cache_error: Some(cache_failure(&path, &error)),
            };
        }
    };
    let key = jsp::resolve(project_root, &[]);
    let cache = match read_cache(&path) {
        Ok(cache) => cache,
        Err(error) => {
            return FreshFindings {
                findings: auto,
                cache_error: Some(cache_failure(&path, &error)),
            };
        }
    };
    let seen: Map<String, Value> = cache
        .get("projects")
        .and_then(|p| p.as_object())
        .and_then(|p| p.get(&key))
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let fresh: Vec<Finding> = notifiable
        .iter()
        .filter(|f| match seen.get(&f.id).and_then(as_number) {
            Some(last) => {
                (now - last).partial_cmp(&RENOTIFY_INTERVAL_MS) != Some(std::cmp::Ordering::Less)
            }
            None => true,
        })
        .cloned()
        .collect();
    let live: Vec<&str> = notifiable.iter().map(|f| f.id.as_str()).collect();
    let mut next = Map::new();
    for (id, v) in &seen {
        if live.contains(&id.as_str()) {
            next.insert(id.clone(), v.clone());
        }
    }
    for f in &fresh {
        next.insert(f.id.clone(), Value::from(now as i64));
    }
    let changed =
        json_compact(&Value::Object(next.clone())) != json_compact(&Value::Object(seen.clone()));
    if changed {
        let mut pruned = prune_cache(&cache, now);
        if let Some(Value::Object(ps)) = pruned.get_mut("projects") {
            ps.insert(key.clone(), Value::Object(next));
        }
        if let Err(error) = write_cache(&path, &pruned) {
            return FreshFindings {
                findings: auto,
                cache_error: Some(cache_failure(&path, &error)),
            };
        }
    }
    let mut out = auto;
    out.extend(fresh);
    FreshFindings {
        findings: out,
        cache_error: None,
    }
}

/// JS: buildStalenessDirective(findings)
pub fn build_staleness_directive(findings: &[Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    let payload: Vec<Value> = findings.iter().map(|f| f.to_value()).collect();
    let has_reportable = findings.iter().any(|f| f.severity != "auto");
    let mut lines = vec![
        format!("CONTEXT_STALE:\n{}", json_pretty(&Value::Array(payload))),
        "Impeccino's own project files have drifted from what this version reads. Do not stop, reorder, or expand the requested task for any of this.".to_string(),
        "By severity: `auto` is a migration the next write to that file performs anyway, so apply it then and do not raise it with the user. `mention` gets one short line in your reply with the offered fix. `route` names the command that owns the repair; offer it, and run it only if the user asks.".to_string(),
        "A finding that reports a deprecated field is binding: treat that field as absent for every decision in this session, whatever value it holds.".to_string(),
    ];
    if has_reportable {
        lines.push("Surface the reportable findings once, after the task response, in at most two sentences. They are already throttled, so say them plainly rather than hedging about whether they matter.".to_string());
    }
    Some(lines.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "impeccino staleness cache {} {} {}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn cache_env(path: &Path) -> Env {
        let mut env = Env::new();
        env.insert(
            "IMPECCINO_STALENESS_CACHE".into(),
            path.to_string_lossy().into_owned(),
        );
        env
    }

    fn finding(id: &str, severity: &'static str) -> Finding {
        Finding {
            id: id.into(),
            artifact: "PRODUCT.md".into(),
            path: Some("PRODUCT.md".into()),
            severity,
            summary: "stale project context".into(),
            fix: "refresh it".into(),
        }
    }

    #[test]
    fn malformed_cache_is_preserved_and_reportable_findings_are_withheld() {
        let root = TempDir::new();
        let cache = root.path().join("staleness.json");
        let malformed = "{\"projects\":";
        std::fs::write(&cache, malformed).unwrap();

        let fresh = filter_fresh_findings(
            &cache_env(&cache),
            vec![finding("stale", "mention")],
            root.path().to_string_lossy().as_ref(),
            10_000.0,
        );

        assert!(
            fresh.findings.is_empty(),
            "reportable findings need a reliable throttle cache"
        );
        assert!(
            fresh.cache_error.is_some(),
            "malformed cache should be reported to stderr"
        );
        assert_eq!(std::fs::read_to_string(cache).unwrap(), malformed);
    }

    #[test]
    fn unwritable_cache_does_not_emit_unthrottled_reportable_findings() {
        let root = TempDir::new();
        let blocker = root.path().join("not-a-directory");
        std::fs::write(&blocker, "file").unwrap();
        let cache = blocker.join("staleness.json");

        let fresh = filter_fresh_findings(
            &cache_env(&cache),
            vec![finding("stale", "route"), finding("migration", "auto")],
            root.path().to_string_lossy().as_ref(),
            10_000.0,
        );

        assert_eq!(
            fresh
                .findings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["migration"]
        );
        assert!(
            fresh.cache_error.is_some(),
            "failed cache writes should be reported to stderr"
        );
        assert!(blocker.is_file());
    }

    #[test]
    fn concurrent_updates_keep_both_project_findings() {
        let root = TempDir::new();
        let cache = root.path().join("staleness.json");
        let env = cache_env(&cache);
        let project_a = root.path().join("app-a");
        let project_b = root.path().join("app-b");
        std::fs::create_dir_all(&project_a).unwrap();
        std::fs::create_dir_all(&project_b).unwrap();
        let project_a = project_a.to_string_lossy().into_owned();
        let project_b = project_b.to_string_lossy().into_owned();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let workers =
            [("first", project_a.clone()), ("second", project_b.clone())].map(|(id, project)| {
                let env = env.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    filter_fresh_findings(&env, vec![finding(id, "mention")], &project, 10_000.0)
                })
            });
        barrier.wait();
        for worker in workers {
            let result = worker.join().unwrap();
            assert_eq!(result.findings.len(), 1);
            assert!(result.cache_error.is_none());
        }

        let saved: Value = serde_json::from_str(&std::fs::read_to_string(cache).unwrap()).unwrap();
        assert!(
            saved["projects"][&project_a]
                .as_object()
                .unwrap()
                .contains_key("first"),
            "saved cache: {}",
            saved
        );
        assert!(
            saved["projects"][&project_b]
                .as_object()
                .unwrap()
                .contains_key("second"),
            "saved cache: {}",
            saved
        );
    }

    #[test]
    fn same_finding_is_throttled_after_a_successful_write() {
        let root = TempDir::new();
        let cache = root.path().join("staleness.json");
        let env = cache_env(&cache);
        let project = root.path().to_string_lossy().into_owned();

        let first =
            filter_fresh_findings(&env, vec![finding("stale", "mention")], &project, 10_000.0);
        let second =
            filter_fresh_findings(&env, vec![finding("stale", "mention")], &project, 10_001.0);

        assert_eq!(first.findings.len(), 1);
        assert!(first.cache_error.is_none());
        assert!(second.findings.is_empty());
        assert!(second.cache_error.is_none());
    }

    #[test]
    fn finding_is_notified_again_at_the_weekly_boundary() {
        let root = TempDir::new();
        let cache = root.path().join("staleness.json");
        let env = cache_env(&cache);
        let project = root.path().to_string_lossy().into_owned();
        let first_notice_at = 10_000_000_000.0;

        let first = filter_fresh_findings(
            &env,
            vec![finding("stale", "mention")],
            &project,
            first_notice_at,
        );
        let before_boundary = filter_fresh_findings(
            &env,
            vec![finding("stale", "mention")],
            &project,
            first_notice_at + RENOTIFY_INTERVAL_MS - 1.0,
        );
        let at_boundary = filter_fresh_findings(
            &env,
            vec![finding("stale", "mention")],
            &project,
            first_notice_at + RENOTIFY_INTERVAL_MS,
        );

        assert_eq!(first.findings.len(), 1);
        assert!(before_boundary.findings.is_empty());
        assert_eq!(at_boundary.findings.len(), 1);
    }

    #[test]
    fn cache_pruning_removes_expired_projects_and_keeps_recent_ones() {
        let root = TempDir::new();
        let cache = root.path().join("staleness.json");
        let env = cache_env(&cache);
        let current_project = root.path().join("current").to_string_lossy().into_owned();
        let expired_project = root.path().join("expired").to_string_lossy().into_owned();
        let retained_project = root.path().join("retained").to_string_lossy().into_owned();
        let now = 10_000_000_000.0;
        let mut projects = Map::new();
        projects.insert(
            expired_project.clone(),
            serde_json::json!({ "old": now - RENOTIFY_INTERVAL_MS - 1.0 }),
        );
        projects.insert(
            retained_project.clone(),
            serde_json::json!({ "recent": now - RENOTIFY_INTERVAL_MS + 1.0 }),
        );
        std::fs::write(
            &cache,
            json_compact(&serde_json::json!({ "projects": projects })),
        )
        .unwrap();

        let result = filter_fresh_findings(
            &env,
            vec![finding("active", "mention")],
            &current_project,
            now,
        );

        assert_eq!(result.findings.len(), 1);
        assert!(result.cache_error.is_none());
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(cache).unwrap()).unwrap();
        assert!(saved["projects"].get(&expired_project).is_none());
        assert!(saved["projects"].get(&retained_project).is_some());
        assert!(saved["projects"][&current_project].get("active").is_some());
    }

    #[test]
    fn all_auto_findings_do_not_touch_a_blocked_cache_path() {
        let root = TempDir::new();
        let blocker = root.path().join("not-a-directory");
        std::fs::write(&blocker, "file").unwrap();
        let cache = blocker.join("staleness.json");

        let result = filter_fresh_findings(
            &cache_env(&cache),
            vec![finding("migration", "auto")],
            root.path().to_string_lossy().as_ref(),
            10_000.0,
        );

        assert_eq!(
            result
                .findings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["migration"]
        );
        assert!(result.cache_error.is_none());
        assert!(blocker.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_failure_preserves_symlink_target_and_withholds_reportable_findings() {
        let root = TempDir::new();
        let target = root.path().join("target.json");
        let cache = root.path().join("staleness.json");
        let original = r#"{"projects":{}}"#;
        std::fs::write(&target, original).unwrap();
        std::os::unix::fs::symlink(&target, &cache).unwrap();

        let fresh = filter_fresh_findings(
            &cache_env(&cache),
            vec![finding("stale", "route"), finding("migration", "auto")],
            root.path().to_string_lossy().as_ref(),
            10_000.0,
        );

        assert_eq!(
            fresh
                .findings
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["migration"]
        );
        assert!(fresh.cache_error.is_some());
        assert_eq!(std::fs::read_to_string(target).unwrap(), original);
    }
}
