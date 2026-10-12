//! Pure DESIGN.md interpretation and validation. Reading remains permissive;
//! only explicit validation and update operations enforce the supported shape.
use crate::design_frontmatter::{parse_mapping, pointer, trim};
use crate::design_metadata::{fence_line, metadata_block, MARKER};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::{collections::BTreeMap, ops::Range, sync::LazyLock};

pub const CANONICAL_SECTIONS: [&str; 8] = [
    "Overview",
    "Colors",
    "Typography",
    "Layout",
    "Elevation",
    "Shapes",
    "Components",
    "Do's and Don'ts",
];
pub const TOKEN_GROUPS: [&str; 5] = ["colors", "typography", "rounded", "spacing", "components"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub source: &'static str,
    pub severity: &'static str,
    pub line: usize,
    pub column: usize,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub canonical: Option<&'static str>,
    pub line: usize,
    pub range: Range<usize>,
}

pub struct DesignDocument {
    pub frontmatter: Option<Map<String, Value>>,
    pub frontmatter_range: Option<Range<usize>>,
    pub sections: Vec<Section>,
    coverage_sections: Vec<&'static str>,
    pub metadata: Option<Value>,
    pub metadata_range: Option<Range<usize>>,
    pub metadata_line: usize,
    pub locations: BTreeMap<String, usize>,
    pub columns: BTreeMap<String, usize>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DesignDocument {
    /// Retain doctor's existing coverage interpretation independently of
    /// the explicit validator's fence-aware canonical section checks.
    pub fn has_section(&self, key: &str) -> bool {
        self.coverage_sections
            .iter()
            .any(|name| name.eq_ignore_ascii_case(key))
    }

    pub(crate) fn diagnostic(
        &self,
        code: &'static str,
        path: &str,
        message: impl Into<String>,
    ) -> Diagnostic {
        let mut parent = path.to_string();
        let line = loop {
            if let Some(line) = self.locations.get(&parent) {
                break *line;
            }
            if let Some((head, _)) = parent.rsplit_once('/') {
                if head.is_empty() {
                    break if path.starts_with("/metadata") {
                        self.metadata_line
                    } else {
                        1
                    };
                }
                parent = head.to_string();
            } else {
                break 1;
            }
        };
        Diagnostic {
            code,
            source: "draft",
            severity: "error",
            line,
            column: self.columns.get(&parent).copied().unwrap_or(1),
            path: path.into(),
            message: message.into(),
        }
    }
}

pub(crate) struct Line<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
    pub visible: bool,
}

pub(crate) fn lines(source: &str) -> Vec<Line<'_>> {
    let mut offset = 0;
    let mut fence = None;
    source
        .split_inclusive('\n')
        .map(|raw| {
            let text = raw.trim_end_matches(['\r', '\n']);
            let mut visible = fence.is_none();
            let indent = text.bytes().take_while(|byte| *byte == b' ').count();
            if let Some((ch, size, suffix)) =
                (indent <= 3).then(|| fence_line(&text[indent..])).flatten()
            {
                match fence {
                    Some((open, count)) if open == ch && size >= count && suffix.is_empty() => {
                        fence = None;
                        visible = false;
                    }
                    None if ch != '`' || !suffix.contains('`') => {
                        fence = Some((ch, size));
                        visible = false;
                    }
                    _ => {}
                }
            }
            let range = offset..offset + raw.len();
            offset += raw.len();
            Line {
                text,
                range,
                visible,
            }
        })
        .collect()
}

fn coverage_section(name: &str) -> Option<&'static str> {
    let name = name.replace(['\u{2018}', '\u{2019}'], "'").to_lowercase();
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    CANONICAL_SECTIONS.into_iter().find(|canonical| {
        let key = canonical.to_lowercase();
        name.match_indices(&key).any(|(start, _)| {
            !start
                .checked_sub(1)
                .and_then(|index| name.as_bytes().get(index))
                .is_some_and(|byte| word(*byte))
                && !name
                    .as_bytes()
                    .get(start + key.len())
                    .is_some_and(|byte| word(*byte))
        })
    })
}

