//! `checkQuality` and its static-DOM helpers from `checks.mjs` Section 5
//! (`resolveFontSizePx`, `hasVisibleBackgroundBoundary`, `isVisuallyHidden`,
//! `isNonRenderedText`, `checkElementQuality`, `checkPageQualityFromDoc`).
//! Only the branches reachable with `rect: null` (the static adapter) are
//! ported; the browser-only rules (line-length, the rect-gated
//! cramped-padding, body-text-viewport-edge) never fire here.

use crate::background::{sv, sv_opt};
use crate::cascade::StyleValues;
use crate::dom::{ChildNode, StaticElement};
use impeccino_core::checks::measures::{
    colors_nearly_match, css_color_is_transparent, resolve_length_px,
};
use impeccino_core::checks::quality::{
    check_buried_raster, check_skipped_headings, check_text_quality,
    is_non_rendered_text_from_style, is_visually_hidden_from_style, TextQualityInput,
    EXEMPT_CONTEXT, FURNITURE_CONTEXT, INTERACTIVE_CONTEXT, SMALLPRINT_CONTEXT,
    TINY_TEXT_UI_CONTEXT,
};
use impeccino_core::checks::rules::RuleHit;
use impeccino_core::checks::text_rules::SR_ONLY_SELECTOR;
use impeccino_core::js::{self, parse_float};
use impeccino_core::js_ext_a::num_truthy;
use impeccino_core::js_ext_b::utf16_len;
use once_cell::sync::Lazy;
use regex::Regex;

static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&format!("{}+", js::WS)).expect("WS_RE"));
/// JS `s.replace(/\s+/g, ' ')`.
pub fn collapse_ws(s: &str) -> String {
    impeccino_core::js_ext_b::collapse_whitespace(s)
}

/// JS `parseFloat(x) || 0`.
pub fn pf0(s: &str) -> f64 {
    let n = parse_float(s);
    if num_truthy(n) {
        n
    } else {
        0.0
    }
}

/// JS: checks.mjs#resolveFontSizePx(el, win)
pub fn resolve_font_size_px(el: &StaticElement<'_>) -> f64 {
    let mut chain: Vec<String> = Vec::new();
    let mut cur = Some(*el);
    while let Some(e) = cur {
        chain.push(sv(e.style(), "fontSize").to_string());
        cur = e.parent_element();
    }
    let mut px = 16.0;
    for v in chain.iter().rev() {
        if v.is_empty() || v == "inherit" {
            continue;
        }
        let num = parse_float(v);
        if num.is_nan() {
            continue;
        }
        if v.ends_with("px") {
            px = num;
        } else if v.ends_with("rem") {
            px = num * 16.0;
        } else if v.ends_with("em") {
            px *= num;
        } else if v.ends_with('%') {
            px *= num / 100.0;
        } else {
            px = num;
        }
    }
    px
}

/// JS: checks.mjs#hasVisibleBackgroundBoundary(style, el, win)
pub fn has_visible_background_boundary(style: &StyleValues, el: &StaticElement<'_>) -> bool {
    let bg = sv(style, "backgroundColor");
    if css_color_is_transparent(Some(bg)) {
        return false;
    }
    let mut parent = el.parent_element();
    while let Some(p) = parent {
        let parent_bg = sv(p.style(), "backgroundColor");
        if !css_color_is_transparent(Some(parent_bg)) {
            return !colors_nearly_match(Some(bg), Some(parent_bg));
        }
        parent = p.parent_element();
    }
    true
}

/// JS: checks.mjs#isVisuallyHidden(el, style)
pub fn is_visually_hidden(el: &StaticElement<'_>, style: &StyleValues) -> bool {
    let clip_path = {
        let value = sv(style, "clipPath");
        if !value.is_empty() {
            value
        } else {
            let webkit = sv(style, "webkitClipPath");
            if !webkit.is_empty() {
                webkit
            } else {
                sv(style, "clip-path")
            }
        }
    };
    is_visually_hidden_from_style(
        el.closest(SR_ONLY_SELECTOR).is_some(),
        sv(style, "position"),
        sv(style, "clip"),
        clip_path,
        sv(style, "width"),
        sv(style, "height"),
        sv(style, "overflow"),
    )
}

/// JS: checks.mjs#isNonRenderedText(el, tag, style)
pub fn is_non_rendered_text(
    el: &StaticElement<'_>,
    tag: &str,
    style: Option<&StyleValues>,
) -> bool {
    let in_head = el.closest("head").is_some();
    let display = style.map(|value| sv(value, "display")).unwrap_or("");
    let visibility = style.map(|value| sv(value, "visibility")).unwrap_or("");
    is_non_rendered_text_from_style(tag, in_head, display, visibility)
}

