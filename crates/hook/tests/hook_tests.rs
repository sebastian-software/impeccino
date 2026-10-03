//! Rust counterparts of the edge cases in the public repo's
//! `tests/hook.test.mjs` that the oracle corpus does not exercise: path
//! classifiers, config merging, cache GC, git exclude, rendering and quoting,
//! target expansion, and the run flows (tiering, suppression, oversized files,
//! symlinked roots, umbrella launches, Stop pass, before-edit shell shapes,
//! hook-admin scoping). Findings come from the real regex engine, so fixture
//! bodies carry known slop: `gradient-text` (immediate tier) and `side-tab`
//! (deferred tier).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use impeccino_common::{jsp, Io};
use impeccino_core::findings::{finding, Finding};
use impeccino_detect::MissingHtmlEngine;
use impeccino_hook::hook_lib::*;
use impeccino_hook::{admin, before_edit, hook};
use serde_json::{json, Map, Value};

static HTML: MissingHtmlEngine = MissingHtmlEngine;

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Tmp {
        // pid + nanos alone can collide when the parallel runner starts two
        // tests inside one clock tick (seen as a paired flake under full
        // workspace load); the per-process counter makes each dir unique.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "impeccino-hook-rs-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&base).unwrap();
        // Canonical path so the JS-style path helpers and the fs agree on macOS.
        // Like Node's `realpathSync`, without the `\\?\` verbatim prefix Windows
        // adds: the kernel takes a verbatim path literally, so a `/` joined
        // under it would not resolve.
        let real = std::fs::canonicalize(&base).unwrap().to_string_lossy().into_owned();
        Tmp(PathBuf::from(real.strip_prefix(r"\\?\").unwrap_or(&real)))
    }
    fn path(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
    fn write(&self, rel: &str, body: &str) -> String {
        // `PathBuf::join` keeps the `/` inside `rel`, which leaves a mixed-form
        // path on Windows; join the way the hook's own path helpers do, so the
        // returned path is what the hook resolves a relative target to.
        let abs = jsp::join(&[&self.path(), rel]);
        let p = std::path::Path::new(&abs).to_path_buf();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
        abs
    }
    fn exists(&self, rel: &str) -> bool {
        self.0.join(rel).exists()
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.0.join(rel)).unwrap()
    }
    /// The hook's session cache for this project, in the user cache.
    fn cache_path(&self) -> String {
        get_cache_path(&self.path())
    }
    fn has_cache(&self) -> bool {
        std::path::Path::new(&self.cache_path()).exists()
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        // Hook state lives in the user cache, keyed by project; drop it with
        // the project so test runs leave nothing behind.
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&self.path())));
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rt_with(cwd: &str, env: HashMap<String, String>) -> Runtime<'static> {
    Runtime::new(
        cwd.to_string(),
        env,
        "/impeccino".to_string(),
        "/opt/bin/impeccino",
        &HTML,
    )
}

fn rt_with_command(cwd: &str, impeccino_command: String) -> Runtime<'static> {
    Runtime::new(
        cwd.to_string(),
        HashMap::new(),
        impeccino_command,
        "/opt/bin/impeccino",
        &HTML,
    )
}

fn rt(cwd: &str) -> Runtime<'static> {
    rt_with(cwd, HashMap::new())
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn install_test_skill(t: &Tmp, skill_rel: &str) {
    let launcher = t.write(&format!("{skill_rel}/scripts/impeccino"), "#!/bin/sh\nexit 0\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = launcher;
    t.write(&format!("{skill_rel}/scripts/impeccino.cmd"), "@echo off\r\nexit /b 0\r\n");
}

fn init_test_git(repo_root: &str) {
    let status = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo_root)
        .status()
        .unwrap();
    assert!(status.success(), "could not initialize test repository at {repo_root}");
}

fn f(id: &str, line: f64, name: &str, description: &str, snippet: &str) -> Finding {
    let mut x = finding(id, "src/Card.tsx", snippet, line);
    x.name = name.to_string();
    x.description = description.to_string();
    x
}

fn edit_event(cwd: &str, file: &str, session: &str) -> String {
    json!({
        "session_id": session, "cwd": cwd, "hook_event_name": "PostToolUse",
        "tool_name": "Edit", "tool_input": { "file_path": file },
    })
    .to_string()
}

fn stop_event(cwd: &str, session: &str) -> String {
    json!({ "session_id": session, "cwd": cwd, "hook_event_name": "Stop", "stop_hook_active": false }).to_string()
}

fn hook_message(stdout: &str) -> String {
    let value: Value = serde_json::from_str(stdout).unwrap();
    value
        .pointer("/hookSpecificOutput/additionalContext")
        .or_else(|| value.get("additionalContext"))
        .or_else(|| value.get("additional_context"))
        .or_else(|| value.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

const GRADIENT_CSS: &str = ".title { background: linear-gradient(90deg, #f472b6, #a78bfa); -webkit-background-clip: text; color: transparent; }\n";
const SIDE_TAB_CSS: &str = ".card { border-left: 4px solid #6366f1; border-radius: 8px; }\n";
const CATCH_ALL_ROUTE_SOURCE: &str = r#"export default function Page() { return <div className="title">Title</div>; }
const styles = css`
.title { background: linear-gradient(90deg, #f472b6, #a78bfa); -webkit-background-clip: text; color: transparent; }
`;
"#;

fn catch_all_route_fixture() -> (Tmp, String, String) {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let route = t.write("app/[...slug]/page.tsx", CATCH_ALL_ROUTE_SOURCE);
    (t, cwd, route)
}

fn monorepo_design_fixture(root_design: bool) -> Tmp {
    let t = Tmp::new();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/a/package.json", "{}");
    t.write("apps/b/package.json", "{}");
    t.write("apps/a/DESIGN.md", "---\ncolors:\n  primary: '#112233'\ntypography:\n  body:\n    fontFamily: Inter\n---\n");
    if root_design {
        t.write("DESIGN.md", "---\ncolors:\n  primary: '#224466'\ntypography:\n  body:\n    fontFamily: Roboto\n---\n");
    }
    t
}

// Run identical cases through all three hook entry points. A font the repo's
// DESIGN.md declares must still count as overused in app A, whose own
// DESIGN.md declares another one; app B inherits the repo document.
fn check_monorepo_design_hook(mode: &str) {
    for (root_design, app, font, expected) in [
        (false, "a", "Inter", false),
        (false, "b", "Inter", true),
        (true, "a", "Roboto", true),
        (true, "b", "Inter", true),
        (true, "b", "Roboto", false),
    ] {
        let t = monorepo_design_fixture(root_design);
        let cwd = t.path();
        let source = format!(".probe {{ font-family: {font}, sans-serif; }}\n");
        let file = t.write(&format!("apps/{app}/src/probe.css"), &source);
        // GitHub Copilot has no Stop pass, so its per-edit run reports the
        // full rule set, overused-font included.
        let r = rt_with(&cwd, env(&[("IMPECCINO_HOOK_HARNESS", "github")]));
        let cache_cwd = resolve_cache_cwd(&r, Some(&file), &cwd);
        let out = match mode {
            "post" => hook::run_hook(&r, &edit_event(&cwd, &file, "s1")).stdout,
            "before" => {
                // A proposed new file must resolve its owning app too.
                std::fs::remove_file(&file).unwrap();
                hbe(&r, &cursor(&cwd, "Write", json!({
                    "file_path": file, "content": source,
                }))).0
            }
            "stop" => {
                let mut cache = read_cache(&cache_cwd);
                touch_file(&mut cache, "s1", &file);
                persist_project_cache(&r, &cache_cwd, &cwd, &mut cache, "s1");
                hook::run_stop_hook(&r, &stop_event(&cwd, "s1")).stdout
            }
            _ => unreachable!(),
        };
        assert_eq!(out.contains("overused-font"), expected,
            "{mode}: root_design={root_design}, app={app}, font={font}: {out}");
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&cache_cwd)));
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&cwd)));
    }
}

#[test]
fn monorepo_design_post_edit() { check_monorepo_design_hook("post"); }

#[test]
fn monorepo_design_before_edit() { check_monorepo_design_hook("before"); }

#[test]
fn monorepo_design_stop() { check_monorepo_design_hook("stop"); }

#[test]
fn cache_root_prefers_nearest_nested_project() {
    let t = Tmp::new();
    let root = t.path();
    let app = t.write("apps/store/package.json", "{}");
    let file = t.write("apps/store/src/app.css", GRADIENT_CSS);
    let app_root = jsp::dirname(&app);
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);

    assert_eq!(
        resolve_cache_cwd(&rt(&root), Some(&file), &root),
        app_root,
        "session state and platform policy should follow the nearest touched app project"
    );
}

#[test]
fn stop_finds_every_project_touched_by_its_session_cwd() {
    let t = Tmp::new();
    let root = t.path(); // umbrella directory, intentionally not a project
    for (app, session) in [("a", "shared"), ("b", "shared"), ("c", "other")] {
        t.write(&format!("apps/{app}/package.json"), "{}");
        let file = t.write(&format!("apps/{app}/src/{app}.css"), GRADIENT_CSS);
        let post = hook::run_hook(&rt(&root), &edit_event(&root, &file, session));
        assert!(post.stdout.contains("[gradient-text]"), "{app}: {}", post.stdout);
        std::fs::write(&file, format!("{GRADIENT_CSS}{SIDE_TAB_CSS}")).unwrap();
    }

    let shared = hook::run_stop_hook(&rt(&root), &stop_event(&root, "shared"));
    assert_eq!(shared.audit["freshFiles"], json!(2), "{}", shared.stdout);
    assert!(shared.stdout.contains("apps/a/src/a.css"), "{}", shared.stdout);
    assert!(shared.stdout.contains("apps/b/src/b.css"), "{}", shared.stdout);
    assert!(!shared.stdout.contains("apps/c/src/c.css"), "{}", shared.stdout);

    let other = hook::run_stop_hook(&rt(&root), &stop_event(&root, "other"));
    assert_eq!(other.audit["freshFiles"], json!(1), "{}", other.stdout);
    assert!(other.stdout.contains("apps/c/src/c.css"), "{}", other.stdout);
    assert!(!other.stdout.contains("apps/a/src/a.css"), "{}", other.stdout);

    for app in ["a", "b", "c"] {
        let app_root = jsp::join(&[&root, "apps", app]);
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
    }
}

#[test]
fn stop_index_finds_nested_project_when_session_cwd_is_a_child_project() {
    let t = Tmp::new();
    let repo = t.path();
    t.write("package.json", "{}");
    t.write("apps/editor/package.json", "{}");
    let session_cwd = jsp::join(&[&repo, "apps", "editor"]);
    t.write("apps/editor/packages/preview/package.json", "{}");
    let file = t.write("apps/editor/packages/preview/src/preview.css", GRADIENT_CSS);

    let post = hook::run_hook(&rt(&session_cwd), &edit_event(&session_cwd, &file, "nested"));
    assert!(post.stdout.contains("[gradient-text]"), "{}", post.stdout);
    std::fs::write(&file, format!("{GRADIENT_CSS}{SIDE_TAB_CSS}")).unwrap();
    let stop = hook::run_stop_hook(&rt(&session_cwd), &stop_event(&session_cwd, "nested"));
    assert_eq!(stop.audit["freshFiles"], json!(1), "{}", stop.stdout);
    assert!(stop.stdout.contains("packages/preview/src/preview.css"), "{}", stop.stdout);

    for root in [&session_cwd, &jsp::join(&[&session_cwd, "packages", "preview"])] {
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(root)));
    }
}

#[test]
fn stop_applies_native_platform_per_registered_project() {
    let t = Tmp::new();
    let root = t.path();
    let native_root = jsp::join(&[&root, "apps", "native"]);
    let web_root = jsp::join(&[&root, "apps", "web"]);
    t.write("apps/native/package.json", "{}");
    t.write("apps/native/PRODUCT.md", "# Product\n\n## Platform\nios and android\n");
    t.write("apps/web/package.json", "{}");
    let native_file = t.write("apps/native/src/native.css", SIDE_TAB_CSS);
    let web_file = t.write("apps/web/src/web.css", SIDE_TAB_CSS);
    let r = rt(&root);
    for (project, file) in [(&native_root, &native_file), (&web_root, &web_file)] {
        let mut cache = read_cache(project);
        touch_file(&mut cache, "mixed", file);
        persist_project_cache(&r, project, &root, &mut cache, "mixed");
    }

    let stop = hook::run_stop_hook(&r, &stop_event(&root, "mixed"));
    assert_eq!(stop.audit["scannedFiles"], json!(1), "{}", stop.stdout);
    assert!(stop.stdout.contains("apps/web/src/web.css"), "{}", stop.stdout);
    assert!(!stop.stdout.contains("apps/native/src/native.css"), "{}", stop.stdout);

    for project in [&native_root, &web_root] {
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(project)));
    }
}

#[test]
fn stop_index_does_not_authorize_outside_or_sensitive_files() {
    let t = Tmp::new();
    let root = t.path();
    let app_root = jsp::join(&[&root, "apps", "web"]);
    t.write("apps/web/package.json", "{}");
    let sensitive = t.write("apps/web/src/.env.css", SIDE_TAB_CSS);
    let outside_dir = Tmp::new();
    let outside = outside_dir.write("outside.css", SIDE_TAB_CSS);
    let r = rt(&root);
    let mut cache = read_cache(&app_root);
    touch_file(&mut cache, "unsafe", &sensitive);
    touch_file(&mut cache, "unsafe", &outside);
    persist_project_cache(&r, &app_root, &root, &mut cache, "unsafe");

    let stop = hook::run_stop_hook(&r, &stop_event(&root, "unsafe"));
    assert_eq!(audit_str(&stop.audit, "skipped"), Some("no-touched-files"));
    assert_eq!(stop.audit["scannedFiles"], json!(0));

    let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
}

#[test]
fn session_project_root_index_is_bounded_even_when_read_from_old_data() {
    let roots: Vec<Value> = (0..100)
        .map(|i| Value::String(format!("/project/{i}")))
        .collect();
    let cache: Cache = serde_json::from_value(json!({
        "version": 1,
        "sessions": { "s": { "projectRoots": roots } }
    }))
    .unwrap();
    assert_eq!(registered_project_roots(&cache, "s").len(), 16);

    let mut next = read_cache("/not/a/real/project");
    for i in 0..100 {
        register_project_root(&mut next, "s", &format!("/project/{i}"));
    }
    assert_eq!(registered_project_roots(&next, "s").len(), 16);
    assert_eq!(registered_project_roots(&next, "s").first().unwrap(), "/project/84");
}

#[test]
fn concurrent_project_registrations_keep_all_roots_in_the_session_index() {
    const PROJECTS: usize = 12;
    let t = Tmp::new();
    let root = t.path();
    let start = Arc::new(Barrier::new(PROJECTS + 1));
    let mut workers = Vec::new();

    for index in 0..PROJECTS {
        let session_cwd = root.clone();
        let project_root = jsp::join(&[&root, &format!("apps/app-{index}")]);
        std::fs::create_dir_all(&project_root).unwrap();
        let start = Arc::clone(&start);
        workers.push(std::thread::spawn(move || {
            let runtime = rt(&session_cwd);
            let mut cache = read_cache(&project_root);
            touch_file(
                &mut cache,
                "parallel-registration",
                &jsp::join(&[&project_root, "src/app.css"]),
            );
            start.wait();
            assert!(persist_project_cache(
                &runtime,
                &project_root,
                &session_cwd,
                &mut cache,
                "parallel-registration",
            ));
        }));
    }
    start.wait();
    for worker in workers {
        worker.join().unwrap();
    }

    let roots = registered_project_roots(&read_cache(&root), "parallel-registration");
    assert_eq!(roots.len(), PROJECTS, "concurrent registrations were lost: {roots:?}");
    for index in 0..PROJECTS {
        let expected = jsp::join(&[&root, &format!("apps/app-{index}")]);
        assert!(roots.contains(&expected), "missing {expected} in {roots:?}");
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&expected)));
    }
}

#[test]
fn stale_same_cwd_cache_write_preserves_another_sessions_project_roots() {
    let t = Tmp::new();
    let root = t.path();
    let app_root = jsp::join(&[&root, "apps/web"]);
    let stale = read_cache(&root);
    let mut child = read_cache(&app_root);
    touch_file(
        &mut child,
        "child-session",
        &jsp::join(&[&app_root, "src/web.css"]),
    );
    let runtime = rt(&root);
    assert!(persist_project_cache(
        &runtime,
        &app_root,
        &root,
        &mut child,
        "child-session",
    ));

    let mut stale = stale;
    assert!(persist_project_cache(
        &runtime,
        &root,
        &root,
        &mut stale,
        "unrelated-session",
    ));
    assert_eq!(
        registered_project_roots(&read_cache(&root), "child-session"),
        vec![app_root.clone()]
    );
    let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
}

