//! Page-level checks: `checkStaticPageTypography` (detect-html.mjs) and the
//! Section 6 document walks from `checks.mjs` (`isCardLike`,
//! `checkPageLayout`, `collectRepeatedContainerTextFindings`,
//! `checkRepeatedContainerTextFromDoc`, `checkCreamPalette`).

use crate::adapters::{class_selector, StyleRef};
use crate::background::{read_own_background_color, resolve_border_radius_px, sv};
use crate::dom::{StaticDocument, StaticElement};
use crate::quality::{has_nonblank_direct_text, pf0};
use impeccino_core::checks::measures::{
    cream_from_class_list, css_color_is_transparent, is_cream_color,
};
use impeccino_core::checks::rules::{
    check_flat_type_hierarchy_samples, is_card_like_from_props, type_hierarchy_role, RuleHit,
    TypeSample, TYPE_HIERARCHY_SELECTOR,
};
use impeccino_core::checks::text_rules::{
    is_repeated_text_container, REPEATED_TEXT_CONTAINER_TAGS, REPEATED_TEXT_SKIP_SELECTOR,
};
use impeccino_core::constants::{CSS_GENERIC_FONTS, OVERUSED_FONTS, SAFE_TAGS};
use impeccino_core::js::{self, number_to_string, parse_float};
use impeccino_core::js_ext_b::{slice_utf16_prefix, utf16_len};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&format!("{}+", js::WS)).expect("WS_RE"));

/// `f.trim().replace(/^['"]|['"]$/g, '').toLowerCase()`
fn font_token(f: &str) -> String {
    let t = js::trim(f);
    let t = t.strip_prefix(['\'', '"']).unwrap_or(t);
    let t = t.strip_suffix(['\'', '"']).unwrap_or(t);
    js::to_lower_case(t)
}

/// JS: detect-html.mjs#checkStaticPageTypography(document, window)
pub fn check_static_page_typography(doc: &StaticDocument) -> Vec<RuleHit> {
    let mut findings = Vec::new();
    let mut font_usage: Vec<(String, usize)> = Vec::new();
    let mut total_text_elements = 0usize;
    for el in doc.query_selector_all(
        "p, h1, h2, h3, h4, h5, h6, li, td, th, dd, blockquote, figcaption, a, button, label, span",
    ) {
        if !has_nonblank_direct_text(&el) {
            continue;
        }
        let ff = sv(el.style(), "fontFamily");
        // JS-PARITY: detect-html.mjs#checkStaticPageTypography uses
        // primaryFontFace(ff) whose default skip is CSS_GENERIC_FONTS, so a
        // system stack keeps its system face as primary (fix #678).
        let primary = ff
            .split(',')
            .map(font_token)
            .find(|f| !f.is_empty() && !CSS_GENERIC_FONTS.contains(&f.as_str()));
        let Some(primary) = primary else {
            continue;
        };

        if let Some((_, count)) = font_usage.iter_mut().find(|(font, _)| font == &primary) {
            *count += 1;
        } else {
            font_usage.push((primary, 1));
        }
        total_text_elements += 1;
    }

    if total_text_elements >= 20 {
        // Keep source order on ties, as Array.prototype.sort does in the
        // browser path, then report only a unique leader. A static file scan
        // has no page hostname, so it cannot apply the browser-only own-domain
        // brand exemption.
        let mut ranked: Vec<&(String, usize)> = font_usage.iter().collect();
        ranked.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        if let Some((font, count)) = ranked.first().map(|(font, count)| (font, *count)) {
            let tied = ranked.get(1).map(|entry| entry.1) == Some(count);
            if !tied && OVERUSED_FONTS.contains(&font.as_str()) {
                findings.push(RuleHit::new(
                    "overused-font",
                    format!(
                        "Primary font: {} ({}% of text)",
                        font,
                        number_to_string(js::math_round(
                            count as f64 / total_text_elements as f64 * 100.0
                        ))
                    ),
                ));
            }
        }
    }
    findings.extend(check_flat_type_hierarchy_from_doc(doc));
    findings
}