/// Inputs of checkQuality as the static adapter builds them.
pub struct QualityInput<'a, 'b> {
    pub el: &'b StaticElement<'a>,
    pub tag: &'b str,
    pub style: &'a StyleValues,
    pub has_direct_text: bool,
    pub text_len: usize,
    pub font_size: f64,
    pub line_height_px: Option<f64>,
    pub letter_spacing_px: Option<f64>,
}

const FLUSH_SKIP_TAGS: &[&str] = &[
    "HTML", "BODY", "MAIN", "HEADER", "FOOTER", "NAV", "ARTICLE", "ASIDE", "BUTTON", "A", "LABEL",
    "SUMMARY", "CODE", "PRE", "INPUT", "TEXTAREA", "SELECT", "FORM", "FIGURE", "TABLE", "TBODY",
    "THEAD", "TR", "TD", "TH",
];

fn side_len(style: &StyleValues, key: &str, font_size: f64) -> f64 {
    resolve_length_px(sv_opt(style, key), font_size).unwrap_or(0.0)
}

/// JS: checks.mjs#checkQuality(opts), static (`rect: null`) branches.
pub fn check_quality(q: &QualityInput<'_, '_>) -> Vec<RuleHit> {
    let el = q.el;
    let tag = q.tag;
    let style = q.style;
    let font_size = q.font_size;
    let text_len = q.text_len;
    let mut findings: Vec<RuleHit> = Vec::new();

    let opacity = parse_float(sv(style, "opacity"));
    let background_image = sv(style, "backgroundImage");
    let alt = el.get_attribute("alt").unwrap_or("");
    let text_content = el.text_content();
    findings.extend(check_buried_raster(
        tag,
        opacity,
        background_image,
        alt,
        &text_content,
    ));

    // --- Line length / cramped padding (rect-gated): never fire statically.

    // --- Flush against a visible boundary ---
    {
        let upper_tag = js::to_upper_case(tag);
        let el_position = sv(style, "position");
        let children = el.children();
        if !FLUSH_SKIP_TAGS.contains(&upper_tag.as_str())
            && !q.has_direct_text
            && el_position != "fixed"
            && el_position != "absolute"
            && !children.is_empty()
        {
            let bw = |k: &str| pf0(sv(style, k));
            let border_w = [
                bw("borderTopWidth"),
                bw("borderRightWidth"),
                bw("borderBottomWidth"),
                bw("borderLeftWidth"),
            ];
            let bc = |k: &str| css_color_is_transparent(Some(sv(style, k)));
            let border_visible = [
                border_w[0] > 0.0 && !bc("borderTopColor"),
                border_w[1] > 0.0 && !bc("borderRightColor"),
                border_w[2] > 0.0 && !bc("borderBottomColor"),
                border_w[3] > 0.0 && !bc("borderLeftColor"),
            ];
            let outline_w = pf0(sv(style, "outlineWidth"));
            let outline_style_val = sv(style, "outlineStyle");
            let outline_color_val = sv(style, "outlineColor");
            // `style.outline` is never set on a static style: the shorthand
            // fallback branch is unreachable here.
            let outline_visible = outline_w > 0.0
                && !css_color_is_transparent(Some(outline_color_val))
                && !outline_style_val.is_empty()
                && outline_style_val != "none";
            let bg_visible = has_visible_background_boundary(style, el);
            let any_visible = border_visible.iter().any(|b| *b) || outline_visible || bg_visible;
            if any_visible {
                let pad = [
                    side_len(style, "paddingTop", font_size),
                    side_len(style, "paddingRight", font_size),
                    side_len(style, "paddingBottom", font_size),
                    side_len(style, "paddingLeft", font_size),
                ];
                const PAD_THRESHOLD: f64 = 2.0;
                const CHILD_INSULATE_THRESHOLD: f64 = 4.0;
                let mut children_insulate = [false; 4];
                for child in &children {
                    let cs = child.style();
                    let child_pad = [
                        side_len(cs, "paddingTop", font_size),
                        side_len(cs, "paddingRight", font_size),
                        side_len(cs, "paddingBottom", font_size),
                        side_len(cs, "paddingLeft", font_size),
                    ];
                    let child_margin = [
                        side_len(cs, "marginTop", font_size),
                        side_len(cs, "marginRight", font_size),
                        side_len(cs, "marginBottom", font_size),
                        side_len(cs, "marginLeft", font_size),
                    ];
                    for s in 0..4 {
                        if child_pad[s] >= CHILD_INSULATE_THRESHOLD
                            || child_margin[s] >= CHILD_INSULATE_THRESHOLD
                        {
                            children_insulate[s] = true;
                        }
                    }
                }
                let side_names = ["top", "right", "bottom", "left"];
                let mut flush_sides: Vec<&str> = Vec::new();
                for s in 0..4 {
                    let bg_bounds_side = bg_visible;
                    let side_bounded = border_visible[s] || outline_visible || bg_bounds_side;
                    if side_bounded && pad[s] <= PAD_THRESHOLD && !children_insulate[s] {
                        flush_sides.push(side_names[s]);
                    }
                }
                if !flush_sides.is_empty() {
                    let has_text_child = children
                        .iter()
                        .any(|c| utf16_len(js::trim(&c.text_content())) > 4);
                    if has_text_child {
                        let cls_all = js::trim(el.class_name());
                        let cls = if cls_all.is_empty() {
                            ""
                        } else {
                            WS_RE.split(cls_all).next().unwrap_or("")
                        };
                        let mut boundary_parts: Vec<String> = Vec::new();
                        let border_sides_visible: Vec<&str> = (0..4)
                            .filter(|i| border_visible[*i])
                            .map(|i| side_names[i])
                            .collect();
                        if border_sides_visible.len() == 4 {
                            boundary_parts.push("border".to_string());
                        } else if !border_sides_visible.is_empty() {
                            boundary_parts
                                .push(format!("border-{}", border_sides_visible.join("/")));
                        }
                        if outline_visible {
                            boundary_parts.push("outline".to_string());
                        }
                        if bg_visible {
                            boundary_parts.push("bg".to_string());
                        }
                        let sides_label = if flush_sides.len() == 4 {
                            "all sides".to_string()
                        } else {
                            flush_sides.join("/")
                        };
                        let tag_lower = js::to_lower_case(tag);
                        let ident = if !cls.is_empty() {
                            format!("<{}> \"{}\"", tag_lower, cls)
                        } else {
                            format!("<{}>", tag_lower)
                        };
                        findings.push(RuleHit::new(
                            "cramped-padding",
                            format!(
                                "{}: children flush against {} on {} (no inset)",
                                ident,
                                boundary_parts.join("+"),
                                sides_label
                            ),
                        ));
                    }
                }
            }
        }
    }

    let direct_text = el.direct_text();
    let text_content = el.text_content();
    findings.extend(check_text_quality(&TextQualityInput {
        tag,
        has_direct_text: q.has_direct_text,
        text_len,
        font_size,
        line_height_px: q.line_height_px,
        letter_spacing_px: q.letter_spacing_px,
        direct_text: &direct_text,
        text_content: &text_content,
        text_align: sv(style, "textAlign"),
        hyphens: sv(style, "hyphens"),
        webkit_hyphens: sv(style, "webkitHyphens"),
        text_transform: sv(style, "textTransform"),
        in_tiny_text_ui_context: el.closest(TINY_TEXT_UI_CONTEXT).is_some(),
        is_non_rendered_text: is_non_rendered_text(el, tag, Some(style)),
        in_exempt_context: el.closest(EXEMPT_CONTEXT).is_some(),
        is_visually_hidden: is_visually_hidden(el, style),
        is_interactive: el.closest(INTERACTIVE_CONTEXT).is_some(),
        is_furniture: el.closest(FURNITURE_CONTEXT).is_some(),
        is_smallprint: el.closest(SMALLPRINT_CONTEXT).is_some(),
    }));
    findings
}

