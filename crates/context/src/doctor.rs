//! JS: doctor.mjs -> `impeccino doctor`

use crate::artifact_schema::{read_product_schema_version, stamp_product_schema, PRODUCT_SCHEMA_VERSION};
use crate::context::*;
use crate::jsp;
use crate::staleness::*;
use crate::staleness_deep::*;
use crate::target_args::{parse_target_options, TargetOptions};
use crate::util::{homedir, json_pretty, opt_string, Env};
use impeccino_common::Io;
use serde_json::{Map, Value};
use std::path::Path;

struct Flags {
    json: bool,
    fix: bool,
    help: bool,
}

fn parse_args(argv: &[String]) -> Result<(Flags, TargetOptions), String> {
    let mut flags = Flags { json: false, fix: false, help: false };
    let mut passthrough: Vec<String> = Vec::new();
    for a in argv {
        match a.as_str() {
            "--json" => flags.json = true,
            "--fix" => flags.fix = true,
            "--help" | "-h" => flags.help = true,
            _ => passthrough.push(a.clone()),
        }
    }
    let t = parse_target_options(&passthrough, true)?;
    Ok((flags, t))
}

fn usage() -> String {
    [
        "Usage: impeccino doctor [--json] [--fix] [--target <path>]",
        "",
        "Report drift between this project's Impeccino artifacts and what the",
        "installed version reads: PRODUCT.md, DESIGN.md and its DESIGN.json",
        "sidecar, SURFACES.md, and the design hook.",
        "",
        "  --json           Emit findings as JSON.",
        "  --fix            Apply the mechanical migrations (severity \"auto\") only.",
        "  --target <path>  Select a workspace in a monorepo.",
    ]
    .join("\n")
}

struct Report {
    ctx: Ctx,
    project_root: String,
    abs_product_path: Option<String>,
    findings: Vec<Finding>,
    workspaces: Vec<WorkspaceRow>,
    rule_registry_available: bool,
}

fn collect(cwd: &str, target: &TargetOptions, env: &Env, provider_id: &str) -> Report {
    let ctx = load_context(cwd, target, env);
    let project_root = if ctx.project_root.is_empty() { cwd.to_string() } else { ctx.project_root.clone() };
    let abs_product_path = ctx.product_path.as_deref().map(|p| jsp::resolve(cwd, &[p]));
    let abs_design_path = ctx.design_path.as_deref().map(|p| jsp::resolve(cwd, &[p]));
    let sidecar_path = design_sidecar_path_for(&project_root, ctx.design_context_dir.as_deref());
    let known = load_known_rule_ids();
    let selection = resolve_target_selection(cwd, target, env);
    let workspace_candidates: Vec<TargetCandidate> = selection.map(|s| s.target_candidates).unwrap_or_default();
    let (ws_findings, workspaces) = check_workspaces(&ctx.repo_root, &workspace_candidates);

    // JS: doctor builds on collectBootFindingGroups, interleaving only its
    // deep checks while preserving the established finding order (upstream
    // 80997663).
    let boot = collect_boot_finding_groups(
        &ctx,
        cwd,
        &BootExtras {
            abs_design_path: abs_design_path.clone(),
            sidecar_path,
            home: Some(homedir(env)),
        },
    );
    let mut findings: Vec<Finding> = Vec::new();
    findings.extend(boot.legacy_state);
    findings.extend(boot.product);
    findings.extend(boot.native_platform);
    findings.extend(boot.design_sidecar);
    findings.extend(check_design_drift(abs_design_path.as_deref(), &project_root, 25));
    findings.extend(check_design_coverage(ctx.design.as_deref(), ctx.design_path.as_deref()));
    findings.extend(check_design_waivers(ctx.design.as_deref(), ctx.design_path.as_deref(), known.as_deref()));
    findings.extend(boot.surface_briefs);
    findings.extend(check_hook_installation(&project_root, Some(&ctx.repo_root), provider_id));
    findings.extend(ws_findings);

    Report {
        ctx,
        project_root,
        abs_product_path,
        findings,
        workspaces,
        rule_registry_available: known.is_some(),
    }
}

fn rel(file_path: &str, root: &str) -> String {
    let v = jsp::relative("/", root, file_path);
    if !v.is_empty() && !v.starts_with("..") {
        jsp::to_posix(&v)
    } else {
        file_path.to_string()
    }
}

struct Fixes {
    applied: Vec<String>,
    skipped: Vec<(String, String)>,
    failed: Vec<(String, String, String)>,
}