#[test]
fn monorepo_design_document_locations_and_sidecars() {
    for location in ["DESIGN.md", "docs/DESIGN.md", ".agents/context/DESIGN.md"] {
        let t = monorepo_design_fixture(true);
        let cwd = t.path();
        let file = t.write("apps/b/src/probe.css", ".probe {}\n");
        let md = t.write(&format!("apps/b/{location}"),
            "---\ntypography:\n  body:\n    fontFamily: Georgia\nrounded:\n  md: 8px\ncolors:\n  primary: '#abcdef'\n---\n");
        let sidecar = t.write(&format!("apps/b/{}", location.replace("DESIGN.md", "DESIGN.json")), "{}");
        let scan = design_system_options_for_file(&rt(&cwd), &read_config(&cwd), &cwd, &file);
        let ds = scan.design_system.as_ref().unwrap();
        assert_eq!(ds.source_path.as_deref(), Some(md.as_str()));
        assert_eq!(ds.sidecar_path.as_deref(), Some(sidecar.as_str()));
        let findings = detector_detect_text(
            ".probe { color: #ff0000; font-family: Verdana; border-radius: 19px; }", &file, &scan);
        for rule in ["design-system-color", "design-system-font", "design-system-radius"] {
            assert!(findings.iter().any(|f| f.antipattern == rule), "{location}: {rule}");
        }
        let allowed = detector_detect_text(
            ".probe { color: #abcdef; font-family: Georgia; border-radius: 8px; }", &file, &scan);
        assert!(allowed.iter().all(|f| !f.antipattern.starts_with("design-system-")));
    }
}

#[test]
fn monorepo_design_local_document_and_disabled_config_do_not_inherit() {
    let t = monorepo_design_fixture(true);
    let cwd = t.path();
    let file = t.write("apps/a/src/probe.css", ".probe {}\n");
    t.write("apps/a/DESIGN.md", "# App-specific prose, with no machine-readable tokens\n");
    let r = rt(&cwd);
    let mut config = read_config(&cwd);
    assert!(design_system_options_for_file(&r, &config, &cwd, &file).design_system.is_none());
    config.design_system_enabled = false;
    let sibling = t.write("apps/b/src/probe.css", ".probe {}\n");
    assert!(design_system_options_for_file(&r, &config, &cwd, &sibling).design_system.is_none());
}

#[test]
fn monorepo_design_batch_notes_follow_the_displayed_file() {
    for mode in ["post-fresh", "post-pending", "post-clean", "stop"] {
        for stale_app in ["a", "b"] {
            let t = monorepo_design_fixture(true);
            let cwd = t.path();
            let source = if mode == "post-clean" { ".probe { color: #112233; }" }
                else { ".probe { font-family: Lato, sans-serif; }" };
            let a = t.write("apps/a/src/probe.css", source);
            let b = t.write("apps/b/src/probe.css", ".probe { color: #224466; }");
            let r = rt_with(&cwd, env(&[("IMPECCINO_HOOK_HARNESS", "github")]));
            if mode == "post-pending" {
                hook::run_hook(&r, &edit_event(&cwd, &a, "s1"));
            }
            let sidecar = t.write(if stale_app == "a" { "apps/a/DESIGN.json" }
                else { "DESIGN.json" }, "{}");
            std::fs::File::options().write(true).open(sidecar).unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)).unwrap();
            let out = if mode == "stop" {
                let a_root = resolve_cache_cwd(&r, Some(&a), &cwd);
                let mut a_cache = read_cache(&a_root);
                touch_file(&mut a_cache, "s1", &a);
                persist_project_cache(&r, &a_root, &cwd, &mut a_cache, "s1");
                let b_root = resolve_cache_cwd(&r, Some(&b), &cwd);
                let mut b_cache = read_cache(&b_root);
                touch_file(&mut b_cache, "s1", &b);
                persist_project_cache(&r, &b_root, &cwd, &mut b_cache, "s1");
                hook::run_stop_hook(&r, &stop_event(&cwd, "s1")).stdout
            } else {
                let event = json!({"session_id":"s1", "cwd":cwd, "hook_event_name":"PostToolUse",
                    "tool_name":"apply_patch", "tool_input":{"command":format!(
                        "*** Begin Patch\n*** Update File: {a}\n*** Update File: {b}\n*** End Patch")}});
                hook::run_hook(&r, &event.to_string()).stdout
            };
            assert!(out.contains("apps/a/src/probe.css"), "{mode}: {out}");
            assert_eq!(out.contains("DESIGN.md is newer"), stale_app == "a", "{mode}: {out}");
            for project in ["apps/a", "apps/b"] {
                let project_root = jsp::join(&[&cwd, project]);
                let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&project_root)));
            }
            let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&cwd)));
        }
    }
}

fn edit_with_original(cwd: &str, file: &str, session: &str, before: &str, old: &str, new: &str) -> String {
    json!({
        "session_id": session, "cwd": cwd, "hook_event_name": "PostToolUse",
        "tool_name": "Edit", "tool_input": {"file_path": file, "old_string": old, "new_string": new},
        "tool_response": {"filePath": file, "originalFile": before, "oldString": old,
            "newString": new, "replaceAll": false, "userModified": false},
    }).to_string()
}

#[test]
fn stop_baseline_import_only_edit_does_not_blame_existing_font() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    // This is the actual working-tree preimage, not HEAD (which might differ).
    let before = "import dead from 'dead';\nconst report = `<style>body { font-family: Fraunces; }</style>`;\n";
    let after = before.replacen("import dead from 'dead';\n", "", 1);
    let file = t.write("query.ts", &after);
    let r = rt(&cwd);
    assert!(detector_detect_text(before, &file, &HookScanOptions::default()).iter().any(|f| f.antipattern == "overused-font"));
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", before, "import dead from 'dead';\n", ""));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(stop.stdout.is_empty(), "{}", stop.stdout);
    assert_eq!(stop.audit["preExistingFindings"], json!(1));
    assert!(!std::fs::read_to_string(t.cache_path()).unwrap().contains("const report"), "do not persist source contents");
    assert!(!t.exists("DESIGN.md") && !t.exists(".impeccino"), "baseline is not a waiver");
    assert!(detector_detect_text(&after, &file, &HookScanOptions::default()).iter().any(|f| f.antipattern == "overused-font"), "explicit scans stay unchanged");
}

#[test]
fn stop_baseline_reports_new_findings_and_keeps_first_preimage() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let file = t.write("card.css", SIDE_TAB_CSS);
    let r = rt(&cwd);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", ".card {}\n", ".card {}\n", SIDE_TAB_CSS));
    let second = format!("/* later */\n{SIDE_TAB_CSS}");
    t.write("card.css", &second);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", SIDE_TAB_CSS, SIDE_TAB_CSS, &second));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(stop.stdout.contains("[side-tab]"));
    assert!(stop.stdout.contains("[new]"), "{}", stop.stdout);
    assert_eq!(stop.audit["newFindings"], json!(1));
}

#[test]
fn stop_baseline_missing_or_mismatched_preimage_stays_unknown() {
    for original in [None, Some("not the actual preimage")] {
        let t = Tmp::new();
        let cwd = t.path();
        t.write("package.json", "{}");
        let file = t.write("card.css", SIDE_TAB_CSS);
        let r = rt(&cwd);
        let event = original.map(|before| edit_with_original(&cwd, &file, "s1", before, ".card {}", SIDE_TAB_CSS))
            .unwrap_or_else(|| edit_event(&cwd, &file, "s1"));
        hook::run_hook(&r, &event);
        let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
        assert!(stop.stdout.contains("[attribution unknown]"), "{}", stop.stdout);
        assert!(stop.stdout.contains("may predate this session"));
        assert_eq!(stop.audit["unknownFindings"], json!(1));
    }
}

#[test]
fn stop_baseline_existing_debt_fixed_then_reintroduced_is_new() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let clean = ".card {}\n";
    let file = t.write("card.css", clean);
    let r = rt(&cwd);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", SIDE_TAB_CSS, SIDE_TAB_CSS, clean));
    t.write("card.css", SIDE_TAB_CSS);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", clean, clean, SIDE_TAB_CSS));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(stop.stdout.contains("[new]"), "{}", stop.stdout);
    assert_eq!(stop.audit["preExistingFindings"], json!(0));
}

#[test]
fn stop_baseline_late_preimage_does_not_relabel_unknown_debt() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let file = t.write("card.css", SIDE_TAB_CSS);
    let r = rt(&cwd);
    hook::run_hook(&r, &edit_event(&cwd, &file, "s1"));
    let second = format!("/* later */\n{SIDE_TAB_CSS}");
    t.write("card.css", &second);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", SIDE_TAB_CSS, SIDE_TAB_CSS, &second));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(stop.audit["unknownFindings"], json!(1));
    assert_eq!(stop.audit["preExistingFindings"], json!(0));
}

#[test]
fn stop_baseline_keeps_indirect_stylesheet_findings_unknown() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    t.write("src/styles.css", SIDE_TAB_CSS);
    let before = "import './styles.css';\nexport const Card = () => <div>Before</div>;\n";
    let after = before.replace("Before", "After");
    let file = t.write("src/Card.tsx", &after);
    let r = rt(&cwd);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", before, "Before", "After"));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(stop.stdout.contains("[side-tab]"), "{}", stop.stdout);
    assert_eq!(stop.audit["unknownFindings"], json!(1));
    assert_eq!(stop.audit["preExistingFindings"], json!(0));
}

#[test]
fn stop_baseline_extra_identical_occurrence_is_not_suppressed() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    // The text detector deduplicates identical snippets within two lines.
    let after = format!("{SIDE_TAB_CSS}\n\n\n{SIDE_TAB_CSS}");
    let file = t.write("card.css", &after);
    let r = rt(&cwd);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", SIDE_TAB_CSS, SIDE_TAB_CSS, &after));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(stop.audit["preExistingFindings"], json!(1));
    assert_eq!(stop.audit["newFindings"], json!(1));
}

#[test]
fn stop_baseline_write_create_is_new_but_missing_update_preimage_is_unknown() {
    for (kind, expected) in [("create", "newFindings"), ("update", "unknownFindings")] {
        let t = Tmp::new();
        let cwd = t.path();
        t.write("package.json", "{}");
        let file = t.write("card.css", SIDE_TAB_CSS);
        let r = rt(&cwd);
        let event = json!({"cwd": cwd, "session_id": "s1", "tool_name": "Write",
            "tool_input": {"file_path": file, "content": SIDE_TAB_CSS},
            "tool_response": {"type": kind, "filePath": file, "content": SIDE_TAB_CSS, "originalFile": null}}).to_string();
        hook::run_hook(&r, &event);
        let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
        assert_eq!(stop.audit[expected], json!(1));
    }
}

#[test]
fn stop_baseline_unknown_notice_respects_small_output_budget() {
    // The output budget is fixed at the default now (docs/adr/0020).
    for (budget, stale) in [(8000, false), (8000, true)] {
        let t = Tmp::new();
        let cwd = t.path();
        t.write("package.json", "{}");
        let file = t.write("card.css", SIDE_TAB_CSS);
        let r = rt(&cwd);
        hook::run_hook(&r, &edit_event(&cwd, &file, "s1"));
        if stale {
            // Make the notice eligible only at Stop; no sleeps or clock races.
            t.write("DESIGN.md", "---\nname: Test\n---\n");
            let sidecar = t.write("DESIGN.json", "{}");
            std::fs::File::options().write(true).open(sidecar).unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)).unwrap();
            assert!(design_system_options(&read_config(&cwd), &cwd).md_newer_than_json());
        }
        let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
        let output: Value = serde_json::from_str(&stop.stdout).unwrap();
        let text = output["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(text.encode_utf16().count() <= budget, "{text}");
        assert!(text.contains("may predate this session"));
        assert!(text.contains("[side-tab]"), "{text}");
        assert!(text.contains("[attribution unknown]"), "{text}");
        assert!(text.contains("card.css"), "{text}");
        if stale {
            assert_eq!(text.contains("DESIGN.md is newer"), budget > 500, "{text}");
            let cache: Value = serde_json::from_str(&std::fs::read_to_string(t.cache_path()).unwrap()).unwrap();
            assert_eq!(cache["sessions"]["s1"]["designNoteShown"] == json!(true), budget > 500);
        }
    }
}

#[test]
fn stop_baseline_deduplicated_unknown_does_not_add_notice_to_new_finding() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let old = t.write("old/card.css", SIDE_TAB_CSS);
    hook::run_hook(&r, &edit_event(&cwd, &old, "s1"));
    assert!(hook::run_stop_hook(&r, &stop_event(&cwd, "s1")).stdout.contains("[attribution unknown]"));
    let new = t.write("new/card.css", SIDE_TAB_CSS);
    // Use a verified create event (an empty Edit preimage is not trusted).
    let create = json!({"cwd": cwd, "session_id": "s1", "tool_name": "Write",
        "tool_input": {"file_path": new, "content": SIDE_TAB_CSS},
        "tool_response": {"type": "create", "filePath": new, "content": SIDE_TAB_CSS, "originalFile": null}}).to_string();
    hook::run_hook(&r, &create);
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(stop.audit["unknownFindings"], json!(1), "audit retains the full scan");
    assert!(stop.stdout.contains("[new]"), "{}", stop.stdout);
    assert!(!stop.stdout.contains("may predate this session"), "{}", stop.stdout);
}

#[test]
fn stop_baseline_uses_dirty_worktree_not_git_head() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    t.write("card.css", ".card {}\n");
    std::fs::create_dir(t.0.join("empty-hooks")).unwrap();
    let hooks = format!("core.hooksPath={}/empty-hooks", cwd);
    let git = |args: &[&str]| {
        let result = std::process::Command::new("git").current_dir(&t.0)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                "-c", "commit.gpgsign=false", "-c", &hooks])
            .args(args).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    git(&["init", "--quiet"]);
    git(&["add", "card.css", "package.json"]);
    git(&["commit", "--quiet", "-m", "clean baseline"]);
    // The user introduced this debt before the agent session; HEAD is clean.
    let before = format!("/* unrelated */\n{SIDE_TAB_CSS}");
    let file = t.write("card.css", SIDE_TAB_CSS);
    let r = rt(&cwd);
    hook::run_hook(&r, &edit_with_original(&cwd, &file, "s1", &before, "/* unrelated */\n", ""));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(stop.audit["preExistingFindings"], json!(1));
    assert!(stop.stdout.is_empty());
}

#[test]
fn stop_baseline_untrusted_shapes_do_not_suppress_findings() {
    for variant in ["modified", "wrong-path", "no-session", "oversized", "ambiguous", "other-provider"] {
        let t = Tmp::new();
        let cwd = t.path();
        t.write("package.json", "{}");
        let file = t.write("card.css", SIDE_TAB_CSS);
        let mut event: Value = serde_json::from_str(&edit_with_original(&cwd, &file, "s1", SIDE_TAB_CSS, SIDE_TAB_CSS, SIDE_TAB_CSS)).unwrap();
        match variant {
            "modified" => event["tool_response"]["userModified"] = json!(true),
            "wrong-path" => event["tool_response"]["filePath"] = json!("another.css"),
            "no-session" => event["session_id"] = Value::Null,
            "oversized" => event["tool_response"]["originalFile"] = json!("x".repeat(512 * 1024 + 1)),
            "ambiguous" => {
                let repeated = SIDE_TAB_CSS.repeat(2);
                t.write("card.css", &repeated);
                event["tool_response"]["originalFile"] = json!(repeated);
            }
            _ => {},
        }
        let r = if variant == "other-provider" { rt_with(&cwd, env(&[("IMPECCINO_HOOK_HARNESS", "codex")])) } else { rt(&cwd) };
        hook::run_hook(&r, &event.to_string());
        let session = if variant == "no-session" { "unknown" } else { "s1" };
        let stop = hook::run_stop_hook(&r, &stop_event(&cwd, session));
        assert!(stop.stdout.contains("[attribution unknown]"), "{variant}: {}", stop.stdout);
        assert_eq!(stop.audit["preExistingFindings"], json!(0), "{variant}");
    }
}

#[test]
fn stop_baseline_scan_suppression_discards_exemptions() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let file = t.write("card.css", SIDE_TAB_CSS);
    let r = rt(&cwd);
    let event = edit_with_original(&cwd, &file, "s1", SIDE_TAB_CSS, SIDE_TAB_CSS, SIDE_TAB_CSS);
    for _ in 0..=EDIT_COUNT_THRESHOLD {
        hook::run_hook(&r, &event);
    }
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(stop.audit["unknownFindings"], json!(1));
    assert_eq!(stop.audit["preExistingFindings"], json!(0));
}

fn audit_str<'a>(a: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

// ── classifiers ───────────────────────────────────────────────────────────

#[test]
fn truthy_and_depth() {
    for v in ["1", "true", "TRUE", "yes", "YES", "on", "On"] {
        assert!(truthy(Some(v)), "{v}");
    }
    for v in ["", "0", "false", "no", "off", "yep"] {
        assert!(!truthy(Some(v)), "{v}");
    }
    assert!(!truthy(None));
    assert!(depth_is_set(Some("2")));
    assert!(depth_is_set(Some(" 1 ")));
    assert!(!depth_is_set(Some("0")));
    assert!(!depth_is_set(Some("")));
    assert!(!depth_is_set(None));
}

#[test]
fn has_path_traversal_matches_parent_segments_only() {
    for path in [
        "/repo/app/[...slug]/page.tsx",
        r"C:\repo\app\[...path]\page.astro",
        "/repo/foo..bar/page.tsx",
        "/repo/%2e%2e/page.tsx",
    ] {
        assert!(!has_path_traversal(path), "not traversal: {path}");
    }
    for path in [
        "..",
        "../outside.tsx",
        "/repo/src/../outside.tsx",
        "/repo/src/..",
        r"..\outside.tsx",
        r"C:\repo\src\..\outside.tsx",
        r"C:\repo/src\..\outside.tsx",
    ] {
        assert!(has_path_traversal(path), "parent segment: {path}");
    }
}

