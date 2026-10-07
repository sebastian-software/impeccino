//! Collect findings from one rendered-page snapshot and build selectors for
//! findings that need a live-page lookup. See `browser/mod.rs` for the module
//! map.

use super::dom::{tag_lower, Dom, ElId};
pub use super::DesignSystemConfig;
use super::{BrowserConfig, BrowserFinding, FindingGroup};

/// The collect result type is shared.
pub use impeccino_foundation::browser::CollectResult;

pub fn scoped_ignore_active(dom: &dyn Dom, el: ElId, rule_id: &str) -> bool {
    let rule = crate::js::to_lower_case(rule_id);
    // Handle 0 is JS null (a missing document.body): the walk never runs.
    let mut cur = if el == 0 { None } else { Some(el) };
    while let Some(c) = cur {
        if let Some(attr) = dom.attr(c, "data-impeccino-ignore") {
            let lowered = crate::js::to_lower_case(crate::js::trim(&attr));
            let rules: Vec<&str> = SPLIT_RE.split(&lowered).filter(|s| !s.is_empty()).collect();
            if rules.is_empty() || rules.contains(&"*") || rules.contains(&rule.as_str()) {
                return true;
            }
        }
        cur = dom.parent(c);
    }
    false
}

static SPLIT_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::Regex::new(&format!("[{},]+", crate::js::WS_CHARS)).expect("SPLIT_RE")
});

/// Group-map insertion (`addBrowserFindings`): scoped-ignore filter, then
/// append to the element's list or start one.
pub fn add_browser_findings(
    dom: &dyn Dom,
    groups: &mut Vec<FindingGroup>,
    el: ElId,
    findings: Vec<BrowserFinding>,
) {
    if findings.is_empty() {
        return;
    }
    let kept: Vec<BrowserFinding> = findings
        .into_iter()
        .filter(|f| !scoped_ignore_active(dom, el, &f.type_))
        .collect();
    if kept.is_empty() {
        return;
    }
    if let Some(g) = groups.iter_mut().find(|g| g.el == el) {
        g.findings.extend(kept);
    } else {
        groups.push(FindingGroup { el, findings: kept });
    }
}

// ─── Design system (index.mjs) ──────────────────────────────────────────────

pub use crate::design_system::DesignSystemSeen as DesignSeen;
const DESIGN_SKIP_TAGS: &[&str] = &[
    "head", "title", "meta", "link", "style", "script", "noscript", "template", "source",
];

pub fn normalize_browser_font_name(value: &str) -> String {
    crate::design_system::normalize_font_name(value)
}

pub fn browser_primary_font(stack: &str) -> String {
    crate::design_system::computed_primary_font(stack)
}

pub fn browser_colors_close(a: &crate::color::Rgba, b: &crate::color::Rgba) -> bool {
    crate::design_system::colors_close(a, b)
}

pub fn is_browser_design_color_allowed(raw: &str, ds: Option<&DesignSystemConfig>) -> bool {
    let Some(ds) = ds else { return true };
    crate::design_system::is_allowed_color_raw(raw, ds.has_colors, &ds.allowed_colors)
}

pub fn is_browser_transparent_css(value: &str) -> bool {
    crate::design_system::is_transparent_css(value)
}

pub fn is_browser_design_radius_allowed(raw: &str, ds: Option<&DesignSystemConfig>) -> bool {
    let Some(ds) = ds else { return true };
    crate::design_system::is_allowed_radius_raw(
        raw,
        ds.has_radii,
        ds.allowed_radii.iter().copied(),
        ds.has_pill_radius,
    )
}

pub fn browser_radius_tokens(value: &str) -> Vec<String> {
    crate::design_system::radius_tokens(value)
}

pub fn browser_has_direct_text(dom: &dyn Dom, el: ElId) -> bool {
    dom.direct_text_nodes(el)
        .iter()
        .any(|t| !crate::js::trim(t).is_empty())
}

pub fn browser_sample_text(dom: &dyn Dom, el: ElId) -> String {
    crate::design_system::sample_text(&dom.text_content(el), 40)
}

pub fn should_skip_design_element(dom: &dyn Dom, el: ElId) -> bool {
    let tag = tag_lower(dom, el);
    DESIGN_SKIP_TAGS.contains(&tag.as_str()) || is_element_hidden(dom, el)
}

pub fn check_element_design_system_dom(
    dom: &dyn Dom,
    el: ElId,
    ds: Option<&DesignSystemConfig>,
    seen: &mut DesignSeen,
) -> Vec<BrowserFinding> {
    let Some(ds) = ds else { return Vec::new() };
    if should_skip_design_element(dom, el) {
        return Vec::new();
    }
    let tag = {
        let tag = tag_lower(dom, el);
        if tag.is_empty() {
            "unknown".to_string()
        } else {
            tag
        }
    };
    let style = crate::design_system::ComputedElementStyle {
        tag,
        sample_text: browser_sample_text(dom, el),
        has_direct_text: browser_has_direct_text(dom, el),
        font_family: dom.style(el, "fontFamily"),
        color: dom.style(el, "color"),
        background_color: dom.style(el, "backgroundColor"),
        border_widths: ["Top", "Right", "Bottom", "Left"]
            .map(|side| super::dom::style_px(dom, el, &format!("border{side}Width"))),
        border_colors: ["Top", "Right", "Bottom", "Left"]
            .map(|side| dom.style(el, &format!("border{side}Color"))),
        outline_width: super::dom::style_px(dom, el, "outlineWidth"),
        outline_color: dom.style(el, "outlineColor"),
        border_radius: dom.style(el, "borderRadius"),
    };
    let tokens = crate::design_system::DesignSystemTokens {
        has_fonts: ds.has_fonts,
        allowed_fonts: &ds.allowed_fonts,
        has_colors: ds.has_colors,
        allowed_colors: &ds.allowed_colors,
        has_radii: ds.has_radii,
        allowed_radii_px: &ds.allowed_radii,
        has_pill_radius: ds.has_pill_radius,
    };
    crate::design_system::check_computed_element(&style, &tokens, seen)
        .into_iter()
        .map(|finding| BrowserFinding {
            type_: finding.type_,
            detail: finding.detail,
            severity: None,
            ignore_value: Some(finding.ignore_value),
        })
        .collect()
}
/// JS `decodeURIComponent(s)`: `None` where it throws (malformed escape,
/// invalid UTF-8).
pub fn decode_uri_component(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let h = (bytes[i + 1] as char).to_digit(16)?;
            let l = (bytes[i + 2] as char).to_digit(16)?;
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

pub fn decode_browser_google_family(value: &str) -> String {
    let family = value.split(':').next().unwrap_or("").replace('+', " ");
    decode_uri_component(&family).unwrap_or(family)
}

static GOOGLE_FAMILY_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::Regex::new(r"[?&]family=([^&]+)").expect("GOOGLE_FAMILY_RE")
});