fn fix_exit_code(fixes: Option<&Fixes>) -> i32 {
    if fixes.map(|f| !f.failed.is_empty()).unwrap_or(false) { 1 } else { 0 }
}

fn apply_fixes(report: &Report) -> Fixes {
    let mut applied = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let mut failed: Vec<(String, String, String)> = Vec::new();
    for entry in &report.findings {
        if entry.severity != "auto" {
            skipped.push((entry.id.clone(), "needs a decision from the user".to_string()));
            continue;
        }
        skipped.push((entry.id.clone(), "no automatic migration implemented".to_string()));
    }
    if let (Some(pp), Some(product)) = (&report.abs_product_path, report.ctx.product.as_deref()) {
        if !product.is_empty()
            && read_product_schema_version(product).is_none()
            && !report.findings.iter().any(|f| f.id == "product-schema-legacy")
        {
            let path = rel(pp, &report.project_root);
            let stamped = stamp_product_schema(product, PRODUCT_SCHEMA_VERSION);
            match impeccino_common::atomic_file::write(Path::new(pp), stamped.as_bytes()) {
                Ok(()) => applied.push(format!("Stamped {} as product-schema {}.", path, PRODUCT_SCHEMA_VERSION)),
                Err(err) => failed.push(("product-schema-stamp".to_string(), path, err.to_string())),
            }
        }
    }
    Fixes { applied, skipped, failed }
}

fn render_text(report: &Report, fixes: Option<&Fixes>, cwd: &str, command: &str, self_cmd: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let pr = rel(&report.project_root, cwd);
    lines.push(format!("Impeccino doctor: {}", if pr.is_empty() { "." } else { &pr }));
    if report.ctx.is_monorepo {
        let rr = rel(&report.ctx.repo_root, cwd);
        lines.push(format!("Monorepo, repo root {}.", if rr.is_empty() { "." } else { &rr }));
    }
    lines.push(String::new());
    if report.findings.is_empty() {
        lines.push("No drift found. Every artifact matches what this version reads.".to_string());
    } else {
        for (severity, label) in [("route", "needs a command"), ("mention", "worth saying"), ("auto", "automatic")] {
            let group: Vec<&Finding> = report.findings.iter().filter(|f| f.severity == severity).collect();
            if group.is_empty() {
                continue;
            }
            lines.push(format!("{} ({}):", label, group.len()));
            for f in group {
                lines.push(format!("  {}{}", f.id, f.path.as_ref().filter(|p| !p.is_empty()).map(|p| format!("  [{}]", p)).unwrap_or_default()));
                lines.push(format!("    {}", f.summary));
                lines.push(format!("    → {}", f.fix));
            }
            lines.push(String::new());
        }
    }
    if !report.workspaces.is_empty() {
        lines.push("Workspaces:".to_string());
        for w in &report.workspaces {
            lines.push(format!(
                "  {}  product: {}  design: {}{}",
                w.path,
                w.product_status,
                w.design_status,
                w.platform.as_ref().filter(|p| !p.is_empty()).map(|p| format!("  platform: {}", p)).unwrap_or_default()
            ));
        }
        lines.push(String::new());
    }
    if !report.rule_registry_available {
        lines.push("Note: the bundled detector could not be resolved, so waived rule ids were not validated.".to_string());
        lines.push(String::new());
    }
    if let Some(f) = fixes {
        lines.push(if f.applied.is_empty() { "Applied nothing.".to_string() } else { "Applied:".to_string() });
        for a in &f.applied {
            lines.push(format!("  {}", a));
        }
        if !f.failed.is_empty() {
            lines.push("Failed:".to_string());
            for (id, path, reason) in &f.failed {
                lines.push(format!("  {} [{}]: {}", id, path, reason));
            }
        }
        let held: Vec<&(String, String)> = f.skipped.iter().filter(|(_, r)| r != "needs a decision from the user").collect();
        if !held.is_empty() {
            lines.push("Left alone:".to_string());
            for (id, reason) in held {
                lines.push(format!("  {}: {}", id, reason));
            }
        }
    } else if report.findings.iter().any(|f| f.severity == "auto") {
        lines.push(format!(
            "Run `{} doctor --fix` to apply the automatic migrations, or `{} doctor` to work through all of them.",
            self_cmd, command
        ));
    }
    lines.join("\n")
}