#[test]
fn sensitive_and_generated_paths() {
    for p in [
        "/x/.env",
        "/x/.env.production",
        "/x/server.pem",
        "/x/id_rsa",
        "/x/id_rsa.pub",
        "/x/api-secret.json",
        "/x/client_secret.ts",
        "/x/credentials.yml",
        "/x/.git/config",
        "/x/secret.json",
    ] {
        assert!(is_sensitive_path(p), "expected sensitive: {p}");
    }
    for p in [
        "/x/src/Card.tsx",
        "/x/app/page.html",
        "/x/styles/main.css",
        "/x/src/CredentialForm.tsx",
        "/x/src/SecretPage.jsx",
        "/x/src/secretary-dashboard.vue",
        "/x/src/credentials-panel.tsx",
        "/x/secretx.json",
    ] {
        assert!(!is_sensitive_path(p), "unexpected sensitive: {p}");
    }
    for p in [
        "/x/src/foo.generated.tsx",
        "/x/types.d.ts",
        "/x/bundle.min.js",
        "/x/node_modules/lib/index.tsx",
        "/x/dist/Card.tsx",
        "/x/build/index.html",
        "/x/pkg.lock.json",
        "/x/.next/server.js",
        "/x/coverage/report.html",
        "/x/site/public/js/generated/counts.js",
        "/x/src/generated/schema.ts",
    ] {
        assert!(is_generated_path(p), "expected generated: {p}");
    }
    for p in [
        "/x/src/generateReport.ts",
        "/x/src/generated-utils.ts",
        "/x/src/components/CodeGenerator.tsx",
        "/x/src/ui/regenerate-button.jsx",
    ] {
        assert!(!is_generated_path(p), "unexpected generated: {p}");
    }
}

#[cfg(unix)]
#[test]
fn inside_project_handles_symlinks_and_unwritten_files() {
    let t = Tmp::new();
    let root = t.path();
    let r = rt(&root);
    let file = t.write("real/src/Card.tsx", "noop");
    let real = format!("{root}/real");
    let link = format!("{root}/link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(is_scan_target_inside_project(&r, &file, &link));
    assert!(is_scan_target_inside_project(
        &r,
        &format!("{link}/src/Card.tsx"),
        &real
    ));
    assert!(is_scan_target_inside_project(
        &r,
        &format!("{link}/src/New.tsx"),
        &real
    ));
    assert!(is_scan_target_inside_project(
        &r,
        &format!("{real}/deep/New.tsx"),
        &link
    ));
    assert!(!is_scan_target_inside_project(
        &r,
        &format!("{root}/elsewhere/New.tsx"),
        &real
    ));
    assert!(is_scan_target_inside_project(&r, &real, &real));
    assert!(!is_scan_target_inside_project(&r, "", &real));
    assert!(!is_scan_target_inside_project(&r, &file, ""));
}

// ── config ────────────────────────────────────────────────────────────────

#[test]
fn design_md_decisions_reach_the_hook_per_file() {
    let t = Tmp::new();
    let cwd = t.path();
    let d = read_config(&cwd);
    assert_eq!(d, HookConfig::default());
    assert_eq!(d.per_edit_rules, "immediate");
    assert_eq!(d.advisory_rules, "exclude");
    assert_eq!((d.limits.max_findings, d.limits.max_chars, d.limits.max_file_bytes), (5.0, 8000.0, 131072.0));
    t.write(
        "DESIGN.md",
        "---\ntypography:\n  body:\n    fontFamily: \"'Inter', sans-serif\"\n---\n# Design\n\n**The Ledger Rule.** Accent rails mark the active row. <!-- impeccino-disable Side-Tab -- the ledger rule -->\n\n```md\n<!-- impeccino-disable gradient-text -->\n```\n",
    );
    let file = t.write("a.css", "a{}");
    let scan = design_system_options_for_file(&rt(&cwd), &d, &cwd, &file);
    assert_eq!(scan.decisions.waived_rules, vec!["side-tab"]);
    assert_eq!(scan.decisions.declared_fonts, vec!["inter"]);
    let mut font = f("overused-font", 2.0, "O", "d", "body { font-family: \"Inter\", sans-serif; }");
    font.file = file.clone();
    let side = f("side-tab", 1.0, "S", "d", "s");
    let gradient = f("gradient-text", 1.0, "G", "d", "s");
    let kept = filter_findings_for(vec![font, side, gradient], &d, &scan);
    assert_eq!(kept.iter().map(|x| x.antipattern.as_str()).collect::<Vec<_>>(), vec!["gradient-text"]);
}

#[test]
fn configured_extensions_match_suffixes() {
    let exts = HookConfig::default().extensions;
    assert_eq!(
        exts.iter().map(|e| e.ext.as_str()).collect::<Vec<_>>(),
        vec![".blade.php", ".twig", ".html.erb", ".erb", ".hbs", ".handlebars"]
    );
    assert!(exts.iter().all(|e| e.engine == "html"));
    assert_eq!(match_configured_extension("/x/show.blade.php", &exts).unwrap().ext, ".blade.php");
    assert_eq!(match_configured_extension("/x/SHOW.HTML.ERB", &exts).unwrap().ext, ".html.erb", "the longest suffix wins");
    assert_eq!(match_configured_extension("/x/a.erb", &exts).unwrap().ext, ".erb");
    assert!(match_configured_extension("/x/.hbs", &exts).is_none(), "bare dotfile name is not a template");
    assert!(match_configured_extension("/x/a.tsx", &exts).is_none());
    assert!(match_configured_extension("/x/a.hbs", &[]).is_none());
}
// ── cache ─────────────────────────────────────────────────────────────────

#[test]
fn cache_round_trip_and_gc() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let mut cache = read_cache(&cwd);
    assert_eq!(bump_edit_count(&mut cache, "s", "/x/a.tsx"), 1.0);
    assert_eq!(bump_edit_count(&mut cache, "s", "/x/a.tsx"), 2.0);
    let g = f("gradient-text", 3.0, "G", "d", "s");
    remember_findings(&mut cache, "s", "/x/a.tsx", &[g.clone()]);
    assert!(persist_cache(&r, &cwd, &cache));
    let back = read_cache(&cwd);
    let entry = &back["sessions"]["s"]["files"]["/x/a.tsx"];
    assert_eq!(entry["editCount"], json!(2));
    assert_eq!(entry["findings"], json!(["gradient-text:3"]));
    let mut cache2 = back.clone();
    assert!(dedupe_against_cache(&[g.clone()], &mut cache2, "s", "/x/a.tsx").is_empty());
    // same-line value-specific findings stay distinct
    let mut a = f("overused-font", 1.0, "O", "d", "font-family: Inter");
    a.extras.insert("ignoreValue".into(), json!("Inter"));
    let mut b = a.clone();
    b.extras.insert("ignoreValue".into(), json!("Roboto"));
    assert_ne!(finding_cache_key(&a), finding_cache_key(&b));
    // gc: 10 sessions -> newest 8 kept
    let mut big = read_cache(&cwd);
    for i in 0..10 {
        let s = ensure_session(&mut big, &format!("s{i}"));
        s.insert("updatedAt".into(), json!(1000 + i));
    }
    assert!(persist_cache(&r, &cwd, &big));
    let kept = read_cache(&cwd);
    let ids: Vec<&String> = kept["sessions"].as_object().unwrap().keys().collect();
    assert_eq!(ids.len(), 8, "11 sessions collapse to the 8 newest");
    for dropped in ["s0", "s1", "s2"] {
        assert!(!ids.contains(&&dropped.to_string()), "{dropped} is oldest");
    }
    assert_eq!(ids[0], "s", "the freshly touched session sorts first");
    assert_eq!(ids[1], "s9");
}

// ── filtering ─────────────────────────────────────────────────────────────

#[test]
fn filter_findings_rules_advisory_and_values() {
    let mut c = HookConfig::default();
    let em = f("em-dash-overuse", 1.0, "E", "d", "s");
    let side = f("side-tab", 1.0, "S", "d", "s");
    let mut font = f(
        "overused-font",
        2.0,
        "O",
        "d",
        "body { font-family: \"Inter\", sans-serif; }",
    );
    font.file = "/p/src/a.css".into();
    assert_eq!(
        filter_findings(vec![em.clone(), side.clone()], &c).len(),
        1,
        "advisory dropped by default"
    );
    c.advisory_rules = "include".into();
    assert_eq!(filter_findings(vec![em.clone(), side.clone()], &c).len(), 2);
    let mut scan = HookScanOptions::default();
    scan.decisions = std::rc::Rc::new(impeccino_detect::design_decisions::DesignDecisions {
        source: None,
        waived_rules: vec!["side-tab".into()],
        declared_fonts: vec!["inter".into()],
    });
    assert!(filter_findings_for(vec![side.clone(), font.clone()], &c, &scan).is_empty());
    scan.decisions = std::rc::Rc::new(impeccino_detect::design_decisions::DesignDecisions {
        declared_fonts: vec!["roboto".into()],
        ..Default::default()
    });
    assert_eq!(filter_findings_for(vec![font.clone()], &c, &scan).len(), 1, "a different declared font does not waive Inter");
    let (imm, def) =
        split_findings_by_tier(vec![f("gradient-text", 1.0, "G", "d", "s"), side.clone()]);
    assert_eq!(imm[0].antipattern, "gradient-text");
    assert_eq!(def[0].antipattern, "side-tab");
    assert!(per_edit_tiering_active(&HookConfig::default(), "claude"));
    assert!(!per_edit_tiering_active(&HookConfig::default(), "cursor"));
    assert!(!per_edit_tiering_active(&HookConfig::default(), "github"));
    let mut all = HookConfig::default();
    all.per_edit_rules = "all".into();
    assert!(!per_edit_tiering_active(&all, "claude"));
}

// ── rendering ─────────────────────────────────────────────────────────────

fn opts(cwd: &str) -> RenderOpts {
    RenderOpts {
        cwd: Some(cwd.to_string()),
        short_footer: false,
        reserve_chars: 0.0,
    }
}

#[test]
fn render_template_caps_and_footers() {
    let r = rt("/x");
    let c = HookConfig::default();
    let many: Vec<Finding> = (0..12)
        .map(|i| f("side-tab", (i + 1) as f64, &format!("R{i}"), "d", "s"))
        .collect();
    let text = render_template(&r, &many, "/x/Card.tsx", &c, &opts("/x"));
    assert!(text.starts_with(
        "[impeccino@1] Design hook findings requiring review in Card.tsx (12 issue(s)):"
    ));
    assert!(text.contains("... and 7 more (see /impeccino audit)."));
    assert_eq!(text.lines().filter(|l| l.starts_with("- L")).count(), 5);
    assert!(text.contains("`impeccino-disable-line <rule>: <who decided, and the evidence>` comment"));
    assert!(text.contains("Self-serve ends at the in-file waiver."));
    assert!(text.contains("`<!-- impeccino-disable <rule>: reason -->`"));
    let short = render_template(
        &r,
        &many[..1],
        "/x/Card.tsx",
        &c,
        &RenderOpts {
            short_footer: true,
            ..opts("/x")
        },
    );
    assert!(short.contains("Triage per the session policy"));
    assert!(short.contains("`impeccino-disable-line <rule>: <reason>` comment"));
    assert!(!short.contains("Triage each finding"));
    let zero = render_template(
        &r,
        &[f("side-tab", 0.0, "X", "d", "s")],
        "/x/a.tsx",
        &c,
        &opts("/x"),
    );
    assert!(zero.contains("\n- [side-tab] X. d\n"));
}

#[test]
fn render_template_dedupes_descriptions_and_quotes_hints() {
    let r = rt("/x");
    let c = HookConfig::default();
    let desc = "Long registry description that should appear once.";
    let text = render_template(
        &r,
        &[
            f(
                "overused-font",
                2.0,
                "Overused font",
                desc,
                "font-family: \"Roboto\"",
            ),
            f(
                "overused-font",
                9.0,
                "Overused font",
                desc,
                "font-family: \"Inter\"",
            ),
        ],
        "/x/fonts.css",
        &c,
        &opts("/x"),
    );
    assert_eq!(text.matches(desc).count(), 1);
    assert!(text.contains(
        "- L9 [overused-font] Overused font. If deliberate, declare `Inter` in DESIGN.md."
    ));
    assert!(text.contains("declare `Roboto` in DESIGN.md"));
    let bounce = render_template(
        &r,
        &[f(
            "bounce-easing",
            1.0,
            "Bounce",
            "d",
            "animation: bounce-ball",
        )],
        "/x/main.css",
        &c,
        &opts("/x"),
    );
    assert!(bounce.contains("If deliberate, waive the line: `impeccino-disable-line bounce-easing: <reason>`."));
    let hostile = render_template(
        &r,
        &[f(
            "overused-font",
            1.0,
            "Overused font",
            "d",
            "body { font-family: \"$(touch pwned)\", sans-serif; }",
        )],
        "/x/fonts.css",
        &c,
        &opts("/x"),
    );
    assert!(hostile.contains("declare `$(touch pwned)` in DESIGN.md."), "a hint, never a command line: {hostile}");
    let no_hint = {
        let mut x = f("side-tab", 1.0, "Side tab", "d", "s");
        x.extras.insert("ignoreValue".into(), json!("Inter"));
        render_template(&r, &[x], "/x/a.tsx", &c, &opts("/x"))
    };
    assert!(!no_hint.contains("If deliberate"));
    // platform quoting (#476 / #533)
    assert_eq!(
        quote_command_arg("Space Grotesk Var", false),
        "'Space Grotesk Var'"
    );
    assert_eq!(
        quote_command_arg("Space Grotesk Var", true),
        "\"Space Grotesk Var\""
    );
    assert_eq!(quote_command_arg("it's", false), "'it'\\''s'");
    assert_eq!(quote_command_arg("a\"b\\c", true), "\"a\\\"b\\\\c\"");
    assert_eq!(quote_command_arg("Inter", true), "Inter");
}

#[test]
fn render_template_clamps_inside_budget_and_keeps_policy() {
    let r = rt("/x");
    let mut c = HookConfig::default();
    c.limits.max_chars = 500.0;
    let long_path = format!("/x/{}Component.tsx", "deeply-nested/".repeat(6));
    let six: Vec<Finding> = (0..6)
        .map(|i| {
            f(
                "side-tab",
                (i + 1) as f64,
                "Side tab",
                "Colored side border.",
                "s",
            )
        })
        .collect();
    let text = render_template(
        &r,
        &six,
        &long_path,
        &c,
        &RenderOpts {
            reserve_chars: 134.0,
            ..opts("/x")
        },
    );
    assert!(text.chars().count() <= 500, "{}", text.chars().count());
    assert!(text.ends_with("unsure, ask in one line."));
    let huge: Vec<Finding> = (0..5)
        .map(|i| f("side-tab", (i + 1) as f64, "X", &"y".repeat(2000), "s"))
        .collect();
    let text = render_template(&r, &huge, "/x/a.tsx", &c, &opts("/x"));
    assert!(text.chars().count() <= 500);
    let one = render_template(&r, &huge[..1], "/x/a.tsx", &c, &opts("/x"));
    assert!(one.chars().count() <= 500);
    assert!(one.contains("[side-tab]"));
    assert!(one.contains("Triage per the session policy"));
    let three: Vec<Finding> = (1..=3)
        .map(|l| f("side-tab", l as f64, "X", "short issue", "s"))
        .collect();
    let text = render_template(&r, &three, "/x/a.tsx", &c, &opts("/x"));
    assert!(text.chars().count() <= 500);
    for l in ["- L1 ", "- L2 ", "- L3 "] {
        assert!(text.contains(l), "{l} survives");
    }
    assert!(text.contains("Triage per the session policy"));
    // grouped render: global cap of 5 across files, per-file "more" line
    let groups = vec![
        Group {
            file_path: "/x/a.css".into(),
            findings: (1..=4)
                .map(|l| f("side-tab", l as f64, "X", "d", "s"))
                .collect(),
        },
        Group {
            file_path: "/x/b.css".into(),
            findings: (1..=3)
                .map(|l| f("gradient-text", l as f64, "G", "d", "s"))
                .collect(),
        },
    ];
    let text = render_grouped_template(&r, &groups, &HookConfig::default(), &opts("/x"));
    assert!(text.starts_with("[impeccino@1] Design hook findings requiring review across 2 files (7 issue(s)):\na.css (4 issue(s)):\n"));
    assert!(text.contains("b.css (3 issue(s)):\n- L1 [gradient-text] G. d\n- ... 2 more in b.css (see /impeccino audit)."));
    let pending = render_pending_ack(
        &r,
        "/x/a.css",
        &["a:1".into(), "b:2".into(), "c:3".into(), "d:4".into()],
        "/x",
    );
    assert!(pending
        .contains("Still has 4 finding(s) flagged earlier this session (a:1, b:2, c:3, +1 more)."));
    assert!(render_clean_ack(&r, "/x/a.css", "/x")
        .ends_with("keep following the project design system and the impeccino skill guidance."));
}

// ── events / targets ──────────────────────────────────────────────────────

