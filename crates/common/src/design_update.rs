//! Preservation-aware draft preview; this module performs no filesystem I/O.
use crate::design_document::{
    lines, parse_design_md, validate, DesignDocument, Diagnostic, TOKEN_GROUPS,
};
use crate::design_frontmatter::pointer;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::LazyLock};

#[derive(Debug, Default, Clone, Serialize)]
pub struct Removals {
    pub tokens: Vec<String>,
    pub waivers: Vec<String>,
    pub metadata: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Change {
    pub path: String,
    pub kind: &'static str,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct UpdatePreview {
    pub content: String,
    pub diagnostics: Vec<Diagnostic>,
    pub changes: Vec<Change>,
    pub preserved_sections: Vec<String>,
}

impl UpdatePreview {
    pub fn valid(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
    }
}

pub fn preview(original: &str, draft: &str, removals: &Removals) -> UpdatePreview {
    preview_inner(original, draft, removals, true)
}

fn preview_inner(
    original: &str,
    draft: &str,
    removals: &Removals,
    preserve_foreign: bool,
) -> UpdatePreview {
    let before = parse_design_md(original);
    let proposal = parse_design_md(draft);
    let proposed_metadata = preserve_foreign
        .then(|| {
            proposal
                .metadata_range
                .as_ref()
                .map(|range| draft[range.clone()].to_string())
        })
        .flatten();
    let mut staged = draft.to_string();
    if preserve_foreign {
        if let Some(range) = &proposal.metadata_range {
            staged.replace_range(range.clone(), "");
        }
    }
    let mut content = if preserve_foreign {
        preserve_foreign_frontmatter(original, &staged, &before)
    } else {
        staged
    };
    let mut preserved_sections = Vec::new();
    for section in before
        .sections
        .iter()
        .filter(|section| preserve_foreign && section.canonical.is_none())
    {
        let mut replacement = original[section.range.clone()].to_string();
        if let Some(range) = before
            .metadata_range
            .as_ref()
            .filter(|range| range.start >= section.range.start && range.end <= section.range.end)
        {
            replacement.replace_range(
                range.start - section.range.start..range.end - section.range.start,
                "",
            );
        }
        let candidate = parse_design_md(&content);
        if let Some(current) = candidate
            .sections
            .iter()
            .find(|current| current.name.eq_ignore_ascii_case(&section.name))
        {
            if content[current.range.clone()] != replacement {
                if current.range.end < content.len() && !replacement.ends_with('\n') {
                    replacement.push('\n');
                }
                content.replace_range(current.range.clone(), &replacement);
                preserved_sections.push(section.name.clone());
            }
        } else {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&replacement);
            preserved_sections.push(section.name.clone());
        }
    }
    if let Some(metadata) = proposed_metadata {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&metadata);
    }
    let after = parse_design_md(&content);
    let mut result = UpdatePreview {
        diagnostics: validate(&content),
        content,
        changes: Vec::new(),
        preserved_sections,
    };
    for diagnostic in proposal
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.code == "metadata-syntax")
    {
        if !result
            .diagnostics
            .iter()
            .any(|existing| existing.code == "metadata-syntax")
        {
            result.diagnostics.push(diagnostic);
        }
    }
    for error in &before.diagnostics {
        let mut diagnostic = error.clone();
        diagnostic.source = "existing";
        diagnostic.code = "existing-syntax";
        diagnostic.message = format!(
            "Repair the existing record before a controlled update: {}",
            diagnostic.message
        );
        result.diagnostics.push(diagnostic);
    }
    let old_tokens = token_values(&before);
    let new_tokens = token_values(&after);
    for path in &removals.tokens {
        let valid = TOKEN_GROUPS
            .iter()
            .any(|group| path == &format!("/{group}") || path.starts_with(&format!("/{group}/")))
            && old_tokens
                .keys()
                .any(|token| authorized(token, std::slice::from_ref(path)));
        if !valid {
            result.diagnostics.push(existing_diagnostic(&before, "removal-selector", path, "This token removal selector does not match an existing token; use its JSON pointer"));
        }
    }
    for (path, value) in &old_tokens {
        let next = new_tokens.get(path);
        if next.is_none() && !authorized(path, &removals.tokens) {
            result.diagnostics.push(existing_diagnostic(
                &before,
                "token-removed",
                path,
                "This draft drops a declared token; restore it or explicitly authorize its removal",
            ));
        }
        if next != Some(value) {
            result.changes.push(Change {
                path: path.clone(),
                kind: if next.is_some() { "changed" } else { "removed" },
                before: Some(value.clone()),
                after: next.cloned(),
            });
        }
    }
    for (path, value) in &new_tokens {
        if !old_tokens.contains_key(path) {
            result.changes.push(Change {
                path: path.clone(),
                kind: "added",
                before: None,
                after: Some(value.clone()),
            });
        }
    }
    let old_waivers = waivers(original);
    let new_waivers = waivers(&result.content);
    for rule in &removals.waivers {
        if !old_waivers
            .iter()
            .any(|(_, rules, _)| rules.contains(&rule.to_lowercase()))
        {
            result.diagnostics.push(existing_diagnostic(
                &before,
                "removal-selector",
                &format!("/waivers/{rule}"),
                "This removal does not name an existing active waiver",
            ));
        }
    }
    let mut remaining: Vec<&str> = new_waivers
        .iter()
        .map(|(text, _, _)| text.as_str())
        .collect();
    for (comment, rules, line) in old_waivers {
        if let Some(index) = remaining.iter().position(|text| *text == comment) {
            remaining.remove(index);
            continue;
        }
        let path = format!("/waivers/{}", rules.join(","));
        if !rules.iter().all(|rule| {
            removals
                .waivers
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(rule))
        }) {
            let mut diagnostic = existing_diagnostic(&before, "waiver-removed", &path, "Preserve the existing waiver comment verbatim outside example fences, or explicitly authorize every rule's removal");
            diagnostic.line = line;
            result.diagnostics.push(diagnostic);
        }
        result.changes.push(Change {
            path,
            kind: "removed",
            before: Some(Value::String(comment)),
            after: None,
        });
    }
    for path in &removals.metadata {
        if before
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.pointer(path))
            .is_none()
        {
            result.diagnostics.push(existing_diagnostic(
                &before,
                "removal-selector",
                &format!("/metadata{path}"),
                "This metadata removal selector does not match an existing value",
            ));
        }
    }
    if let Some(metadata) = &before.metadata {
        compare_metadata(
            &before,
            "",
            metadata,
            after.metadata.as_ref(),
            removals,
            &mut result,
        );
    }
    if let Some(metadata) = &after.metadata {
        let mut additions = BTreeMap::new();
        flatten(metadata, "", &mut additions);
        for (path, value) in additions {
            if before
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.pointer(&path))
                .is_none()
            {
                result.changes.push(Change {
                    path: format!("/metadata{path}"),
                    kind: "added",
                    before: None,
                    after: Some(value),
                });
            }
        }
    }
    result
}

