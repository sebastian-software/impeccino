//! Port of `cli/engine/shared/inline-ignores.mjs`: eslint-disable-style
//! waivers that live in the scanned file (`impeccino-disable`,
//! `impeccino-disable-line`, `impeccino-disable-next-line`).

use crate::js::{self, ci, WS};
use once_cell::sync::Lazy;
use regex::Regex;

/// JS `DIRECTIVE_RE` =
/// `/impeccino-(disable-next-line|disable-line|disable)\b[ \t]*([^\n\r]*)/gi`.
static DIRECTIVE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"{imp}-({dnl}|{dl}|{d})(?-u:\b)[ \t]*([^\n\r]*)",
        imp = ci("impeccino"),
        dnl = ci("disable-next-line"),
        dl = ci("disable-line"),
        d = ci("disable")
    ))
    .unwrap()
});

/// JS `TRAILING_CLOSER_RE` = `/\s*(?:\*\/\}?|--+>|\*\}|#\}|%>|\}\})\s*$/`.
static TRAILING_CLOSER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"{WS}*(?:\*/\}}?|--+>|\*\}}|#\}}|%>|\}}\}}){WS}*$"
    ))
    .unwrap()
});

/// JS `/\s*(?:--+|:)\s*/`.
static REASON_SEP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"{WS}*(?:--+|:){WS}*")).unwrap());

/// JS `/[\s,]+/`.
static TOKEN_SPLIT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"[{},]+", js::WS_CHARS)).unwrap());

/// Cheap bail-out `/impeccino-disable/i`.
static HAS_DIRECTIVE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&ci("impeccino-disable")).unwrap());

/// An insertion-ordered set of rule ids (JS `Set<string>`).
pub type RuleSet = Vec<String>;

/// The parsed directives of one file (JS `parseInlineIgnores` result).
/// `line` and `next_line` are insertion-ordered maps keyed by the 1-based
/// line the directive targets (JS `Map<number, Set<string>>`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InlineIgnores {
    pub file: RuleSet,
    pub line: Vec<(usize, RuleSet)>,
    pub next_line: Vec<(usize, RuleSet)>,
}

/// JS `normalizeRule(token)`.
pub fn normalize_rule(token: &str) -> String {
    js::to_lower_case(js::trim(token))
}

/// JS `parseRuleList(remainder)`.
fn parse_rule_list(remainder: &str) -> Vec<String> {
    let stripped = TRAILING_CLOSER_RE.replace(remainder, "");
    let mut text: &str = js::trim(&stripped);
    if let Some(m) = REASON_SEP_RE.find(text) {
        text = &text[..m.start()];
    }
    let tokens: Vec<String> = TOKEN_SPLIT_RE
        .split(text)
        .map(normalize_rule)
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() || tokens.iter().any(|t| t == "*") {
        return vec!["*".to_string()];
    }
    tokens
}

fn add_rules(set: &mut RuleSet, rules: &[String]) {
    for rule in rules {
        if !set.iter().any(|r| r == rule) {
            set.push(rule.clone());
        }
    }
}

fn get_set(map: &mut Vec<(usize, RuleSet)>, key: usize) -> &mut RuleSet {
    if let Some(pos) = map.iter().position(|(k, _)| *k == key) {
        &mut map[pos].1
    } else {
        map.push((key, Vec::new()));
        &mut map.last_mut().unwrap().1
    }
}

fn fence_delimiter(line: &str) -> Option<(char, usize, &str)> {
    let line = line.trim_end_matches('\r');
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let marker = rest.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let count = rest.chars().take_while(|c| *c == marker).count();
    if count < 3 {
        return None;
    }
    Some((marker, count, &rest[count..]))
}