pub fn canonical_section(name: &str) -> Option<&'static str> {
    let name = name.replace(['\u{2018}', '\u{2019}'], "'").to_lowercase();
    match name.as_str() {
        "overview" | "brand & style" | "brand and style" => Some("Overview"),
        "colors" => Some("Colors"),
        "typography" => Some("Typography"),
        "layout" | "layout & spacing" => Some("Layout"),
        "elevation" | "elevation & depth" => Some("Elevation"),
        "shapes" => Some("Shapes"),
        "components" => Some("Components"),
        "do's and don'ts" => Some("Do's and Don'ts"),
        _ => None,
    }
}

static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^##\s+(?:\d+\.\s*)?([^:\n]+?)(?::\s*.+)?$").unwrap());
static REFERENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^{}]+)\}").unwrap());

pub fn parse_design_md(source: &str) -> DesignDocument {
    let records = lines(source);
    let mut document = DesignDocument {
        frontmatter: None,
        frontmatter_range: None,
        sections: Vec::new(),
        coverage_sections: Vec::new(),
        metadata: None,
        metadata_range: None,
        metadata_line: 1,
        locations: BTreeMap::new(),
        columns: BTreeMap::new(),
        diagnostics: Vec::new(),
    };
    let mut body_start = 0;
    if records.first().is_some_and(|line| trim(line.text) == "---") {
        if let Some(end) = records
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, line)| trim(line.text) == "---")
            .map(|(index, _)| index)
        {
            let yaml_start = records[0].range.end;
            let mapping = parse_mapping(&source[yaml_start..records[end].range.start], 1);
            document.locations = mapping.locations;
            document.columns = mapping.columns;
            document.frontmatter = Some(mapping.values);
            document.frontmatter_range = Some(0..records[end].range.end);
            body_start = end + 1;
            for (line, path, message) in mapping.problems {
                // Unknown top-level formats (such as omitted arrays) remain
                // opaque and are preserved by the update operation.
                if TOKEN_GROUPS.iter().any(|group| {
                    path == format!("/{group}") || path.starts_with(&format!("/{group}/"))
                }) || (message.starts_with("Duplicate") && path.matches('/').count() == 1)
                {
                    let mut diagnostic = document.diagnostic("frontmatter-syntax", &path, message);
                    diagnostic.line = line;
                    document.diagnostics.push(diagnostic);
                }
            }
        } else {
            document.diagnostics.push(document.diagnostic(
                "frontmatter-unclosed",
                "/",
                "Close the YAML frontmatter with a --- line",
            ));
        }
    }
    match metadata_block(source) {
        Ok(Some(block)) => {
            let mut json_line = block.json_line;
            let mut json_column = 1;
            let mut previous_offset = 0;
            for (path, offset) in json_locations(&source[block.json_range.clone()], &block.value) {
                for character in source
                    [block.json_range.start + previous_offset..block.json_range.start + offset]
                    .chars()
                {
                    if character == '\n' {
                        json_line += 1;
                        json_column = 1;
                    } else {
                        json_column += 1;
                    }
                }
                document.locations.insert(path.clone(), json_line);
                document.columns.insert(path, json_column);
                previous_offset = offset;
            }
            document.metadata = Some(block.value);
            document.metadata_range = Some(block.range);
            document.metadata_line = block.json_line;
        }
        Ok(None) => {}
        Err(error) => {
            let line = records
                .iter()
                .position(|line| line.visible && line.text.trim() == MARKER)
                .map(|index| index + 1)
                .unwrap_or(1);
            let mut diagnostic = document.diagnostic("metadata-syntax", "/metadata", error);
            diagnostic.line = line;
            document.diagnostics.push(diagnostic);
        }
    }
    for line in records.iter().skip(body_start) {
        if let Some(capture) = HEADING.captures(line.text.trim_end()) {
            if let Some(section) = coverage_section(trim(&capture[1])) {
                if !document.coverage_sections.contains(&section) {
                    document.coverage_sections.push(section);
                }
            }
        }
    }
    let mut in_comment = false;
    for (index, line) in records.iter().enumerate().skip(body_start) {
        if !line.visible {
            continue;
        }
        let hidden = in_comment;
        for part in line.text.split_inclusive("-->") {
            if part.contains("<!--") {
                in_comment = true;
            }
            if part.ends_with("-->") {
                in_comment = false;
            }
        }
        if hidden {
            continue;
        }
        let Some(capture) = HEADING.captures(line.text) else {
            continue;
        };
        let name = trim(&capture[1]).to_string();
        document.sections.push(Section {
            canonical: canonical_section(&name),
            name,
            line: index + 1,
            range: line.range.start..source.len(),
        });
    }
    for index in 0..document.sections.len() {
        let end = document
            .sections
            .get(index + 1)
            .map(|section| section.range.start)
            .unwrap_or(source.len());
        document.sections[index].range.end = end;
    }
    document
}

