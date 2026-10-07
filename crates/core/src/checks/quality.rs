//! Shared layout-independent quality checks for static and rendered DOM adapters.

use crate::checks::rules::RuleHit;
use crate::checks::text_rules::NON_RENDERED_TAGS;
use crate::js::{self, number_to_string, parse_float, to_fixed};
use crate::js_ext_b::{collapse_whitespace, slice_utf16_prefix, utf16_len};
use regex::Regex;
use std::sync::LazyLock as Lazy;

static RASTER_URL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"{}\(", js::ci("url"))).expect("RASTER_URL_RE"));
static CLIP_RECT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"rect\({}*0", js::WS)).expect("CLIP_RECT_RE"));
static CLIP_INSET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"inset\({}*(?:50%|99|100%)", js::WS)).expect("CLIP_INSET_RE")
});

pub const TINY_TEXT_UI_CONTEXT: &str = "button, a, label, summary, pre, [role=\"button\"], [role=\"link\"], [role=\"tab\"], [role=\"menuitem\"], [role=\"option\"], nav, footer, [aria-hidden=\"true\"], [class*=\"badge\" i], [class*=\"caption\" i], [class*=\"chip\" i], [class*=\"code\" i], [class*=\"console\" i], [class*=\"diff\" i], [class*=\"label\" i], [class*=\"meta\" i], [class*=\"mock\" i], [class*=\"pill\" i], [class*=\"preview\" i], [class*=\"tag\" i], [class*=\"terminal\" i], [class*=\"writes\" i]";
pub const EXEMPT_CONTEXT: &str = "pre, code, kbd, samp, var, svg, [aria-hidden=\"true\"], [class*=\"terminal\" i], [class*=\"console\" i], [class*=\"code\" i], [class*=\"mock\" i], [class*=\"editor\" i], [class*=\"syntax\" i], [class*=\"diff\" i]";
pub const INTERACTIVE_CONTEXT: &str = "a[href], button, summary, label, select, textarea, [role=\"button\"], [role=\"link\"], [role=\"tab\"], [role=\"menuitem\"], [role=\"menuitemcheckbox\"], [role=\"menuitemradio\"], [role=\"option\"], [role=\"checkbox\"], [role=\"radio\"], [role=\"switch\"], [role=\"treeitem\"], [tabindex]";
pub const FURNITURE_CONTEXT: &str = "nav, [role=\"navigation\"], td, th, [role=\"gridcell\"], [role=\"cell\"], caption, figcaption, dt, dd, footer, [class*=\"meta\" i], [class*=\"label\" i], [class*=\"badge\" i], [class*=\"chip\" i], [class*=\"pill\" i], [class*=\"tag\" i], [class*=\"kicker\" i], [class*=\"eyebrow\" i], [class*=\"breadcrumb\" i], [class*=\"timestamp\" i], [class*=\"category\" i], [class*=\"caption\" i], [class*=\"nav\" i]";
pub const SMALLPRINT_CONTEXT: &str = "small, footer, [class*=\"legal\" i], [class*=\"copyright\" i], [class*=\"fineprint\" i], [class*=\"fine-print\" i], [class*=\"smallprint\" i], [class*=\"small-print\" i], [class*=\"disclaimer\" i], [class*=\"disclosure\" i], [class*=\"footnote\" i]";

/// Plain computed/text facts for quality checks that do not need page geometry.
#[derive(Debug, Clone, Copy)]
pub struct TextQualityInput<'a> {
    pub tag: &'a str,
    pub has_direct_text: bool,
    pub text_len: usize,
    pub font_size: f64,
    pub line_height_px: Option<f64>,
    pub letter_spacing_px: Option<f64>,
    pub direct_text: &'a str,
    pub text_content: &'a str,
    pub text_align: &'a str,
    pub hyphens: &'a str,
    pub webkit_hyphens: &'a str,
    pub text_transform: &'a str,
    pub in_tiny_text_ui_context: bool,
    pub is_non_rendered_text: bool,
    pub in_exempt_context: bool,
    pub is_visually_hidden: bool,
    pub is_interactive: bool,
    pub is_furniture: bool,
    pub is_smallprint: bool,
}

