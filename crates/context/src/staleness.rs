//! JS: lib/staleness.mjs (Tier 1)

use crate::artifact_schema::*;
use crate::context::{BriefSummary, Ctx};
use crate::jsp;
use crate::util::{exists, is_dir, mtime_ms, read_json};
use impeccino_common::project_files::{DESIGN_SIDECAR_FILE, LEGACY_STATE_DIR, SURFACES_FILE};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Value};

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Finding {
    pub id: String,
    pub artifact: String,
    pub path: Option<String>,
    pub severity: &'static str,
    pub summary: String,
    pub fix: String,
}

impl Finding {
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), Value::String(self.id.clone()));
        m.insert("artifact".into(), Value::String(self.artifact.clone()));
        m.insert(
            "path".into(),
            self.path.clone().map(Value::String).unwrap_or(Value::Null),
        );
        m.insert("severity".into(), Value::String(self.severity.to_string()));
        m.insert("summary".into(), Value::String(self.summary.clone()));
        m.insert("fix".into(), Value::String(self.fix.clone()));
        Value::Object(m)
    }
}

pub fn finding(
    id: &str,
    artifact: &str,
    path: Option<String>,
    severity: &'static str,
    summary: String,
    fix: String,
) -> Finding {
    Finding {
        id: id.to_string(),
        artifact: artifact.to_string(),
        path,
        severity,
        summary,
        fix,
    }
}

struct NativeEvidence {
    platform: &'static str,
    reason: &'static str,
}
const NATIVE_EVIDENCE_PATHS: [(&str, &str, &str); 5] = [
    ("pubspec.yaml", "adaptive", "a Flutter pubspec.yaml"),
    ("ios/Podfile", "ios", "an ios/Podfile"),
    ("android/build.gradle", "android", "an android/build.gradle"),
    (
        "android/build.gradle.kts",
        "android",
        "an android/build.gradle.kts",
    ),
    ("ios/Runner.xcodeproj", "ios", "an ios/Runner.xcodeproj"),
];
const NATIVE_EVIDENCE_DEPENDENCIES: [(&str, &str, &str); 3] = [
    ("react-native", "adaptive", "a react-native dependency"),
    ("expo", "adaptive", "an expo dependency"),
    (
        "@react-native/metro-config",
        "adaptive",
        "a React Native metro config dependency",
    ),
];

/// The DESIGN.md sidecar: `DESIGN.json` next to DESIGN.md, or at the project
/// root when there is no DESIGN.md (docs/adr/0020).
pub fn design_sidecar_path_for(project_root: &str, design_dir: Option<&str>) -> String {
    jsp::join(&[design_dir.unwrap_or(project_root), DESIGN_SIDECAR_FILE])
}

fn has_section(markdown: &str, heading: &str) -> bool {
    Regex::new(&format!(r"(?im)^##\s+{}\s*$", regex::escape(heading)))
        .map(|r| r.is_match(markdown))
        .unwrap_or(false)
}

pub fn to_relative(file_path: Option<&str>, root: &str) -> Option<String> {
    let fp = file_path?;
    let rel = jsp::relative("/", root, fp);
    if !rel.is_empty() && !rel.starts_with("..") && !jsp::is_absolute(&rel) {
        Some(jsp::to_posix(&rel))
    } else {
        Some(fp.to_string())
    }
}