#[test]
fn harness_detection_and_github_normalization() {
    let r = rt("/p");
    let gh: Map<String, Value> = json!({"sessionId": "g1", "toolName": "edit", "toolArgs": "{\"path\":\"src/a.tsx\",\"old_str\":\"a\"}"})
        .as_object()
        .cloned()
        .unwrap();
    assert_eq!(resolve_harness(&r, Some(&gh)), "github");
    let ev = normalize_hook_event(&r, &gh, "/p", "github");
    assert_eq!(ev["tool_input"]["file_path"], json!("src/a.tsx"));
    assert_eq!(ev["session_id"], json!("g1"));
    assert_eq!(ev["cwd"], json!("/p"));
    let patch: Map<String, Value> = json!({"sessionId": "g1", "toolName": "apply_patch", "toolArgs": "*** Begin Patch\n*** Add File: /abs/app.css\n+x\n*** End Patch"})
        .as_object()
        .cloned()
        .unwrap();
    let ev = normalize_hook_event(&r, &patch, "/p", "github");
    assert_eq!(ev["tool_name"], json!("apply_patch"));
    assert_eq!(resolve_target_files(&r, &ev, "/p"), vec!["/abs/app.css"]);
    // an edit whose content carries patch markers is still an edit
    let tricky: Map<String, Value> = json!({"toolName": "edit", "toolArgs": "{\"path\":\"/p/x.css\",\"new_str\":\"*** Begin Patch\"}"})
        .as_object()
        .cloned()
        .unwrap();
    let ev = normalize_hook_event(&r, &tricky, "/p", "github");
    assert_eq!(ev["tool_input"]["file_path"], json!("/p/x.css"));
    let cursor: Map<String, Value> = json!({"conversation_id": "c", "workspace_roots": ["/w"], "tool_input": {"path": "src/App.jsx"}})
        .as_object()
        .cloned()
        .unwrap();
    assert_eq!(resolve_harness(&r, Some(&cursor)), "cursor");
    let ev = normalize_hook_event(&r, &cursor, "/p", "cursor");
    assert_eq!(ev["cwd"], json!("/w"));
    assert_eq!(ev["session_id"], json!("c"));
    assert_eq!(ev["tool_input"]["file_path"], json!("src/App.jsx"));
    // c9e7cd8a: an explicit codex harness now keeps its own identity so the
    // Stop pass can emit the Codex decision/block contract.
    let forced = rt_with("/p", env(&[("IMPECCINO_HOOK_HARNESS", "codex")]));
    assert_eq!(resolve_harness(&forced, Some(&gh)), "codex");
    assert_eq!(
        parse_apply_patch_paths(&r, "*** Begin Patch\n*** Update File: a.css\r\n*** Add File: /abs/b.css\n*** Delete File: c.css\n", "/p"),
        // A relative patch path is resolved against the cwd with the host's
        // path semantics; an already-absolute one is passed through.
        vec![jsp::join(&["/p", "a.css"]), "/abs/b.css".to_string()]
    );
    assert_eq!(
        payload("t", "Stop", "claude"),
        r#"{"hookSpecificOutput":{"hookEventName":"Stop","additionalContext":"t"}}"#
    );
    assert_eq!(
        payload("t", "PostToolUse", "cursor"),
        r#"{"additional_context":"t"}"#
    );
    assert_eq!(
        payload("t", "PostToolUse", "github"),
        r#"{"additionalContext":"t"}"#
    );
}

#[test]
fn expand_scan_targets_follows_styles() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let app = t.write(
        "src/App.jsx",
        "import './theme.scss';\nimport styles from \"./App.module.less\";\nexport default 1;\n",
    );
    t.write("src/styles.css", "a{}");
    t.write("src/index.sass", "a{}");
    t.write("src/theme.scss", "a{}");
    t.write("src/App.module.less", "a{}");
    let out = expand_scan_targets(&r, &[app.clone()], &cwd);
    assert_eq!(out[0], app);
    for name in [
        "src/theme.scss",
        "src/App.module.less",
        "src/styles.css",
        "src/index.sass",
    ] {
        assert!(out.contains(&jsp::join(&[&cwd, name])), "{name} in {out:?}");
    }
    let rel = expand_scan_targets(&r, &["src/App.jsx".into()], &cwd);
    assert_eq!(
        rel[0], app,
        "relative primaries resolve against the project cwd"
    );
    let trav = expand_scan_targets(&r, &[format!("{cwd}/src/../src/App.jsx")], &cwd);
    assert_eq!(
        trav.len(),
        1,
        "traversal-looking primaries are not expanded"
    );
    let css = t.write("src/main.css", "a{}");
    assert_eq!(expand_scan_targets(&r, &[css.clone()], &cwd), vec![css]);
    assert!(expand_scan_targets(&r, &[], &cwd).is_empty());
    let capped: Vec<String> = (0..9).map(|i| format!("{cwd}/f{i}.css")).collect();
    assert_eq!(
        normalize_scan_targets(&r, &capped, &cwd).len(),
        MAX_SCAN_TARGETS
    );
}

// ── run_hook ──────────────────────────────────────────────────────────────

#[test]
fn run_hook_fresh_then_pending_then_stop() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let css = t.write("src/a.css", &format!("{GRADIENT_CSS}{SIDE_TAB_CSS}"));
    let ev = edit_event(&cwd, &css, "s1");
    let one = hook::run_hook(&r, &ev);
    assert!(one
        .stdout
        .contains("Design hook findings requiring review in src/a.css (1 issue(s))"));
    assert!(
        one.stdout.contains("[gradient-text]"),
        "immediate tier only"
    );
    assert!(!one.stdout.contains("[side-tab]"));
    assert_eq!(one.audit["deferred"], json!(1));
    assert_eq!(one.audit["freshFindings"], json!(1));
    assert!(t.has_cache());
    assert!(!t.exists(".impeccino"), "hook state never lands in the project");
    let two = hook::run_hook(&r, &ev);
    assert!(two
        .stdout
        .contains("Still has 1 finding(s) flagged earlier this session (gradient-text:1)"));
    assert_eq!(audit_str(&two.audit, "kind"), Some("pending"));
    assert!(two
        .stdout
        .contains("Handle them before finalizing — the previous reminder still applies."));
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(stop.stdout.contains("\"hookEventName\":\"Stop\""));
    assert!(
        stop.stdout.contains("[side-tab]"),
        "deep pass surfaces the deferred tier"
    );
    assert!(
        !stop.stdout.contains("[gradient-text]"),
        "already reported per edit"
    );
    assert!(
        stop.stdout.contains("Triage per the session policy"),
        "short footer after the first fire"
    );
    // 3c442af7: the Stop pass now syncs the remembered set to the live scan
    // (including the per-edit-surfaced gradient-text), so a second Stop with
    // nothing new is silent instead of re-reporting it.
    let again = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(again.stdout, "", "second Stop is clean: {}", again.stdout);
    assert_eq!(audit_str(&again.audit, "skipped"), Some("stop-clean"));
    // a session whose deep pass found everything is silent on the next Stop
    let css2 = t.write("src/b.css", SIDE_TAB_CSS);
    hook::run_hook(&r, &edit_event(&cwd, &css2, "s2"));
    assert!(hook::run_stop_hook(&r, &stop_event(&cwd, "s2"))
        .stdout
        .contains("[side-tab]"));
    assert_eq!(
        audit_str(
            &hook::run_stop_hook(&r, &stop_event(&cwd, "s2")).audit,
            "skipped"
        ),
        Some("stop-clean")
    );
    let active = hook::run_stop_hook(&r, &json!({"session_id": "s1", "cwd": cwd, "hook_event_name": "Stop", "stop_hook_active": true}).to_string());
    assert_eq!(
        audit_str(&active.audit, "skipped"),
        Some("stop-hook-active")
    );
    let none = hook::run_stop_hook(&r, &stop_event(&cwd, "nobody"));
    assert_eq!(audit_str(&none.audit, "skipped"), Some("no-touched-files"));
}

#[test]
fn run_hook_scans_catch_all_route_files() {
    let (_t, cwd, route) = catch_all_route_fixture();
    let r = rt(&cwd);
    let res = hook::run_hook(&r, &edit_event(&cwd, &route, "s1"));
    assert_ne!(audit_str(&res.audit, "skipped"), Some("sensitive"), "{}", res.stdout);
    assert!(res.stdout.contains("[gradient-text]"), "{}", res.stdout);
}

#[test]
fn stop_scans_catch_all_route_files() {
    let (_t, cwd, route) = catch_all_route_fixture();
    let r = rt(&cwd);
    let mut cache = read_cache(&cwd);
    touch_file(&mut cache, "s1", &route);
    persist_cache(&r, &cwd, &cache);
    let stop = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(stop.stdout.contains("[gradient-text]"), "{}", stop.stdout);
}

#[test]
fn run_hook_acks_and_quiet_modes() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let clean = t.write("src/Clean.tsx", "export const A = () => <p>hi</p>;\n");
    let ev = edit_event(&cwd, &clean, "s1");
    let one = hook::run_hook(&r, &ev);
    assert!(one
        .stdout
        .contains("No deterministic design-quality issues found"));
    assert!(
        !t.has_cache(),
        "a clean edit with no session cache yet writes nothing"
    );
    assert!(persist_cache(&r, &cwd, &read_cache(&cwd)));
    let one = hook::run_hook(&r, &ev);
    assert_eq!(audit_str(&one.audit, "kind"), Some("clean"));
    let two = hook::run_hook(&r, &ev);
    assert_eq!(audit_str(&two.audit, "skipped"), Some("clean-ack-deduped"));
    assert!(two.stdout.is_empty());
    let ts = t.write("src/util.ts", "export const x = 1;\n");
    let three = hook::run_hook(&r, &edit_event(&cwd, &ts, "s1"));
    assert_eq!(audit_str(&three.audit, "skipped"), Some("non-ui-ack"));
    let ts_slop = t.write("src/slop.ts", "const s = `.t{background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent;}`;\n");
    let four = hook::run_hook(&r, &edit_event(&cwd, &ts_slop, "s1"));
    assert!(
        four.stdout.contains("[gradient-text]"),
        "findings still surface for .ts"
    );
    let quiet = rt_with(&cwd, env(&[("IMPECCINO_HOOK_QUIET", "1")]));
    let q = hook::run_hook(&quiet, &edit_event(&cwd, &clean, "s2"));
    assert_eq!(q.audit["quiet"], json!(true));
    assert!(q.stdout.is_empty());
    let re = rt_with(&cwd, env(&[("CLAUDE_HOOK_DEPTH", "2")]));
    assert_eq!(hook::run_hook(&re, &ev).audit["reentrant"], json!(true));
    let off = rt_with(&cwd, env(&[("IMPECCINO_HOOK_DISABLED", "yes")]));
    assert_eq!(
        audit_str(&hook::run_hook(&off, &ev).audit, "skipped"),
        Some("env-disabled")
    );
    assert_eq!(
        audit_str(&hook::run_hook(&r, "").audit, "skipped"),
        Some("stdin-malformed")
    );
    assert_eq!(
        audit_str(&hook::run_hook(&r, "[1]").audit, "skipped"),
        Some("stdin-empty")
    );
}

#[test]
fn run_hook_skips_unsafe_and_foreign_targets() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let go = |file: &str| hook::run_hook(&r, &edit_event(&cwd, file, "s1"));
    assert_eq!(
        audit_str(&go(&format!("{cwd}/.env.local")).audit, "skipped"),
        Some("sensitive")
    );
    assert_eq!(
        audit_str(&go(&format!("{cwd}/dist/a.css")).audit, "skipped"),
        Some("generated")
    );
    assert_eq!(
        audit_str(&go(&format!("{cwd}/src/../a.css")).audit, "skipped"),
        Some("sensitive")
    );
    assert_eq!(
        audit_str(&go(&format!("{cwd}/README.md")).audit, "skipped"),
        Some("extension")
    );
    assert_eq!(
        audit_str(&go(&format!("{cwd}/src/nope.tsx")).audit, "skipped"),
        Some("file-missing")
    );
    let scratch = Tmp::new();
    let outside = scratch.write("landing.css", GRADIENT_CSS);
    assert_eq!(
        audit_str(&go(&outside).audit, "skipped"),
        Some("outside-project")
    );
    assert!(!t.has_cache());
    // template extensions (#316): server templates go to the HTML engine by
    // default, no config needed
    let blade = t.write("views/a.blade.php", "<style>.t{background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent;}</style>");
    let res = go(&blade);
    assert_ne!(audit_str(&res.audit, "skipped"), Some("extension"));
    assert_eq!(audit_str(&res.audit, "ext"), Some(".blade.php"));
    // the project's git metadata keeps files out: .gitignore, and
    // .gitattributes linguist-generated / linguist-vendored
    std::fs::create_dir_all(t.0.join(".git")).unwrap();
    t.write(".gitignore", "scratch/\n");
    t.write(".gitattributes", "src/api.css linguist-generated\nthird_party/** linguist-vendored\n");
    for rel in ["scratch/a.css", "src/api.css", "third_party/lib/a.css"] {
        let file = t.write(rel, GRADIENT_CSS);
        assert_eq!(audit_str(&go(&file).audit, "skipped"), Some("git-ignored"), "{rel}");
    }
    let own = t.write("src/own.css", GRADIENT_CSS);
    assert!(go(&own).stdout.contains("[gradient-text]"));
}

#[cfg(unix)]
#[test]
fn run_hook_symlinked_cwd_and_umbrella_launch() {
    let t = Tmp::new();
    let root = t.path();
    let r = rt(&root);
    t.write("realproj/package.json", "{}");
    let file = t.write(
        "realproj/src/Card.tsx",
        "export const A = () => <p>hi</p>;\n",
    );
    let link = format!("{root}/proj-link");
    std::os::unix::fs::symlink(format!("{root}/realproj"), &link).unwrap();
    let res = hook::run_hook(&r, &edit_event(&link, &file, "sym"));
    assert_ne!(audit_str(&res.audit, "skipped"), Some("outside-project"));
    assert!(res
        .stdout
        .contains("No deterministic design-quality issues found"));
    // umbrella: cwd has no marker; the child project gets the cache
    t.write("app/package.json", "{}");
    let css = t.write("app/src/a.css", GRADIENT_CSS);
    let res = hook::run_hook(&r, &edit_event(&root, &css, "u1"));
    assert!(res.stdout.contains("Design hook findings requiring review"));
    assert_eq!(
        audit_str(&res.audit, "cwd"),
        Some(format!("{root}/app").as_str())
    );
    let app_cache = get_cache_path(&format!("{root}/app"));
    assert!(std::path::Path::new(&app_cache).exists());
    assert_eq!(registered_project_roots(&read_cache(&root), "u1"), vec![format!("{root}/app")]);
    let _ = std::fs::remove_dir_all(jsp::dirname(&app_cache));
}

#[test]
fn run_hook_oversized_files_and_suppression() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let big = t.write("bundle.js", &format!("/* {} */", "x".repeat(200 * 1024)));
    let res = hook::run_hook(&r, &edit_event(&cwd, &big, "s1"));
    assert_eq!(audit_str(&res.audit, "skipped"), Some("too-large"));
    assert_eq!(res.audit["bytes"], json!(200 * 1024 + 6));
    let main = t.write(
        "main.css",
        &format!("/* {} */\n{GRADIENT_CSS}", "x".repeat(90 * 1024)),
    );
    let res = hook::run_hook(&r, &edit_event(&cwd, &main, "s1"));
    assert_eq!(res.audit["emitted"], json!(true));
    // the byte count never rides along on another file's audit line
    let small = t.write("a.css", "a{}");
    let patch = json!({"session_id": "p", "cwd": cwd, "hook_event_name": "PostToolUse", "tool_name": "apply_patch",
        "tool_input": {"command": format!("*** Begin Patch\n*** Update File: {big}\n*** Update File: {small}\n*** End Patch")}})
    .to_string();
    let res = hook::run_hook(&r, &patch);
    assert!(res.audit.get("bytes").is_none(), "{:?}", res.audit);
    // suppression: the 7th edit emits the notice once, later edits stay silent
    let css = t.write("src/b.css", GRADIENT_CSS);
    let mut outputs = Vec::new();
    for _ in 0..9 {
        outputs.push(hook::run_hook(&r, &edit_event(&cwd, &css, "sup")));
    }
    assert!(outputs[6].stdout.contains("Suppressing further design hints on src/b.css. More than 6 edits in this session reached. Run /impeccino audit to revisit."));
    assert_eq!(outputs[6].audit["suppressed"], json!(true));
    assert!(outputs[7].stdout.is_empty());
    assert_eq!(outputs[8].audit["emitted"], json!(false));
    assert_eq!(outputs[8].audit["editCount"], json!(9));
}

