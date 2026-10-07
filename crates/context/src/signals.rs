//! Repository context signals for `impeccino signals`.

use crate::context::{extract_platform, load_context_without_visual_scan};
use crate::jsp;
use crate::target_args::TargetOptions;
use crate::util::{exists, js_trim, json_pretty, opt_string, Env};
use impeccino_common::Io;
use serde_json::{Map, Value};
use std::process::{Command, Stdio};

fn has_code(cwd: &str) -> bool {
    if exists(&jsp::join(&[cwd, "package.json"])) {
        return true;
    }
    ["src", "app", "pages", "site", "public", "components", "lib"]
        .iter()
        .any(|d| exists(&jsp::join(&[cwd, d])))
}

/// git with stdout only; None on non-zero exit / spawn failure.
pub fn git_run(args: &[&str], cwd: &str, trim: bool, timeout_ms: Option<u64>) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    impeccino_common::proc::hide_window(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let out = if let Some(t) = timeout_ms {
        // Poll for completion up to the timeout, then kill (execFileSync timeout semantics).
        let start = std::time::Instant::now();
        let mut stdout = child.stdout.take()?;
        let reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            use std::io::Read;
            let _ = stdout.read_to_end(&mut buf);
            buf
        });
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let buf = reader.join().unwrap_or_default();
                    if !status.success() {
                        return None;
                    }
                    break String::from_utf8_lossy(&buf).into_owned();
                }
                Ok(None) => {
                    if start.elapsed().as_millis() as u64 > t {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(_) => return None,
            }
        }
    } else {
        let o = child.wait_with_output().ok()?;
        if !o.status.success() {
            return None;
        }
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    Some(if trim { js_trim(&out).to_string() } else { out })
}

