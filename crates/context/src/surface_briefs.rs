//! Surface briefs: one top-level `SURFACES.md` per project, one section per
//! surface (docs/adr/0020-project-state-is-top-level-files.md).
//!
//! A section is a `## <target>` heading followed directly by a marker line
//! that carries the normalized targets as JSON:
//!
//! ```text
//! ## src/pages/index.astro
//! <!-- impeccino:surface {"target":"src/pages/index.astro","related":["route:/"]} -->
//!
//! Mode: Persuade
//! ...
//! ```
//!
//! The marker, not the heading, is the authority: a section runs from its
//! heading to the heading of the next marked section, so a brief body may
//! carry headings of its own (`### Direction contract`, even another `##`).
//! Markers inside fenced code blocks are ignored. `write` replaces exactly
//! the section whose marker names the primary target, or appends one, and
//! leaves every other byte of the other sections alone.

use crate::jsp;
use crate::url;
use crate::util::{exists, js_trim, safe_read};
use impeccino_common::project_files::SURFACES_FILE;
use serde_json::Value;

const MARKER_PREFIX: &str = "<!-- impeccino:surface ";
const MARKER_SUFFIX: &str = "-->";

const DEFAULT_PREAMBLE: &str = "# Surfaces\n\nPer-surface strategy: each section holds one surface's mode, direction contract, and other decisions that belong to that route or artifact alone. `impeccino surface-brief write` replaces a section by the target named in its marker comment. Edit the prose freely, but keep each heading and the marker line below it together.";

/// `<project root>/SURFACES.md`.
pub fn surfaces_path(project_root: &str) -> String {
    jsp::join(&[project_root, SURFACES_FILE])
}

fn normalize_route_target(route: &str) -> Option<String> {
    if !route.starts_with('/') || route.contains("..") {
        return None;
    }
    let cut = route.split(|c| c == '?' || c == '#').next().unwrap_or("");
    // collapse //+ -> /
    let mut collapsed = String::with_capacity(cut.len());
    let mut prev_slash = false;
    for c in cut.chars() {
        if c == '/' {
            if !prev_slash {
                collapsed.push('/');
            }
            prev_slash = true;
        } else {
            prev_slash = false;
            collapsed.push(c);
        }
    }
    // strip one trailing '/'
    let stripped = collapsed.strip_suffix('/').unwrap_or(&collapsed);
    let normalized = if stripped.is_empty() { "/" } else { stripped };
    Some(format!("route:{}", normalized))
}

/// JS: normalizeSurfaceTarget(target, { projectRoot })
pub fn normalize_surface_target(target: Option<&str>, project_root: &str) -> Option<String> {
    let target = target?;
    let trimmed = js_trim(target);
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        let mut u = url::parse(trimmed)?;
        u.hash.clear();
        u.search.clear();
        let s = u.to_string();
        let s = s.strip_suffix('/').unwrap_or(&s).to_string();
        return Some(if s.is_empty() { u.origin() } else { s });
    }
    if lower.starts_with("route:") {
        let idx = trimmed.find(':').unwrap();
        return normalize_route_target(js_trim(&trimmed[idx + 1..]));
    }
    if trimmed == "/" {
        return normalize_route_target(trimmed);
    }
    if trimmed.starts_with('/') {
        let absolute = jsp::resolve(project_root, &[trimmed]);
        let rel = jsp::relative(project_root, project_root, &absolute);
        let is_project_file = !rel.is_empty() && !rel.starts_with("..") && !jsp::is_absolute(&rel);
        if !is_project_file && !exists(&absolute) {
            return normalize_route_target(trimmed);
        }
    }
    let abs = if jsp::is_absolute(trimmed) {
        trimmed.to_string()
    } else {
        jsp::resolve(project_root, &[trimmed])
    };
    let rel = jsp::relative(project_root, project_root, &abs);
    if rel.is_empty() || rel == "." || rel.starts_with("..") || jsp::is_absolute(&rel) {
        return None;
    }
    Some(jsp::to_posix(&rel))
}