pub fn check_browser_design_system_sources(
    dom: &dyn Dom,
    ds: Option<&DesignSystemConfig>,
    seen: &mut DesignSeen,
) -> Vec<BrowserFinding> {
    let Some(ds) = ds else { return Vec::new() };
    if !ds.has_fonts {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for link in dom
        .query_all(None, "link[href*=\"fonts.googleapis.com/css\"]")
        .unwrap_or_default()
    {
        let href = dom.attr(link, "href").unwrap_or_default();
        for m in GOOGLE_FAMILY_RE.captures_iter(&href) {
            let display = decode_browser_google_family(&m[1]);
            let font = normalize_browser_font_name(&display);
            if font.is_empty() || ds.allowed_fonts.contains(&font) || seen.fonts.contains(&font) {
                continue;
            }
            seen.fonts.push(font);
            findings.push(BrowserFinding {
                type_: "design-system-font".to_string(),
                detail: format!(
                    "Google Fonts: {} is not declared in DESIGN.md typography",
                    display
                ),
                severity: None,
                ignore_value: Some(display),
            });
        }
    }
    findings
}

// ─── Regex-on-HTML pass ─────────────────────────────────────────────────────

static ONLY_COMMAS_WS_RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::Regex::new(&format!("^[,{}]*$", crate::js::WS_CHARS)).expect("ONLY_COMMAS_WS_RE")
});