#[test]
fn run_hook_co_located_styles_and_tiering_config() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let app = t.write(
        "src/App.jsx",
        "export default () => <div className=\"card\" />;\n",
    );
    t.write("src/styles.css", &format!("{SIDE_TAB_CSS}{GRADIENT_CSS}"));
    let res = hook::run_hook(&r, &edit_event(&cwd, &app, "s1"));
    assert!(
        res.stdout
            .contains("Design hook findings requiring review in src/styles.css (1 issue(s))"),
        "{}",
        res.stdout
    );
    let cache = read_cache(&cwd);
    assert_eq!(
        cache["sessions"]["s1"]["files"][&app]["editCount"],
        json!(1)
    );
    assert_eq!(
        cache["sessions"]["s1"]["files"][jsp::join(&[&cwd, "src/styles.css"])]["editCount"],
        json!(0),
        "co-scanned styles do not bump"
    );
    // the github harness has no Stop pass, so it keeps the full set per edit
    let gh = rt_with(&cwd, env(&[("IMPECCINO_HOOK_HARNESS", "github")]));
    let res = hook::run_hook(&gh, &edit_event(&cwd, &app, "s3"));
    assert!(res.stdout.starts_with("{\"additionalContext\":"));
    assert!(res.stdout.contains("[side-tab]"));
    // .gitignore keeps a file out; a DESIGN.md waiver turns a rule off
    std::fs::create_dir_all(t.0.join(".git")).unwrap();
    t.write(".gitignore", "src/\n");
    let res = hook::run_hook(&r, &edit_event(&cwd, &app, "s4"));
    assert_eq!(audit_str(&res.audit, "skipped"), Some("git-ignored"));
    std::fs::remove_file(t.0.join(".gitignore")).unwrap();
    t.write("DESIGN.md", "# Design\n\n<!-- impeccino-disable gradient-text -- the brand mark is a gradient -->\n");
    let res = hook::run_hook(&r, &edit_event(&cwd, &app, "s5"));
    assert!(
        res.stdout
            .contains("No deterministic design-quality issues found"),
        "{}",
        res.stdout
    );
    // native platform gate
    t.write("PRODUCT.md", "# P\n\n## Platform\nios and android\n");
    let res = hook::run_hook(&r, &edit_event(&cwd, &app, "s6"));
    assert_eq!(audit_str(&res.audit, "skipped"), Some("native-platform"));
    assert_eq!(audit_str(&res.audit, "platform"), Some("adaptive"));
}

#[test]
fn write_audit_log_targets() {
    let t = Tmp::new();
    let cwd = t.path();
    let mut entry = Map::new();
    entry.insert("event".into(), json!("PostToolUse"));
    entry.insert("cwd".into(), json!(cwd));
    let r = rt_with(
        "/elsewhere",
        env(&[("IMPECCINO_HOOK_LOG", "logs/a.ndjson")]),
    );
    assert!(write_audit_log(&r, &entry, "/elsewhere"));
    let line = t.read("logs/a.ndjson");
    assert!(
        line.starts_with("{\"ts\":\"")
            && line.contains("\",\"event\":\"PostToolUse\",\"cwd\":\"")
            && line.ends_with("\"}\n")
    );
    let r2 = rt("/elsewhere");
    assert!(
        !write_audit_log(&r2, &entry, "/elsewhere"),
        "no target, no-op"
    );
    let r3 = rt_with("/elsewhere", env(&[("IMPECCINO_HOOK_LOG", "~/h.ndjson"), ("HOME", &cwd)]));
    assert!(write_audit_log(&r3, &entry, "/elsewhere"));
    assert!(t.exists("h.ndjson"));
}

// ── hook-before-edit ──────────────────────────────────────────────────────

fn hbe(r: &Runtime, stdin: &str) -> (String, i32) {
    let (mut io, cap) = Io::captured("", PathBuf::from(&r.proc_cwd), r.env.clone());
    let code = before_edit::run(r, stdin, &mut io);
    drop(io);
    let out = String::from_utf8(cap.stdout.borrow().clone()).unwrap();
    drop(cap);
    (out, code)
}

fn cursor(cwd: &str, tool: &str, input: Value) -> String {
    json!({"hook_event_name": "preToolUse", "conversation_id": "cv1", "workspace_roots": [cwd], "tool_name": tool, "tool_input": input}).to_string()
}

#[test]
fn before_edit_uses_the_touched_apps_platform_inside_a_workspace() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/native/package.json", "{}");
    t.write("apps/native/PRODUCT.md", "# Product\n\n## Platform\nios and android\n");
    t.write("apps/web/package.json", "{}");
    let r = rt(&root);
    let slop = ".t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n";

    let (native, code) = hbe(
        &r,
        &cursor(
            &root,
            "Write",
            json!({"file_path": "apps/native/src/new.css", "content": slop}),
        ),
    );
    assert_eq!(code, 0);
    assert!(native.starts_with("{\"permission\":\"allow\""), "{native}");

    let (web, code) = hbe(
        &r,
        &cursor(
            &root,
            "Write",
            json!({"file_path": "apps/web/src/new.css", "content": slop}),
        ),
    );
    assert_eq!(code, 0);
    assert!(web.starts_with("{\"permission\":\"deny\""), "{web}");

    for app in ["native", "web"] {
        let app_root = jsp::join(&[&root, "apps", app]);
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
    }
}

#[test]
fn post_edit_applies_native_platform_per_project_in_one_patch() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/native/package.json", "{}");
    t.write("apps/native/PRODUCT.md", "# Product\n\n## Platform\nios and android\n");
    t.write("apps/web/package.json", "{}");
    let native = t.write("apps/native/src/native.css", GRADIENT_CSS);
    let web = t.write("apps/web/src/web.css", GRADIENT_CSS);
    let patch = format!("*** Begin Patch\n*** Update File: {native}\n*** Update File: {web}\n*** End Patch\n");
    let event = json!({
        "session_id": "mixed",
        "cwd": root,
        "hook_event_name": "PostToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch}
    })
    .to_string();

    let out = hook::run_hook(&rt(&root), &event);
    assert!(out.stdout.contains("apps/web/src/web.css"), "{}", out.stdout);
    assert!(!out.stdout.contains("apps/native/src/native.css"), "{}", out.stdout);
    assert!(out.stdout.contains("[gradient-text]"), "{}", out.stdout);
    assert_eq!(audit_str(&out.audit, "skipped"), None);
    assert_eq!(audit_str(&out.audit, "cwd"), Some(jsp::join(&[&root, "apps", "web"]).as_str()));
    assert_eq!(
        registered_project_roots(&read_cache(&root), "mixed"),
        vec![jsp::join(&[&root, "apps", "web"])]
    );
    let web_root = jsp::join(&[&root, "apps", "web"]);
    let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&web_root)));
}

#[test]
fn post_edit_resolves_native_platform_per_target_without_package_marker() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/native/PRODUCT.md", "# Product\n\n## Platform\nios and android\n");
    let native = t.write("apps/native/src/native.css", GRADIENT_CSS);
    let web = t.write("apps/web/src/web.css", GRADIENT_CSS);
    let patch = format!("*** Begin Patch\n*** Update File: {native}\n*** Update File: {web}\n*** End Patch\n");
    let event = json!({
        "session_id": "mixed-target-platform",
        "cwd": root,
        "hook_event_name": "PostToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch}
    })
    .to_string();

    let out = hook::run_hook(&rt(&root), &event);
    assert!(out.stdout.contains("apps/web/src/web.css"), "{}", out.stdout);
    assert!(!out.stdout.contains("apps/native/src/native.css"), "{}", out.stdout);
    assert_eq!(audit_str(&out.audit, "skipped"), None);
}

#[test]
fn multi_project_post_edit_shares_target_expansion_budget() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    let mut primaries = Vec::new();
    let mut styles = Vec::new();
    for app in ["a", "b"] {
        t.write(&format!("apps/{app}/package.json"), "{}");
        for page in ["One", "Two"] {
            let primary = t.write(
                &format!("apps/{app}/src/{page}.jsx"),
                &format!("import './{page}-theme.css';\nexport default 1;\n"),
            );
            primaries.push(primary.clone());
            styles.push(t.write(
                &format!("apps/{app}/src/{page}-theme.css"),
                GRADIENT_CSS,
            ));
        }
    }
    let patch = format!(
        "*** Begin Patch\n{}*** End Patch\n",
        primaries
            .iter()
            .map(|file| format!("*** Update File: {file}\n"))
            .collect::<String>()
    );
    let event = json!({
        "session_id": "shared-target-budget",
        "cwd": root,
        "hook_event_name": "PostToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch}
    })
    .to_string();

    let out = hook::run_hook(&rt(&root), &event);
    assert_eq!(out.audit["freshFiles"], json!(2), "{}", out.stdout);
    assert!(out.stdout.contains("apps/a/src/One-theme.css"), "{}", out.stdout);
    assert!(out.stdout.contains("apps/a/src/Two-theme.css"), "{}", out.stdout);
    assert!(!out.stdout.contains("apps/b/src/One-theme.css"), "{}", out.stdout);
    assert!(!out.stdout.contains("apps/b/src/Two-theme.css"), "{}", out.stdout);
}

#[test]
fn multi_project_post_edit_shares_rendered_finding_budget() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/a/package.json", "{}");
    t.write("apps/b/package.json", "{}");
    let mut files = Vec::new();
    for index in 0..5 {
        files.push(t.write(&format!("apps/a/src/app-{index}.css"), GRADIENT_CSS));
    }
    files.push(t.write("apps/b/src/app.css", GRADIENT_CSS));
    let patch = format!(
        "*** Begin Patch\n{}*** End Patch\n",
        files
            .iter()
            .map(|file| format!("*** Update File: {file}\n"))
            .collect::<String>()
    );
    let event = json!({
        "session_id": "shared-finding-budget",
        "cwd": root,
        "hook_event_name": "PostToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch}
    })
    .to_string();

    let out = hook::run_hook(&rt(&root), &event);
    let message = hook_message(&out.stdout);
    assert_eq!(out.audit["freshFindings"], json!(6), "{message}");
    let visible_findings = message
        .lines()
        .filter(|line| line.starts_with("- L") || line.starts_with("- ["))
        .count();
    assert_eq!(visible_findings, 5, "{message}");
    assert!(message.contains("- Real design problem:"), "{message}");
    assert!(message.contains("More touched-project findings were omitted"), "{message}");
    assert!(message.contains("Triage each finding"), "{message}");
    assert!(!message.contains("apps/b/src/app.css"), "{message}");
}

#[test]
fn multi_project_post_edit_caps_actual_payload_at_8000_utf16_chars() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    // A stale-sidecar note repeats the configured command path. This makes
    // real child-rendered messages large enough to exercise the outer join
    // budget without depending on platform-specific maximum path lengths.
    let runtime = rt_with_command(&root, format!("/{}", "x".repeat(6_500)));
    let mut files = Vec::new();
    for index in 0..3 {
        let app = format!("app-{index}");
        t.write(&format!("apps/{app}/package.json"), "{}");
        if index == 0 {
            t.write("apps/app-0/DESIGN.md", "---\nname: Test\n---\n");
            let sidecar = t.write("apps/app-0/DESIGN.json", "{}");
            std::fs::File::options()
                .write(true)
                .open(sidecar)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000))
                .unwrap();
        }
        files.push(t.write(&format!("apps/{app}/src/app.css"), GRADIENT_CSS));
    }
    let patch = format!(
        "*** Begin Patch\n{}*** End Patch\n",
        files
            .iter()
            .map(|file| format!("*** Update File: {file}\n"))
            .collect::<String>()
    );
    let event = json!({
        "session_id": "post-char-budget",
        "cwd": root,
        "hook_event_name": "PostToolUse",
        "tool_name": "apply_patch",
        "tool_input": {"command": patch}
    })
    .to_string();

    let out = hook::run_hook(&runtime, &event);
    let message = hook_message(&out.stdout);
    assert_eq!(out.audit["freshFindings"], json!(3), "{}", out.stdout);
    assert!(message.contains("DESIGN.md is newer"), "{message}");
    assert!(message.encode_utf16().count() <= 8_000, "{} chars", message.encode_utf16().count());
    assert!(message.contains("apps/app-0/src/app.css"), "{message}");
    assert!(!message.contains("apps/app-1/src/app.css"), "{message}");
    assert!(message.contains("More touched-project findings were omitted"), "{message}");
    assert!(message.contains("Triage each finding"), "{message}");
    for index in 0..3 {
        let app_root = jsp::join(&[&root, &format!("apps/app-{index}")]);
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
    }
}

#[test]
fn stop_resolves_native_platform_per_target_without_package_marker() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/native/PRODUCT.md", "# Product\n\n## Platform\nios and android\n");
    let native = t.write("apps/native/src/native.css", GRADIENT_CSS);
    let web = t.write("apps/web/src/web.css", GRADIENT_CSS);
    let mut cache = read_cache(&root);
    touch_file(&mut cache, "mixed-target-platform", &native);
    touch_file(&mut cache, "mixed-target-platform", &web);
    persist_cache(&rt(&root), &root, &cache);

    let stop = hook::run_stop_hook(&rt(&root), &stop_event(&root, "mixed-target-platform"));
    assert!(stop.stdout.contains("apps/web/src/web.css"), "{}", stop.stdout);
    assert!(!stop.stdout.contains("apps/native/src/native.css"), "{}", stop.stdout);
    assert_eq!(stop.audit["scannedFiles"], json!(1), "{}", stop.stdout);
}

#[test]
fn stop_skips_touched_files_over_the_per_file_byte_limit() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", "{}");
    let content = format!("{}{}", " ".repeat(131_073), SIDE_TAB_CSS);
    let file = t.write("src/oversized.css", &content);
    let mut cache = read_cache(&root);
    touch_file(&mut cache, "oversized-stop", &file);
    assert!(persist_cache(&rt(&root), &root, &cache));

    let stop = hook::run_stop_hook(&rt(&root), &stop_event(&root, "oversized-stop"));
    assert_eq!(stop.audit["scannedFiles"], json!(0), "{}", stop.stdout);
    assert!(!stop.stdout.contains("[side-tab]"), "{}", stop.stdout);
}

#[test]
fn stop_shares_rendered_finding_budget_across_project_roots() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    let runtime = rt(&root);
    for index in 0..6 {
        let app_root = jsp::join(&[&root, &format!("apps/app-{index}")]);
        t.write(&format!("apps/app-{index}/package.json"), "{}");
        let file = t.write(&format!("apps/app-{index}/src/app.css"), GRADIENT_CSS);
        let mut cache = read_cache(&app_root);
        touch_file(&mut cache, "stop-finding-budget", &file);
        assert!(persist_project_cache(
            &runtime,
            &app_root,
            &root,
            &mut cache,
            "stop-finding-budget",
        ));
    }

    let stop = hook::run_stop_hook(&runtime, &stop_event(&root, "stop-finding-budget"));
    let message = hook_message(&stop.stdout);
    let visible_findings = message
        .lines()
        .filter(|line| line.starts_with("- L") || line.starts_with("- ["))
        .count();
    assert_eq!(stop.audit["freshFindings"], json!(6), "{message}");
    assert_eq!(visible_findings, 5, "{message}");
    assert!(message.contains("- Real design problem:"), "{message}");
    for index in 0..6 {
        let app_root = jsp::join(&[&root, &format!("apps/app-{index}")]);
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
    }
}

#[test]
fn stop_caps_actual_payload_at_8000_utf16_chars_across_project_roots() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    let runtime = rt_with_command(&root, format!("/{}", "x".repeat(5_700)));
    for index in 0..3 {
        let app_root = jsp::join(&[&root, &format!("apps/app-{index}")]);
        t.write(&format!("apps/app-{index}/package.json"), "{}");
        if index == 0 {
            t.write("apps/app-0/DESIGN.md", "---\nname: Test\n---\n");
            let sidecar = t.write("apps/app-0/DESIGN.json", "{}");
            std::fs::File::options()
                .write(true)
                .open(sidecar)
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000))
                .unwrap();
        }
        let file = t.write(&format!("apps/app-{index}/src/app.css"), SIDE_TAB_CSS);
        if index == 0 {
            assert!(design_system_options_for_file(
                &runtime,
                &read_config(&app_root),
                &app_root,
                &file,
            )
            .md_newer_than_json());
        }
        let mut cache = read_cache(&app_root);
        touch_file(&mut cache, "stop-char-budget", &file);
        assert!(persist_project_cache(
            &runtime,
            &app_root,
            &root,
            &mut cache,
            "stop-char-budget",
        ));
    }

    let stop = hook::run_stop_hook(&runtime, &stop_event(&root, "stop-char-budget"));
    let message = hook_message(&stop.stdout);
    assert_eq!(stop.audit["freshFindings"], json!(3), "{}", stop.stdout);
    assert!(message.contains("DESIGN.md is newer"), "{message}");
    assert!(message.encode_utf16().count() <= 8_000, "{} chars", message.encode_utf16().count());
    assert!(message.contains("apps/app-0/src/app.css"), "{message}");
    assert!(!message.contains("apps/app-2/src/app.css"), "{message}");
    assert!(message.contains("More touched-project findings were omitted"), "{message}");
    assert!(message.contains("Triage each finding"), "{message}");
    for index in 0..3 {
        let app_root = jsp::join(&[&root, &format!("apps/app-{index}")]);
        let _ = std::fs::remove_dir_all(jsp::dirname(&get_cache_path(&app_root)));
    }
}