#[derive(Debug, Clone)]
pub struct SurfaceBrief {
    /// Absolute path of the SURFACES.md that holds this brief.
    pub path: String,
    /// The whole section: heading, marker, and body.
    pub text: String,
    /// The section body below the marker, trimmed.
    pub body: String,
    pub primary_target: String,
    pub related_targets: Vec<String>,
    /// Primary first, then related.
    pub targets: Vec<String>,
}

/// One marked section of SURFACES.md.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Heading through the end of the body, trailing whitespace trimmed.
    pub raw: String,
    pub target: String,
    pub related: Vec<String>,
    pub body: String,
}

/// SURFACES.md split into the text before the first section and the sections.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfacesDoc {
    pub preamble: String,
    pub sections: Vec<Section>,
}

/// The marker's targets, or None when the line is not a well-formed marker.
fn parse_marker(line: &str) -> Option<(String, Vec<String>)> {
    let t = line.trim();
    let inner = t.strip_prefix(MARKER_PREFIX)?.strip_suffix(MARKER_SUFFIX)?;
    let v: Value = serde_json::from_str(inner.trim()).ok()?;
    let target = v.get("target")?.as_str()?.trim().to_string();
    if target.is_empty() {
        return None;
    }
    let related = match v.get("related") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    Some((target, related))
}

/// Parse SURFACES.md text. Line endings are normalized to `\n`.
pub fn parse_surfaces(text: &str) -> SurfacesDoc {
    let text = text.replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let outside_fence = impeccino_core::inline_ignores::markdown_fenced_code_line_mask(&text);
    // (start line, marker line, target, related)
    let mut starts: Vec<(usize, usize, String, Vec<String>)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if !outside_fence[i] {
            continue;
        }
        let Some((target, related)) = parse_marker(line) else {
            continue;
        };
        let start = if i > 0 && lines[i - 1].starts_with("## ") {
            i - 1
        } else {
            i
        };
        // A heading already claimed by the previous marker cannot start
        // another section; fall back to the marker line itself.
        let start = match starts.last() {
            Some((_, prev_marker, _, _)) if start <= *prev_marker => i,
            _ => start,
        };
        starts.push((start, i, target, related));
    }
    let preamble_end = starts.first().map(|s| s.0).unwrap_or(lines.len());
    let preamble = lines[..preamble_end].join("\n").trim_end().to_string();
    let mut sections = Vec::new();
    for (n, (start, marker, target, related)) in starts.iter().enumerate() {
        let end = starts.get(n + 1).map(|s| s.0).unwrap_or(lines.len());
        let raw = lines[*start..end].join("\n").trim_end().to_string();
        let body = js_trim(&lines[marker + 1..end].join("\n")).to_string();
        sections.push(Section {
            raw,
            target: target.clone(),
            related: related.clone(),
            body,
        });
    }
    SurfacesDoc { preamble, sections }
}

/// JSON for the marker line; `>` is escaped so a target can never close the
/// comment early.
fn marker_json(target: &str, related: &[String]) -> String {
    let v = serde_json::json!({ "target": target, "related": related });
    serde_json::to_string(&v).unwrap().replace('>', "\\u003e")
}

/// The section text `write` produces for one surface.
pub fn render_section(target: &str, related: &[String], body: &str) -> String {
    let head = format!(
        "## {}\n{}{} {}",
        target,
        MARKER_PREFIX,
        marker_json(target, related),
        MARKER_SUFFIX
    );
    let body = js_trim(body);
    if body.is_empty() {
        head
    } else {
        format!("{}\n\n{}", head, body)
    }
}

/// Serialize a parsed document: preamble, then every section, separated by
/// one blank line.
pub fn render_surfaces(doc: &SurfacesDoc) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if !doc.preamble.is_empty() {
        parts.push(&doc.preamble);
    }
    for s in &doc.sections {
        parts.push(&s.raw);
    }
    format!("{}\n", parts.join("\n\n"))
}