/// Whether each Markdown line is outside a fenced code block. Fence
/// delimiters and their contents are false. The opener's marker and length
/// govern its close, matching CommonMark's fence rules used by both DESIGN.md
/// waiver parsing and SURFACES.md section parsing.
#[doc(hidden)]
pub fn markdown_fenced_code_line_mask(markdown: &str) -> Vec<bool> {
    let mut fence: Option<(char, usize)> = None;
    let mut outside = Vec::new();
    for line in markdown.split('\n') {
        if let Some((open_marker, open_len)) = fence {
            let closes = fence_delimiter(line)
                .map(|(marker, len, tail)| {
                    marker == open_marker && len >= open_len && tail.chars().all(char::is_whitespace)
                })
                .unwrap_or(false);
            outside.push(false);
            if closes {
                fence = None;
            }
            continue;
        }
        let opening = fence_delimiter(line).and_then(|(marker, len, tail)| {
            if marker == '`' && tail.contains('`') {
                None
            } else {
                Some((marker, len))
            }
        });
        if let Some(opening) = opening {
            fence = Some(opening);
            outside.push(false);
        } else {
            outside.push(true);
        }
    }
    outside
}

/// JS `parseInlineIgnores(content)`.
pub fn parse_inline_ignores(content: Option<&str>) -> InlineIgnores {
    let mut result = InlineIgnores::default();
    let text = content.unwrap_or("");
    if !HAS_DIRECTIVE_RE.is_match(text) {
        return result;
    }
    for (i, line) in text.split('\n').enumerate() {
        for m in DIRECTIVE_RE.captures_iter(line) {
            let variant = js::to_lower_case(m.get(1).unwrap().as_str());
            let rules = parse_rule_list(m.get(2).map(|g| g.as_str()).unwrap_or(""));
            if variant == "disable" {
                add_rules(&mut result.file, &rules);
            } else if variant == "disable-line" {
                add_rules(get_set(&mut result.line, i + 1), &rules);
            } else {
                // disable-next-line on line i+1 targets line i+2.
                add_rules(get_set(&mut result.next_line, i + 2), &rules);
            }
        }
    }
    result
}

fn set_matches(set: Option<&RuleSet>, rule: &str) -> bool {
    match set {
        Some(set) => set.iter().any(|r| r == "*" || r == rule),
        None => false,
    }
}

fn map_get<'a>(map: &'a [(usize, RuleSet)], key: usize) -> Option<&'a RuleSet> {
    map.iter().find(|(k, _)| *k == key).map(|(_, s)| s)
}

/// The two fields `isInlineIgnored` reads off a finding.
pub trait IgnorableFinding {
    /// JS `finding.antipattern` (None when absent / not a string).
    fn antipattern(&self) -> Option<&str>;
    /// JS `Number(finding.line)`.
    fn line_number(&self) -> f64;
}

/// JS `isInlineIgnored(finding, directives)`.
pub fn is_inline_ignored<F: IgnorableFinding + ?Sized>(
    finding: &F,
    directives: &InlineIgnores,
) -> bool {
    let rule = normalize_rule(finding.antipattern().unwrap_or(""));
    if rule.is_empty() {
        return false;
    }
    if set_matches(Some(&directives.file), &rule) {
        return true;
    }
    // `Number(finding.line) || 0`
    let mut line = finding.line_number();
    if line.is_nan() {
        line = 0.0;
    }
    if line > 0.0 {
        // Map keys are the integer line numbers written by the parser; a
        // non-integer line can never match one.
        if line.fract() == 0.0 && line <= usize::MAX as f64 {
            let key = line as usize;
            if set_matches(map_get(&directives.line, key), &rule) {
                return true;
            }
            if set_matches(map_get(&directives.next_line, key), &rule) {
                return true;
            }
        }
    }
    false
}

/// JS `hasDirectives(directives)`.
pub fn has_directives(directives: &InlineIgnores) -> bool {
    !directives.file.is_empty() || !directives.line.is_empty() || !directives.next_line.is_empty()
}