/// Replace only the marked metadata block, keeping all other Markdown bytes.
pub fn metadata_preview(original: &str, draft: &str, removals: &Removals) -> UpdatePreview {
    let before = parse_design_md(original);
    let proposal = parse_design_md(draft);
    if proposal
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "metadata-syntax")
    {
        return UpdatePreview {
            content: original.into(),
            diagnostics: proposal.diagnostics,
            changes: Vec::new(),
            preserved_sections: Vec::new(),
        };
    }
    let mut replacement = proposal
        .metadata_range
        .as_ref()
        .map(|range| &draft[range.clone()])
        .unwrap_or("")
        .to_string();
    let mut updated = original.to_string();
    if let Some(range) = before.metadata_range {
        if range.end < original.len() && !replacement.is_empty() && !replacement.ends_with('\n') {
            replacement.push('\n');
        }
        updated.replace_range(range, &replacement);
    } else if !replacement.is_empty() {
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(&replacement);
    }
    preview_inner(original, &updated, removals, false)
}

fn existing_diagnostic(
    document: &DesignDocument,
    code: &'static str,
    path: &str,
    message: impl Into<String>,
) -> Diagnostic {
    let mut diagnostic = document.diagnostic(code, path, message);
    diagnostic.source = "existing";
    diagnostic
}