/// JS: checks.mjs#isRenderedTypeElement over the static cascade.
///
/// JS-PARITY: jsdom's `el.hidden` reflects the `hidden` attribute, which the
/// attribute test already covers. `contentVisibility` only ever reads its
/// `STATIC_DEFAULT_STYLE` default here: css-cascade.mjs#STATIC_PROP_MAP has no
/// `content-visibility` entry, so a declared `content-visibility: hidden`
/// never reaches the static computed style.
fn is_rendered_type_element(el: &StaticElement<'_>) -> bool {
    let mut current = Some(*el);
    while let Some(node) = current {
        if node.get_attribute("hidden").is_some() {
            return false;
        }
        let style = node.style();
        let display = js::to_lower_case(sv(style, "display"));
        let visibility = js::to_lower_case(sv(style, "visibility"));
        let content_visibility = js::to_lower_case(sv(style, "contentVisibility"));
        if display == "none"
            || visibility == "hidden"
            || visibility == "collapse"
            || content_visibility == "hidden"
        {
            return false;
        }
        let opacity = parse_float(sv(style, "opacity"));
        if opacity.is_finite() && opacity <= 0.01 {
            return false;
        }
        current = node.parent_element();
    }
    true
}

/// JS: checks.mjs#checkFlatTypeHierarchyFromDoc over the static document.
pub fn check_flat_type_hierarchy_from_doc(doc: &StaticDocument) -> Vec<RuleHit> {
    let mut samples: Vec<TypeSample> = Vec::new();
    for el in doc.query_selector_all(TYPE_HIERARCHY_SELECTOR) {
        if js::trim(&el.text_content()).is_empty() || !is_rendered_type_element(&el) {
            continue;
        }
        let font_size = parse_float(sv(el.style(), "fontSize"));
        if !font_size.is_finite() || font_size < 8.0 || font_size >= 200.0 {
            continue;
        }
        samples.push(TypeSample {
            role: type_hierarchy_role(&el.tag_lower()),
            size: font_size,
        });
    }
    check_flat_type_hierarchy_samples(&samples)
}

// ─── Nested cards ───────────────────────────────────────────────────────────

static SHADOW_CLASS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?-u:\b)shadow(?:-sm|-md|-lg|-xl|-2xl)?(?-u:\b)").expect("SHADOW_CLASS_RE")
});
static BOX_SHADOW_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("(?i)box-shadow").expect("BOX_SHADOW_RE"));
static BORDER_CLASS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?-u:\b)border(?-u:\b)").expect("BORDER_CLASS_RE"));
static ROUNDED_CLASS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?-u:\b)rounded(?:-sm|-md|-lg|-xl|-2xl|-full)?(?-u:\b)").expect("ROUNDED_CLASS_RE")
});
static BORDER_RADIUS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("(?i)border-radius").expect("BORDER_RADIUS_RE"));
static BG_CLASS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?-u:\b)bg-(?:white|gray-[0-9]+|slate-[0-9]+)(?-u:\b)").expect("BG_CLASS_RE")
});
static POSITIONED_CLASS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?-u:\b)(?:absolute|fixed)(?-u:\b)").expect("POSITIONED_CLASS_RE"));
static POSITIONED_STYLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"(?i)position{ws}*:{ws}*(?:absolute|fixed)",
        ws = js::WS
    ))
    .expect("POSITIONED_STYLE_RE")
});
static OVERLAY_CLASS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:dropdown|popover|tooltip|menu|modal|dialog)(?-u:\b)")
        .expect("OVERLAY_CLASS_RE")
});