/// JS `s.replace(/,\s*(?=,|$)/g, '')`: drop a comma (and the whitespace
/// after it) that is followed by another comma or the end.
fn drop_dangling_commas(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < chars.len() && crate::js::is_js_whitespace(chars[j]) {
                j += 1;
            }
            if j >= chars.len() || chars[j] == ',' {
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_selector_name_char(c: Option<char>) -> bool {
    matches!(c, Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

///
/// Rewrites a selector so its pseudo-elements resolve to the live element
/// that originates them: `.card::before` to `.card`, and a hostless
/// `main > ::before` to `main > *`. `None` when the selector carries no
/// pseudo-element at all, which is the caller's signal that the full
/// selector is queryable as written.
///
/// JS-PARITY: the JS indexes UTF-16 code units; this walks chars, which
/// differs only for an astral character inside a selector literal.
pub fn pseudo_element_host_selector(selector: &str) -> Option<String> {
    const LEGACY_NAMES: &[&str] = &["before", "after", "first-letter", "first-line"];
    let raw: Vec<char> = selector.chars().collect();
    let consume_function = |start: usize| -> usize {
        let mut depth: i32 = 0;
        let mut quote: Option<char> = None;
        let mut i = start;
        while i < raw.len() {
            let ch = raw[i];
            if ch == '\\' {
                i += 2;
                continue;
            }
            if let Some(q) = quote {
                if ch == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            if ch == '"' || ch == '\'' {
                quote = Some(ch);
                i += 1;
                continue;
            }
            if ch == '(' {
                depth += 1;
            }
            if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            i += 1;
        }
        raw.len()
    };

    let mut output = String::new();
    let mut found = false;
    let mut i = 0usize;
    while i < raw.len() {
        let ch = raw[i];
        if ch == '\\' {
            let end = raw.len().min(i + 2);
            output.extend(&raw[i..end]);
            i += 2;
            continue;
        }
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let start = i;
            i += 1;
            while i < raw.len() {
                if raw[i] == '\\' {
                    i += 2;
                    continue;
                }
                let value = raw[i];
                i += 1;
                if value == quote {
                    break;
                }
            }
            output.extend(&raw[start..raw.len().min(i)]);
            continue;
        }
        if ch != ':' {
            output.push(ch);
            i += 1;
            continue;
        }

        let (mut end, is_pseudo_element) = if raw.get(i + 1) == Some(&':') {
            let mut end = i + 2;
            let name_start = end;
            while is_selector_name_char(raw.get(end).copied()) {
                end += 1;
            }
            (end, end > name_start)
        } else {
            let mut end = i + 1;
            let name_start = end;
            while is_selector_name_char(raw.get(end).copied()) {
                end += 1;
            }
            let name: String = raw[name_start..end].iter().collect();
            (
                end,
                LEGACY_NAMES.contains(&crate::js::to_lower_case(&name).as_str()),
            )
        };
        if !is_pseudo_element {
            output.push(ch);
            i += 1;
            continue;
        }
        if raw.get(end) == Some(&'(') {
            end = consume_function(end);
        }
        found = true;
        let last = output.chars().last();
        if last.is_none()
            || matches!(last, Some(c) if crate::js::is_js_whitespace(c)
                || c == '>' || c == '+' || c == '~' || c == ',')
        {
            output.push('*');
        }
        i = end;
    }
    if !found {
        return None;
    }
    Some(drop_dangling_commas(crate::js::trim(&output)))
}

///
/// `None` means "unresolvable": the DOM API refused the selector, or the
/// pseudo-element rewrite left nothing queryable. An empty vector from a
/// selector the DOM did accept is authoritative, so an inactive
/// `:hover` / `:focus` / `:not()` rule is never broadened to its host.
pub fn selector_nodes_for_live_dom(dom: &dyn Dom, selector: &str) -> Option<Vec<ElId>> {
    let raw = crate::js::trim(selector);
    if raw.is_empty() {
        return None;
    }
    let Some(fallback) = pseudo_element_host_selector(raw) else {
        return dom.query_all(None, raw).ok();
    };
    if fallback.is_empty() || ONLY_COMMAS_WS_RE.is_match(&fallback) {
        return None;
    }
    dom.query_all(None, &fallback).ok()
}

/// The regex-on-HTML pass of collectBrowserFindings: `checkHtmlPatterns` on
/// the live document's HTML, selector-scoped filtering against the live DOM
/// (a selector matching nothing drops the finding; a match under a
/// data-impeccino-ignore ancestor is waived), and the mapping with the
/// pulsing-dot hero promotion. Returns `{ type, detail, severity? }`; the
/// caller applies `_ruleOk`.
pub fn scoped_html_pattern_findings(dom: &dyn Dom) -> Vec<BrowserFinding> {
    let html = dom.document_html_for_patterns();
    // Linked stylesheets are absent from the page's outerHTML, so the probe
    // hands their readable, live-resolving rules to the style corpus (pbakaus/impeccable#709).
    let mut corpora = crate::checks::html_patterns::build_html_pattern_corpora(&html);
    let linked_css = dom.linked_stylesheet_text();
    if !linked_css.is_empty() {
        corpora.style_text.push('\n');
        corpora.style_text.push_str(&linked_css);
    }
    let all = crate::checks::html_patterns::check_html_patterns(&html, Some(&corpora));
    let mut out = Vec::new();
    for f in all {
        if let Some(selector) = f.selector.as_deref().filter(|s| !s.is_empty()) {
            let Some(matches) = selector_nodes_for_live_dom(dom, selector) else {
                continue;
            };
            if matches.is_empty() {
                continue;
            }
            if !matches
                .iter()
                .any(|el| !scoped_ignore_active(dom, *el, &f.id))
            {
                continue;
            }
        }
        let mut item = BrowserFinding::new(f.id.clone(), f.snippet.clone());
        if let Some(sev) = f.severity.as_ref().filter(|s| !s.is_empty()) {
            item.severity = Some(sev.clone());
        } else if f.id == "pulsing-dot" {
            if let Some(selector) = f.selector.as_deref().filter(|s| !s.is_empty()) {
                if let Ok(Some(dot)) = dom.query_one(None, selector) {
                    let rect = dom.rect(dot);
                    let page_top = rect.top + dom.scroll_y();
                    if page_top <= 900.0 {
                        item.severity = Some("error".to_string());
                    }
                }
            }
        }
        out.push(item);
    }
    out
}

// JS `/^(css|sc|emotion|jsx|module)-[\w-]{4,}$/i`, `/^_[\w-]{5,}$/`,
// `/^[a-z0-9]{6,}$/i` (JS `\w` is ASCII; the `i` flag folds ASCII only).
static HASHED_1: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
    regex::Regex::new(&format!(
        "^({}|{}|{}|{}|{})-[A-Za-z0-9_-]{{4,}}$",
        crate::js::ci("css"),
        crate::js::ci("sc"),
        crate::js::ci("emotion"),
        crate::js::ci("jsx"),
        crate::js::ci("module")
    ))
    .expect("HASHED_1")
});
static HASHED_2: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| regex::Regex::new(r"^_[A-Za-z0-9_-]{5,}$").expect("HASHED_2"));
static HASHED_3: once_cell::sync::Lazy<regex::Regex> =
    once_cell::sync::Lazy::new(|| regex::Regex::new(r"^[a-zA-Z0-9]{6,}$").expect("HASHED_3"));

pub fn is_likely_hashed_class(c: &str) -> bool {
    if c.is_empty() {
        return true;
    }
    if HASHED_1.is_match(c) {
        return true;
    }
    if HASHED_2.is_match(c) {
        return true;
    }
    if HASHED_3.is_match(c) && c.bytes().any(|b| b.is_ascii_digit()) {
        return true;
    }
    false
}

/// JS `[...el.classList]`: the class attribute split on ASCII whitespace,
/// duplicates removed (DOMTokenList semantics), order preserved.
fn class_list(dom: &dyn Dom, el: ElId) -> Vec<String> {
    let cls = dom.attr(el, "class").unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for tok in cls.split([' ', '\t', '\n', '\x0C', '\r']) {
        if tok.is_empty() || out.iter().any(|t| t == tok) {
            continue;
        }
        out.push(tok.to_string());
    }
    out
}

pub fn build_selector_segment(dom: &dyn Dom, el: ElId) -> String {
    let tag = tag_lower(dom, el);
    let mut sel = tag.clone();
    let classes: Vec<String> = class_list(dom, el)
        .into_iter()
        .filter(|c| !c.starts_with("impeccino-") && !is_likely_hashed_class(c))
        .take(2)
        .collect();
    if !classes.is_empty() {
        sel.push('.');
        sel.push_str(
            &classes
                .iter()
                .map(|c| dom.css_escape(c))
                .collect::<Vec<_>>()
                .join("."),
        );
    }
    if let Some(parent) = dom.parent(el) {
        match dom.query_all(Some(parent), &format!(":scope > {sel}")) {
            Ok(matching) => {
                if matching.len() > 1 {
                    let tag_name = dom.tag_name(el);
                    let same_type: Vec<ElId> = dom
                        .children(parent)
                        .into_iter()
                        .filter(|c| dom.tag_name(*c) == tag_name)
                        .collect();
                    let idx = same_type
                        .iter()
                        .position(|c| *c == el)
                        .map(|i| i as i64)
                        .unwrap_or(-1)
                        + 1;
                    sel.push_str(&format!(":nth-of-type({idx})"));
                }
            }
            Err(_) => {
                let idx = dom
                    .children(parent)
                    .iter()
                    .position(|c| *c == el)
                    .map(|i| i as i64)
                    .unwrap_or(-1)
                    + 1;
                sel = format!("{tag}:nth-child({idx})");
            }
        }
    }
    sel
}

pub fn generate_selector(dom: &dyn Dom, el: ElId) -> String {
    let body = dom.body();
    let root = dom.document_element();
    if Some(el) == body {
        return "body".to_string();
    }
    if Some(el) == root {
        return "html".to_string();
    }
    let el_id = super::dom::safe_id(dom, el);
    if !el_id.is_empty() {
        return format!("#{}", dom.css_escape(&el_id));
    }
    let mut parts: Vec<String> = Vec::new();
    let mut current = Some(el);
    let mut depth = 0;
    const MAX_DEPTH: usize = 10;
    while let Some(cur) = current {
        if Some(cur) == body || Some(cur) == root || depth >= MAX_DEPTH {
            break;
        }
        parts.insert(0, build_selector_segment(dom, cur));
        // JS `current.id` (the raw property, truthy check). Where the
        // property is not a string (shadowed by a named control) the object
        // is truthy and CSS.escape stringifies it; that garbage-selector case
        // is exactly issue pbakaus/impeccable#407 for the anchor path and is left as the JS
        // does it: id_prop None means the getter returned an element, which
        // is truthy → escape("[object HTMLInputElement]").
        let cur_id = match dom.id_prop(cur) {
            Some(id) => id,
            None => "[object HTMLInputElement]".to_string(),
        };
        if !cur_id.is_empty() {
            parts[0] = format!("#{}", dom.css_escape(&cur_id));
            break;
        }
        let try_selector = parts.join(" > ");
        if let Ok(matches) = dom.query_all(None, &try_selector) {
            if matches.len() == 1 && matches[0] == el {
                return try_selector;
            }
        }
        current = dom.parent(cur);
        depth += 1;
    }
    parts.join(" > ")
}

pub fn is_element_hidden(dom: &dyn Dom, el: ElId) -> bool {
    if Some(el) == dom.body() || Some(el) == dom.document_element() {
        return false;
    }
    if let Some(v) = dom.check_visibility(el) {
        return !v;
    }
    dom.offset_width(el) == 0.0 && dom.offset_height(el) == 0.0
}

fn hits(v: Vec<crate::checks::rules::RuleHit>) -> Vec<BrowserFinding> {
    v.iter().map(BrowserFinding::from_hit).collect()
}

pub fn collect_browser_findings(dom: &dyn Dom, config: &BrowserConfig) -> CollectResult {
    use super::element_checks as ec;
    use super::page_checks as pc;
    use super::quality as q;
    use super::text_collectors as tc;

    let mut groups: Vec<FindingGroup> = Vec::new();
    let mut page_level: Vec<BrowserFinding> = Vec::new();
    let design_system = config.design_system.as_ref();
    let mut design_seen = DesignSeen::default();
    // The AI palette is read over the whole page: neon ink on a near-black
    // ground waits here until a second tell hue turns up somewhere, so one
    // deliberate accent stays an accent (REN-405).
    let mut palette_tells: Vec<ec::TellHue> = Vec::new();
    let mut palette_ink: Vec<(ElId, BrowserFinding)> = Vec::new();
    let body = dom.body();
    let root = dom.document_element();
    // JS `document.body` may be null on a bare document; every
    // `addBrowserFindings(groupMap, document.body, ...)` then keys on null.
    // Elements never equal null, so the page-level groups collapse under
    // handle 0 the same way they collapse under null.
    let body_key = body.unwrap_or(0);

    for el in dom.query_all(None, "*").unwrap_or_default() {
        if Some(el) == body || Some(el) == root {
            continue;
        }

        let mut findings: Vec<BrowserFinding> = Vec::new();
        findings.extend(hits(ec::check_element_borders_dom(dom, el)));
        findings.extend(hits(ec::check_element_pseudo_stripe_dom(dom, el)));
        findings.extend(hits(ec::check_element_colors_dom(dom, el)));
        findings.extend(hits(ec::check_element_motion_dom(dom, el)));
        findings.extend(hits(ec::check_element_glow_dom(dom, el)));
        let palette = ec::check_element_ai_palette_dom(dom, el, design_system);
        // An ignored subtree gets no vote in the page-wide reading. A cyan
        // tell inside `data-impeccino-ignore="ai-color-palette"` would
        // otherwise open the two-hue gate and charge neon ink somewhere else
        // on the page that nobody waived — ignored content changing the
        // result for content that was not ignored.
        if !scoped_ignore_active(dom, el, "ai-color-palette") {
            palette_tells.extend(palette.tells.iter().copied());
        }
        if let Some(ink) = palette.ink {
            palette_ink.push((el, BrowserFinding::new(ink.id, ink.snippet)));
        }
        findings.extend(hits(palette.hits));
        findings.extend(hits(ec::check_element_radial_spotlight_dom(dom, el)));
        findings.extend(hits(ec::check_element_icon_tile_dom(dom, el)));
        findings.extend(hits(ec::check_element_italic_serif_dom(dom, el)));
        findings.extend(hits(q::check_element_quality_dom(dom, el)));
        findings.extend(hits(ec::check_element_oversized_h1_dom(dom, el)));
        findings.extend(hits(ec::check_element_clipped_overflow_dom(dom, el)));
        findings.extend(hits(ec::check_element_gpt_border_shadow_dom(dom, el)));
        findings.extend(hits(ec::check_element_text_overflow_dom(dom, el)));
        findings.extend(ec::check_element_blinking_cursor_dom(dom, el));
        findings.extend(check_element_design_system_dom(
            dom,
            el,
            design_system,
            &mut design_seen,
        ));
        add_browser_findings(dom, &mut groups, el, findings);

        // Hero eyebrow: highlight the previous sibling instead.
        let eyebrow = hits(ec::check_element_hero_eyebrow_dom(dom, el));
        if !eyebrow.is_empty() {
            if let Some(prev) = dom.previous_element_sibling(el) {
                add_browser_findings(dom, &mut groups, prev, eyebrow);
            }
        }
    }

    // Two different tell hues on one page is the palette; one is an accent.
    if palette_tells.contains(&ec::TellHue::Cyan) && palette_tells.contains(&ec::TellHue::Purple) {
        for (el, finding) in palette_ink {
            add_browser_findings(dom, &mut groups, el, vec![finding]);
        }
    }

    let page_pass = |groups: &mut Vec<FindingGroup>,
                     page_level: &mut Vec<BrowserFinding>,
                     list: Vec<BrowserFinding>| {
        if !list.is_empty() {
            page_level.extend(list.iter().cloned());
            add_browser_findings(dom, groups, body_key, list);
        }
    };

    let el_pass = |groups: &mut Vec<FindingGroup>, list: Vec<super::ElFinding>| {
        for f in list {
            let target = f.el.unwrap_or(body_key);
            add_browser_findings(
                dom,
                groups,
                target,
                vec![BrowserFinding::new(
                    f.finding.type_.clone(),
                    f.finding.detail.clone(),
                )],
            );
        }
    };

    page_pass(
        &mut groups,
        &mut page_level,
        check_browser_design_system_sources(dom, design_system, &mut design_seen),
    );
    page_pass(&mut groups, &mut page_level, pc::check_typography(dom));
    el_pass(
        &mut groups,
        tc::check_kicker_above_heading_dom(dom, design_system),
    );
    page_pass(
        &mut groups,
        &mut page_level,
        hits(tc::check_numbered_section_labels_dom(dom)),
    );
    page_pass(
        &mut groups,
        &mut page_level,
        hits(tc::check_repeated_container_text_dom(dom)),
    );
    page_pass(
        &mut groups,
        &mut page_level,
        hits(tc::check_em_dash_overuse_dom(dom)),
    );

    el_pass(&mut groups, pc::check_layout(dom));
    el_pass(&mut groups, pc::check_heading_rhythm_dom(dom));
    el_pass(&mut groups, pc::check_edge_flush_cards_dom(dom));
    el_pass(&mut groups, pc::check_text_occlusion_dom(dom));
    el_pass(
        &mut groups,
        pc::check_first_viewport_column_overflow_dom(dom),
    );

    page_pass(&mut groups, &mut page_level, q::check_page_quality_dom(dom));
    page_pass(
        &mut groups,
        &mut page_level,
        hits(pc::check_cream_palette(dom)),
    );
    page_pass(
        &mut groups,
        &mut page_level,
        scoped_html_pattern_findings(dom),
    );

    CollectResult { groups, page_level }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::fake_dom::FakeDom;

    fn design_system() -> DesignSystemConfig {
        DesignSystemConfig {
            declared_selectors: vec![],
            has_fonts: true,
            allowed_fonts: vec!["inter".to_string()],
            has_colors: true,
            allowed_colors: vec![crate::color::Rgba {
                r: 10.0,
                g: 20.0,
                b: 30.0,
                a: None,
            }],
            has_radii: true,
            allowed_radii: vec![8.0],
            has_pill_radius: false,
        }
    }

    #[test]
    fn primary_font_and_normalization() {
        assert_eq!(
            browser_primary_font("\"Inter\", system-ui, sans-serif"),
            "inter"
        );
        assert_eq!(browser_primary_font("system-ui, sans-serif"), "");
        assert_eq!(browser_primary_font("system-ui, Roboto"), "roboto");
        assert_eq!(browser_primary_font("var(--font)"), "");
        assert_eq!(
            normalize_browser_font_name("  'Space+Grotesk'  "),
            "space grotesk"
        );
    }

    #[test]
    fn design_element_findings_and_seen_dedupe() {
        let mut d = FakeDom::new();
        let (_h, body) = d.with_page();
        let p = d.add(Some(body), "p");
        d.add_text(p, "Hello   world");
        d.set_styles(
            p,
            &[
                ("fontFamily", "Comic Sans MS, cursive"),
                ("color", "rgb(255, 0, 0)"),
                ("backgroundColor", "rgba(0, 0, 0, 0)"),
                ("borderTopWidth", "0px"),
                ("borderRightWidth", "0px"),
                ("borderBottomWidth", "1px"),
                ("borderLeftWidth", "0px"),
                ("borderBottomColor", "rgb(10, 20, 30)"),
                ("outlineWidth", "0px"),
                ("borderRadius", "8px 3px / 2px"),
            ],
        );
        d.el_mut(p).check_visibility = Some(true);
        let ds = design_system();
        let mut seen = DesignSeen::default();
        let f = check_element_design_system_dom(&d, p, Some(&ds), &mut seen);
        let types: Vec<&str> = f.iter().map(|x| x.type_.as_str()).collect();
        assert_eq!(
            types,
            vec![
                "design-system-font",
                "design-system-color",
                "design-system-radius",
                "design-system-radius"
            ]
        );
        assert_eq!(
            f[0].detail,
            "p \"Hello world\" uses comic sans ms; not declared in DESIGN.md typography"
        );
        assert_eq!(f[0].ignore_value.as_deref(), Some("comic sans ms"));
        assert_eq!(
            f[1].detail,
            "text color rgb(255, 0, 0) on p \"Hello world\" is outside DESIGN.md colors"
        );
        assert_eq!(
            f[2].detail,
            "border-radius 3px on p \"Hello world\" is outside the DESIGN.md rounded scale"
        );
        assert_eq!(f[3].ignore_value.as_deref(), Some("2px"));
        // Second element with the same offenders adds nothing.
        let q = d.add(Some(body), "p");
        d.add_text(q, "Again");
        for (k, v) in d.el(p).styles.clone() {
            d.set_style(q, &k, &v);
        }
        d.el_mut(q).check_visibility = Some(true);
        assert!(check_element_design_system_dom(&d, q, Some(&ds), &mut seen).is_empty());
        // Hidden elements are skipped.
        d.el_mut(q).check_visibility = Some(false);
        let mut seen2 = DesignSeen::default();
        assert!(check_element_design_system_dom(&d, q, Some(&ds), &mut seen2).is_empty());
    }

    #[test]
    fn google_font_sources() {
        let mut d = FakeDom::new();
        let (html, _body) = d.with_page();
        let head = d.add(Some(html), "head");
        let link = d.add(Some(head), "link");
        d.set_attr(
            link,
            "href",
            "https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;700&family=Inter&family=Bad%ZZ",
        );
        d.add_selector(link, "link[href*=\"fonts.googleapis.com/css\"]");
        let ds = DesignSystemConfig {
            has_fonts: true,
            allowed_fonts: vec!["inter".to_string()],
            ..Default::default()
        };
        let mut seen = DesignSeen::default();
        let f = check_browser_design_system_sources(&d, Some(&ds), &mut seen);
        assert_eq!(f.len(), 2);
        assert_eq!(
            f[0].detail,
            "Google Fonts: Space Grotesk is not declared in DESIGN.md typography"
        );
        assert_eq!(f[0].ignore_value.as_deref(), Some("Space Grotesk"));
        // malformed escape: decodeURIComponent throws, the raw family stays
        assert_eq!(f[1].ignore_value.as_deref(), Some("Bad%ZZ"));
        assert_eq!(decode_uri_component("caf%C3%A9"), Some("café".to_string()));
        assert_eq!(decode_uri_component("%E2%82"), None);
    }

    #[test]
    fn selector_generation() {
        let mut d = FakeDom::new();
        let (_h, body) = d.with_page();
        let main = d.add(Some(body), "main");
        d.set_attr(main, "id", "app");
        let sec = d.add(Some(main), "section");
        d.set_attr(sec, "class", "hero css-1a2b3c impeccino-x   hero-inner");
        let a = d.add(Some(sec), "p");
        let b = d.add(Some(sec), "p");
        d.set_attr(b, "class", "lead");
        // The fake matches `:scope > p` for both paragraphs and the composed
        // selectors document-wide.
        d.add_selector(a, ":scope > p");
        d.add_selector(b, ":scope > p");
        d.add_selector(b, ":scope > p.lead");
        assert_eq!(generate_selector(&d, main), "#app");
        assert_eq!(generate_selector(&d, body), "body");
        // p without class: two `:scope > p` matches → nth-of-type; the
        // partial `section.hero.hero-inner > p:nth-of-type(1)` matches nothing
        // in the fake, so the walk continues to the #app anchor.
        assert_eq!(
            generate_selector(&d, a),
            "#app > section.hero.hero-inner > p:nth-of-type(1)"
        );
        // Unique partial selector returns early.
        d.add_selector(b, "p.lead");
        assert_eq!(generate_selector(&d, b), "p.lead");
        assert!(is_likely_hashed_class("css-1a2b3c"));
        assert!(is_likely_hashed_class("_2x4hG"));
        assert!(is_likely_hashed_class("a1b2c3"));
        assert!(!is_likely_hashed_class("hero"));
        assert!(!is_likely_hashed_class("abcdefg"));
    }

    #[test]
    fn scans_former_overlay_and_provider_owned_elements() {
        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let mut targets = Vec::new();
        for (id, class) in [
            ("claude-generated", None),
            ("cic-generated", None),
            ("ordinary-overlay", Some("impeccino-overlay")),
            ("impeccino-live-old", None),
        ] {
            let h1 = dom.add(Some(body), "h1");
            dom.set_attr(h1, "id", id);
            if let Some(class) = class {
                dom.set_attr(h1, "class", class);
                dom.add_selector(h1, &format!(".{class}"));
            }
            dom.add_text(h1, "A deliberately oversized headline for this page");
            dom.set_style(h1, "fontSize", "96px");
            dom.set_rect(h1, 0.0, 0.0, 1200.0, 300.0);
            targets.push(h1);
        }

        let result = collect_browser_findings(&dom, &BrowserConfig::default());
        for target in targets {
            assert!(
                result.groups.iter().any(|group| {
                    group.el == target
                        && group
                            .findings
                            .iter()
                            .any(|finding| finding.type_ == "oversized-h1")
                }),
                "former tool markup must not suppress element {target}"
            );
        }
    }

    /// REN-405. Northwind's Slate system: near-black ground, light ink, one
    /// teal accent. The accent lit 18 places on a page with nothing wrong with
    /// it. It stays quiet until the page shows the other half of the palette.
    #[test]
    fn one_accent_hue_on_dark_is_not_the_ai_palette() {
        let build = |gradient: bool| {
            let mut d = FakeDom::new();
            let (_html, body) = d.with_page();
            d.set_style(body, "backgroundColor", "rgb(15, 18, 17)");
            d.set_rect(body, 0.0, 0.0, 1440.0, 900.0);
            for i in 0..3 {
                let a = d.add(Some(body), "a");
                d.add_text(a, "Open the ledger");
                d.set_rect(a, 40.0, 40.0 + 30.0 * (i as f64), 160.0, 20.0);
                d.set_styles(a, &[("color", "rgb(47, 184, 166)")]);
            }
            if gradient {
                let hero = d.add(Some(body), "div");
                d.set_rect(hero, 0.0, 200.0, 1440.0, 320.0);
                d.set_style(
                    hero,
                    "backgroundImage",
                    "linear-gradient(135deg, rgb(124, 58, 237) 0%, rgb(168, 85, 247) 100%)",
                );
            }
            d
        };
        let ids = |d: &FakeDom| {
            collect_browser_findings(d, &BrowserConfig::default())
                .groups
                .iter()
                .flat_map(|g| g.findings.iter())
                .filter(|f| f.type_ == "ai-color-palette")
                .map(|f| f.detail.clone())
                .collect::<Vec<_>>()
        };
        // One teal accent on near-black: an accent.
        assert_eq!(ids(&build(false)), Vec::<String>::new());
        // The same accent beside a purple gradient: the palette, and every
        // place it shows is named.
        assert_eq!(
            ids(&build(true)),
            vec![
                "Purple/violet gradient background".to_string(),
                "Cyan neon text on dark background".to_string(),
                "Cyan neon text on dark background".to_string(),
                "Cyan neon text on dark background".to_string(),
            ]
        );
    }

    /// End to end through the collector: a page whose colors are all its own
    /// documented oklch tokens must not report `ai-color-palette`, while a
    /// color the DESIGN.md never declared still reports both rules.
    #[test]
    fn ai_palette_respects_the_design_system_palette() {
        // oklch(24% 0 0) instrument face carrying oklch(70% 0.12 188) verdigris.
        let make_dom = |text_color: &str, second_color: &str| {
            let mut d = FakeDom::new();
            let (_h, body) = d.with_page();
            let panel = d.add(Some(body), "div");
            d.set_styles(panel, &[("backgroundColor", "rgb(58, 58, 58)")]);
            d.el_mut(panel).check_visibility = Some(true);
            let label = d.add(Some(panel), "span");
            d.add_text(label, "Live");
            d.set_styles(
                label,
                &[
                    ("color", text_color),
                    ("backgroundColor", "rgba(0, 0, 0, 0)"),
                    ("fontFamily", "Inter, sans-serif"),
                ],
            );
            d.el_mut(label).check_visibility = Some(true);
            let second = d.add(Some(panel), "span");
            d.add_text(second, "Status");
            d.set_styles(
                second,
                &[("color", second_color), ("fontFamily", "Inter, sans-serif")],
            );
            d.el_mut(second).check_visibility = Some(true);
            d
        };
        let types = |out: &CollectResult| -> Vec<String> {
            out.groups
                .iter()
                .flat_map(|g| g.findings.iter().map(|f| f.type_.clone()))
                .collect()
        };
        let design_system = DesignSystemConfig {
            has_fonts: true,
            allowed_fonts: vec!["inter".to_string()],
            has_colors: true,
            allowed_colors: vec![
                crate::color::Rgba {
                    r: 15.0,
                    g: 182.0,
                    b: 172.0,
                    a: None,
                },
                crate::color::Rgba {
                    r: 168.0,
                    g: 85.0,
                    b: 247.0,
                    a: None,
                },
                crate::color::Rgba {
                    r: 58.0,
                    g: 58.0,
                    b: 58.0,
                    a: None,
                },
            ],
            ..Default::default()
        };
        let with_ds = BrowserConfig {
            design_system: Some(design_system),
        };
        let without_ds = BrowserConfig::default();

        // No DESIGN.md: two unexplained hues form a palette, not one accent.
        let out = collect_browser_findings(
            &make_dom("rgb(15, 182, 172)", "rgb(168, 85, 247)"),
            &without_ds,
        );
        assert!(types(&out).contains(&"ai-color-palette".to_string()));

        // Declared token: neither the palette rule nor the drift rule fires.
        let out = collect_browser_findings(
            &make_dom("rgb(15, 182, 172)", "rgb(168, 85, 247)"),
            &with_ds,
        );
        assert!(
            !types(&out).contains(&"ai-color-palette".to_string()),
            "{:?}",
            types(&out)
        );
        assert!(
            !types(&out).contains(&"design-system-color".to_string()),
            "{:?}",
            types(&out)
        );

        // A declared purple does not open the two-hue gate for undeclared cyan.
        let out =
            collect_browser_findings(&make_dom("rgb(0, 229, 255)", "rgb(168, 85, 247)"), &with_ds);
        assert!(
            !types(&out).contains(&"ai-color-palette".to_string()),
            "{:?}",
            types(&out)
        );
        assert!(
            types(&out).contains(&"design-system-color".to_string()),
            "{:?}",
            types(&out)
        );

        // Two undeclared hues still report both rules.
        let out =
            collect_browser_findings(&make_dom("rgb(0, 229, 255)", "rgb(220, 0, 255)"), &with_ds);
        assert!(
            types(&out).contains(&"ai-color-palette".to_string()),
            "{:?}",
            types(&out)
        );
        assert!(
            types(&out).contains(&"design-system-color".to_string()),
            "{:?}",
            types(&out)
        );
    }

    /// An ignored subtree does not get to open the page-wide palette gate.
    /// `ai-color-palette` holds neon ink until a second tell hue turns up
    /// somewhere on the page; a cyan tell inside a
    /// `data-impeccino-ignore="ai-color-palette"` subtree used to count
    /// toward that, so waiving one component charged an unrelated one.
    #[test]
    fn ignored_colors_do_not_contribute_tell_hues() {
        let build = |ignore: bool| {
            let mut d = FakeDom::new();
            let (_h, body) = d.with_page();
            d.set_style(body, "backgroundColor", "rgb(5, 6, 10)");

            // The waived component: cyan neon ink on near-black.
            let demo = d.add(Some(body), "div");
            d.set_style(demo, "backgroundColor", "rgb(5, 6, 10)");
            if ignore {
                d.set_attr(demo, "data-impeccino-ignore", "ai-color-palette");
            }
            let cyan = d.add(Some(demo), "span");
            d.add_text(cyan, "Terminal output");
            d.set_style(cyan, "color", "rgb(34, 238, 238)");
            d.set_style(cyan, "backgroundColor", "rgba(0, 0, 0, 0)");
            d.el_mut(cyan).check_visibility = Some(true);

            // Somewhere else on the page, and waived by nobody.
            let card = d.add(Some(body), "div");
            d.set_style(card, "backgroundColor", "rgb(5, 6, 10)");
            let purple = d.add(Some(card), "span");
            d.add_text(purple, "Upgrade");
            d.set_style(purple, "color", "rgb(180, 60, 245)");
            d.set_style(purple, "backgroundColor", "rgba(0, 0, 0, 0)");
            d.el_mut(purple).check_visibility = Some(true);
            d
        };
        let charged = |d: &FakeDom| -> Vec<String> {
            collect_browser_findings(d, &BrowserConfig::default())
                .groups
                .iter()
                .flat_map(|g| g.findings.iter().map(|f| f.type_.clone()))
                .filter(|t| t == "ai-color-palette")
                .collect()
        };
        // Two tell hues, neither waived: the palette is the page's.
        assert_eq!(charged(&build(false)).len(), 2);
        // The cyan half waived: one tell hue is an accent, and the purple ink
        // outside the ignored subtree is not charged either.
        assert!(charged(&build(true)).is_empty());
    }

    #[test]
    fn scoped_ignore_filters_findings() {
        let mut d = FakeDom::new();
        let (_h, body) = d.with_page();
        let wrap = d.add(Some(body), "div");
        d.set_attr(wrap, "data-impeccino-ignore", "low-contrast, glow-effect");
        let p = d.add(Some(wrap), "p");
        assert!(scoped_ignore_active(&d, p, "LOW-CONTRAST"));
        assert!(!scoped_ignore_active(&d, p, "side-tab"));
        let mut groups = Vec::new();
        add_browser_findings(
            &d,
            &mut groups,
            p,
            vec![
                BrowserFinding::new("low-contrast", "browser contrast 2.0:1"),
                BrowserFinding::new("glow-effect", "glow"),
            ],
        );
        assert!(groups.is_empty());

        d.set_attr(wrap, "data-impeccino-ignore", "glow-effect");
        add_browser_findings(
            &d,
            &mut groups,
            p,
            vec![
                BrowserFinding::new("low-contrast", "browser contrast 2.0:1"),
                BrowserFinding::new("glow-effect", "glow"),
            ],
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].findings.len(), 1);
        assert_eq!(groups[0].findings[0].type_, "low-contrast");
    }
}