/// JS `applyInlineIgnores(findings, content)`: drop findings waived by an
/// inline directive in the same file's source text.
pub fn apply_inline_ignores<F: IgnorableFinding>(
    findings: Vec<F>,
    content: Option<&str>,
) -> Vec<F> {
    if findings.is_empty() {
        return findings;
    }
    let directives = parse_inline_ignores(content);
    if !has_directives(&directives) {
        return findings;
    }
    findings
        .into_iter()
        .filter(|f| !is_inline_ignored(f, &directives))
        .collect()
}

/// The text of every HTML comment outside fenced code blocks.
fn html_comments(md: &str) -> Vec<String> {
    let mut prose = String::new();
    let outside_fence = markdown_fenced_code_line_mask(md);
    for (line, outside) in md.split('\n').zip(outside_fence) {
        if outside {
            prose.push_str(line);
        }
        prose.push('\n');
    }
    let mut out = Vec::new();
    let mut rest = prose.as_str();
    while let Some(start) = rest.find("<!--") {
        let after = &rest[start + 4..];
        let Some(end) = after.find("-->") else { break };
        out.push(after[..end].to_string());
        rest = &after[end + 3..];
    }
    out
}

/// Project-wide waivers recorded in DESIGN.md (docs/adr/0020): the rule ids
/// of every `<!-- impeccino-disable <rule-id> [-- reason] -->` comment.
/// Only HTML comments outside code fences count, so prose that explains the
/// syntax waives nothing; the `-line` / `-next-line` variants have no
/// meaning in a design document and are ignored; and a bare
/// `impeccino-disable` (every rule) is not honored at project scope.
pub fn parse_design_waivers(md: &str) -> Vec<String> {
    let mut rules: Vec<String> = Vec::new();
    for comment in html_comments(md) {
        let one_line = comment.replace(['\n', '\r'], " ");
        for rule in parse_inline_ignores(Some(&one_line)).file {
            if rule != "*" && !rules.contains(&rule) {
                rules.push(rule);
            }
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directives() {
        let src = "<!-- impeccino-disable low-contrast -- exported -->\nx /* impeccino-disable-line design-system-font */\n// impeccino-disable-next-line bounce-easing: reason\nfoo\n";
        let d = parse_inline_ignores(Some(src));
        assert_eq!(d.file, vec!["low-contrast"]);
        assert_eq!(d.line, vec![(2, vec!["design-system-font".to_string()])]);
        assert_eq!(d.next_line, vec![(4, vec!["bounce-easing".to_string()])]);
        let bare = parse_inline_ignores(Some("<!-- impeccino-disable -->"));
        assert_eq!(bare.file, vec!["*"]);
    }

    #[test]
    fn design_waivers_come_from_comments_outside_fences() {
        let md = "# Design\n\n## Named Rules\n\n**The Ink Rule.** Hairlines carry the grid. <!-- impeccino-disable side-tab, GRADIENT-TEXT -- the ledger rule -->\n\nWrite `impeccino-disable overused-font` to waive a rule.\n\n```md\n<!-- impeccino-disable bounce-easing -->\n```\n\n<!-- impeccino-disable -->\n<!--\nimpeccino-disable line-length: long legal copy\n-->\n";
        assert_eq!(parse_design_waivers(md), vec!["side-tab", "gradient-text", "line-length"]);
    }

    #[test]
    fn design_waivers_ignore_shorter_fences_nested_in_longer_fences() {
        let md = "````md\n```html\n<!-- impeccino-disable low-contrast -->\n```\n````\n";
        assert!(parse_design_waivers(md).is_empty());
    }

    #[test]
    fn markdown_fences_require_a_matching_marker_and_valid_close() {
        let md = "  ````md\n~~~html\n```html\n<!-- impeccino-disable low-contrast -->\n```` trailing\n````\n<!-- impeccino-disable side-tab -->";
        let mask = markdown_fenced_code_line_mask(md);
        assert_eq!(mask, vec![false, false, false, false, false, false, true]);
        assert_eq!(parse_design_waivers(md), vec!["side-tab"]);
    }
}