/// The shared branches of `checkQuality` which need text and styles, not rects.
pub fn check_text_quality(q: &TextQualityInput<'_>) -> Vec<RuleHit> {
    let mut findings = Vec::new();
    let is_heading = matches!(q.tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6");

    if q.has_direct_text && q.text_len > 50 && !is_heading {
        if let Some(line_height) = q.line_height_px {
            if q.font_size > 0.0 {
                let ratio = line_height / q.font_size;
                if ratio > 0.0 && ratio < 1.3 {
                    findings.push(RuleHit::new(
                        "tight-leading",
                        format!("line-height {}x (need >=1.3)", to_fixed(ratio, 2)),
                    ));
                }
            }
        }
    }

    if q.has_direct_text && q.text_align == "justify" {
        let hyphens = if !q.hyphens.is_empty() {
            q.hyphens
        } else {
            q.webkit_hyphens
        };
        if hyphens != "auto" {
            findings.push(RuleHit::new(
                "justified-text",
                "text-align: justify without hyphens: auto".to_string(),
            ));
        }
    }

    if q.has_direct_text && q.text_len > 20 && q.font_size < 12.0 {
        const SKIP_TAGS: &[&str] = &[
            "sub",
            "sup",
            "code",
            "kbd",
            "samp",
            "var",
            "caption",
            "figcaption",
        ];
        let is_uppercase = q.text_transform == "uppercase";
        if !SKIP_TAGS.contains(&q.tag)
            && !q.in_tiny_text_ui_context
            && !is_uppercase
            && !q.is_non_rendered_text
        {
            findings.push(RuleHit::new(
                "tiny-text",
                format!("{}px body text", number_to_string(q.font_size)),
            ));
        }
    }

    let direct_text = js::trim(&collapse_whitespace(q.direct_text)).to_string();
    let direct_text_len = utf16_len(&direct_text);
    const UI_SKIP_TAGS: &[&str] = &["sub", "sup", "option"];
    if q.font_size > 0.0
        && q.font_size < 11.0
        && direct_text_len >= 2
        && !UI_SKIP_TAGS.contains(&q.tag)
        && !q.is_non_rendered_text
        && !q.in_exempt_context
        && !q.is_visually_hidden
    {
        let floor = if !q.is_interactive && q.is_smallprint {
            10.0
        } else {
            11.0
        };
        if q.font_size < floor && (q.is_interactive || q.is_furniture || direct_text_len <= 20) {
            let excerpt = slice_utf16_prefix(&direct_text, 40);
            findings.push(RuleHit::new(
                "undersized-ui-text",
                format!(
                    "{}px functional text \"{}\" (below {}px floor)",
                    number_to_string(q.font_size),
                    excerpt,
                    number_to_string(floor)
                ),
            ));
        }
    }

    if q.has_direct_text && q.text_len > 30 && q.text_transform == "uppercase" && !is_heading {
        findings.push(RuleHit::new(
            "all-caps-body",
            format!(
                "text-transform: uppercase on {} chars of body text",
                q.text_len
            ),
        ));
    }

    if q.has_direct_text && q.text_len > 20 && q.text_transform != "uppercase" {
        if let Some(letter_spacing) = q.letter_spacing_px {
            if letter_spacing > 0.0 && q.font_size > 0.0 {
                let tracking_em = letter_spacing / q.font_size;
                if tracking_em > 0.05 {
                    findings.push(RuleHit::new(
                        "wide-tracking",
                        format!(
                            "letter-spacing: {}em on body text",
                            to_fixed(tracking_em, 2)
                        ),
                    ));
                }
            }
        }
    }

    if q.has_direct_text && q.text_len > 20 && q.font_size > 0.0 {
        if let Some(letter_spacing) = q.letter_spacing_px {
            if letter_spacing < 0.0 {
                let tracking_em = letter_spacing / q.font_size;
                if tracking_em <= -0.05 {
                    let excerpt =
                        slice_utf16_prefix(js::trim(&collapse_whitespace(q.text_content)), 40);
                    findings.push(RuleHit::new(
                        "extreme-negative-tracking",
                        format!(
                            "letter-spacing: {}em — \"{}\"",
                            to_fixed(tracking_em, 2),
                            excerpt
                        ),
                    ));
                }
            }
        }
    }

    findings
}

/// Shared per-element quality finding for a raster hidden by near-zero opacity.
pub fn check_buried_raster(
    tag: &str,
    opacity: f64,
    background_image: &str,
    alt: &str,
    text_content: &str,
) -> Vec<RuleHit> {
    if !(0.0..0.15).contains(&opacity)
        || (tag != "img" && !RASTER_URL_RE.is_match(background_image))
    {
        return Vec::new();
    }
    let label = if tag == "img" {
        alt.to_string()
    } else {
        slice_utf16_prefix(js::trim(text_content), 40)
    };
    vec![RuleHit::new(
        "buried-raster",
        format!(
            "{} at opacity {}{}",
            if tag == "img" {
                "<img>"
            } else {
                "raster background"
            },
            number_to_string(opacity),
            if label.is_empty() {
                String::new()
            } else {
                format!(" \"{label}\"")
            }
        ),
    )]
}

/// Shared CSS and element facts for the screen-reader-only text check.
pub fn is_visually_hidden_from_style(
    matches_screen_reader_only_selector: bool,
    position: &str,
    clip: &str,
    clip_path: &str,
    width: &str,
    height: &str,
    overflow: &str,
) -> bool {
    if matches_screen_reader_only_selector {
        return true;
    }
    if position != "absolute" && position != "fixed" {
        return false;
    }
    if CLIP_RECT_RE.is_match(clip) || CLIP_INSET_RE.is_match(clip_path) {
        return true;
    }
    let width_px = parse_float(width);
    let height_px = parse_float(height);
    (width_px == 1.0 || height_px == 1.0) && (overflow == "hidden" || overflow == "clip")
}

/// Shared tag, ancestry, and style test for text omitted from rendered output.
pub fn is_non_rendered_text_from_style(
    tag: &str,
    in_head: bool,
    display: &str,
    visibility: &str,
) -> bool {
    let tag = js::to_lower_case(tag);
    NON_RENDERED_TAGS.contains(&tag.as_str())
        || in_head
        || display == "none"
        || visibility == "hidden"
        || visibility == "collapse"
}

/// The static and rendered page-quality wrappers feed the same ordered headings.
pub fn check_skipped_headings(headings: &[(String, String)]) -> Vec<RuleHit> {
    let mut findings = Vec::new();
    let mut previous_level: i64 = 0;
    let mut previous_text = String::new();
    for (tag, source_text) in headings {
        let level = tag
            .as_bytes()
            .get(1)
            .and_then(|byte| (*byte as char).to_digit(10))
            .unwrap_or(0) as i64;
        let text = slice_utf16_prefix(js::trim(&collapse_whitespace(source_text)), 60);
        if previous_level > 0 && level > previous_level + 1 {
            findings.push(RuleHit::new(
                "skipped-heading",
                format!(
                    "<h{}> \"{}\" followed by <h{}> \"{}\" (missing h{})",
                    previous_level,
                    previous_text,
                    level,
                    text,
                    previous_level + 1
                ),
            ));
        }
        previous_level = level;
        previous_text = text;
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_quality_inputs_share_layout_independent_findings() {
        let base = TextQualityInput {
            tag: "p",
            has_direct_text: true,
            text_len: 60,
            font_size: 16.0,
            line_height_px: Some(18.0),
            letter_spacing_px: None,
            direct_text: "A sentence long enough to expose text warnings.",
            text_content: "A sentence long enough to expose text warnings.",
            text_align: "justify",
            hyphens: "manual",
            webkit_hyphens: "",
            text_transform: "none",
            in_tiny_text_ui_context: false,
            is_non_rendered_text: false,
            in_exempt_context: false,
            is_visually_hidden: false,
            is_interactive: false,
            is_furniture: false,
            is_smallprint: false,
        };
        let hits = check_text_quality(&base);
        let ids: Vec<_> = hits.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(ids, vec!["tight-leading", "justified-text"]);

        let tiny = TextQualityInput {
            text_len: 30,
            font_size: 10.0,
            line_height_px: None,
            text_align: "start",
            hyphens: "",
            ..base
        };
        assert_eq!(
            check_text_quality(&tiny)
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["tiny-text"]
        );
    }

    #[test]
    fn skipped_heading_reports_one_level_gap_with_shared_utf16_snippets() {
        let hits = check_skipped_headings(&[
            ("H1".to_string(), "Overview".to_string()),
            ("H3".to_string(), "Details".to_string()),
        ]);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].snippet,
            "<h1> \"Overview\" followed by <h3> \"Details\" (missing h2)"
        );
    }
}