fn authorized(path: &str, selectors: &[String]) -> bool {
    selectors.iter().any(|selector| {
        path == selector
            || path
                .strip_prefix(selector)
                .is_some_and(|tail| tail.starts_with('/'))
    })
}

fn flatten(value: &Value, path: &str, values: &mut BTreeMap<String, Value>) {
    if let Some(mapping) = value.as_object().filter(|mapping| !mapping.is_empty()) {
        for (key, value) in mapping {
            flatten(
                value,
                &format!("{path}{}", pointer(std::slice::from_ref(key))),
                values,
            );
        }
    } else {
        values.insert(path.to_string(), value.clone());
    }
}

fn token_values(document: &DesignDocument) -> BTreeMap<String, Value> {
    let mut values = BTreeMap::new();
    if let Some(frontmatter) = &document.frontmatter {
        for group in TOKEN_GROUPS {
            if let Some(tokens) = frontmatter.get(group).and_then(Value::as_object) {
                for (key, value) in tokens {
                    flatten(
                        value,
                        &format!("/{group}{}", pointer(std::slice::from_ref(key))),
                        &mut values,
                    );
                }
            }
        }
    }
    values
}

fn root_ranges(
    source: &str,
    document: &DesignDocument,
) -> BTreeMap<String, std::ops::Range<usize>> {
    let Some(frontmatter) = &document.frontmatter_range else {
        return BTreeMap::new();
    };
    let records = lines(source);
    let end = records
        .iter()
        .find(|line| line.range.end == frontmatter.end)
        .map(|line| line.range.start)
        .unwrap_or(frontmatter.end);
    let mut roots: Vec<_> = document
        .locations
        .iter()
        .filter(|(path, _)| path.matches('/').count() == 1)
        .collect();
    roots.sort_by_key(|(_, line)| **line);
    roots
        .iter()
        .enumerate()
        .map(|(index, (path, line))| {
            let start = records[**line - 1].range.start;
            let next = roots
                .get(index + 1)
                .map(|(_, line)| records[**line - 1].range.start)
                .unwrap_or(end);
            ((*path).clone(), start..next)
        })
        .collect()
}

fn preserve_foreign_frontmatter(original: &str, draft: &str, before: &DesignDocument) -> String {
    let mut content = draft.to_string();
    let mut fields: Vec<_> = root_ranges(original, before).into_iter().collect();
    fields.sort_by_key(|(_, range)| range.start);
    for (path, range) in fields {
        if TOKEN_GROUPS
            .iter()
            .chain(["name", "description", "version"].iter())
            .any(|group| path == format!("/{group}"))
        {
            continue;
        }
        let candidate = parse_design_md(&content);
        let ranges = root_ranges(&content, &candidate);
        if let Some(current) = ranges.get(&path) {
            content.replace_range(current.clone(), &original[range]);
        } else if let Some(frontmatter) = &candidate.frontmatter_range {
            let records = lines(&content);
            let closing = records
                .iter()
                .find(|line| line.range.end == frontmatter.end)
                .unwrap()
                .range
                .start;
            content.insert_str(closing, &original[range]);
        } else {
            content = format!("---\n{}---\n{content}", &original[range]);
        }
    }
    content
}

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<!--(.*?)-->").unwrap());
static DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)impeccino-disable(?-u:\b)([^\n\r]*)").unwrap());
static REASON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*(?:--+|:)\s*").unwrap());