/// JS: checks.mjs#checkElementQuality(el, style, tag, window)
pub fn check_element_quality(
    el: &StaticElement<'_>,
    style: &StyleValues,
    tag: &str,
) -> Vec<RuleHit> {
    let has_direct_text = el.has_direct_text_longer_than(10);
    let text_len = utf16_len(js::trim(&el.text_content()));
    let font_size = resolve_font_size_px(el);
    let line_height_px = resolve_length_px(sv_opt(style, "lineHeight"), font_size);
    let letter_spacing_px = resolve_length_px(sv_opt(style, "letterSpacing"), font_size);
    check_quality(&QualityInput {
        el,
        tag,
        style,
        has_direct_text,
        text_len,
        font_size,
        line_height_px,
        letter_spacing_px,
    })
}

/// JS: checks.mjs#checkPageQualityFromDoc(doc)
pub fn check_page_quality_from_doc(doc: &crate::dom::StaticDocument) -> Vec<RuleHit> {
    let headings: Vec<(String, String)> = doc
        .query_selector_all("h1, h2, h3, h4, h5, h6")
        .into_iter()
        .map(|heading| (heading.tag_upper(), heading.text_content()))
        .collect();
    check_skipped_headings(&headings)
}

pub fn has_nonblank_direct_text(el: &StaticElement<'_>) -> bool {
    el.child_nodes()
        .iter()
        .any(|c| matches!(c, ChildNode::Text(t) if !js::trim(t).is_empty()))
}