#[test]
fn before_edit_resolves_native_platform_per_target_without_package_marker() {
    let t = Tmp::new();
    let root = t.path();
    t.write("package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("apps/native/PRODUCT.md", "# Product\n\n## Platform\nios and android\n");
    let r = rt(&root);
    let slop = ".t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n";

    let (native, code) = hbe(
        &r,
        &cursor(
            &root,
            "Write",
            json!({"file_path": "apps/native/src/new.css", "content": slop}),
        ),
    );
    assert_eq!(code, 0);
    assert!(native.starts_with("{\"permission\":\"allow\""), "{native}");

    let (web, code) = hbe(
        &r,
        &cursor(
            &root,
            "Write",
            json!({"file_path": "apps/web/src/new.css", "content": slop}),
        ),
    );
    assert_eq!(code, 0);
    assert!(web.starts_with("{\"permission\":\"deny\""), "{web}");
}

#[test]
fn before_edit_skips_oversized_proposed_content() {
    // Over-cap proposed content (a huge paste or hostile envelope arriving
    // via stdin) must skip the gate instead of being scanned, the same
    // fail-open shape as an unreadable original (triage A3). The envelope
    // carries slop that denies at normal size, so an allow proves the cap
    // fired rather than the scan passing.
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let slop = ".t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n";
    let (small_out, _) = hbe(
        &r,
        &cursor(&cwd, "Write", json!({"file_path": "src/x.css", "content": slop})),
    );
    assert!(small_out.starts_with("{\"permission\":\"deny\""), "{small_out}");
    let big = format!("{slop}/*{}*/\n", "a".repeat(2 * 1024 * 1024));
    let (out, code) = hbe(
        &r,
        &cursor(&cwd, "Write", json!({"file_path": "src/x.css", "content": big})),
    );
    assert_eq!(code, 0);
    assert_eq!(out, "{\"permission\":\"allow\"}");
}

#[test]
fn before_edit_scans_catch_all_route_files() {
    let (_t, cwd, route) = catch_all_route_fixture();
    let r = rt(&cwd);
    let (out, code) = hbe(
        &r,
        &cursor(
            &cwd,
            "Write",
            json!({"file_path": route, "content": CATCH_ALL_ROUTE_SOURCE}),
        ),
    );
    assert_eq!(code, 0);
    assert!(out.starts_with("{\"permission\":\"deny\""), "{out}");
    assert!(out.contains("[gradient-text]"), "{out}");
}

#[test]
fn before_edit_denies_shell_and_edit_shapes() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let slop = ".t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n";
    let deny = |stdin: &str, label: &str| {
        let (out, code) = hbe(&r, stdin);
        assert_eq!(code, 0);
        assert!(out.starts_with("{\"permission\":\"deny\",\"user_message\":\"[impeccino@1] Impeccino design hook blocked this write before it landed. Design hook findings requiring review in"), "{label}: {out}");
        out
    };
    let allow = |stdin: &str, label: &str| {
        let (out, _) = hbe(&r, stdin);
        assert_eq!(out, "{\"permission\":\"allow\"}", "{label}");
    };
    deny(
        &cursor(
            &cwd,
            "Shell",
            json!({"command": format!("cat > src/x.css <<'EOF'\n{slop}EOF\n")}),
        ),
        "heredoc",
    );
    deny(
        &cursor(
            &cwd,
            "Shell",
            json!({"command": format!("python3 - <<'PY'\nfrom pathlib import Path\nPath(\"src/y.css\").write_text(\"\"\"{slop}\"\"\")\nPY\n")}),
        ),
        "python heredoc",
    );
    deny(
        &cursor(
            &cwd,
            "Shell",
            json!({"command": format!("cat >> src/z.css <<EOF\n{slop}EOF")}),
        ),
        "append redirect",
    );
    deny(
        &cursor(
            &cwd,
            "Shell",
            json!({"command": format!("cat <<'EOF' | tee src/t.css\n{slop}EOF\n")}),
        ),
        "tee",
    );
    t.write("src/src.css", slop);
    deny(
        &cursor(
            &cwd,
            "Shell",
            json!({"command": "cp -f src/src.css src/copy.css"}),
        ),
        "cp",
    );
    let orig = t.write("src/e.css", ".card { color: #111; }\n");
    deny(
        &cursor(
            &cwd,
            "StrReplace",
            json!({"path": orig, "old_string": ".card {", "new_string": format!("{slop}.card {{")}),
        ),
        "edit projection",
    );
    allow(
        &cursor(&cwd, "Edit", json!({"path": orig, "new_string": "x"})),
        "fragment-only",
    );
    allow(
        &cursor(
            &cwd,
            "Edit",
            json!({"path": orig, "old_string": "NOPE", "new_string": "x"}),
        ),
        "old string missing",
    );
    allow(
        &cursor(&cwd, "Shell", json!({"command": "echo hi > src/q.css"})),
        "redirect without content",
    );
    allow(
        &cursor(&cwd, "Write", json!({"path": "src/new.css", "content": ""})),
        "empty content",
    );
    allow(
        &cursor(
            &cwd,
            "Write",
            json!({"path": "src/data.json", "content": "{}"}),
        ),
        "non-ui",
    );
    allow(&cursor(&cwd, "Shell", json!({"command": "ls"})), "no file");
    allow("", "empty stdin");
    allow("{", "malformed stdin");
    let off = rt_with(&cwd, env(&[("IMPECCINO_HOOK_DISABLED", "true")]));
    let (out, _) = hbe(&off, "{");
    assert_eq!(out, "{\"permission\":\"allow\"}");
    // repeated identical denials downgrade to allow-with-warning after the threshold
    let write = cursor(
        &cwd,
        "Write",
        json!({"path": "src/new.css", "content": slop}),
    );
    let mut last = String::new();
    for _ in 0..7 {
        last = hbe(&r, &write).0;
    }
    assert!(
        last.starts_with("{\"permission\":\"allow\",\"user_message\":\""),
        "{last}"
    );
    assert!(last.contains("This is the 7th repeated denial for the same file and finding signature, so Impeccino is allowing this write to avoid a loop."));
    let cache = read_cache(&cwd);
    assert_eq!(
        cache["sessions"]["cv1"]["files"][jsp::join(&[&cwd, "src/new.css"])]["cursorDenials"]
            ["gradient-text:1"],
        json!(7)
    );
    assert_eq!(cache["sessions"]["cv1"]["footerShown"], json!(true));
}

// ── hook-admin ────────────────────────────────────────────────────────────

fn admin_run(r: &Runtime, args: &[&str]) -> (String, String, i32) {
    let (mut io, cap) = Io::captured("", PathBuf::from(&r.proc_cwd), r.env.clone());
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let code = admin::run(r, &argv, &mut io);
    drop(io);
    let out = String::from_utf8(cap.stdout.borrow().clone()).unwrap();
    let err = String::from_utf8(cap.stderr.borrow().clone()).unwrap();
    drop(cap);
    (out, err, code)
}

fn assert_claude_launcher_entry(entry: &Value, unix_command: &str, launcher_suffix: &str) {
    if cfg!(windows) {
        assert_eq!(entry["command"], json!("powershell.exe"));
        let args = entry["args"].as_array().unwrap();
        assert_eq!(args[0], json!("-NoProfile"));
        assert_eq!(args[1], json!("-Command"));
        let script = args[2].as_str().unwrap();
        assert!(script.contains(launcher_suffix), "{script}");
        assert!(script.contains("Test-Path -LiteralPath"), "{script}");
        assert!(impeccino_context::hook_markers::is_launcher_design_hook_command(script), "{script}");
    } else {
        assert_eq!(entry["command"], json!(unix_command));
        assert!(entry.get("args").is_none());
    }
}

#[test]
fn admin_retired_ignore_actions_point_at_design_md() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    for action in ["ignore-value", "ignore-rule", "ignore-file"] {
        let (out, err, code) = admin_run(&r, &[action, "overused-font", "Inter"]);
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(err.starts_with(&format!("\"{action}\" was removed: Impeccino keeps no config file.")), "{err}");
        assert!(err.contains("<!-- impeccino-disable <rule>: reason --> in DESIGN.md"), "{err}");
    }
    let (_, err, code) = admin_run(&r, &["bogus"]);
    assert_eq!(code, 1);
    assert_eq!(err, "Unknown action: bogus\nValid: status, on, off, reset\n");
    let (out, _, _) = admin_run(&r, &["reset"]);
    assert_eq!(out, "No hook entries or cache to remove.\n");
    assert!(!t.exists(".impeccino"), "admin writes no config");
}
#[test]
fn admin_on_off_status_and_reset_follow_the_manifests() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write(".git/keep", "");
    let (out, _, _) = admin_run(&r, &["status"]);
    assert!(out.contains("installed:    no (run /impeccino hooks on to install)\n"), "{out}");
    assert!(out.contains("DESIGN.md:    not present (no project waivers)\n"), "{out}");
    install_test_skill(&t, ".github/skills/impeccino");
    let (out, _, _) = admin_run(&r, &["on"]);
    assert_eq!(out, "Installed or repaired hook manifests for: .github.\n");
    assert!(t
        .read(".github/hooks/impeccino.json")
        .contains("\"matcher\": \"edit|create|apply_patch\""));
    let (out, _, _) = admin_run(&r, &["on"]);
    assert_eq!(out, "Hook manifests already installed for: .github.\n");
    t.write("DESIGN.md", "# D\n\n<!-- impeccino-disable side-tab -- ledger rails -->\n");
    let (out, _, _) = admin_run(&r, &["status"]);
    assert!(out.contains(".github/hooks/impeccino.json"), "{out}");
    assert!(out.contains("DESIGN.md:    DESIGN.md (waived rules: side-tab; declared fonts: none)\n"), "{out}");
    assert!(!t.exists(".impeccino"), "no config, no consent record");
    let (out, _, _) = admin_run(&r, &["off"]);
    assert_eq!(out, "Removed hook entries from: .github.\n");
    assert!(!t.exists(".github/hooks/impeccino.json"));
    let (out, _, _) = admin_run(&r, &["off"]);
    assert_eq!(out, "No local hook entries to remove.\n");
    // reset also clears the session cache in the user cache
    admin_run(&r, &["on"]);
    assert!(persist_cache(&r, &cwd, &read_cache(&cwd)));
    assert!(t.has_cache());
    let (out, _, _) = admin_run(&r, &["reset"]);
    assert!(out.starts_with("Removed hook entries from: .github. Cleared the hook's session cache ("), "{out}");
    assert!(!t.has_cache());
    // a hook in the team-shared Claude settings is named, never edited
    t.write(
        ".claude/settings.json",
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"\"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook"}]}]}}"#,
    );
    let (out, _, _) = admin_run(&r, &["off"]);
    assert!(out.contains("Still installed in .claude/settings.json, which the team shares"), "{out}");
    assert!(t.read(".claude/settings.json").contains("impeccino"));
}
#[test]
fn admin_on_prunes_local_manifest_when_shared_settings_carry_the_hook() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    install_test_skill(&t, ".claude/skills/impeccino");
    t.write(
        ".claude/settings.json",
        r#"{"hooks":{"PostToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"node \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/hook.mjs\""}]}]}}"#,
    );
    t.write(
        ".claude/settings.local.json",
        "{\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(ls)\"\n    ]\n  },\n  \"hooks\": {\n    \"PostToolUse\": [\n      {\n        \"matcher\": \"Edit\",\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"node old/skills/impeccino/scripts/hook.mjs\"\n          }\n        ]\n      }\n    ]\n  }\n}\n",
    );
    let (out, _, _) = admin_run(&r, &["on"]);
    assert!(
        out.ends_with("Hook manifests already installed for: .claude.\n"),
        "{out}"
    );
    let local: Value = serde_json::from_str(&t.read(".claude/settings.local.json")).unwrap();
    assert!(
        local.get("hooks").is_none(),
        "impeccino entries pruned, empty hooks dropped: {local}"
    );
    assert_eq!(local["permissions"]["allow"], json!(["Bash(ls)"]));
    // a local manifest holding only impeccino entries is deleted outright
    t.write(
        ".claude/settings.local.json",
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"node x/skills/impeccino/scripts/hook.mjs"}]}]}}"#,
    );
    admin_run(&r, &["on"]);
    assert!(!t.exists(".claude/settings.local.json"));
}

#[test]
fn admin_on_writes_launcher_manifests_for_every_harness() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write(".git/keep", "");
    for skill in [
        ".claude/skills/impeccino",
        ".agents/skills/impeccino",
        ".cursor/skills/impeccino",
        ".github/skills/impeccino",
    ] {
        install_test_skill(&t, skill);
    }
    let (out, _, code) = admin_run(&r, &["on"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("Installed or repaired hook manifests for: .claude, .agents, .cursor, .github.\n"), "{out}");

    let claude: Value = serde_json::from_str(&t.read(".claude/settings.local.json")).unwrap();
    let cmd = "if [ -x \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" ]; then \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook; fi";
    assert_claude_launcher_entry(
        &claude["hooks"]["PostToolUse"][0]["hooks"][0],
        cmd,
        ".claude/skills/impeccino/scripts/impeccino.cmd",
    );
    assert_claude_launcher_entry(
        &claude["hooks"]["Stop"][0]["hooks"][0],
        cmd,
        ".claude/skills/impeccino/scripts/impeccino.cmd",
    );
    assert!(claude["hooks"]["PostToolUse"][0]["hooks"][0].get("commandWindows").is_none());
    assert!(!t.read(".claude/settings.local.json").contains("node "));

    let codex: Value = serde_json::from_str(&t.read(".codex/hooks.json")).unwrap();
    let entry = &codex["hooks"]["PostToolUse"][0]["hooks"][0];
    assert_eq!(entry["command"], json!("if [ -x \"$(git rev-parse --show-toplevel)/.agents/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.agents/skills/impeccino/scripts/impeccino\" hook; fi"));
    assert_eq!(entry["commandWindows"], json!("for /f \"delims=\" %i in ('git rev-parse --show-toplevel') do if exist \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" hook"));
    assert_eq!(
        entry.as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
        vec!["type", "command", "commandWindows", "timeout", "statusMessage"]
    );
    let stop = &codex["hooks"]["Stop"][0]["hooks"][0];
    assert_eq!(stop["command"], json!("if [ -x \"$(git rev-parse --show-toplevel)/.agents/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.agents/skills/impeccino/scripts/impeccino\" hook; fi"));
    assert_eq!(stop["commandWindows"], json!("for /f \"delims=\" %i in ('git rev-parse --show-toplevel') do if exist \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" hook"));
    assert_eq!(stop["timeout"], json!(30));

    let cursor: Value = serde_json::from_str(&t.read(".cursor/hooks.json")).unwrap();
    assert_eq!(
        cursor["hooks"]["preToolUse"][0]["command"],
        json!("if [ -x \"$(git rev-parse --show-toplevel)/.cursor/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.cursor/skills/impeccino/scripts/impeccino\" hook-before-edit; fi")
    );
    let github: Value = serde_json::from_str(&t.read(".github/hooks/impeccino.json")).unwrap();
    assert_eq!(
        github["hooks"]["postToolUse"][0]["bash"],
        json!("if [ -x \"$(git rev-parse --show-toplevel)/.github/skills/impeccino/scripts/impeccino\" ]; then \"$(git rev-parse --show-toplevel)/.github/skills/impeccino/scripts/impeccino\" hook; fi")
    );

    // A second `on` is a no-op against the manifests it just wrote.
    let (out, _, _) = admin_run(&r, &["on"]);
    assert!(out.ends_with("Hook manifests already installed for: .claude, .agents, .cursor, .github.\n"), "{out}");
}

#[test]
fn admin_on_repairs_legacy_mjs_manifests_to_the_launcher_form() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write(".git/keep", "");
    install_test_skill(&t, ".claude/skills/impeccino");
    install_test_skill(&t, ".agents/skills/impeccino");
    // JS-era Claude manifest with a foreign entry alongside the impeccino one.
    t.write(
        ".claude/settings.local.json",
        r#"{"permissions":{"allow":["Bash(ls)"]},"hooks":{"PostToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"echo other"}]},{"matcher":"Edit|Write|MultiEdit","hooks":[{"type":"command","command":"node \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/hook.mjs\"","timeout":5}]}],"Stop":[{"hooks":[{"type":"command","command":"[ ! -f '/x/.claude/skills/impeccino/scripts/hook.mjs' ] || node '/x/.claude/skills/impeccino/scripts/hook.mjs'"}]}]}}"#,
    );
    // JS-era Codex manifest carrying the CLI installer's commandWindows sibling.
    t.write(
        ".codex/hooks.json",
        r#"{"hooks":{"PostToolUse":[{"matcher":"Edit|Write|apply_patch","hooks":[{"type":"command","command":"[ ! -f \".agents/skills/impeccino/scripts/hook.mjs\" ] || node \".agents/skills/impeccino/scripts/hook.mjs\"","commandWindows":"if exist \".agents/skills/impeccino/scripts/hook.mjs\" (node \".agents/skills/impeccino/scripts/hook.mjs\" & exit /b)","timeout":5}]}]}}"#,
    );
    let (out, _, code) = admin_run(&r, &["on"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with("Installed or repaired hook manifests for: .claude, .agents.\n"), "{out}");

    let claude = t.read(".claude/settings.local.json");
    assert!(!claude.contains(".mjs"), "legacy entries replaced: {claude}");
    let claude: Value = serde_json::from_str(&claude).unwrap();
    assert_eq!(claude["permissions"]["allow"], json!(["Bash(ls)"]));
    let post = claude["hooks"]["PostToolUse"].as_array().unwrap();
    assert_eq!(post.len(), 2, "foreign entry kept, impeccino entry replaced once: {post:?}");
    assert_eq!(post[0]["hooks"][0]["command"], json!("echo other"));
    assert_claude_launcher_entry(
        &post[1]["hooks"][0],
        "if [ -x \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" ]; then \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook; fi",
        ".claude/skills/impeccino/scripts/impeccino.cmd",
    );
    // Upstream 611147a3: the repaired Claude group must exist and carry the
    // current Edit|Write matcher (MultiEdit is retired; see 55fb8e8).
    assert_eq!(post[1]["matcher"], json!("Edit|Write"));
    assert_eq!(claude["hooks"]["Stop"].as_array().unwrap().len(), 1);
    assert_claude_launcher_entry(
        &claude["hooks"]["Stop"][0]["hooks"][0],
        "if [ -x \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" ]; then \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook; fi",
        ".claude/skills/impeccino/scripts/impeccino.cmd",
    );

    let codex = t.read(".codex/hooks.json");
    assert!(!codex.contains(".mjs"), "{codex}");
    let codex: Value = serde_json::from_str(&codex).unwrap();
    assert_eq!(codex["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
    assert_eq!(
        codex["hooks"]["PostToolUse"][0]["hooks"][0]["commandWindows"],
        json!("for /f \"delims=\" %i in ('git rev-parse --show-toplevel') do if exist \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" \"%i\\.agents\\skills\\impeccino\\scripts\\impeccino.cmd\" hook")
    );

    // A launcher-form manifest written by another checkout is recognized as
    // ours too: shared settings carrying it make `on` prune the local file.
    t.write(
        ".claude/settings.json",
        r#"{"hooks":{"PostToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"\"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook"}]}]}}"#,
    );
    let (out, _, _) = admin_run(&r, &["on"]);
    assert!(out.contains("already installed for: .claude"), "{out}");
    let local = t.read(".claude/settings.local.json");
    assert!(!local.contains("skills/impeccino"), "launcher-form entries pruned: {local}");
    let local: Value = serde_json::from_str(&local).unwrap();
    assert_eq!(local["hooks"]["PostToolUse"][0]["hooks"][0]["command"], json!("echo other"));
    assert!(local["hooks"].get("Stop").is_none());
    assert_eq!(local["permissions"]["allow"], json!(["Bash(ls)"]));
}

#[test]
fn admin_on_without_a_supported_project_launcher_fails_actionably() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("No supported project-local Impeccino launcher"), "{err}");
    assert!(!t.exists(".claude/settings.local.json"));
    assert!(!t.exists(".codex/hooks.json"));
}

#[test]
fn admin_rejects_percent_workspace_paths_only_when_codex_windows_command_is_needed() {
    let t = Tmp::new();
    let repo_marker = t.write("repo/.git/keep", "");
    let _repo_root = jsp::dirname(&repo_marker);
    t.write("repo/%workspace%/package.json", r#"{"workspaces":["apps/*"]}"#);
    let app = t.write("repo/%workspace%/apps/site/package.json", "{}");
    install_test_skill(&t, "repo/%workspace%/.agents/skills/impeccino");
    let r = rt(&jsp::dirname(&app));

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("path contains `%`"), "{err}");
    assert!(!t.exists("repo/%workspace%/.codex/hooks.json"));

    t.write("repo/%cursor-workspace%/package.json", r#"{"workspaces":["apps/*"]}"#);
    let cursor_app = t.write("repo/%cursor-workspace%/apps/site/package.json", "{}");
    install_test_skill(&t, "repo/%cursor-workspace%/.cursor/skills/impeccino");
    let r = rt(&jsp::dirname(&cursor_app));
    let (out, err, code) = admin_run(&r, &["on"]);
    assert_eq!(code, 0, "{err} {out}");
    assert!(t.exists("repo/%cursor-workspace%/.cursor/hooks.json"));
}

#[test]
fn admin_on_rejects_a_skill_folder_without_its_launcher() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    std::fs::create_dir_all(t.0.join(".github/skills/impeccino")).unwrap();

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("launcher") && err.contains(".github/skills/impeccino/scripts/impeccino"), "{err}");
    assert!(!t.exists(".github/hooks/impeccino.json"));
}