// JSON object keys appear in the same preorder as the preserve_order AST.
// Scan complete string literals so quoted key syntax inside values is ignored.
fn json_locations(text: &str, value: &Value) -> Vec<(String, usize)> {
    fn paths(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
        if let Some(object) = value.as_object() {
            for (key, value) in object {
                let path = format!("{path}{}", pointer(std::slice::from_ref(key)));
                out.push((path.clone(), key.clone()));
                paths(value, &path, out);
            }
        } else if let Some(array) = value.as_array() {
            for (index, value) in array.iter().enumerate() {
                paths(value, &format!("{path}/{index}"), out);
            }
        }
    }
    let mut keys = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() {
            if bytes[index] == b'\\' {
                index += 2;
            } else if bytes[index] == b'"' {
                index += 1;
                break;
            } else {
                index += 1;
            }
        }
        let mut next = index;
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if bytes.get(next) == Some(&b':') {
            if let Ok(key) = serde_json::from_str::<String>(&text[start..index]) {
                keys.push((key, start));
            }
        }
    }
    let mut fields = Vec::new();
    paths(value, "/metadata", &mut fields);
    let mut remaining = keys.as_slice();
    fields
        .into_iter()
        .filter_map(|(path, key)| {
            let index = remaining
                .iter()
                .position(|(candidate, _)| *candidate == key)?;
            let offset = remaining[index].1;
            remaining = &remaining[index + 1..];
            Some((path, offset))
        })
        .collect()
}