/// JS: checks.mjs#isCardLike(el, win)
pub fn is_card_like(el: &StaticElement<'_>) -> bool {
    let tag = el.tag_lower();
    if SAFE_TAGS.contains(&tag.as_str())
        || matches!(
            tag.as_str(),
            "input" | "select" | "textarea" | "img" | "video" | "canvas" | "picture"
        )
    {
        return false;
    }
    let style = el.style();
    let raw_style = el.get_attribute("style").unwrap_or("");
    let cls = el.get_attribute("class").unwrap_or("");
    let box_shadow = sv(style, "boxShadow");
    let has_shadow = (!box_shadow.is_empty() && box_shadow != "none")
        || SHADOW_CLASS_RE.is_match(cls)
        || BOX_SHADOW_RE.is_match(raw_style);
    let has_border = BORDER_CLASS_RE.is_match(cls);
    let width_px = pf0(sv(style, "width"));
    let has_radius = resolve_border_radius_px(style, width_px) > 0.0
        || ROUNDED_CLASS_RE.is_match(cls)
        || BORDER_RADIUS_RE.is_match(raw_style);
    let background = sv(style, "backgroundColor");
    let has_bg = (!background.is_empty() && !css_color_is_transparent(Some(background)))
        || BG_CLASS_RE.is_match(cls);
    is_card_like_from_props(has_shadow, has_border, has_radius, has_bg)
}

/// JS: checks.mjs#checkPageLayout(doc, win)
pub fn check_page_layout(doc: &StaticDocument) -> Vec<RuleHit> {
    let mut findings = Vec::new();
    let all = doc.query_selector_all("*");
    let mut flagged: Vec<StaticElement<'_>> = Vec::new();
    for el in &all {
        if !is_card_like(el) {
            continue;
        }
        if flagged.contains(el) {
            continue;
        }
        let tag = el.tag_lower();
        let cls = el.get_attribute("class").unwrap_or("");
        let raw_style = el.get_attribute("style").unwrap_or("");
        if tag == "pre" || tag == "code" {
            continue;
        }
        let position = sv(el.style(), "position");
        if position == "absolute"
            || position == "fixed"
            || POSITIONED_CLASS_RE.is_match(cls)
            || POSITIONED_STYLE_RE.is_match(raw_style)
        {
            continue;
        }
        if utf16_len(js::trim(&el.text_content())) < 10 {
            continue;
        }
        if OVERLAY_CLASS_RE.is_match(cls) {
            continue;
        }
        let mut parent = el.parent_element();
        while let Some(p) = parent {
            if is_card_like(&p) {
                flagged.push(*el);
                break;
            }
            parent = p.parent_element();
        }
    }
    for el in &flagged {
        let is_ancestor_of_flagged = flagged
            .iter()
            .any(|other| other != el && el.contains(other));
        if !is_ancestor_of_flagged {
            findings.push(RuleHit::new(
                "nested-cards",
                format!("Card inside card ({})", el.tag_lower()),
            ));
        }
    }
    findings
}

// ─── Repeated container text ────────────────────────────────────────────────

static ICON_CLASS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"(?i)icon|material-symbols|(?:^|{ws})fa[srlbd]?(?:{ws}|-|$)",
        ws = js::WS
    ))
    .expect("ICON_CLASS_RE")
});
static ALPHA_RE: Lazy<Regex> = Lazy::new(|| Regex::new("[a-zA-Z]").expect("ALPHA_RE"));

fn is_visible(el: &StaticElement<'_>) -> bool {
    sv(el.style(), "display") != "none"
}