#[test]
fn admin_on_skips_malformed_claude_settings_without_replacing_them_or_their_backup() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    install_test_skill(&t, ".claude/skills/impeccino");
    t.write(".claude/settings.local.json", "{ broken");
    t.write(".claude/settings.local.json.bak", "keep this older backup");

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains(".claude/settings.local.json") && err.contains("valid JSON"), "{err}");
    assert_eq!(t.read(".claude/settings.local.json"), "{ broken");
    assert_eq!(t.read(".claude/settings.local.json.bak"), "keep this older backup");
}

#[test]
fn admin_on_preserves_non_object_claude_settings() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    install_test_skill(&t, ".claude/skills/impeccino");
    t.write(".claude/settings.local.json", "[]");

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains(".claude/settings.local.json") && err.contains("JSON object"), "{err}");
    assert_eq!(t.read(".claude/settings.local.json"), "[]");
    assert!(!t.exists(".claude/settings.local.json.bak"));
}

#[test]
fn admin_on_backs_up_non_object_dedicated_manifest_before_replacing_it() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write(".git/keep", "");
    install_test_skill(&t, ".github/skills/impeccino");
    t.write(".github/hooks/impeccino.json", "[]");

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 0, "{err}");
    assert!(
        out.replace('\\', "/")
            .contains("Backed up malformed manifest(s): .github/hooks/impeccino.json.bak"),
        "{out}"
    );
    assert_eq!(t.read(".github/hooks/impeccino.json.bak"), "[]");
    assert!(t.read(".github/hooks/impeccino.json").contains("impeccino"));
}

#[test]
fn admin_off_does_not_claim_success_for_a_malformed_manifest_with_an_impeccino_entry() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let malformed = r#"{"hooks":{"preToolUse":[{"command":"\".cursor/skills/impeccino/scripts/impeccino\" hook-before-edit"}]}} broken"#;
    t.write(".cursor/hooks.json", malformed);

    let (out, err, code) = admin_run(&r, &["off"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("malformed") && err.contains(".cursor/hooks.json"), "{err}");
    assert_eq!(t.read(".cursor/hooks.json"), malformed);
}

#[test]
fn admin_on_never_overwrites_an_existing_backup() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write(".git/keep", "");
    install_test_skill(&t, ".cursor/skills/impeccino");
    t.write(".cursor/hooks.json", "{ broken");
    t.write(".cursor/hooks.json.bak", "keep this older backup");

    let (out, _, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 0, "{out}");
    assert_eq!(t.read(".cursor/hooks.json.bak"), "keep this older backup");
    assert_eq!(t.read(".cursor/hooks.json.bak.1"), "{ broken");
    assert!(t.read(".cursor/hooks.json").contains("hook-before-edit"));
}

#[cfg(unix)]
#[test]
fn admin_on_refuses_to_replace_a_symlink_manifest() {
    use std::os::unix::fs::symlink;

    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write(".git/keep", "");
    install_test_skill(&t, ".github/skills/impeccino");
    let outside = t.write("outside.json", "{\"hooks\":{}}\n");
    std::fs::create_dir_all(t.0.join(".github/hooks")).unwrap();
    symlink(&outside, t.0.join(".github/hooks/impeccino.json")).unwrap();

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("symlink"), "{err}");
    assert!(std::fs::symlink_metadata(t.0.join(".github/hooks/impeccino.json")).unwrap().file_type().is_symlink());
    assert_eq!(t.read("outside.json"), "{\"hooks\":{}}\n");
}

#[test]
fn admin_on_with_global_claude_skill_writes_only_the_current_projects_local_settings() {
    let t = Tmp::new();
    let repo = t.write("repo/.git/keep", "");
    let repo_root = jsp::dirname(&repo);
    let home = jsp::join(&[&t.path(), "home with spaces"]);
    let skill_dir = jsp::join(&[&home, ".claude/skills/impeccino"]);
    install_test_skill(&t, "home with spaces/.claude/skills/impeccino");
    let r = rt_with(&repo_root, env(&[("HOME", &home), ("USERPROFILE", &home), ("IMPECCINO_SKILL_DIR", &skill_dir)]));

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 0, "{err}");
    assert!(out.contains(".claude"), "{out}");
    assert!(t.exists("repo/.claude/settings.local.json"), "{out} {err}");
    let manifest: Value = serde_json::from_str(&t.read("repo/.claude/settings.local.json")).unwrap();
    let entry = &manifest["hooks"]["PostToolUse"][0]["hooks"][0];
    if cfg!(windows) {
        assert_eq!(entry["command"], json!("powershell.exe"));
        let args = entry["args"].as_array().unwrap();
        assert_eq!(args[0], json!("-NoProfile"));
        assert_eq!(args[1], json!("-Command"));
        let script = args[2].as_str().unwrap();
        let launcher = PathBuf::from(&skill_dir).join("scripts/impeccino.cmd");
        let launcher = launcher.to_string_lossy().replace('\\', "/");
        assert!(script.contains(&launcher), "{script}");
        assert!(!script.contains("\\\\?\\"), "extended path leaked into command: {script}");
        assert!(script.contains("Test-Path -LiteralPath"), "{script}");
        assert!(script.contains("hook"), "{script}");
        assert!(impeccino_context::hook_markers::is_launcher_design_hook_command(script));
    } else {
        let command = entry["command"].as_str().unwrap();
        let canonical_skill_dir = std::fs::canonicalize(&skill_dir).unwrap().to_string_lossy().replace('\\', "/");
        let canonical_skill_dir = canonical_skill_dir.strip_prefix("//?/").unwrap_or(&canonical_skill_dir);
        assert!(command.replace('\\', "/").contains(canonical_skill_dir), "{command}");
    }
    assert!(!t.exists("home with spaces/.claude/settings.json"));

    let (status, _, code) = admin_run(&r, &["status"]);
    assert_eq!(code, 0);
    assert!(status.contains(".claude/settings.local.json"), "{status}");
    let (out, _, code) = admin_run(&r, &["on"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("already installed for: .claude"), "{out}");
    let stable: Value = serde_json::from_str(&t.read("repo/.claude/settings.local.json")).unwrap();
    assert_eq!(stable["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);

    let (out, _, code) = admin_run(&r, &["off"]);
    assert_eq!(code, 0, "{out}");
    assert!(!t.exists("repo/.claude/settings.local.json"));
    assert!(!t.exists("home with spaces/.claude/settings.json"));
}

#[test]
fn admin_on_does_not_guess_a_harness_for_a_global_agents_skill() {
    let t = Tmp::new();
    let repo = t.write("repo/.git/keep", "");
    let repo_root = jsp::dirname(&repo);
    let home = jsp::join(&[&t.path(), "home"]);
    let skill_dir = jsp::join(&[&home, ".agents/skills/impeccino"]);
    install_test_skill(&t, "home/.agents/skills/impeccino");
    let r = rt_with(&repo_root, env(&[("HOME", &home), ("USERPROFILE", &home), ("IMPECCINO_SKILL_DIR", &skill_dir)]));

    let (out, err, code) = admin_run(&r, &["on"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("supported project-local") || err.contains("ambiguous"), "{err}");
    assert!(!t.exists("repo/.claude/settings.local.json"));
    assert!(!t.exists("repo/.codex/hooks.json"));
    assert!(!t.exists("home/.codex/hooks.json"));
}

#[test]
fn admin_actions_from_a_nested_directory_use_the_repository_manifests() {
    let t = Tmp::new();
    let repo_marker = t.write("repo/keep", "");
    init_test_git(&jsp::dirname(&repo_marker));
    t.write("repo/workspace/package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("repo/workspace/apps/site/package.json", "{}");
    install_test_skill(&t, "repo/workspace/.agents/skills/impeccino");
    install_test_skill(&t, "repo/workspace/.cursor/skills/impeccino");
    install_test_skill(&t, "repo/workspace/.github/skills/impeccino");
    let nested = t.write("repo/workspace/apps/site/src/keep", "");
    let nested_cwd = jsp::dirname(&nested);
    let r = rt(&nested_cwd);
    let workspace_root = jsp::join(&[&t.path(), "repo/workspace"]);
    let workspace = rt(&workspace_root);

    for launcher in [
        "repo/workspace/.agents/skills/impeccino/scripts/impeccino",
        "repo/workspace/.cursor/skills/impeccino/scripts/impeccino",
    ] {
        t.write(launcher, "#!/bin/sh\nprintf '%s' \"$1\" > \"$HOOK_TEST_MARKER\"\n");
    }

    let (out, err, code) = admin_run(&r, &["on"]);
    assert_eq!(code, 0, "{err} {out}");
    assert!(t.exists("repo/workspace/.github/hooks/impeccino.json"));
    assert!(t.exists("repo/workspace/.codex/hooks.json"));
    assert!(t.exists("repo/workspace/.cursor/hooks.json"));
    assert!(!t.exists("repo/workspace/apps/site/.github/hooks/impeccino.json"));

    let (out, err, code) = admin_run(&workspace, &["on"]);
    assert_eq!(code, 0, "{err} {out}");
    assert!(out.contains("Hook manifests already installed for:"), "{out}");
    let (workspace_status, _, workspace_code) = admin_run(&workspace, &["status"]);
    let (nested_status, _, nested_code) = admin_run(&r, &["status"]);
    assert_eq!(workspace_code, 0);
    assert_eq!(nested_code, 0);
    for manifest in [".github/hooks/impeccino.json", ".codex/hooks.json", ".cursor/hooks.json"] {
        assert!(workspace_status.contains(manifest), "{workspace_status}");
        assert!(nested_status.contains(manifest), "{nested_status}");
    }

    #[cfg(unix)]
    {
        use std::process::Command;
        for (manifest, command_field, marker_name, expected) in [
            ("repo/workspace/.codex/hooks.json", "command", "codex-hook-ran", "hook"),
            ("repo/workspace/.cursor/hooks.json", "command", "cursor-hook-ran", "hook-before-edit"),
        ] {
            let parsed: Value = serde_json::from_str(&t.read(manifest)).unwrap();
            let command = if manifest.contains("codex") {
                parsed["hooks"]["PostToolUse"][0]["hooks"][0][command_field].as_str().unwrap()
            } else {
                parsed["hooks"]["preToolUse"][0][command_field].as_str().unwrap()
            };
            let marker = jsp::join(&[&t.path(), marker_name]);
            let status = Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&nested_cwd)
                .env("HOOK_TEST_MARKER", &marker)
                .status()
                .unwrap();
            assert!(status.success(), "{command}");
            assert_eq!(std::fs::read_to_string(marker).unwrap(), expected, "{command}");

            let launcher = if manifest.contains("codex") {
                "repo/workspace/.agents/skills/impeccino/scripts/impeccino"
            } else {
                "repo/workspace/.cursor/skills/impeccino/scripts/impeccino"
            };
            std::fs::remove_file(t.0.join(launcher)).unwrap();
            let marker = jsp::join(&[&t.path(), marker_name]);
            std::fs::remove_file(&marker).unwrap();
            let skipped = Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&nested_cwd)
                .env("HOOK_TEST_MARKER", &marker)
                .status()
                .unwrap();
            assert!(skipped.success(), "missing launcher should be a silent no-op: {command}");
            assert!(!Path::new(&marker).exists(), "missing launcher should not run: {command}");
        }
    }

    let (out, _, code) = admin_run(&workspace, &["off"]);
    assert_eq!(code, 0, "{out}");
    assert!(!t.exists("repo/workspace/.github/hooks/impeccino.json"));
    assert!(!t.exists("repo/workspace/.codex/hooks.json"));
    assert!(!t.exists("repo/workspace/.cursor/hooks.json"));
    let (workspace_status, _, workspace_code) = admin_run(&workspace, &["status"]);
    let (nested_status, _, nested_code) = admin_run(&r, &["status"]);
    assert_eq!(workspace_code, 0);
    assert_eq!(nested_code, 0);
    assert!(workspace_status.contains("installed:    no"), "{workspace_status}");
    assert!(nested_status.contains("installed:    no"), "{nested_status}");
    let (out, _, code) = admin_run(&r, &["off"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, "No local hook entries to remove.\n");
}

#[cfg(windows)]
#[test]
fn admin_codex_command_windows_executes_from_nested_cwd_and_skips_missing_launcher() {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let t = Tmp::new();
    let repo_marker = t.write("repo/keep", "");
    let repo_root = jsp::dirname(&repo_marker);
    init_test_git(&repo_root);
    install_test_skill(&t, "repo/.agents/skills/impeccino");
    let launcher = t.write(
        "repo/.agents/skills/impeccino/scripts/impeccino.cmd",
        "@echo off\r\necho %1> \"%HOOK_TEST_MARKER%\"\r\nexit /b 0\r\n",
    );
    let nested = t.write("repo/apps/site/src/keep", "");
    let nested_cwd = jsp::dirname(&nested);
    let rt = rt(&nested_cwd);
    let (out, err, code) = admin_run(&rt, &["on"]);
    assert_eq!(code, 0, "{err} {out}");

    let manifest: Value = serde_json::from_str(&t.read("repo/.codex/hooks.json")).unwrap();
    let command = manifest["hooks"]["PostToolUse"][0]["hooks"][0]["commandWindows"]
        .as_str()
        .unwrap();
    let marker = jsp::join(&[&t.path(), "codex-hook-ran"]);
    let status = Command::new("cmd.exe")
        .args(["/d", "/s", "/c"])
        .raw_arg(command)
        .current_dir(&nested_cwd)
        .env("HOOK_TEST_MARKER", &marker)
        .status()
        .unwrap();
    assert!(status.success(), "{command}");
    assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), "hook");

    std::fs::remove_file(launcher).unwrap();
    std::fs::remove_file(&marker).unwrap();
    let status = Command::new("cmd.exe")
        .args(["/d", "/s", "/c"])
        .raw_arg(command)
        .current_dir(&nested_cwd)
        .env("HOOK_TEST_MARKER", &marker)
        .status()
        .unwrap();
    assert!(status.success(), "missing launcher should be a silent no-op: {command}");
    assert!(!Path::new(&marker).exists());
}

#[cfg(windows)]
#[test]
fn admin_global_claude_exec_form_is_detected_and_runs_with_powershell_args() {
    use std::process::Command;

    let t = Tmp::new();
    let repo_marker = t.write("repo/.git/keep", "");
    let repo_root = jsp::dirname(&repo_marker);
    let home = jsp::join(&[&t.path(), "home with spaces"]);
    let skill_dir = jsp::join(&[&home, ".claude/skills/impeccino"]);
    install_test_skill(&t, "home with spaces/.claude/skills/impeccino");
    let launcher = t.write(
        "home with spaces/.claude/skills/impeccino/scripts/impeccino.cmd",
        "@echo off\r\necho %1> \"%HOOK_TEST_MARKER%\"\r\nexit /b 0\r\n",
    );
    let test_env = env(&[
        ("HOME", &home),
        ("USERPROFILE", &home),
        ("IMPECCINO_SKILL_DIR", &skill_dir),
    ]);
    let runtime = rt_with(&repo_root, test_env.clone());
    let (out, err, code) = admin_run(&runtime, &["on"]);
    assert_eq!(code, 0, "{err} {out}");
    let manifest: Value = serde_json::from_str(&t.read("repo/.claude/settings.local.json")).unwrap();
    let entry = &manifest["hooks"]["PostToolUse"][0]["hooks"][0];
    assert_eq!(entry["command"], json!("powershell.exe"));
    let args = entry["args"].as_array().unwrap();
    let args = args.iter().map(|value| value.as_str().unwrap()).collect::<Vec<_>>();
    let marker = jsp::join(&[&t.path(), "claude-hook-ran"]);
    let status = Command::new("powershell.exe")
        .args(&args)
        .env("HOOK_TEST_MARKER", &marker)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read_to_string(&marker).unwrap().trim(), "hook");

    let context = impeccino_context::context::load_context(
        &repo_root,
        &impeccino_context::target_args::TargetOptions::default(),
        &test_env,
    );
    let provider = impeccino_context::provider::detect(&test_env, &repo_root);
    assert_eq!(
        impeccino_context::context_cli::automatic_hook_mode(&context, &repo_root, &test_env, &provider),
        "stop",
        "context must recognize the exec-form launcher in args",
    );

    std::fs::remove_file(launcher).unwrap();
    std::fs::remove_file(&marker).unwrap();
    let status = Command::new("powershell.exe")
        .args(&args)
        .env("HOOK_TEST_MARKER", &marker)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!Path::new(&marker).exists());
}

