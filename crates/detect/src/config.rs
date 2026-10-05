//! Detector finding filters: the rule and value waivers `impeccino detect`
//! and the design hook apply after a scan. There is no config file
//! (docs/adr/0020); the waivers come from DESIGN.md
//! (`design_decisions`), and in-file waivers from `impeccino-disable`
//! comments (`impeccino_core::inline_ignores`).

use impeccino_core::findings::Finding;
use impeccino_core::js;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

use crate::util::{re, WS};

/// One value waiver: `rule` stops firing for `value` (normalized with
/// `normalize_ignore_value`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IgnoreValueEntry {
    pub rule: String,
    pub value: String,
}

/// The waivers a filter applies.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetectionConfig {
    pub ignore_rules: Vec<String>,
    pub ignore_values: Vec<IgnoreValueEntry>,
}

impl DetectionConfig {
    /// No waivers.
    pub fn raw() -> Self {
        DetectionConfig::default()
    }
}

re!(EDGE_QUOTE_RE, r#"^["']|["']$"#);
re!(WS_RUN_RE, format!("{WS}+"));

/// JS: impeccino-config.mjs#normalizeIgnoreValue
pub fn normalize_ignore_value(value: &str) -> String {
    let t = js::trim(value);
    let t = EDGE_QUOTE_RE.replace_all(t, "");
    let t = t.replace('+', " ");
    let t = WS_RUN_RE.replace_all(&t, " ");
    js::to_lower_case(&t)
}

/// JS `normalizeIgnoreRule`.
pub fn normalize_ignore_rule(rule: &str) -> String {
    js::to_lower_case(js::trim(rule))
}

/// Drop findings whose rule is waived, or whose extracted value is.
pub fn filter_detection_findings(findings: Vec<Finding>, config: &DetectionConfig) -> Vec<Finding> {
    if findings.is_empty() {
        return vec![];
    }
    let ignore_rules: Vec<String> = config
        .ignore_rules
        .iter()
        .map(|r| normalize_ignore_rule(r))
        .collect();
    findings
        .into_iter()
        .filter(|f| {
            if ignore_rules.contains(&normalize_ignore_rule(&f.antipattern)) {
                return false;
            }
            !is_ignored_finding_value(f, &config.ignore_values)
        })
        .collect()
}

fn is_ignored_finding_value(finding: &Finding, ignore_values: &[IgnoreValueEntry]) -> bool {
    if ignore_values.is_empty() {
        return false;
    }
    let rule = normalize_ignore_rule(&finding.antipattern);
    if rule.is_empty() {
        return false;
    }
    let value = extract_finding_ignore_value(finding);
    !value.is_empty()
        && ignore_values
            .iter()
            .any(|entry| entry.rule == rule && normalize_ignore_value(&entry.value) == value)
}

const DIRECT_VALUE_RULES: &[&str] = &[
    "overused-font",
    "bounce-easing",
    "design-system-font",
    "design-system-color",
    "design-system-radius",
    "design-system-font-size",
];

/// JS: impeccino-config.mjs#extractFindingIgnoreValue
pub fn extract_finding_ignore_value(finding: &Finding) -> String {
    let rule = normalize_ignore_rule(&finding.antipattern);
    if !DIRECT_VALUE_RULES.contains(&rule.as_str()) {
        return String::new();
    }
    normalize_ignore_value(&extract_finding_ignore_value_raw(finding, &rule))
}

fn extra_str<'a>(finding: &'a Finding, key: &str) -> Option<&'a str> {
    match finding.extras.get(key) {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

re!(
    PRIMARY_FONT_RE,
    format!("(?i:Primary font):{WS}*([^()\n;]+)")
);
re!(
    GOOGLE_LABEL_RE,
    format!("(?i:Google Fonts):{WS}*([^()\n;]+)")
);
re!(
    FAMILY_RE,
    format!(r#"(?i:font-family){WS}*:{WS}*["']?([^'",;\n]+)"#)
);
re!(GOOGLE_PARAM_RE, "[?&](?i:family)=([^&:;\n]+)");

fn extract_finding_ignore_value_raw(finding: &Finding, rule: &str) -> String {
    let direct_src = extra_str(finding, "ignoreValue")
        .filter(|s| !s.is_empty())
        .or_else(|| extra_str(finding, "value").filter(|s| !s.is_empty()))
        .unwrap_or("");
    let direct = clean_ignore_value_display(direct_src);
    if !direct.is_empty() {
        return direct;
    }
    let mut candidates: Vec<&str> = Vec::new();
    if let Some(d) = extra_str(finding, "detail") {
        if !d.is_empty() {
            candidates.push(d);
        }
    }
    if !finding.snippet.is_empty() {
        candidates.push(&finding.snippet);
    }
    for text in candidates {
        if rule == "bounce-easing" {
            let motion = extract_motion_ignore_value(text);
            if !motion.is_empty() {
                return motion;
            }
            continue;
        }
        if let Some(m) = PRIMARY_FONT_RE.captures(text) {
            return clean_ignore_value_display(&m[1]);
        }
        if let Some(m) = GOOGLE_LABEL_RE.captures(text) {
            return clean_ignore_value_display(&m[1]);
        }
        if let Some(m) = FAMILY_RE.captures(text) {
            return clean_ignore_value_display(&m[1]);
        }
        if let Some(m) = GOOGLE_PARAM_RE.captures(text) {
            return clean_ignore_value_display(&decode_uri_component(&m[1]));
        }
    }
    String::new()
}

/// JS `decodeURIComponent` with the source's `try { } catch { raw }` fallback.
pub fn decode_uri_component(s: &str) -> String {
    impeccino_core::browser::driver::decode_uri_component(s).unwrap_or_else(|| s.to_string())
}

re!(ANIMATE_BOUNCE_RE, "(?-u:\\b)(?i:animate-bounce)(?-u:\\b)");
re!(BEZIER_RE, r"(?i:cubic-bezier)\([^)]+\)");
re!(
    ANIMATION_RE,
    format!("(?i:animation)(?:-(?i:name))?{WS}*:{WS}*([^;\n]+)")
);
re!(MOTION_TOKEN_RE, "(?i:bounce|elastic|wobble|jiggle|spring)");
static COMMA_WS_SPLIT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("[,{}]+", impeccino_core::js::WS_CHARS)).unwrap());

fn extract_motion_ignore_value(text: &str) -> String {
    if let Some(m) = ANIMATE_BOUNCE_RE.find(text) {
        return clean_ignore_value_display(m.as_str());
    }
    if let Some(m) = BEZIER_RE.find(text) {
        return clean_ignore_value_display(m.as_str());
    }
    if let Some(m) = ANIMATION_RE.captures(text) {
        let token = COMMA_WS_SPLIT_RE
            .split(&m[1])
            .find(|part| MOTION_TOKEN_RE.is_match(part));
        if let Some(t) = token {
            return clean_ignore_value_display(t);
        }
    }
    String::new()
}

fn clean_ignore_value_display(value: &str) -> String {
    let t = js::trim(value);
    let t = EDGE_QUOTE_RE.replace_all(t, "");
    let t = t.replace('+', " ");
    WS_RUN_RE.replace_all(&t, " ").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize() {
        assert_eq!(normalize_ignore_value(" 'Open+Sans' "), "open sans");
        assert_eq!(decode_uri_component("Open%20Sans"), "Open Sans");
        assert_eq!(decode_uri_component("bad%zz"), "bad%zz");
    }

    #[test]
    fn uri_decode_falls_back_on_percent_before_non_ascii() {
        let input = "file:///x/100%日.html";
        assert_eq!(decode_uri_component(input), input);
    }
}