/// Every brief in SURFACES.md, in file order. Empty when the file is missing.
pub fn list_surface_briefs(project_root: &str) -> Vec<SurfaceBrief> {
    let path = surfaces_path(project_root);
    let Some(text) = safe_read(&path) else {
        return vec![];
    };
    parse_surfaces(&text)
        .sections
        .into_iter()
        .map(|s| {
            let mut targets = vec![s.target.clone()];
            targets.extend(s.related.iter().cloned());
            SurfaceBrief {
                path: path.clone(),
                text: s.raw,
                body: s.body,
                primary_target: s.target,
                related_targets: s.related,
                targets,
            }
        })
        .collect()
}

pub struct SurfaceResolution {
    pub brief: Option<SurfaceBrief>,
    pub candidates: Vec<SurfaceBrief>,
    pub reason: &'static str,
}

/// Find the brief for `target`: the section whose primary target matches,
/// else the one section that lists it as related. Without a target, the
/// only brief, if there is exactly one.
pub fn resolve_surface_brief(project_root: &str, target: Option<&str>) -> SurfaceResolution {
    let briefs = list_surface_briefs(project_root);
    let Some(target) = target.filter(|t| !t.is_empty()) else {
        let n = briefs.len();
        return SurfaceResolution {
            brief: if n == 1 {
                Some(briefs[0].clone())
            } else {
                None
            },
            reason: if n == 1 {
                "only-brief"
            } else if n > 1 {
                "ambiguous"
            } else {
                "none"
            },
            candidates: briefs,
        };
    };
    let Some(normalized) = normalize_surface_target(Some(target), project_root) else {
        return SurfaceResolution {
            brief: None,
            candidates: briefs,
            reason: "invalid-target",
        };
    };
    if let Some(exact) = briefs.iter().find(|b| b.primary_target == normalized) {
        return SurfaceResolution {
            brief: Some(exact.clone()),
            candidates: briefs,
            reason: "primary",
        };
    }
    let mapped: Vec<SurfaceBrief> = briefs
        .iter()
        .filter(|b| b.related_targets.contains(&normalized))
        .cloned()
        .collect();
    let n = mapped.len();
    SurfaceResolution {
        brief: if n == 1 {
            Some(mapped[0].clone())
        } else {
            None
        },
        candidates: if n > 1 { mapped } else { briefs },
        reason: if n == 1 {
            "mapping"
        } else if n > 1 {
            "ambiguous-target"
        } else {
            "not-found"
        },
    }
}