/// JS: checkProduct
pub fn check_product(product: Option<&str>, product_path: &str) -> Vec<Finding> {
    let Some(product) = product.filter(|p| !p.is_empty()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (heading, reason) in PRODUCT_DEPRECATED_SECTIONS {
        if !has_section(product, heading) {
            continue;
        }
        out.push(finding(
            &format!("product-deprecated-{}", heading.to_lowercase()),
            "PRODUCT.md",
            Some(product_path.to_string()),
            "mention",
            format!("PRODUCT.md still carries a `## {}` section. {}", heading, reason),
            format!(
                "Treat `## {}` as absent for every decision this session. Offer to delete the section; do not let its value influence the work either way.",
                heading
            ),
        ));
    }
    let stamped = read_product_schema_version(product);
    if stamped.is_none() && !PRODUCT_V4_SECTIONS.iter().any(|s| has_section(product, s)) {
        out.push(finding(
            "product-schema-legacy",
            "PRODUCT.md",
            Some(product_path.to_string()),
            "route",
            format!(
                "PRODUCT.md has no schema stamp and none of the sections the current record adds ({}), so it predates this version of the product record.",
                PRODUCT_V4_SECTIONS.join(", ")
            ),
            "Offer `init`, which preserves confirmed answers and fills the gaps by interview. Do not rewrite the file from inference.".to_string(),
        ));
    } else if let Some(v) = stamped {
        if v < PRODUCT_SCHEMA_VERSION {
            out.push(finding(
                "product-schema-outdated",
                "PRODUCT.md",
                Some(product_path.to_string()),
                "route",
                format!(
                    "PRODUCT.md is stamped product-schema {}; the current record is {}.",
                    v, PRODUCT_SCHEMA_VERSION
                ),
                "Offer `init` to bring the record current, preserving confirmed answers."
                    .to_string(),
            ));
        }
    }
    out
}

/// JS: checkNativePlatformEvidence
pub fn check_native_platform_evidence(
    project_root: &str,
    platform: Option<&str>,
    product: Option<&str>,
    product_path: Option<&str>,
) -> Vec<Finding> {
    if project_root.is_empty() {
        return vec![];
    }
    if let Some(p) = platform {
        if !p.is_empty() && p != "web" {
            return vec![];
        }
    }
    let mut evidence: Vec<NativeEvidence> = Vec::new();
    for (rel, platform, reason) in NATIVE_EVIDENCE_PATHS {
        if exists(&jsp::join(&[project_root, rel])) {
            evidence.push(NativeEvidence { platform, reason });
        }
    }
    if let Some(pkg) = read_json(&jsp::join(&[project_root, "package.json"])) {
        // JS: { ...pkg.dependencies, ...pkg.devDependencies } then deps[name] truthy
        let dep_truthy = |name: &str| -> bool {
            let mut v: Option<&Value> = None;
            if let Some(d) = pkg.get("dependencies").and_then(|d| d.as_object()) {
                if let Some(x) = d.get(name) {
                    v = Some(x);
                }
            }
            if let Some(d) = pkg.get("devDependencies").and_then(|d| d.as_object()) {
                if let Some(x) = d.get(name) {
                    v = Some(x);
                }
            }
            match v {
                None => false,
                Some(x) => js_truthy(x),
            }
        };
        if pkg.is_object() || pkg.is_array() || (!pkg.is_null() && js_truthy(&pkg)) {
            for (name, platform, reason) in NATIVE_EVIDENCE_DEPENDENCIES {
                if dep_truthy(name) {
                    evidence.push(NativeEvidence { platform, reason });
                }
            }
        }
    }
    if evidence.is_empty() {
        return vec![];
    }
    let mut platforms: Vec<&str> = Vec::new();
    for e in &evidence {
        if !platforms.contains(&e.platform) {
            platforms.push(e.platform);
        }
    }
    let suggested = if platforms.len() > 1 || platforms.contains(&"adaptive") {
        "adaptive"
    } else {
        platforms[0]
    };
    let declared = if platform == Some("web") {
        "PRODUCT.md declares `## Platform: web`"
    } else if product.map(|p| !p.is_empty()).unwrap_or(false) {
        "PRODUCT.md has no `## Platform` section, so the project resolves to web"
    } else {
        "no PRODUCT.md declares a platform, so the project resolves to web"
    };
    vec![finding(
        "platform-native-evidence",
        "PRODUCT.md",
        product_path.map(|s| s.to_string()),
        "mention",
        format!(
            "{}, but the project carries {}. Web guidance is being applied to a native codebase, and the iOS and Android references never load.",
            declared,
            evidence.iter().map(|e| e.reason).collect::<Vec<_>>().join(" and ")
        ),
        format!(
            "Ask the user whether `## Platform` should be `{}`. If it should, write the value and load the matching native reference before designing.",
            suggested
        ),
    )]
}

pub fn js_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// JS: checkDesignSidecar
pub fn check_design_sidecar(
    design_path: Option<&str>,
    sidecar_path: &str,
    project_root: &str,
) -> Vec<Finding> {
    let mut out = Vec::new();
    if !exists(sidecar_path) {
        return out;
    }
    let rel_present = to_relative(Some(sidecar_path), project_root).unwrap();
    let sidecar = read_json(sidecar_path);
    let schema_version = read_sidecar_schema_version(sidecar.as_ref());
    if let Some(sc) = &sidecar {
        if js_truthy(sc)
            && (schema_version.is_none() || schema_version.unwrap() < DESIGN_SIDECAR_SCHEMA_VERSION)
        {
            out.push(finding(
                "design-sidecar-schema-outdated",
                DESIGN_SIDECAR_FILE,
                Some(rel_present.clone()),
                "route",
                format!(
                    "{} is schemaVersion {}; the current sidecar is {}. Token primitives moved to the DESIGN.md frontmatter, so the old shape carries values that are now read from two places.",
                    rel_present,
                    schema_version.map(|v| v.to_string()).unwrap_or_else(|| "unset".to_string()),
                    DESIGN_SIDECAR_SCHEMA_VERSION
                ),
                "Offer `document` to regenerate the sidecar. It reads the existing DESIGN.md, so no interview is needed.".to_string(),
            ));
        }
    }
    if let Some(dp) = design_path {
        let dm = mtime_ms(dp);
        let sm = mtime_ms(sidecar_path);
        if let (Some(d), Some(s)) = (dm, sm) {
            if d > s {
                out.push(finding(
                    "design-sidecar-stale",
                    DESIGN_SIDECAR_FILE,
                    Some(rel_present.clone()),
                    "mention",
                    format!(
                        "DESIGN.md was edited after {} was generated, so the sidecar's ramps, shadows, motion tokens, and component snippets may contradict it.",
                        rel_present
                    ),
                    "Offer `document` to refresh the sidecar, preserving DESIGN.md.".to_string(),
                ));
            }
        }
    }
    out
}

/// A `.impeccino/` directory left by the layout before docs/adr/0020. One
/// stat at the project root, so it is cheap enough for the boot (Tier 1).
/// The home directory is skipped: an older launcher kept its engine cache in
/// `~/.impeccino/`.
pub fn check_legacy_state_dir(project_root: &str, home: Option<&str>) -> Vec<Finding> {
    if project_root.is_empty() {
        return vec![];
    }
    if let Some(h) = home.filter(|h| !h.is_empty()) {
        if jsp::resolve(h, &[]) == jsp::resolve(project_root, &[]) {
            return vec![];
        }
    }
    if !is_dir(&jsp::join(&[project_root, LEGACY_STATE_DIR])) {
        return vec![];
    }
    vec![finding(
        "legacy-state-dir",
        ".impeccino/",
        Some(format!("{}/", LEGACY_STATE_DIR)),
        "mention",
        "A `.impeccino/` directory from an earlier Impeccino layout sits at the project root. Nothing reads it any more: project state now lives in top-level files and Impeccino keeps no config file.".to_string(),
        "Tell the user where its contents belong, then offer to delete the directory once they have moved what they want to keep: `design.json` becomes `DESIGN.json` next to DESIGN.md; each `surfaces/*.md` brief becomes a section of `SURFACES.md` (write it with `impeccino surface-brief write`); detector ignores in `config.json` and `config.local.json` become `<!-- impeccino-disable <rule> -->` waivers or declared tokens in DESIGN.md, or `.gitignore` / `.gitattributes` entries for whole files; decisions in `critique/ignore.md` become brand commitments in PRODUCT.md or rules in DESIGN.md. Everything else (critique snapshots, hook caches, review screenshots) can be deleted.".to_string(),
    )]
}

pub fn unique_roots(a: &str, b: Option<&str>) -> Vec<String> {
    let mut roots = vec![jsp::resolve(a, &[])];
    if let Some(b) = b {
        if !b.is_empty() {
            let r = jsp::resolve(b, &[]);
            if !roots.contains(&r) {
                roots.push(r);
            }
        }
    }
    roots
}

/// JS: checkSurfaceBriefs
pub fn check_surface_briefs(candidates: &[BriefSummary], project_root: &str) -> Vec<Finding> {
    if project_root.is_empty() {
        return vec![];
    }
    let mut orphaned: Vec<&BriefSummary> = Vec::new();
    for b in candidates {
        let t = b.primary_target.as_str();
        if t.is_empty() {
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") || t.starts_with("route:")
        {
            continue;
        }
        if !exists(&jsp::join(&[project_root, t])) {
            orphaned.push(b);
        }
    }
    if orphaned.is_empty() {
        return vec![];
    }
    let path = orphaned
        .first()
        .map(|b| b.path.clone())
        .filter(|p| !p.is_empty());
    vec![finding(
        "surface-brief-orphaned",
        SURFACES_FILE,
        path,
        "mention",
        format!(
            "{} surface brief(s) in {} name a primary target that no longer exists: {}.",
            orphaned.len(),
            SURFACES_FILE,
            orphaned.iter().map(|b| format!("`{}`", b.primary_target)).collect::<Vec<_>>().join(", ")
        ),
        format!(
            "Ask whether the surface moved (write its brief again under the new target and delete the old section) or was removed (delete its section from {}). Until then the brief is authority for a file that is gone.",
            SURFACES_FILE
        ),
    )]
}

pub struct BootExtras {
    pub abs_design_path: Option<String>,
    pub sidecar_path: String,
    /// The user's home directory, which never counts as a project with a
    /// leftover `.impeccino/`.
    pub home: Option<String>,
}

/// JS: collectBootFindingGroups(ctx, extras) — the boot artifact checks
/// grouped by artifact, so deeper reports (doctor) can interleave their own
/// checks without rebuilding this policy (upstream 80997663).
pub struct BootFindingGroups {
    pub legacy_state: Vec<Finding>,
    pub product: Vec<Finding>,
    pub native_platform: Vec<Finding>,
    pub design_sidecar: Vec<Finding>,
    pub surface_briefs: Vec<Finding>,
}

pub fn collect_boot_finding_groups(ctx: &Ctx, cwd: &str, extras: &BootExtras) -> BootFindingGroups {
    let project_root = if ctx.project_root.is_empty() {
        cwd.to_string()
    } else {
        ctx.project_root.clone()
    };
    BootFindingGroups {
        legacy_state: check_legacy_state_dir(&project_root, extras.home.as_deref()),
        product: check_product(
            ctx.product.as_deref(),
            ctx.product_path.as_deref().unwrap_or("PRODUCT.md"),
        ),
        // Only checked once a PRODUCT.md exists. Without one the boot already
        // emits NO_PRODUCT_MD and routes into init, which asks for the
        // platform directly; a second signal saying the same thing is noise.
        native_platform: if ctx
            .product
            .as_deref()
            .map(|p| !p.is_empty())
            .unwrap_or(false)
        {
            check_native_platform_evidence(
                &project_root,
                ctx.platform.as_deref(),
                ctx.product.as_deref(),
                ctx.product_path.as_deref(),
            )
        } else {
            Vec::new()
        },
        design_sidecar: check_design_sidecar(
            extras.abs_design_path.as_deref(),
            &extras.sidecar_path,
            &project_root,
        ),
        surface_briefs: check_surface_briefs(&ctx.surface_brief_candidates, &project_root),
    }
}

/// JS: collectBootFindings(ctx, extras)
pub fn collect_boot_findings(ctx: &Ctx, cwd: &str, extras: &BootExtras) -> Vec<Finding> {
    let groups = collect_boot_finding_groups(ctx, cwd, extras);
    let mut out = Vec::new();
    out.extend(groups.legacy_state);
    out.extend(groups.product);
    out.extend(groups.native_platform);
    out.extend(groups.design_sidecar);
    out.extend(groups.surface_briefs);
    out
}

pub static _UNUSED: Lazy<()> = Lazy::new(|| ());