#[test]
fn admin_off_from_child_finds_workspace_manifest_after_skill_removal() {
    let t = Tmp::new();
    let repo_marker = t.write("repo/keep", "");
    let repo_root = jsp::dirname(&repo_marker);
    init_test_git(&repo_root);
    t.write("repo/workspace/package.json", r#"{"workspaces":["apps/*"]}"#);
    t.write("repo/workspace/apps/site/package.json", "{}");
    install_test_skill(&t, "repo/workspace/.github/skills/impeccino");
    let workspace_root = jsp::join(&[&t.path(), "repo/workspace"]);
    let nested = t.write("repo/workspace/apps/site/src/keep", "");
    let nested_cwd = jsp::dirname(&nested);
    let workspace = rt(&workspace_root);
    let nested = rt(&nested_cwd);

    let (out, err, code) = admin_run(&workspace, &["on"]);
    assert_eq!(code, 0, "{err} {out}");
    assert!(t.exists("repo/workspace/.github/hooks/impeccino.json"));
    std::fs::remove_dir_all(t.0.join("repo/workspace/.github/skills/impeccino")).unwrap();

    let (status, _, code) = admin_run(&nested, &["status"]);
    assert_eq!(code, 0);
    assert!(status.contains(".github/hooks/impeccino.json"), "{status}");
    let (out, err, code) = admin_run(&nested, &["on"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("No supported project-local"), "{err}");
    let (out, err, code) = admin_run(&nested, &["off"]);
    assert_eq!(code, 0, "{err} {out}");
    assert!(out.contains("Removed hook entries from: .github"), "{out}");
    assert!(!t.exists("repo/workspace/.github/hooks/impeccino.json"));

    let (workspace_status, _, workspace_code) = admin_run(&workspace, &["status"]);
    let (nested_status, _, nested_code) = admin_run(&nested, &["status"]);
    assert_eq!(workspace_code, 0);
    assert_eq!(nested_code, 0);
    assert!(workspace_status.contains("installed:    no"), "{workspace_status}");
    assert!(nested_status.contains("installed:    no"), "{nested_status}");
}

#[test]
fn admin_reset_reports_state_file_delete_errors() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let pending = get_pending_path(&cwd);
    std::fs::create_dir_all(&pending).unwrap();

    let (out, err, code) = admin_run(&r, &["reset"]);

    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("could not remove hook state file"), "{err}");
    assert!(err.contains(&pending), "{err}");
}

// ── Grok Build + Codex (#646, #603, upstream 35ae0733/bfe634e2/3c442af7/c9e7cd8a) ──

const MIXED_CSS: &str = ".title { background: linear-gradient(90deg, #f472b6, #a78bfa); -webkit-background-clip: text; color: transparent; }\n.card { border-left: 4px solid #6366f1; border-radius: 8px; }\n";

#[test]
fn grok_and_codex_harness_detection() {
    let r = rt("/p");
    // Grok Build camelCase envelope wins over the old GitHub heuristic.
    let grok = json!({
        "hookEventName": "post_tool_use", "sessionId": "g1", "cwd": "/w",
        "toolName": "str_replace", "toolInput": { "file_path": "src/a.css" },
    })
    .as_object()
    .cloned()
    .unwrap();
    assert_eq!(resolve_harness(&r, Some(&grok)), "grok");
    // GitHub Copilot still classifies by toolArgs.
    let gh = json!({ "toolName": "str_replace_editor", "toolArgs": "{}" }).as_object().cloned().unwrap();
    assert_eq!(resolve_harness(&r, Some(&gh)), "github");
    // A Stop envelope with only hookEventName is Grok too.
    let grok_stop = json!({ "hookEventName": "stop", "sessionId": "g1", "cwd": "/w" }).as_object().cloned().unwrap();
    assert_eq!(resolve_harness(&r, Some(&grok_stop)), "grok");
    // Codex turn-scoped events carry turn_id; Claude Code does not.
    let codex = json!({ "hook_event_name": "Stop", "session_id": "s", "turn_id": "t-1" }).as_object().cloned().unwrap();
    assert_eq!(resolve_harness(&r, Some(&codex)), "codex");
    let claude = json!({ "hook_event_name": "Stop", "session_id": "s" }).as_object().cloned().unwrap();
    assert_eq!(resolve_harness(&r, Some(&claude)), "claude");
    // Explicit env overrides.
    let forced = rt_with("/p", env(&[("IMPECCINO_HOOK_HARNESS", "grok")]));
    assert_eq!(resolve_harness(&forced, Some(&gh)), "grok");
    // Stop detection matches both casings, on the raw stdin shape.
    assert!(is_stop_event(&grok_stop));
    assert!(is_stop_event(&claude));
    assert!(!is_stop_event(&grok));
}

#[test]
fn grok_post_tool_use_scans_and_stop_reports_everything() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let css = t.write("src/a.css", MIXED_CSS);
    // Grok Build PostToolUse: camelCase fields must normalize into a scan,
    // not skip with no-file-path (#646).
    let post = json!({
        "hookEventName": "post_tool_use", "sessionId": "g1", "cwd": cwd,
        "toolName": "str_replace", "toolInput": { "file_path": css },
    })
    .to_string();
    let one = hook::run_hook(&r, &post);
    assert!(one.stdout.contains("gradient-text"), "{}", one.stdout);
    assert_eq!(audit_str(&one.audit, "harness"), Some("grok"));

    // Grok discards PostToolUse stdout, so Stop must re-report the immediate
    // tier alongside the deferred one: the per-edit pass only touched the
    // file, remembering nothing.
    let stop = json!({ "hookEventName": "stop", "sessionId": "g1", "cwd": cwd, "reason": "end_turn" }).to_string();
    let deep = hook::run_stop_hook(&r, &stop);
    assert!(
        deep.stdout.contains("[gradient-text]") && deep.stdout.contains("[side-tab]"),
        "{}",
        deep.stdout
    );
    assert!(deep.stdout.contains("\"hookEventName\":\"Stop\""), "Grok takes the Claude Stop payload shape");

    // The observe-only shutdown fire never re-emits.
    let shutdown = json!({ "hookEventName": "stop", "sessionId": "g1", "cwd": cwd, "reason": "shutdown" }).to_string();
    let second = hook::run_stop_hook(&r, &shutdown);
    assert_eq!(second.stdout, "");
    assert_eq!(audit_str(&second.audit, "skipped"), Some("stop-reason"));

    // stopHookActive (camelCase) guards re-entry like stop_hook_active.
    let active = json!({ "hookEventName": "stop", "sessionId": "g1", "cwd": cwd, "stopHookActive": true }).to_string();
    let re = hook::run_stop_hook(&r, &active);
    assert_eq!(audit_str(&re.audit, "skipped"), Some("stop-hook-active"));
}

#[test]
fn stop_cache_syncs_after_clean_scan_so_reintroductions_fire() {
    // 3c442af7: a finding that was fixed and then reintroduced must fire
    // again — the clean Stop in between rewrites the remembered set.
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let css = t.write("src/a.css", SIDE_TAB_CSS);
    hook::run_hook(&r, &edit_event(&cwd, &css, "s1"));
    let first = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(first.stdout.contains("[side-tab]"), "{}", first.stdout);
    // Fix it: the next Stop is clean and syncs the remembered set to empty.
    t.write("src/a.css", ".card { color: #333; }\n");
    let clean = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert_eq!(audit_str(&clean.audit, "skipped"), Some("stop-clean"));
    // Reintroduce: the deep pass reports it again instead of staying silent.
    t.write("src/a.css", SIDE_TAB_CSS);
    let again = hook::run_stop_hook(&r, &stop_event(&cwd, "s1"));
    assert!(again.stdout.contains("[side-tab]"), "{}", again.stdout);
}

#[test]
fn codex_stop_emits_decision_block() {
    // #603: Codex Stop rejects Claude's hookSpecificOutput shape; findings
    // that should continue the turn are a top-level blocking decision.
    assert_eq!(payload("findings", "Stop", "codex"), r#"{"decision":"block","reason":"findings"}"#);
    assert_eq!(payload("  ", "Stop", "codex"), "");
    // PostToolUse keeps the shared additional-context shape.
    assert_eq!(
        payload("t", "PostToolUse", "codex"),
        r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"t"}}"#
    );

    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    let css = t.write("src/a.css", SIDE_TAB_CSS);
    hook::run_hook(&r, &edit_event(&cwd, &css, "s1"));
    let stop = json!({ "session_id": "s1", "cwd": cwd, "hook_event_name": "Stop", "turn_id": "t-9" }).to_string();
    let deep = hook::run_stop_hook(&r, &stop);
    let out: Value = serde_json::from_str(&deep.stdout).unwrap();
    assert_eq!(out["decision"], json!("block"));
    assert!(out["reason"].as_str().unwrap().contains("[side-tab]"));
    assert!(out.get("hookSpecificOutput").is_none());
}

// ── live-preview stand-down ───────────────────────────────────────────────
//
// A live variant session owns files carrying preview scaffolding
// (`data-impeccino-variants=` wrappers, `impeccino-carbonize-start`
// blocks). Every hook entry stands down on them: findings there are noise
// and acting on them derails the session; live-complete verifies the file
// once the accepted variant is permanent.

#[test]
fn run_hook_stands_down_on_live_preview_markers() {
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    // Control: the same slop without markers is reported.
    let plain = t.write("src/plain.css", GRADIENT_CSS);
    let reported = hook::run_hook(&r, &edit_event(&cwd, &plain, "s1"));
    assert!(reported.stdout.contains("[gradient-text]"), "{}", reported.stdout);
    // A carbonize block in flight: skipped, nothing emitted.
    let carbonized = t.write(
        "src/carbonized.css",
        &format!("/* impeccino-carbonize-start ab12cd34 */\n{GRADIENT_CSS}/* impeccino-carbonize-end ab12cd34 */\n"),
    );
    let skipped = hook::run_hook(&r, &edit_event(&cwd, &carbonized, "s1"));
    assert_eq!(skipped.stdout, "", "no findings while live markers are in the file");
    assert_eq!(skipped.audit["skipped"], json!("live-preview"));
    // A published variants wrapper, same stand-down.
    let wrapped = t.write(
        "src/wrapped.html",
        "<!-- impeccino-variants-start ab12cd34 --><div data-impeccino-variants=\"ab12cd34\" data-impeccino-variant-count=\"3\"></div>\n",
    );
    let skipped = hook::run_hook(&r, &edit_event(&cwd, &wrapped, "s1"));
    assert_eq!(skipped.stdout, "");
    assert_eq!(skipped.audit["skipped"], json!("live-preview"));
}

#[test]
fn before_edit_stands_down_on_live_preview_markers() {
    let t = Tmp::new();
    let cwd = t.path();
    t.write("package.json", "{}");
    let r = rt(&cwd);
    let slop = ".t { background: linear-gradient(90deg,#f00,#00f); -webkit-background-clip: text; color: transparent; }\n";
    // Control: denied at normal size without markers.
    let (out, _) = hbe(&r, &cursor(&cwd, "Write", json!({"file_path": "src/x.css", "content": slop})));
    assert!(out.starts_with("{\"permission\":\"deny\""), "{out}");
    // The very first variants write introduces the markers in the proposed
    // content itself.
    let proposed = format!("/* impeccino-carbonize-start ab12cd34 */\n{slop}");
    let (out, code) = hbe(&r, &cursor(&cwd, "Write", json!({"file_path": "src/x.css", "content": proposed})));
    assert_eq!(code, 0);
    assert_eq!(out, "{\"permission\":\"allow\"}");
    // Later fragment edits touch a file that already carries them on disk:
    // the proposed content alone looks like plain slop.
    t.write("src/y.css", "/* impeccino-carbonize-start ab12cd34 */\n.v { color: red; }\n");
    let (out, code) = hbe(&r, &cursor(&cwd, "Write", json!({"file_path": "src/y.css", "content": slop})));
    assert_eq!(code, 0);
    assert_eq!(out, "{\"permission\":\"allow\"}");
}

#[test]
fn run_hook_stands_down_for_the_whole_edit_when_the_primary_carries_live_markers() {
    // The edited JSX carries the wrapper; the stylesheet it imports does not.
    // Co-scanning would still speak up about the stylesheet mid-session, so
    // the whole event stands down. The same files without markers prove the
    // co-scan is otherwise live.
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    t.write("src/styles.css", GRADIENT_CSS);
    let plain = t.write("src/Plain.jsx", "import './styles.css';\nexport default function Plain() { return <h1 className=\"hero-title\">Hi</h1>; }\n");
    let reported = hook::run_hook(&r, &edit_event(&cwd, &plain, "s1"));
    assert!(reported.stdout.contains("[gradient-text]"), "co-scanned stylesheet is reported without markers: {}", reported.stdout);
    let wrapped = t.write(
        "src/App.jsx",
        "import './styles.css';\n{/* impeccino-variants-start ab12cd34 */}<div data-impeccino-variants=\"ab12cd34\" data-impeccino-variant-count=\"3\"></div>\n",
    );
    let skipped = hook::run_hook(&r, &edit_event(&cwd, &wrapped, "s2"));
    assert_eq!(skipped.stdout, "", "nothing is emitted while the edited file is in a live session");
    assert_eq!(skipped.audit["skipped"], json!("live-preview"));
    let audited = audit_str(&skipped.audit, "file").unwrap_or("").replace('\\', "/");
    assert!(audited.ends_with("src/App.jsx"), "the audit names the edited file, not the companion: {audited}");
}

#[test]
fn run_hook_stands_down_before_the_edit_cap_can_suppress_a_live_file() {
    // A file edited past the per-session cap would be skipped as
    // "suppressed" (with the notice) before its content is read. A live
    // wrap on such a file must stand down instead, every time.
    let t = Tmp::new();
    let cwd = t.path();
    let r = rt(&cwd);
    // Seven plain edits cross the cap: the 7th carries the notice.
    let css = t.write("src/b.css", GRADIENT_CSS);
    let mut outputs = Vec::new();
    for _ in 0..7 {
        outputs.push(hook::run_hook(&r, &edit_event(&cwd, &css, "cap")));
    }
    assert_eq!(outputs[6].audit["suppressed"], json!(true));
    assert!(outputs[6].stdout.contains("Suppressing further design hints"));
    // Now a live session carbonizes into that same file: stand down, never
    // suppress.
    t.write(
        "src/b.css",
        &format!("/* impeccino-carbonize-start ab12cd34 */\n{GRADIENT_CSS}/* impeccino-carbonize-end ab12cd34 */\n"),
    );
    for i in 0..3 {
        let out = hook::run_hook(&r, &edit_event(&cwd, &css, "cap"));
        assert_eq!(out.stdout, "", "edit {i}: nothing emitted");
        assert_eq!(out.audit["skipped"], json!("live-preview"), "edit {i}");
        assert!(out.audit.get("suppressed").is_none(), "edit {i}: {:?}", out.audit);
        assert!(out.audit.get("editCount").is_none(), "edit {i}: the cap is not bumped for a live wrap");
    }
}