pub fn run(args: &[String], io: &mut Io) -> i32 {
    let cwd = io.cwd.to_string_lossy().into_owned();
    let env = io.env.clone();
    let provider = crate::provider::detect(&env, &cwd);
    // The printed fix command spells the launcher when the launcher exported
    // IMPECCINO_SELF, and the plain `impeccino` verb otherwise.
    let self_cmd = env
        .get("IMPECCINO_SELF")
        .filter(|v| !v.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "impeccino".to_string());
    let self_cmd = impeccino_common::quote_executable_path(&self_cmd, cfg!(windows));
    let (flags, target) = match parse_args(args) {
        Ok(v) => v,
        Err(msg) => {
            io.err(&format!("{}\n", msg));
            return 1;
        }
    };
    if flags.help {
        io.out(&format!("{}\n", usage()));
        return 0;
    }
    let report = collect(&cwd, &target, &env, &provider.id);
    let fixes = if flags.fix { Some(apply_fixes(&report)) } else { None };
    if flags.json {
        let mut m = Map::new();
        m.insert("projectRoot".into(), Value::String(report.project_root.clone()));
        m.insert("repoRoot".into(), Value::String(report.ctx.repo_root.clone()));
        m.insert("isMonorepo".into(), Value::Bool(report.ctx.is_monorepo));
        m.insert("productPath".into(), opt_string(&report.ctx.product_path));
        m.insert("designPath".into(), opt_string(&report.ctx.design_path));
        m.insert("platform".into(), opt_string(&report.ctx.platform));
        m.insert("ruleRegistryAvailable".into(), Value::Bool(report.rule_registry_available));
        m.insert("findings".into(), Value::Array(report.findings.iter().map(|f| f.to_value()).collect()));
        m.insert("workspaces".into(), Value::Array(report.workspaces.iter().map(|w| w.to_value()).collect()));
        if let Some(f) = &fixes {
            let mut fm = Map::new();
            fm.insert("applied".into(), Value::Array(f.applied.iter().cloned().map(Value::String).collect()));
            fm.insert(
                "skipped".into(),
                Value::Array(
                    f.skipped
                        .iter()
                        .map(|(id, reason)| {
                            let mut e = Map::new();
                            e.insert("id".into(), Value::String(id.clone()));
                            e.insert("reason".into(), Value::String(reason.clone()));
                            Value::Object(e)
                        })
                        .collect(),
                ),
            );
            if !f.failed.is_empty() {
                fm.insert(
                    "failures".into(),
                    Value::Array(
                        f.failed
                            .iter()
                            .map(|(id, path, reason)| {
                                let mut e = Map::new();
                                e.insert("id".into(), Value::String(id.clone()));
                                e.insert("path".into(), Value::String(path.clone()));
                                e.insert("reason".into(), Value::String(reason.clone()));
                                Value::Object(e)
                            })
                            .collect(),
                    ),
                );
            }
            m.insert("fixes".into(), Value::Object(fm));
        }
        io.out(&format!("{}\n", json_pretty(&Value::Object(m))));
        return fix_exit_code(fixes.as_ref());
    }
    io.out(&format!("{}\n", render_text(&report, fixes.as_ref(), &cwd, &provider.command, &self_cmd)));
    fix_exit_code(fixes.as_ref())
}

#[cfg(test)]
mod tests {
    use super::{apply_fixes, collect, fix_exit_code, render_text};
    use crate::target_args::TargetOptions;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let suffix = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "impeccino doctor write {} {} {}",
                std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
                suffix
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn failed_product_schema_stamp_is_not_reported_as_applied() {
        let root = TempDir::new();
        let product = root.0.join("PRODUCT.md");
        std::fs::write(&product, "# Example\n\n## Positioning\nA current product record.\n").unwrap();
        let cwd = root.0.to_string_lossy().into_owned();
        let env = crate::util::Env::new();
        let report = collect(&cwd, &TargetOptions::default(), &env, "");
        assert!(report.ctx.product.is_some());
        assert!(report.findings.iter().all(|finding| finding.id != "product-schema-legacy"));

        // Simulate the parent directory disappearing after the report read
        // it. This is a deterministic write failure and does not rely on
        // chmod behavior (fs::write would recreate a missing leaf file).
        std::fs::remove_dir_all(&root.0).unwrap();
        let fixes = apply_fixes(&report);
        assert!(fixes.applied.is_empty());
        assert_eq!(fixes.failed.len(), 1);
        assert_eq!(fixes.failed[0].0, "product-schema-stamp");
        assert_eq!(fixes.failed[0].1, "PRODUCT.md");
        assert!(!fixes.failed[0].2.is_empty());
        assert_eq!(fix_exit_code(Some(&fixes)), 1);
        let text = render_text(&report, Some(&fixes), &cwd, "impeccino", "impeccino");
        assert!(text.contains("Failed:\n  product-schema-stamp [PRODUCT.md]:"));
    }
}