fn waivers(source: &str) -> Vec<(String, Vec<String>, usize)> {
    let mut masked = source.as_bytes().to_vec();
    for line in lines(source) {
        if !line.visible {
            for byte in &mut masked[line.range] {
                if !matches!(*byte, b'\n' | b'\r') {
                    *byte = b' ';
                }
            }
        }
    }
    let masked = String::from_utf8(masked).expect("Masking complete lines leaves UTF-8 intact");
    COMMENT
        .captures_iter(&masked)
        .filter_map(|comment| {
            let body = comment[1].replace(['\n', '\r'], " ");
            let directive = DIRECTIVE.captures(&body)?;
            if directive[1].starts_with('-') {
                return None;
            }
            let list = REASON.split(&directive[1]).next().unwrap_or("");
            let rules: Vec<_> = list
                .split(|ch: char| crate::design_frontmatter::is_yaml_whitespace(ch) || ch == ',')
                .filter(|rule| !rule.is_empty())
                .map(str::to_lowercase)
                .collect();
            if rules.is_empty() || rules.iter().any(|rule| rule == "*") {
                return None;
            }
            let range = comment.get(0).unwrap().range();
            let line = source[..range.start]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            Some((source[range].to_string(), rules, line))
        })
        .collect()
}

fn consumed_metadata(path: &str) -> bool {
    let parts: Vec<_> = path.split('/').skip(1).collect();
    matches!(
        parts.as_slice(),
        ["schemaVersion"]
            | ["extensions"]
            | ["extensions", "colorMeta"]
            | ["extensions", "roundedMeta"]
            | ["extensions", "shadows"]
            | ["extensions", "colorMeta", _]
            | ["extensions", "roundedMeta", _]
            | ["extensions", "colorMeta", _, "canonical" | "tonalRamp", ..]
            | [
                "extensions",
                "roundedMeta",
                _,
                "canonical" | "value" | "values" | "aliases" | "role",
                ..
            ]
            | ["extensions", "shadows", _, "value"]
    )
}

fn compare_metadata(
    document: &DesignDocument,
    path: &str,
    before: &Value,
    after: Option<&Value>,
    removals: &Removals,
    result: &mut UpdatePreview,
) {
    if path.is_empty() && before.as_object().is_some_and(|mapping| mapping.is_empty()) {
        return;
    }
    if after.is_some_and(|value| value != before)
        && !before.is_object()
        && !consumed_metadata(path)
        && !authorized(path, &removals.metadata)
    {
        result.diagnostics.push(existing_diagnostic(
            document,
            "metadata-protected",
            &format!("/metadata{path}"),
            "Preserve this opaque metadata value, or explicitly authorize replacing the old value",
        ));
    }
    if let Some(mapping) = before.as_object().filter(|mapping| !mapping.is_empty()) {
        for (key, value) in mapping {
            compare_metadata(
                document,
                &format!("{path}{}", pointer(std::slice::from_ref(key))),
                value,
                after.and_then(|after| after.get(key)),
                removals,
                result,
            );
        }
        return;
    }
    if let (Some(old), Some(new)) = (before.as_array(), after.and_then(Value::as_array)) {
        let mut remaining: Vec<_> = new.iter().collect();
        for (index, value) in old.iter().enumerate() {
            if let Some(position) = remaining.iter().position(|new| *new == value) {
                remaining.remove(position);
            } else {
                compare_metadata(
                    document,
                    &format!("{path}/{index}"),
                    value,
                    None,
                    removals,
                    result,
                );
            }
        }
        if before != after.unwrap() {
            result.changes.push(Change {
                path: format!("/metadata{path}"),
                kind: "changed",
                before: Some(before.clone()),
                after: after.cloned(),
            });
        }
        return;
    }
    let qualified = format!("/metadata{path}");
    if after.is_none() && !authorized(path, &removals.metadata) {
        result.diagnostics.push(existing_diagnostic(document, "metadata-removed", &qualified, "This draft drops existing detector metadata; retain the value or explicitly authorize its removal"));
    }
    if after != Some(before) {
        result.changes.push(Change {
            path: qualified,
            kind: if after.is_none() {
                "removed"
            } else {
                "changed"
            },
            before: Some(before.clone()),
            after: after.cloned(),
        });
    }
}