pub fn validate(source: &str) -> Vec<Diagnostic> {
    let document = parse_design_md(source);
    let mut diagnostics = document.diagnostics.clone();
    if let Some(frontmatter) = &document.frontmatter {
        for group in TOKEN_GROUPS {
            let Some(value) = frontmatter.get(group) else {
                continue;
            };
            let path = format!("/{group}");
            let Some(tokens) = value.as_object() else {
                diagnostics.push(document.diagnostic(
                    "token-group",
                    &path,
                    "Use a mapping of token names to values",
                ));
                continue;
            };
            for (name, token) in tokens {
                let token_path = format!("{path}{}", pointer(std::slice::from_ref(name)));
                if name.is_empty() {
                    diagnostics.push(document.diagnostic(
                        "token-name",
                        &token_path,
                        "Give this token a nonempty name",
                    ));
                }
                match group {
                    "colors" => {
                        check_scalar(&document, &token_path, token, false, &mut diagnostics)
                    }
                    "rounded" | "spacing" => {
                        check_scalar(&document, &token_path, token, true, &mut diagnostics)
                    }
                    "typography" => {
                        // The detector also reads legacy scalar sizes under typography.scale.
                        if name == "scale" {
                            check_scalar_map(&document, &token_path, token, &mut diagnostics);
                        } else {
                            check_typography(&document, &token_path, token, &mut diagnostics);
                        }
                    }
                    "components" => {
                        check_component(&document, &token_path, token, &mut diagnostics)
                    }
                    _ => unreachable!(),
                }
                check_references(
                    &document,
                    &token_path,
                    token,
                    group == "components",
                    &mut diagnostics,
                );
            }
        }
    }
    let mut seen = Vec::new();
    let mut order = None;
    let mut canonical_seen = Vec::new();
    for section in &document.sections {
        let normalized = section.name.to_lowercase();
        let duplicate = seen.contains(&normalized);
        seen.push(normalized);
        let mut problem =
            duplicate.then_some(("section-duplicate", "Keep one section with this heading"));
        if let Some(canonical) = section.canonical {
            let position = CANONICAL_SECTIONS
                .iter()
                .position(|name| *name == canonical)
                .unwrap();
            if canonical_seen.contains(&canonical) {
                problem = Some((
                    "section-duplicate",
                    "Keep one section for this canonical heading and its aliases",
                ));
            } else if order.is_some_and(|previous| previous > position) {
                problem = Some(("section-order", "Keep present canonical sections in Overview, Colors, Typography, Layout, Elevation, Shapes, Components, Do's and Don'ts order"));
            }
            canonical_seen.push(canonical);
            order = Some(position);
        }
        if let Some((code, message)) = problem {
            let mut diagnostic =
                document.diagnostic(code, &format!("/sections/{}", section.name), message);
            diagnostic.line = section.line;
            diagnostics.push(diagnostic);
        }
    }
    if let Some(metadata) = &document.metadata {
        check_metadata(&document, metadata, &mut diagnostics);
    }
    diagnostics
}

fn scalar(value: &Value, number: bool) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty()) || (number && value.is_number())
}

fn check_scalar(
    document: &DesignDocument,
    path: &str,
    value: &Value,
    number: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !scalar(value, number) {
        diagnostics.push(document.diagnostic(
            "token-value",
            path,
            if number {
                "Use a nonempty source string or number"
            } else {
                "Use a nonempty source string"
            },
        ));
    }
}

fn check_scalar_map(
    document: &DesignDocument,
    path: &str,
    value: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(mapping) = value.as_object() {
        for (key, value) in mapping {
            check_scalar(
                document,
                &format!("{path}{}", pointer(std::slice::from_ref(key))),
                value,
                true,
                diagnostics,
            );
        }
    } else {
        diagnostics.push(document.diagnostic("token-value", path, "Use a mapping of named sizes"));
    }
}

fn check_typography(
    document: &DesignDocument,
    path: &str,
    value: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(properties) = value.as_object() else {
        diagnostics.push(document.diagnostic(
            "typography-token",
            path,
            "Use a mapping of typography properties",
        ));
        return;
    };
    for (property, value) in properties {
        let path = format!("{path}{}", pointer(std::slice::from_ref(property)));
        check_scalar(
            document,
            &path,
            value,
            matches!(
                property.as_str(),
                "fontSize" | "fontWeight" | "lineHeight" | "letterSpacing"
            ),
            diagnostics,
        );
    }
}

fn check_component(
    document: &DesignDocument,
    path: &str,
    value: &Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(properties) = value.as_object() else {
        diagnostics.push(document.diagnostic(
            "component-token",
            path,
            "Use a mapping of component properties",
        ));
        return;
    };
    for (property, value) in properties {
        let path = format!("{path}{}", pointer(std::slice::from_ref(property)));
        if property == "typography" && value.is_object() {
            check_typography(document, &path, value, diagnostics);
        } else {
            check_scalar(document, &path, value, true, diagnostics);
        }
        if ![
            "backgroundColor",
            "textColor",
            "typography",
            "rounded",
            "padding",
            "size",
            "height",
            "width",
        ]
        .contains(&property.as_str())
        {
            let mut warning = document.diagnostic(
                "component-property",
                &path,
                "Unknown component property is preserved; the detector may not consume it",
            );
            warning.severity = "warning";
            diagnostics.push(warning);
        }
    }
}