/// JS: checks.mjs#collectRepeatedContainerTextFindings(doc, getStyle, opts)
/// with `isVisible = display !== 'none'` (`checkRepeatedContainerTextFromDoc`).
pub fn check_repeated_container_text_from_doc(doc: &StaticDocument) -> Vec<RuleHit> {
    let mut findings = Vec::new();
    let mut containers: Vec<StaticElement<'_>> = Vec::new();
    let mut container_set: HashSet<ego_tree::NodeId> = HashSet::new();
    for el in doc.query_selector_all("*") {
        if !REPEATED_TEXT_CONTAINER_TAGS.contains(&el.tag_lower().as_str()) {
            continue;
        }
        if el.closest(REPEATED_TEXT_SKIP_SELECTOR).is_some() {
            continue;
        }
        if !is_repeated_text_container(Some(&StyleRef(el.style()))) {
            continue;
        }
        containers.push(el);
        container_set.insert(el.id());
    }

    for container in &containers {
        if !is_visible(container) {
            continue;
        }
        let descendants = container.query_selector_all("*");
        if descendants.len() > 250 {
            continue;
        }
        // text -> signatures, in first-seen order.
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for d in &descendants {
            let mut anc = d.parent_element();
            let mut owned_by_inner = false;
            while let Some(a) = anc {
                if a == *container {
                    break;
                }
                if container_set.contains(&a.id()) {
                    owned_by_inner = true;
                    break;
                }
                anc = a.parent_element();
            }
            if owned_by_inner {
                continue;
            }
            if d.closest(REPEATED_TEXT_SKIP_SELECTOR).is_some() {
                continue;
            }
            if ICON_CLASS_RE.is_match(d.get_attribute("class").unwrap_or("")) {
                continue;
            }
            if !is_visible(d) {
                continue;
            }
            let direct = crate::adapters::clean_inline_text(d);
            let len = utf16_len(&direct);
            if !(4..=48).contains(&len) {
                continue;
            }
            if !ALPHA_RE.is_match(&direct) {
                continue;
            }
            let mut sig: Vec<String> = Vec::new();
            let mut cur = Some(*d);
            while let Some(c) = cur {
                if c == *container {
                    break;
                }
                let raw_cls = js::trim(c.get_attribute("class").unwrap_or(""));
                let mut cls: Vec<&str> = if raw_cls.is_empty() {
                    Vec::new()
                } else {
                    WS_RE.split(raw_cls).filter(|s| !s.is_empty()).collect()
                };
                cls.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                let cls = cls.join(".");
                sig.push(if cls.is_empty() {
                    c.tag_lower()
                } else {
                    format!("{}.{}", c.tag_lower(), cls)
                });
                cur = c.parent_element();
            }
            let joined = sig.join(">");
            match groups.iter_mut().find(|(t, _)| *t == direct) {
                Some((_, sigs)) => sigs.push(joined),
                None => groups.push((direct, vec![joined])),
            }
        }
        for (text, sigs) in &groups {
            if sigs.len() < 3 {
                continue;
            }
            let distinct: HashSet<&String> = sigs.iter().collect();
            if distinct.len() < 3 {
                continue;
            }
            findings.push(RuleHit::new(
                "repeated-container-text",
                format!(
                    "\"{}\" rendered {}× in distinct spots inside {}",
                    slice_utf16_prefix(text, 40),
                    sigs.len(),
                    class_selector(container)
                ),
            ));
        }
    }
    findings
}

// ─── Cream palette ──────────────────────────────────────────────────────────

