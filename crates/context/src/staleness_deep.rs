//! JS: lib/staleness-deep.mjs (Tier 2, doctor only)

use crate::context::{extract_platform, TargetCandidate};
use crate::design_parser::parse_design_md;
use crate::jsp;
use crate::signals::git_run;
use crate::staleness::{check_native_platform_evidence, finding, js_truthy, to_relative, unique_roots, Finding};
use crate::util::{exists, js_trim, read_json, safe_read};
use impeccino_core::inline_ignores::parse_design_waivers;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Value};

const VISUAL_SOURCE_DIRS: [&str; 7] = ["src", "app", "pages", "components", "site", "styles", "public"];

fn git(args: &[&str], cwd: &str) -> Option<String> {
    git_run(args, cwd, true, Some(5000))
}

/// JS: checkDesignDrift
pub fn check_design_drift(design_path: Option<&str>, project_root: &str, threshold: usize) -> Vec<Finding> {
    let Some(design_path) = design_path else { return vec![] };
    if project_root.is_empty() {
        return vec![];
    }
    match git(&["rev-parse", "--is-inside-work-tree"], project_root) {
        Some(s) if !s.is_empty() => {}
        _ => return vec![],
    }
    let rel_design = to_relative(Some(design_path), project_root).unwrap();
    let last = match git(&["log", "-1", "--format=%H", "--", &rel_design], project_root) {
        Some(s) if !s.is_empty() => s,
        _ => return vec![],
    };
    let dirs: Vec<&str> = VISUAL_SOURCE_DIRS.iter().copied().filter(|d| exists(&jsp::join(&[project_root, d]))).collect();
    if dirs.is_empty() {
        return vec![];
    }
    let mut args: Vec<&str> = vec!["log", "--oneline"];
    let range = format!("{}..HEAD", last);
    args.push(&range);
    args.push("--");
    args.extend(dirs.iter());
    let Some(log) = git(&args, project_root) else { return vec![] };
    let commits = if log.is_empty() { 0 } else { log.split('\n').filter(|l| !l.is_empty()).count() };
    if commits < threshold {
        return vec![];
    }
    let when = git(&["log", "-1", "--format=%ad", "--date=short", "--", &rel_design], project_root).filter(|s| !s.is_empty());
    vec![finding(
        "design-md-drift",
        "DESIGN.md",
        Some(rel_design.clone()),
        "route",
        format!(
            "{} commits have touched {} since {} was last edited{}. This counts commits, not contradictions: it says the document is worth re-reading, not that it is wrong.",
            commits,
            dirs.join(", "),
            rel_design,
            when.map(|w| format!(" ({})", w)).unwrap_or_default()
        ),
        "Read DESIGN.md against the current tokens and components before trusting it as authority. If it has genuinely drifted, `document` regenerates it from the code.".to_string(),
    )]
}

fn has_coverage_value(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Array(a)) => a.iter().any(|x| has_coverage_value(Some(x))),
        Some(Value::Object(o)) => o.values().any(|x| has_coverage_value(Some(x))),
        Some(Value::String(s)) => {
            let t = js_trim(s);
            if t.is_empty() {
                return false;
            }
            // /^(?:\[\s*\]|\{\s*\})$/
            let inner_empty = |open: char, close: char| -> bool {
                t.starts_with(open) && t.ends_with(close) && t.len() >= 2 && t[1..t.len() - 1].chars().all(|c| c.is_whitespace())
            };
            !(inner_empty('[', ']') || inner_empty('{', '}'))
        }
        _ => false,
    }
}

const SEED_MARKER_TAIL: &str = "impeccino document once there's code to capture the actual tokens and components. -->";