/// Write one surface's section into SURFACES.md: replace the section whose
/// marker names the same primary target, or append a new one. Returns the
/// file path.
pub fn write_surface_brief(
    project_root: &str,
    primary_target: &str,
    related_targets: &[String],
    body: &str,
) -> Result<String, String> {
    let normalized_primary = normalize_surface_target(Some(primary_target), project_root)
        .ok_or_else(|| {
            "surface brief requires a concrete project-relative primary target or URL".to_string()
        })?;
    let mut related: Vec<String> = Vec::new();
    for t in related_targets {
        if let Some(n) = normalize_surface_target(Some(t), project_root) {
            if n != normalized_primary && !related.contains(&n) {
                related.push(n);
            }
        }
    }
    let file_path = surfaces_path(project_root);
    let mut doc = match safe_read(&file_path) {
        Some(text) => parse_surfaces(&text),
        None => SurfacesDoc {
            preamble: DEFAULT_PREAMBLE.to_string(),
            sections: vec![],
        },
    };
    let section = Section {
        raw: render_section(&normalized_primary, &related, body),
        target: normalized_primary.clone(),
        related,
        body: js_trim(body).to_string(),
    };
    match doc
        .sections
        .iter()
        .position(|s| s.target == normalized_primary)
    {
        Some(i) => doc.sections[i] = section,
        None => doc.sections.push(section),
    }
    std::fs::write(&file_path, render_surfaces(&doc)).map_err(|e| e.to_string())?;
    Ok(file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TMP_SEQ: AtomicUsize = AtomicUsize::new(0);

    fn tmp() -> String {
        let root = std::env::temp_dir().join(format!(
            "impeccino-surfaces-{}-{}",
            std::process::id(),
            TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        root.to_string_lossy().into_owned()
    }

    #[test]
    fn body_headings_and_fenced_markers_stay_inside_their_section() {
        let text = "# Surfaces\n\nIntro.\n\n## a.html\n<!-- impeccino:surface {\"target\":\"a.html\",\"related\":[]} -->\n\n## Direction contract\nTHESIS: one.\n\n```md\n## b.html\n<!-- impeccino:surface {\"target\":\"b.html\"} -->\n```\n\n## c.html\n<!-- impeccino:surface {\"target\":\"c.html\",\"related\":[\"route:/c\"]} -->\n\nC body.\n";
        let doc = parse_surfaces(text);
        assert_eq!(doc.preamble, "# Surfaces\n\nIntro.");
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(doc.sections[0].target, "a.html");
        assert!(doc.sections[0].body.starts_with("## Direction contract"));
        assert!(
            doc.sections[0].body.contains("b.html"),
            "fenced marker stays body text"
        );
        assert_eq!(doc.sections[1].related, vec!["route:/c".to_string()]);
        assert_eq!(doc.sections[1].body, "C body.");
        assert_eq!(render_surfaces(&doc), text);
    }

    #[test]
    fn markers_inside_shorter_fences_remain_body_text() {
        let text = "## a.html\n<!-- impeccino:surface {\"target\":\"a.html\"} -->\n\n````md\n```html\n## fake.html\n<!-- impeccino:surface {\"target\":\"fake.html\"} -->\n```\n````\n";
        let doc = parse_surfaces(text);
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].target, "a.html");
        assert!(doc.sections[0].body.contains("fake.html"));
    }

    #[test]
    fn malformed_markers_are_body_text() {
        let doc = parse_surfaces("## a\n<!-- impeccino:surface {not json} -->\n## b\n<!-- impeccino:surface {\"related\":[]} -->\n");
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn write_replaces_exactly_one_section() {
        let root = tmp();
        write_surface_brief(&root, "a.html", &[], "A one.").unwrap();
        write_surface_brief(&root, "b.html", &["route:/b/".to_string()], "B one.").unwrap();
        write_surface_brief(&root, "c.html", &[], "C one.").unwrap();
        let before = std::fs::read_to_string(surfaces_path(&root)).unwrap();
        write_surface_brief(
            &root,
            "b.html",
            &[],
            "B two.\n\n### Direction contract\nTHESIS: two.",
        )
        .unwrap();
        let after = std::fs::read_to_string(surfaces_path(&root)).unwrap();
        let doc = parse_surfaces(&after);
        assert_eq!(
            doc.sections
                .iter()
                .map(|s| s.target.as_str())
                .collect::<Vec<_>>(),
            vec!["a.html", "b.html", "c.html"]
        );
        assert!(doc.sections[1].related.is_empty());
        assert!(doc.sections[1].body.ends_with("THESIS: two."));
        let unchanged = |t: &str| {
            parse_surfaces(&before)
                .sections
                .into_iter()
                .find(|s| s.target == t)
                .unwrap()
                .raw
        };
        assert_eq!(doc.sections[0].raw, unchanged("a.html"));
        assert_eq!(doc.sections[2].raw, unchanged("c.html"));
        assert_eq!(
            resolve_surface_brief(&root, Some("b.html")).reason,
            "primary"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn markers_cannot_be_closed_by_a_target() {
        let s = render_section("https://example.com/a-->b", &[], "x");
        let doc = parse_surfaces(&s);
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].target, "https://example.com/a-->b");
    }
}