/// One unified hunk around the changed lines; unchanged records have no diff.
pub fn unified_diff(original: &str, updated: &str) -> String {
    if original == updated {
        return String::new();
    }
    let old: Vec<_> = original.split_inclusive('\n').collect();
    let new: Vec<_> = updated.split_inclusive('\n').collect();
    let prefix = old
        .iter()
        .zip(&new)
        .take_while(|(old, new)| old == new)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(old, new)| old == new)
        .count();
    let start = prefix.saturating_sub(3);
    let old_end = (old.len() - suffix + 3).min(old.len());
    let new_end = (new.len() - suffix + 3).min(new.len());
    let mut diff = format!(
        "--- a/DESIGN.md\n+++ b/DESIGN.md\n@@ -{},{} +{},{} @@\n",
        if old.is_empty() { 0 } else { start + 1 },
        old_end - start,
        if new.is_empty() { 0 } else { start + 1 },
        new_end - start
    );
    let add_line = |diff: &mut String, prefix: char, line: &str| {
        diff.push(prefix);
        diff.push_str(line);
        if !line.ends_with('\n') {
            diff.push_str("\n\\ No newline at end of file\n");
        }
    };
    for line in &old[start..prefix] {
        add_line(&mut diff, ' ', line);
    }
    for line in &old[prefix..old.len() - suffix] {
        add_line(&mut diff, '-', line);
    }
    for line in &new[prefix..new.len() - suffix] {
        add_line(&mut diff, '+', line);
    }
    for line in &old[old.len() - suffix..old_end] {
        add_line(&mut diff, ' ', line);
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINAL: &str = "---\nname: Record\ncolors:\n  primary: \"#123456\"\ntypography:\n  body:\n    fontFamily: Example\n---\n<!-- impeccino-disable side-tab: ink rule -->\n## Colors\nOriginal palette.\n## Iconography\nKeep this exact foreign section.\n";
    const DRAFT: &str =
        "---\nname: Record\ncolors:\n  primary: \"#654321\"\n---\n## Colors\nObserved palette.\n";

    #[test]
    fn omission_never_authorizes_token_or_waiver_removal() {
        let result = preview(ORIGINAL, DRAFT, &Removals::default());
        assert!(!result.valid());
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "/typography/body/fontFamily"));
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "waiver-removed"));
        assert!(result
            .content
            .contains("## Iconography\nKeep this exact foreign section.\n"));
    }

    #[test]
    fn explicit_removals_are_scoped_and_unknown_sections_are_kept_verbatim() {
        let result = preview(
            ORIGINAL,
            DRAFT,
            &Removals {
                tokens: vec!["/typography/body".into()],
                waivers: vec!["side-tab".into()],
                ..Default::default()
            },
        );
        assert!(result.valid(), "{:?}", result.diagnostics);
        assert!(result
            .content
            .ends_with("## Iconography\nKeep this exact foreign section.\n"));
        assert_eq!(result.preserved_sections, vec!["Iconography"]);
        assert!(result
            .changes
            .iter()
            .any(|change| change.path == "/colors/primary" && change.kind == "changed"));
    }

    #[test]
    fn a_waiver_in_an_example_does_not_preserve_an_active_waiver() {
        let draft = format!("{DRAFT}\n```md\n<!-- impeccino-disable side-tab: ink rule -->\n```\n");
        let result = preview(
            ORIGINAL,
            &draft,
            &Removals {
                tokens: vec!["/typography/body".into()],
                ..Default::default()
            },
        );
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "waiver-removed"));
    }

    #[test]
    fn metadata_unknown_fields_and_array_values_cannot_disappear_silently() {
        let original = "# Design\n<!-- impeccino:design-metadata -->\n```json\n{\"schemaVersion\":2,\"unknown\":{\"keep\":true},\"extensions\":{\"roundedMeta\":{\"pill\":{\"aliases\":[\"99px\",\"999px\"]}}}}\n```\n";
        let draft = original
            .replace("\"unknown\":{\"keep\":true},", "")
            .replace("\"99px\",", "");
        let result = preview(original, &draft, &Removals::default());
        assert!(!result.valid());
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "/metadata/unknown/keep"));
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path
                    == "/metadata/extensions/roundedMeta/pill/aliases/0")
        );
        let allowed = Removals {
            metadata: vec![
                "/unknown".into(),
                "/extensions/roundedMeta/pill/aliases/0".into(),
            ],
            ..Default::default()
        };
        assert!(preview(original, &draft, &allowed).valid());
    }

    #[test]
    fn foreign_sections_preserve_text_on_both_sides_of_embedded_metadata() {
        let block = "<!-- impeccino:design-metadata -->\n```json\n{\"schemaVersion\":2,\"extensions\":{}}\n```\n";
        let original = format!("## Iconography\nBefore metadata.\n{block}After metadata.\n");
        let draft = format!("## Overview\nUpdated direction.\n{block}");
        let result = preview(&original, &draft, &Removals::default());
        assert!(result.valid(), "{:?}", result.diagnostics);
        assert!(result
            .content
            .contains("Before metadata.\nAfter metadata.\n"));
        assert_eq!(
            result.content.matches("impeccino:design-metadata").count(),
            1
        );
        assert!(result.content.ends_with(block));
        assert_eq!(
            metadata_preview(&original, block, &Removals::default()).content,
            original
        );
    }

    #[test]
    fn foreign_preservation_never_hides_malformed_proposed_metadata() {
        let original = "## Iconography\nKeep this guidance.\n";
        let draft = "## Iconography\nChanged guidance.\n<!-- impeccino:design-metadata -->\n```json\ninvalid\n```\n";
        let result = preview(original, draft, &Removals::default());
        assert!(!result.valid());
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "metadata-syntax"));
    }

    #[test]
    fn opaque_metadata_values_need_explicit_replacement_scope() {
        let original = "<!-- impeccino:design-metadata -->\n```json\n{\"schemaVersion\":2,\"unknown\":{\"keep\":true}}\n```\n";
        let draft = original.replace("true", "false");
        let result = preview(original, &draft, &Removals::default());
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "metadata-protected"));
        assert!(preview(
            original,
            &draft,
            &Removals {
                metadata: vec!["/unknown/keep".into()],
                ..Default::default()
            }
        )
        .valid());
        let empty = "<!-- impeccino:design-metadata -->\n```json\n{}\n```\n";
        assert!(preview(empty, original, &Removals::default()).valid());
    }

    #[test]
    fn unknown_frontmatter_content_is_kept_even_when_the_subset_cannot_interpret_it() {
        let original = "---\nname: Record\nomitted:\n  - section: rounded\n    reason: \"No corners\"\ncustom:\n  note: \"Keep # and Unicode ✓\"\n---\n## Overview\nBrand.\n";
        let draft = "---\nname: Updated\n---\n## Overview\nUpdated brand.\n";
        let result = preview(original, draft, &Removals::default());
        assert!(result.valid(), "{:?}", result.diagnostics);
        assert!(result.content.contains("omitted:\n  - section: rounded\n    reason: \"No corners\"\ncustom:\n  note: \"Keep # and Unicode ✓\"\n"));
    }
}