fn git_signals(cwd: &str) -> Value {
    let run = |args: &[&str]| git_run(args, cwd, true, None);
    let mut m = Map::new();
    if run(&["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        m.insert("isRepo".into(), Value::Bool(false));
        m.insert("branch".into(), Value::Null);
        m.insert("base".into(), Value::Null);
        m.insert("changedFiles".into(), Value::Array(vec![]));
        m.insert("changedCount".into(), Value::from(0));
        return Value::Object(m);
    }
    let branch = run(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let remotes: Vec<String> = run(&["remote"])
        .unwrap_or_default()
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let resolve_upstream = || -> Option<(String, String)> {
        let full = run(&["rev-parse", "--symbolic-full-name", "@{u}"])?;
        if full.is_empty() {
            return None;
        }
        if let Some(name) = full.strip_prefix("refs/heads/") {
            return Some((name.to_string(), name.to_string()));
        }
        if let Some(rest) = full.strip_prefix("refs/remotes/") {
            if let Some(i) = rest.find('/') {
                if i > 0 {
                    return Some((rest[i + 1..].to_string(), rest.to_string()));
                }
            }
        }
        None
    };
    let conventional = ["develop", "main", "master"];
    let mut remote_heads: Vec<(String, String)> = Vec::new();
    let mut seen_r: Vec<String> = Vec::new();
    for r in std::iter::once("origin".to_string()).chain(remotes.iter().cloned()) {
        if seen_r.contains(&r) {
            continue;
        }
        seen_r.push(r.clone());
        if let Some(reff) = run(&[
            "symbolic-ref",
            "--short",
            &format!("refs/remotes/{}/HEAD", r),
        ]) {
            if !reff.is_empty() {
                if let Some(name) = reff.strip_prefix(&format!("{}/", r)) {
                    remote_heads.push((name.to_string(), reff.clone()));
                }
            }
        }
    }
    let branch_s = branch.clone().unwrap_or_default();
    let on_integration = branch.as_deref() == Some("HEAD")
        || conventional.contains(&branch_s.as_str())
        || remote_heads
            .iter()
            .any(|(n, _)| Some(n.as_str()) == branch.as_deref());
    let mut base: Option<String> = None;
    let mut base_rev: Option<String> = None;
    if !on_integration {
        let upstream = resolve_upstream();
        let mut remote_order: Vec<String> = vec!["origin".to_string()];
        remote_order.extend(remotes.iter().filter(|n| *n != "origin").cloned());
        let revs_for = |name: &str| -> Vec<String> {
            let mut v = vec![name.to_string()];
            v.extend(remote_order.iter().map(|r| format!("{}/{}", r, name)));
            v
        };
        let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let add = |name: &str,
                   revs: Vec<String>,
                   candidates: &mut Vec<(String, Vec<String>)>,
                   seen: &mut Vec<String>| {
            if name.is_empty() || Some(name) == branch.as_deref() || seen.iter().any(|s| s == name)
            {
                return;
            }
            seen.push(name.to_string());
            candidates.push((name.to_string(), revs));
        };
        if let Some((n, r)) = &upstream {
            add(n, vec![r.clone()], &mut candidates, &mut seen);
        }
        let advertised = |name: &str| -> Vec<String> {
            remote_heads
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, r)| r.clone())
                .collect()
        };
        let uniq = |v: Vec<String>| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for x in v {
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            out
        };
        let mut dev = advertised("develop");
        dev.extend(revs_for("develop"));
        add("develop", uniq(dev), &mut candidates, &mut seen);
        for (n, r) in &remote_heads {
            let mut v = vec![r.clone()];
            v.extend(revs_for(n));
            add(n, uniq(v), &mut candidates, &mut seen);
        }
        for n in ["main", "master"] {
            add(n, revs_for(n), &mut candidates, &mut seen);
        }
        for (name, revs) in &candidates {
            if let Some(rev) = revs
                .iter()
                .find(|r| run(&["rev-parse", "--verify", "--quiet", r]).is_some())
            {
                base = Some(name.clone());
                base_rev = Some(rev.clone());
                break;
            }
        }
    }
    let diff_base = match (&base, &branch) {
        (Some(b), Some(br)) if !b.is_empty() && !br.is_empty() && br != b => Some(b.clone()),
        _ => None,
    };
    let from_diff = if diff_base.is_some() {
        git_run(
            &[
                "diff",
                "--relative",
                "--name-only",
                "-z",
                &format!("{}...HEAD", base_rev.as_deref().unwrap_or("")),
                "--",
                ".",
            ],
            cwd,
            false,
            None,
        )
    } else {
        None
    };
    let status_prefix = git_run(&["rev-parse", "--show-prefix"], cwd, false, None)
        .map(|prefix| strip_show_prefix_line_ending(&prefix).to_string())
        .unwrap_or_default();
    let from_status = git_run(
        &["status", "--porcelain=v1", "-z", "--", "."],
        cwd,
        false,
        None,
    );
    let mut changed: Vec<String> = Vec::new();
    if let Some(d) = from_diff.filter(|d| !d.is_empty()) {
        changed = d
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    } else if let Some(s) = from_status.filter(|s| !s.is_empty()) {
        let entries: Vec<&str> = s.split('\0').collect();
        let mut i = 0;
        while i < entries.len() {
            let record = entries[i];
            i += 1;
            if record.is_empty() {
                continue;
            }
            let bytes = record.as_bytes();
            if bytes.len() < 4 || bytes[2] != b' ' {
                continue;
            }
            let status = &bytes[..2];
            let path = String::from_utf8_lossy(&bytes[3..]);
            if let Some(relative) = path.strip_prefix(&status_prefix) {
                changed.push(relative.to_string());
            }
            // With porcelain -z, rename/copy records contain destination
            // first and the original path in the next NUL-delimited field.
            // Signals should describe the path that exists after the change.
            if status.contains(&b'R') || status.contains(&b'C') {
                i += 1;
            }
        }
    }
    m.insert("isRepo".into(), Value::Bool(true));
    m.insert("branch".into(), opt_string(&branch));
    m.insert("base".into(), opt_string(&diff_base));
    m.insert(
        "changedFiles".into(),
        Value::Array(
            changed
                .iter()
                .take(50)
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    m.insert("changedCount".into(), Value::from(changed.len()));
    Value::Object(m)
}

fn strip_show_prefix_line_ending(prefix: &str) -> &str {
    prefix
        .strip_suffix("\r\n")
        .or_else(|| prefix.strip_suffix('\n'))
        .unwrap_or(prefix)
}

const SCANNABLE_EXT: [&str; 11] = [
    ".html", ".htm", ".css", ".scss", ".jsx", ".tsx", ".js", ".ts", ".vue", ".svelte", ".astro",
];
const SOURCE_DIRS: [&str; 5] = ["src", "app", "components", "pages", "public"];

fn is_vendored_path(rel: &str) -> bool {
    let segs: Vec<&str> = rel.split(['/', '\\']).collect();
    let dirs = &segs[..segs.len().saturating_sub(1)];
    dirs.iter().any(|seg| {
        (seg.starts_with('.')
            && *seg != ".vitepress"
            && *seg != ".vuepress"
            && *seg != ".storybook")
            || *seg == "node_modules"
            || *seg == "dist"
            || *seg == "build"
            || *seg == "__pycache__"
    })
}

fn scan_targets(cwd: &str, git: &Value) -> Value {
    let is_repo = git.get("isRepo").and_then(|v| v.as_bool()).unwrap_or(false);
    let changed: Vec<String> = git
        .get("changedFiles")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let mut m = Map::new();
    if is_repo && !changed.is_empty() {
        let c: Vec<String> = changed
            .into_iter()
            .filter(|f| SCANNABLE_EXT.contains(&jsp::extname(f).to_lowercase().as_str()))
            .filter(|f| !is_vendored_path(f))
            .filter(|f| exists(&jsp::join(&[cwd, f])))
            .collect();
        if !c.is_empty() {
            m.insert(
                "targets".into(),
                Value::Array(c.into_iter().take(50).map(Value::String).collect()),
            );
            m.insert("via".into(), Value::String("git-changes".into()));
            return Value::Object(m);
        }
    }
    let dirs: Vec<&str> = SOURCE_DIRS
        .iter()
        .copied()
        .filter(|d| exists(&jsp::join(&[cwd, d])))
        .collect();
    if !dirs.is_empty() {
        m.insert(
            "targets".into(),
            Value::Array(
                dirs.into_iter()
                    .map(|d| Value::String(d.to_string()))
                    .collect(),
            ),
        );
        m.insert("via".into(), Value::String("source-dir".into()));
        return Value::Object(m);
    }
    if exists(&jsp::join(&[cwd, "index.html"])) {
        m.insert(
            "targets".into(),
            Value::Array(vec![Value::String("index.html".into())]),
        );
        m.insert("via".into(), Value::String("html".into()));
        return Value::Object(m);
    }
    if has_code(cwd) {
        m.insert(
            "targets".into(),
            Value::Array(vec![Value::String(".".into())]),
        );
        m.insert("via".into(), Value::String("root".into()));
        return Value::Object(m);
    }
    m.insert("targets".into(), Value::Array(vec![]));
    m.insert("via".into(), Value::Null);
    Value::Object(m)
}

pub fn gather_signals(cwd: &str, env: &Env) -> Value {
    let ctx = load_context_without_visual_scan(cwd, &TargetOptions::default(), env);
    let git = git_signals(cwd);
    let mut setup = Map::new();
    setup.insert("hasProduct".into(), Value::Bool(ctx.has_product));
    setup.insert("productPath".into(), opt_string(&ctx.product_path));
    setup.insert("hasDesign".into(), Value::Bool(ctx.has_design));
    setup.insert("designPath".into(), opt_string(&ctx.design_path));
    setup.insert("hasCode".into(), Value::Bool(has_code(cwd)));
    setup.insert(
        "platform".into(),
        opt_string(&extract_platform(ctx.product.as_deref())),
    );
    let scan = scan_targets(cwd, &git);
    let mut m = Map::new();
    m.insert("setup".into(), Value::Object(setup));
    m.insert("git".into(), git);
    m.insert("scan".into(), scan);
    Value::Object(m)
}

pub fn run(_args: &[String], io: &mut Io) -> i32 {
    let cwd = io.cwd.to_string_lossy().into_owned();
    let env = io.env.clone();
    let v = gather_signals(&cwd, &env);
    io.out(&format!("{}\n", json_pretty(&v)));
    0
}

#[cfg(test)]
mod tests {
    use super::{git_signals, scan_targets, strip_show_prefix_line_ending};
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "impeccino signals cwd {} {} {}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                suffix
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

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Impeccino test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(root)
            .output()
            .expect("git must be installed for these tests");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn changed_files(git: &Value) -> Vec<String> {
        git["changedFiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    fn targets(scan: &Value) -> Vec<String> {
        scan["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn show_prefix_strips_only_lf_or_crlf_protocol_ending() {
        assert_eq!(strip_show_prefix_line_ending("apps/web/\n"), "apps/web/");
        assert_eq!(strip_show_prefix_line_ending("apps/web/\r\n"), "apps/web/");
        assert_eq!(strip_show_prefix_line_ending("apps/\r/\n"), "apps/\r/");
        assert_eq!(strip_show_prefix_line_ending("apps/web\r"), "apps/web\r");
        assert_eq!(strip_show_prefix_line_ending(""), "");
    }

    #[test]
    fn feature_diff_from_nested_app_is_relative_scoped_and_space_safe() {
        let root = TempDir::new();
        write(
            root.path(),
            "apps/web app/src/Old page.html",
            "<main>old</main>\n",
        );
        write(
            root.path(),
            "apps/other/src/Other.html",
            "<main>other</main>\n",
        );
        git(root.path(), &["init", "--initial-branch=main", "-q"]);
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "initial"]);
        git(root.path(), &["checkout", "-qb", "feature/ui"]);
        write(
            root.path(),
            "apps/web app/src/Old page.html",
            "<main>updated</main>\n",
        );
        write(
            root.path(),
            "apps/web app/src/hero page.html",
            "<main>hero</main>\n",
        );
        write(
            root.path(),
            "apps/other/src/Other.html",
            "<main>changed elsewhere</main>\n",
        );
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "feature changes"]);

        let app = root
            .path()
            .join("apps/web app")
            .to_string_lossy()
            .into_owned();
        let git = git_signals(&app);
        assert_eq!(git["base"], "main");
        assert_eq!(
            changed_files(&git),
            vec!["src/Old page.html", "src/hero page.html"]
        );
        let scan = scan_targets(&app, &git);
        assert_eq!(scan["via"], "git-changes");
        assert_eq!(
            targets(&scan),
            vec!["src/Old page.html", "src/hero page.html"]
        );
    }

    #[test]
    fn status_from_nested_app_parses_renames_and_spaces_and_skips_deleted_paths() {
        let root = TempDir::new();
        write(
            root.path(),
            "apps/web app/src/Old page.html",
            "<main>old</main>\n",
        );
        write(
            root.path(),
            "apps/web app/src/deleted.html",
            "<main>deleted</main>\n",
        );
        write(
            root.path(),
            "apps/other/src/Other.html",
            "<main>other</main>\n",
        );
        git(root.path(), &["init", "--initial-branch=main", "-q"]);
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "initial"]);
        git(
            root.path(),
            &[
                "mv",
                "apps/web app/src/Old page.html",
                "apps/web app/src/Renamed page.html",
            ],
        );
        std::fs::remove_file(root.path().join("apps/web app/src/deleted.html")).unwrap();
        write(
            root.path(),
            "apps/web app/src/New page.html",
            "<main>new</main>\n",
        );
        write(
            root.path(),
            "apps/other/src/Outside.html",
            "<main>outside</main>\n",
        );

        let app = root
            .path()
            .join("apps/web app")
            .to_string_lossy()
            .into_owned();
        let git = git_signals(&app);
        assert_eq!(git["base"], Value::Null);
        assert_eq!(
            changed_files(&git),
            vec![
                "src/Renamed page.html",
                "src/deleted.html",
                "src/New page.html"
            ]
        );
        let scan = scan_targets(&app, &git);
        assert_eq!(scan["via"], "git-changes");
        assert_eq!(
            targets(&scan),
            vec!["src/Renamed page.html", "src/New page.html"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn status_from_nested_app_keeps_newline_filename_as_one_path() {
        let root = TempDir::new();
        write(
            root.path(),
            "apps/web app/src/App.html",
            "<main>app</main>\n",
        );
        git(root.path(), &["init", "--initial-branch=main", "-q"]);
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "initial"]);
        write(
            root.path(),
            "apps/web app/src/New\npage.html",
            "<main>new</main>\n",
        );

        let app = root
            .path()
            .join("apps/web app")
            .to_string_lossy()
            .into_owned();
        let git = git_signals(&app);
        assert_eq!(changed_files(&git), vec!["src/New\npage.html"]);
        let scan = scan_targets(&app, &git);
        assert_eq!(scan["via"], "git-changes");
        assert_eq!(targets(&scan), vec!["src/New\npage.html"]);
    }

    #[test]
    fn untracked_source_directory_keeps_source_dir_fallback() {
        let root = TempDir::new();
        write(
            root.path(),
            "apps/web app/src/App.html",
            "<main>app</main>\n",
        );
        git(root.path(), &["init", "--initial-branch=main", "-q"]);
        git(root.path(), &["add", "."]);
        git(root.path(), &["commit", "-qm", "initial"]);
        write(
            root.path(),
            "apps/web app/src/new-components/Button.html",
            "<button>new</button>\n",
        );

        let app = root
            .path()
            .join("apps/web app")
            .to_string_lossy()
            .into_owned();
        let git = git_signals(&app);
        let scan = scan_targets(&app, &git);
        assert_eq!(scan["via"], "source-dir");
        assert_eq!(targets(&scan), vec!["src"]);
    }
}