/// JS: checkDesignCoverage
pub fn check_design_coverage(design: Option<&str>, design_path: Option<&str>) -> Vec<Finding> {
    let Some(design) = design.filter(|d| !d.is_empty()) else { return vec![] };
    let model = parse_design_md(design);
    let is_seed = ["/", "$"].iter().any(|p| {
        design.contains(&format!("<!-- SEED: established with the user before implementation; re-run {}{}", p, SEED_MARKER_TAIL))
    });
    let required: Vec<&str> = if is_seed { vec!["colors", "typography"] } else { vec!["colors", "typography", "components"] };
    let missing: Vec<&str> = required
        .into_iter()
        .filter(|s| !model.has_section(s) && !has_coverage_value(model.frontmatter.as_ref().and_then(|f| f.get(*s))))
        .collect();
    if missing.is_empty() {
        return vec![];
    }
    vec![finding(
        "design-md-coverage",
        "DESIGN.md",
        design_path.map(|s| s.to_string()),
        "mention",
        format!(
            "{} has no {} section. Agents generating new screens must infer those rules from the implementation.",
            design_path.filter(|p| !p.is_empty()).unwrap_or("DESIGN.md"),
            missing.join(", ")
        ),
        "Ask whether the section never applied or was never written. `document` fills it from the code if the project has the answer in its CSS.".to_string(),
    )]
}

fn wrap_ticks(items: &[String]) -> String {
    items.iter().map(|k| format!("`{}`", k)).collect::<Vec<_>>().join(", ")
}

/// Project-wide waivers in DESIGN.md (`<!-- impeccino-disable <rule> -->`)
/// that name a rule the detector does not have.
pub fn check_design_waivers(design: Option<&str>, design_path: Option<&str>, known_rule_ids: Option<&[String]>) -> Vec<Finding> {
    let (Some(design), Some(known)) = (design.filter(|d| !d.is_empty()), known_rule_ids) else { return vec![] };
    let unknown: Vec<String> = parse_design_waivers(design).into_iter().filter(|r| !known.contains(r)).collect();
    if unknown.is_empty() {
        return vec![];
    }
    let shown = design_path.filter(|p| !p.is_empty()).unwrap_or("DESIGN.md");
    vec![finding(
        "design-waiver-unknown-rule",
        "DESIGN.md",
        design_path.map(|s| s.to_string()),
        "mention",
        format!(
            "{} waives rule id(s) the detector does not have: {}. Either the rule was renamed or removed, or the id was mistyped and has never waived anything.",
            shown,
            wrap_ticks(&unknown)
        ),
        "Report the exact ids. Fix a typo in place; drop a waiver whose rule is gone, keeping the design rule it sat next to.".to_string(),
    )]
}

fn collect_hook_commands(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            if crate::hook_markers::is_design_hook_command(s) {
                out.push(s.clone());
            }
        }
        Value::Array(a) => {
            for e in a {
                collect_hook_commands(e, out);
            }
        }
        Value::Object(o) => {
            for e in o.values() {
                collect_hook_commands(e, out);
            }
        }
        _ => {}
    }
}

static PLACEHOLDER: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{[^}]*\}|\$[A-Za-z_]").unwrap());

/// The `.mjs` script (JS-era manifests) or the launcher (binary-era
/// manifests) a hook command runs; see `hook_markers`.
fn hook_script_token_from(command: &str) -> Option<String> {
    crate::hook_markers::hook_program_token(command)
}

fn resolve_hook_script_path(token: &str, root: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    if token.contains("$(") || token.contains('`') {
        return None;
    }
    let expanded = token.replace("${CLAUDE_PROJECT_DIR}", root);
    if PLACEHOLDER.is_match(&expanded) {
        return None;
    }
    Some(if jsp::is_absolute(&expanded) { expanded } else { jsp::join(&[root, &expanded]) })
}