/// JS: checks.mjs#checkCreamPalette(doc, win)
pub fn check_cream_palette(doc: &StaticDocument) -> Vec<RuleHit> {
    let mut findings = Vec::new();
    let Some(body) = doc.body() else {
        return findings;
    };
    let html = doc.document_element();
    let mut bg = read_own_background_color(&body, body.style());
    if bg.is_none() || bg.is_some_and(|c| c.a == Some(0.0)) {
        if let Some(h) = html.as_ref() {
            bg = read_own_background_color(h, h.style());
        }
    }
    if is_cream_color(bg.as_ref()) {
        let c = bg.unwrap();
        findings.push(RuleHit::new(
            "cream-palette",
            format!(
                "cream/beige page background rgb({}, {}, {})",
                number_to_string(c.r),
                number_to_string(c.g),
                number_to_string(c.b)
            ),
        ));
        return findings;
    }
    for el in [Some(body), html] {
        let cls = el.and_then(|e| e.get_attribute("class")).unwrap_or("");
        if let Some(tok) = cream_from_class_list(Some(cls)) {
            findings.push(RuleHit::new(
                "cream-palette",
                format!("cream/beige page background (Tailwind {})", tok),
            ));
            break;
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::check_element_clipped_overflow;
    use crate::cascade::{build_static_style_map, collect_static_css_text};
    use std::path::Path;

    fn styled_document(source: &str) -> StaticDocument {
        let mut doc = StaticDocument::parse(source);
        let css = collect_static_css_text(&doc, Path::new("."), None);
        build_static_style_map(&mut doc, &css);
        doc
    }

    #[test]
    fn overused_font_requires_a_uniquely_dominant_face_on_a_real_page() {
        let mut content = String::new();
        for i in 0..19 {
            content.push_str(&format!("<p class='georgia'>Georgia copy {i}</p>"));
        }
        content.push_str("<p class='inter'>One Inter exception</p>");
        let doc = styled_document(&format!(
            "<html><head><style>.georgia {{ font-family: Georgia, serif; }} .inter {{ font-family: Inter, sans-serif; }}</style></head><body>{content}</body></html>"
        ));

        let findings = check_static_page_typography(&doc);
        assert!(
            findings.iter().all(|hit| hit.id != "overused-font"),
            "a secondary Inter face must not be reported as the page's primary font: {findings:?}"
        );
    }

    #[test]
    fn overused_font_matches_unique_majority_detail_without_a_majority_threshold() {
        let mut content = String::new();
        for i in 0..40 {
            content.push_str(&format!("<p class='inter'>Inter copy {i}</p>"));
        }
        for i in 0..35 {
            content.push_str(&format!("<p class='georgia'>Georgia copy {i}</p>"));
        }
        for i in 0..25 {
            content.push_str(&format!("<p class='lato'>Lato copy {i}</p>"));
        }
        let doc = styled_document(&format!(
            "<html><head><style>.inter {{ font-family: Inter, sans-serif; }} .georgia {{ font-family: Georgia, serif; }} .lato {{ font-family: Lato, sans-serif; }}</style></head><body>{content}</body></html>"
        ));

        let findings = check_static_page_typography(&doc);
        let font = findings
            .iter()
            .find(|hit| hit.id == "overused-font")
            .unwrap();
        assert_eq!(font.snippet, "Primary font: inter (40% of text)");
    }

    #[test]
    fn overused_font_ignores_tied_leaders() {
        let content = (0..20)
            .map(|i| format!("<p class='inter'>Inter copy {i}</p>"))
            .chain((0..20).map(|i| format!("<p class='georgia'>Georgia copy {i}</p>")))
            .collect::<String>();
        let doc = styled_document(&format!(
            "<html><head><style>.inter {{ font-family: Inter, sans-serif; }} .georgia {{ font-family: Georgia, serif; }}</style></head><body>{content}</body></html>"
        ));

        let findings = check_static_page_typography(&doc);
        assert!(
            findings.iter().all(|hit| hit.id != "overused-font"),
            "{findings:?}"
        );
    }

    #[test]
    fn overused_font_counts_text_under_former_overlay_nodes() {
        let divs = (0..19)
            .map(|i| format!("<div class='inter'>Inter div {i}</div>"))
            .collect::<String>();
        let own_tool = (0..20)
            .map(|i| format!("<p class='inter'>Overlay text {i}</p>"))
            .collect::<String>();
        let content = format!(
            "<p class='georgia'>One regular paragraph</p>{divs}<div class='impeccino-overlay'>{own_tool}</div>"
        );
        let doc = styled_document(&format!(
            "<html><head><style>.inter {{ font-family: Inter, sans-serif; }} .georgia {{ font-family: Georgia, serif; }}</style></head><body>{content}</body></html>"
        ));

        let findings = check_static_page_typography(&doc);
        let font = findings
            .iter()
            .find(|hit| hit.id == "overused-font")
            .unwrap();
        assert_eq!(font.snippet, "Primary font: inter (95% of text)");
    }

    #[test]
    fn overused_font_keeps_platform_faces_ahead_of_overused_fallbacks() {
        let stacks = [
            (
                "font: 16px/1.5 -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, \"Helvetica Neue\", Arial, sans-serif;",
                "-apple-system",
            ),
            (
                "font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, \"Helvetica Neue\", Arial, sans-serif;",
                "-apple-system",
            ),
            ("font-family: system-ui, sans-serif;", "system-ui"),
            ("font-family: \"Segoe UI\", Roboto, sans-serif;", "segoe ui"),
            ("font-family: ui-sans-serif, Roboto, sans-serif;", "ui-sans-serif"),
        ];

        for (declaration, expected_primary) in stacks {
            let content = (0..20)
                .map(|i| format!("<p class='stack'>Platform copy {i}</p>"))
                .collect::<String>();
            let doc = styled_document(&format!(
                "<html><head><style>.stack {{ {declaration} }}</style></head><body>{content}</body></html>"
            ));
            let first = doc.query_selector("p").unwrap();
            assert!(
                font_token(sv(first.style(), "fontFamily")).starts_with(expected_primary),
                "{declaration}: {}",
                sv(first.style(), "fontFamily")
            );
            let findings = check_static_page_typography(&doc);
            assert!(
                findings.iter().all(|hit| hit.id != "overused-font"),
                "{declaration}: {findings:?}"
            );
        }
    }

    #[test]
    fn nested_cards_uses_background_from_the_computed_cascade() {
        let doc = styled_document(
            "<html><head><style>.surface { box-shadow: 0 2px 8px #000; background-color: white; }</style></head><body><div class='surface'>Outer card content that is long enough<div class='surface'>Inner card content that is long enough</div></div></body></html>",
        );

        let findings = check_page_layout(&doc);
        assert_eq!(
            findings
                .iter()
                .filter(|hit| hit.id == "nested-cards")
                .count(),
            1,
            "{findings:?}"
        );
    }

    #[test]
    fn nested_cards_uses_position_from_the_computed_cascade() {
        for position in ["absolute", "fixed"] {
            let doc = styled_document(&format!(
                "<html><head><style>.card {{ box-shadow: 0 2px 8px #000; border-radius: 8px; }} .lifted {{ position: {position}; }}</style></head><body><div class='card'>Outer card content that is long enough<div class='card lifted'>Inner card content that is long enough</div></div></body></html>"
            ));

            let findings = check_page_layout(&doc);
            assert!(
                findings.iter().all(|hit| hit.id != "nested-cards"),
                "{position}: {findings:?}"
            );
        }
    }

    #[test]
    fn nested_cards_ignores_transparent_computed_backgrounds() {
        for background in ["transparent", "rgba(255, 255, 255, 0)"] {
            let doc = styled_document(&format!(
                "<html><head><style>.surface {{ box-shadow: 0 2px 8px #000; }}</style></head><body><div class='surface' style='background-color: {background}'>Outer card content that is long enough<div class='surface' style='background-color: {background}'>Inner card content that is long enough</div></div></body></html>"
            ));

            let findings = check_page_layout(&doc);
            assert!(
                findings.iter().all(|hit| hit.id != "nested-cards"),
                "{background}: {findings:?}"
            );
        }
    }

    #[test]
    fn clipped_overflow_uses_ascii_viewport_identifier_boundaries() {
        for ident in ["écarousel", "édemo-area"] {
            let doc = styled_document(&format!(
                "<html><head></head><body><div class='{ident}' style='overflow: hidden'><div style='position: absolute; top: 100%'>A slide outside the viewport</div></div></body></html>"
            ));
            let viewport = doc.query_selector(&format!(".{ident}")).unwrap();

            assert!(
                check_element_clipped_overflow(&viewport, viewport.style()).is_empty(),
                "{ident}"
            );
        }
    }
}