fn check_references(
    document: &DesignDocument,
    path: &str,
    value: &Value,
    component: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(properties) = value.as_object() {
        for (key, value) in properties {
            check_references(
                document,
                &format!("{path}{}", pointer(std::slice::from_ref(key))),
                value,
                component,
                diagnostics,
            );
        }
    } else if let Some(text) = value.as_str() {
        for reference in REFERENCE.captures_iter(text) {
            let parts: Vec<String> = reference[1].split('.').map(str::to_string).collect();
            let target = document
                .frontmatter
                .as_ref()
                .and_then(|root| root.get(&parts[0]))
                .and_then(|value| {
                    parts[1..]
                        .iter()
                        .try_fold(value, |value, key| value.get(key))
                });
            let composite_typography = path.ends_with("/typography")
                && parts.first().is_some_and(|part| part == "typography")
                && parts.len() == 2;
            if !component
                || !parts.first().is_some_and(|part| {
                    ["colors", "typography", "rounded", "spacing"].contains(&part.as_str())
                })
                || target.is_none_or(|value| {
                    (!value.is_string() && !value.is_number())
                        && !(composite_typography && value.is_object())
                })
            {
                diagnostics.push(document.diagnostic("token-reference", path, format!("Resolve {{{}}} to a declared primitive (or a typography token for component typography); primitive tokens cannot reference other tokens", &reference[1])));
            }
        }
    }
}