/// JS: checkHookInstallation
pub fn check_hook_installation(project_root: &str, repo_root: Option<&str>, provider_id: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let manifests = crate::context_cli::hook_manifests_for(provider_id);
    if manifests.is_empty() {
        return out;
    }
    let roots = unique_roots(project_root, repo_root);
    for root in &roots {
        for rel in manifests {
            let mp = jsp::join(&[root, rel]);
            let Some(raw) = read_json(&mp) else { continue };
            let Some(hooks) = raw.get("hooks") else { continue };
            if !js_truthy(hooks) {
                continue;
            }
            let mut commands = Vec::new();
            collect_hook_commands(hooks, &mut commands);
            if commands.is_empty() {
                continue;
            }
            let base = if project_root.is_empty() { root.as_str() } else { project_root };
            let installed_at = to_relative(Some(&mp), base);
            let broken: Vec<&String> = commands
                .iter()
                .filter(|c| {
                    let Some(token) = hook_script_token_from(c) else { return false };
                    let Some(abs) = resolve_hook_script_path(&token, root) else { return false };
                    !exists(&abs)
                })
                .collect();
            if !broken.is_empty() {
                let ia = installed_at.clone().unwrap();
                out.push(finding(
                    "hook-script-missing",
                    "hook manifest",
                    Some(ia.clone()),
                    "mention",
                    format!(
                        "{} installs the design hook, but its script path does not exist: {}. The hook runs as a no-op, so UI edits have been going unscanned while the project looks covered.",
                        ia,
                        broken.iter().map(|c| format!("`{}`", c)).collect::<Vec<_>>().join(", ")
                    ),
                    "Reinstall with `impeccino hooks on`, which rewrites the manifest against the skill's current location.".to_string(),
                ));
            }
        }
    }
    out
}

pub struct WorkspaceRow {
    pub name: String,
    pub path: String,
    pub product_status: &'static str,
    pub product_path: Option<String>,
    pub design_status: &'static str,
    pub design_path: Option<String>,
    pub platform: Option<String>,
}

impl WorkspaceRow {
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), Value::String(self.name.clone()));
        m.insert("path".into(), Value::String(self.path.clone()));
        m.insert("productStatus".into(), Value::String(self.product_status.to_string()));
        m.insert("productPath".into(), self.product_path.clone().map(Value::String).unwrap_or(Value::Null));
        m.insert("designStatus".into(), Value::String(self.design_status.to_string()));
        m.insert("designPath".into(), self.design_path.clone().map(Value::String).unwrap_or(Value::Null));
        m.insert("platform".into(), self.platform.clone().map(Value::String).unwrap_or(Value::Null));
        Value::Object(m)
    }
}

/// JS: checkWorkspaces
pub fn check_workspaces(repo_root: &str, candidates: &[TargetCandidate]) -> (Vec<Finding>, Vec<WorkspaceRow>) {
    if repo_root.is_empty() || candidates.is_empty() {
        return (vec![], vec![]);
    }
    let mut findings = Vec::new();
    let mut workspaces = Vec::new();
    for c in candidates {
        let workspace_root = jsp::join(&[repo_root, &c.path]);
        let product_path = c.product_path.as_deref().map(|p| jsp::join(&[repo_root, p]));
        let product = product_path.as_deref().and_then(safe_read);
        let platform = extract_platform(product.as_deref());
        workspaces.push(WorkspaceRow {
            name: c.name.clone(),
            path: c.path.clone(),
            product_status: c.product_status,
            product_path: c.product_path.clone(),
            design_status: c.design_status,
            design_path: c.design_path.clone(),
            platform: platform.clone().or_else(|| {
                if product.as_deref().map(|p| !p.is_empty()).unwrap_or(false) {
                    Some("web (default)".to_string())
                } else {
                    None
                }
            }),
        });
        let native = check_native_platform_evidence(&workspace_root, platform.as_deref(), product.as_deref(), c.product_path.as_deref());
        for entry in native {
            let inherited = c.product_status == "inherited";
            findings.push(finding(
                "workspace-platform-native-evidence",
                "PRODUCT.md",
                Some(c.product_path.clone().unwrap_or_else(|| format!("{}/PRODUCT.md", c.path))),
                "mention",
                format!(
                    "Workspace `{}` {} that resolves to web, but the workspace itself carries native build files. {}",
                    c.path,
                    if inherited { "inherits the repo-root PRODUCT.md" } else { "has a PRODUCT.md" },
                    entry.summary
                ),
                if inherited {
                    format!(
                        "Give `{}` its own PRODUCT.md with the right `## Platform`. An inherited record cannot describe two platforms at once.",
                        c.path
                    )
                } else {
                    entry.fix
                },
            ));
        }
    }
    let inherited: Vec<&WorkspaceRow> = workspaces.iter().filter(|w| w.product_status == "inherited").collect();
    if !inherited.is_empty() {
        findings.push(finding(
            "workspace-context-inherited",
            "PRODUCT.md",
            None,
            "mention",
            format!(
                "{} of {} workspace(s) inherit the repo-root PRODUCT.md: {}. Inheritance is intended; whether one record truthfully describes these apps is not something this check can tell.",
                inherited.len(),
                workspaces.len(),
                inherited.iter().map(|w| format!("`{}`", w.path)).collect::<Vec<_>>().join(", ")
            ),
            "Ask the user whether the inherited record describes each app. Where it does not, `init` in that workspace writes a child PRODUCT.md that overrides it.".to_string(),
        ));
    }
    (findings, workspaces)
}