#[cfg(test)]
mod pseudo_host_tests {
    use super::pseudo_element_host_selector as host;

    #[test]
    fn pseudo_element_hosts() {
        // No pseudo-element: the full selector stays queryable as written.
        assert_eq!(host(".card"), None);
        assert_eq!(host("a:hover"), None);
        assert_eq!(host("li:not(.x)"), None);
        // Attached and hostless pseudo-elements.
        assert_eq!(host(".card::before"), Some(".card".to_string()));
        assert_eq!(host("main > ::before"), Some("main > *".to_string()));
        assert_eq!(host("::after"), Some("*".to_string()));
        // The legacy one-colon spellings only.
        assert_eq!(host(".c:before"), Some(".c".to_string()));
        assert_eq!(host(".c:focus"), None);
        // Functional pseudo-elements consume their argument list.
        assert_eq!(host("p::part(label) span"), Some("p span".to_string()));
        // Literals are preserved, colons inside them are not pseudo starts.
        assert_eq!(host("[data-x=\"a::b\"]"), None);
        assert_eq!(
            host("[data-x=\"a::b\"]::before"),
            Some("[data-x=\"a::b\"]".to_string())
        );
        // A pseudo-class keeps its colon while a pseudo-element resolves.
        assert_eq!(host("a:hover::after"), Some("a:hover".to_string()));
        // Values recorded from the JS on origin/main (pbakaus/impeccable#709).
        assert_eq!(host(".a::before, .b"), Some(".a, .b".to_string()));
        assert_eq!(host(".a::before,"), Some(".a".to_string()));
        assert_eq!(host("::before ::after"), Some("* *".to_string()));
        assert_eq!(host(r"\:esc::before"), Some(r"\:esc".to_string()));
        assert_eq!(host("a::before("), Some("a".to_string()));
        assert_eq!(host("div::first-line"), Some("div".to_string()));
    }
}