fn check_metadata(document: &DesignDocument, metadata: &Value, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(version) = metadata.get("schemaVersion") {
        if !version
            .as_u64()
            .is_some_and(|version| (1..=2).contains(&version))
        {
            diagnostics.push(document.diagnostic("metadata-schema", "/metadata/schemaVersion", "Use a supported integer schemaVersion (1 or 2); artifact schemas are independent of releases"));
        }
    }
    let Some(extensions) = metadata.get("extensions") else {
        return;
    };
    let Some(extensions) = extensions.as_object() else {
        diagnostics.push(document.diagnostic(
            "metadata-value",
            "/metadata/extensions",
            "Use a JSON object for extensions",
        ));
        return;
    };
    for group in ["colorMeta", "roundedMeta"] {
        let Some(values) = extensions.get(group) else {
            continue;
        };
        let base = format!("/metadata/extensions/{group}");
        let Some(values) = values.as_object() else {
            diagnostics.push(document.diagnostic(
                "metadata-value",
                &base,
                "Use a JSON object mapping token names to metadata",
            ));
            continue;
        };
        for (name, value) in values {
            let path = format!("{base}{}", pointer(std::slice::from_ref(name)));
            if group == "roundedMeta" && scalar(value, true) {
                continue;
            }
            let Some(properties) = value.as_object() else {
                diagnostics.push(document.diagnostic(
                    "metadata-value",
                    &path,
                    "Use a metadata object (roundedMeta also accepts a source string or number)",
                ));
                continue;
            };
            for (property, value) in properties {
                let item_path = format!("{path}{}", pointer(std::slice::from_ref(property)));
                match property.as_str() {
                    "canonical" | "value" => check_scalar(
                        document,
                        &item_path,
                        value,
                        group == "roundedMeta",
                        diagnostics,
                    ),
                    "role" if group == "roundedMeta" => {
                        check_scalar(document, &item_path, value, false, diagnostics)
                    }
                    "tonalRamp" if group == "colorMeta" => {
                        check_metadata_array(document, &item_path, value, false, diagnostics)
                    }
                    "values" | "aliases" if group == "roundedMeta" => {
                        check_metadata_array(document, &item_path, value, true, diagnostics)
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(shadows) = extensions.get("shadows") {
        if let Some(shadows) = shadows.as_array() {
            for (index, shadow) in shadows.iter().enumerate() {
                let path = format!("/metadata/extensions/shadows/{index}/value");
                if let Some(value) = shadow.get("value") {
                    check_scalar(document, &path, value, false, diagnostics);
                } else {
                    diagnostics.push(document.diagnostic(
                        "metadata-value",
                        &path,
                        "Record each shadow as an object with a source value string",
                    ));
                }
            }
        } else {
            diagnostics.push(document.diagnostic(
                "metadata-value",
                "/metadata/extensions/shadows",
                "Use an array of shadow objects",
            ));
        }
    }
}

fn check_metadata_array(
    document: &DesignDocument,
    path: &str,
    value: &Value,
    number: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(values) = value.as_array() {
        for (index, value) in values.iter().enumerate() {
            check_scalar(
                document,
                &format!("{path}/{index}"),
                value,
                number,
                diagnostics,
            );
        }
    } else {
        diagnostics.push(document.diagnostic(
            "metadata-value",
            path,
            "Use an array of source values",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_and_optional_sections_validate_without_inventing_tokens() {
        assert!(validate(
            "---\nname: Seed\ndescription: Provisional\n---\n## Overview\nDirection only.\n"
        )
        .is_empty());
        assert!(validate("## Colors\nA palette.\n## Shapes\nSquare.\n").is_empty());
    }

    #[test]
    fn malformed_groups_references_and_heading_order_have_source_locations() {
        let source = "---\ncolors: true\ncomponents:\n  button:\n    textColor: \"{colors.missing}\"\n---\n## Shapes\n## Colors\n";
        let errors = validate(source);
        assert!(errors
            .iter()
            .any(|error| error.path == "/colors" && error.line == 2));
        assert!(errors
            .iter()
            .any(|error| error.code == "token-reference" && error.line == 5));
        assert!(errors
            .iter()
            .any(|error| error.code == "section-order" && error.line == 8));
    }

    #[test]
    fn headings_and_metadata_markers_in_examples_do_not_validate_as_records() {
        let source = "## Colors\n````markdown\n## Colors\n<!-- impeccino:design-metadata -->\n```json\n[]\n```\n````\n## Components\n";
        assert!(validate(source).is_empty());
    }

    #[test]
    fn nested_token_and_metadata_diagnostics_point_to_the_actual_field() {
        let yaml = "---\ncolors:\n  primary: false\n---\n";
        let diagnostics = validate(yaml);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "/colors/primary"
                && diagnostic.line == 3
                && diagnostic.column == 3));
        let json = "<!-- impeccino:design-metadata -->\n```json\n{\n  \"extensions\": {\n    \"colorMeta\": {\n      \"primary\": {\n        \"tonalRamp\": true\n      }\n    }\n  }\n}\n```\n";
        assert!(validate(json)
            .iter()
            .any(|diagnostic| diagnostic.path.ends_with("/tonalRamp")
                && diagnostic.line == 7
                && diagnostic.column == 9));
    }

    #[test]
    fn consumed_metadata_fields_are_validated_and_unknown_fields_survive() {
        let invalid = "# Design\n<!-- impeccino:design-metadata -->\n```json\n{\"schemaVersion\":2,\"extensions\":{\"colorMeta\":{\"primary\":{\"tonalRamp\":true}}}}\n```\n";
        let errors = validate(invalid);
        assert!(errors.iter().any(|error| error.path
            == "/metadata/extensions/colorMeta/primary/tonalRamp"
            && error.line >= 4));
        let legacy = "<!-- impeccino:design-metadata -->\n```json\n{\"schemaVersion\":1,\"unknown\":{\"keep\":[true]}}\n```\n";
        assert!(validate(legacy).is_empty());
    }
}