/// JS: loadKnownRuleIds -> the bundled registry, lowercased ids.
pub fn load_known_rule_ids() -> Option<Vec<String>> {
    Some(impeccino_core::registry::ANTIPATTERNS.iter().map(|r| r.id.to_lowercase()).collect())
}

#[cfg(test)]
mod tests {
    use super::{check_design_coverage, check_hook_installation};

    static TMP_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn tmp() -> String {
        let base = std::env::temp_dir().join(format!(
            "impeccino-doctor-hook-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            // A per-process counter: Windows' clock is coarse enough that two
            // parallel tests can share a nanosecond stamp and then delete each
            // other's directories.
            TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&base).unwrap();
        // Like Node's `realpathSync`: on Windows, `canonicalize` yields a
        // `\\?\` verbatim path, which the kernel takes literally, so the `/`
        // separators a hook command appends would not resolve under it.
        let real = std::fs::canonicalize(&base).unwrap().to_string_lossy().into_owned();
        real.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(real)
    }

    fn write(root: &str, rel: &str, body: &str) {
        let p = std::path::Path::new(root).join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn design_coverage_describes_missing_guidance_without_a_live_panel() {
        let findings =
            check_design_coverage(Some("## Colors\nPrimary: #222.\n"), Some("DESIGN.md"));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "design-md-coverage");
        assert!(findings[0]
            .summary
            .contains("must infer those rules from the implementation"));
        assert!(!findings[0].summary.contains("live design panel"));
    }

    #[test]
    fn hook_script_missing_resolves_launcher_and_legacy_forms() {
        let root = tmp();
        // Launcher form pointing at a launcher that exists: no finding.
        write(&root, ".claude/skills/impeccino/scripts/impeccino", "#!/bin/sh\n");
        write(
            &root,
            ".claude/settings.local.json",
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"\"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/impeccino\" hook"}]}]}}"#,
        );
        assert!(check_hook_installation(&root, None, "claude-code").is_empty());

        // Launcher form pointing at a missing launcher: reported.
        write(
            &root,
            ".claude/settings.local.json",
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"\"${CLAUDE_PROJECT_DIR}/.other/skills/impeccino/scripts/impeccino\" hook"}]}]}}"#,
        );
        let f = check_hook_installation(&root, None, "claude-code");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].id, "hook-script-missing");

        // Legacy .mjs form is still resolved: reported when the script is gone.
        write(
            &root,
            ".claude/settings.local.json",
            r#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"node \"${CLAUDE_PROJECT_DIR}/.claude/skills/impeccino/scripts/hook.mjs\""}]}]}}"#,
        );
        let f = check_hook_installation(&root, None, "claude-code");
        assert_eq!(f.len(), 1, "{f:?}");
        write(&root, ".claude/skills/impeccino/scripts/hook.mjs", "");
        assert!(check_hook_installation(&root, None, "claude-code").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
