//! Pure CSS value helpers shared by the static and rendered design-system DOM
//! checks. Source scanning has extra syntax guards; computed-style consumers
//! use these same value semantics after they have read a property.

use crate::color::{parse_any_color, Rgba};
use crate::constants::GENERIC_FONTS;
use crate::css::measures::resolve_length_px;
use crate::js::{self, math_max3, math_round, parse_float};
use crate::js_ext_b::slice_utf16_prefix;

use once_cell::sync::Lazy;
use regex::Regex;

const DESIGN_COLOR_TOLERANCE: f64 = 6.0;
const DESIGN_RADIUS_TOLERANCE_PX: f64 = 0.5;

#[derive(Debug, Default)]
pub struct DesignSystemSeen {
    pub fonts: Vec<String>,
    pub colors: Vec<String>,
    pub radii: Vec<String>,
}

pub struct DesignSystemTokens<'a> {
    pub has_fonts: bool,
    pub allowed_fonts: &'a [String],
    pub has_colors: bool,
    pub allowed_colors: &'a [Rgba],
    pub has_radii: bool,
    pub allowed_radii_px: &'a [f64],
    pub has_pill_radius: bool,
}

pub struct ComputedElementStyle {
    pub tag: String,
    /// Already collapsed and trimmed, with the optional surrounding spaces
    /// and quotes that appear in the final finding text.
    pub sample_text: String,
    pub has_direct_text: bool,
    pub font_family: String,
    pub color: String,
    pub background_color: String,
    /// top, right, bottom, left
    pub border_widths: [f64; 4],
    /// top, right, bottom, left
    pub border_colors: [String; 4],
    pub outline_width: f64,
    pub outline_color: String,
    pub border_radius: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignSystemFinding {
    pub type_: String,
    pub detail: String,
    pub ignore_value: String,
}

static IMPORTANT_TAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!("{}*{}{}*$", js::WS, js::ci("!important"), js::WS))
        .expect("IMPORTANT_TAIL_RE")
});
static EDGE_QUOTE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^["']|["']$"#).expect("EDGE_QUOTE_RE"));
static WS_RUN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("{}+", js::WS)).expect("WS_RUN_RE"));
static VAR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("{}\\(", js::ci("var"))).expect("VAR_RE"));
static SLASH_WS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("{}*/{}*", js::WS, js::WS)).expect("SLASH_WS_RE"));
static HSL_FALLBACK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        "{hsl}[aA]?\\({ws}*([-0-9.]+)(?:{deg})?{ws}*,?{ws}*([0-9.]+)%{ws}*,?{ws}*([0-9.]+)%(?:{ws}*[,/]{ws}*([0-9.]+))?{ws}*\\)",
        hsl = js::ci("hsl"),
        deg = js::ci("deg"),
        ws = js::WS
    ))
    .expect("HSL_FALLBACK_RE")
});

/// JS normalizeFontName for a single computed family or design-system token.
pub fn normalize_font_name(value: &str) -> String {
    let t = js::trim(value);
    let t = IMPORTANT_TAIL_RE.replace(t, "");
    let t = js::trim(&t);
    let t = EDGE_QUOTE_RE.replace_all(t, "");
    let t = t.replace('+', " ");
    let t = WS_RUN_RE.replace_all(&t, " ");
    js::to_lower_case(&t)
}

/// JS splitFontStack.
pub fn split_font_stack(stack: &str) -> Vec<String> {
    let t = IMPORTANT_TAIL_RE.replace(stack, "");
    t.split(',')
        .map(normalize_font_name)
        .filter(|font| !font.is_empty())
        .collect()
}

/// Resolve the primary non-generic family from a computed font stack. The
/// source scanner separately rejects expressions that are not literal CSS.
pub fn computed_primary_font(stack: &str) -> String {
    if stack.is_empty() || VAR_RE.is_match(stack) {
        return String::new();
    }
    split_font_stack(stack)
        .into_iter()
        .find(|font| !GENERIC_FONTS.contains(&font.as_str()))
        .unwrap_or_default()
}

/// JS cssColorLabel.
pub fn css_color_label(raw: &str) -> String {
    WS_RUN_RE.replace_all(js::trim(raw), " ").into_owned()
}

/// Format the shared text sample included in computed design-system findings.
pub fn sample_text(text: &str, max_utf16_units: usize) -> String {
    let collapsed = WS_RUN_RE.replace_all(text, " ");
    let trimmed = js::trim(&collapsed);
    if trimmed.is_empty() {
        String::new()
    } else {
        format!(" \"{}\"", slice_utf16_prefix(trimmed, max_utf16_units))
    }
}

/// JS design-system color conversion, including its legacy HSL fallback.
pub fn parse_design_color(value: &str) -> Option<Rgba> {
    let text = js::trim(value);
    if let Some(parsed) = parse_any_color(Some(text)) {
        return Some(parsed);
    }
    let m = HSL_FALLBACK_RE.captures(text)?;
    Some(hsl_to_rgb(
        parse_float(&m[1]),
        parse_float(&m[2]) / 100.0,
        parse_float(&m[3]) / 100.0,
        m.get(4)
            .map(|alpha| parse_float(alpha.as_str()))
            .unwrap_or(1.0),
    ))
}

fn hsl_to_rgb(hh: f64, ss: f64, ll: f64, alpha: f64) -> Rgba {
    let h = (((hh % 360.0) + 360.0) % 360.0) / 360.0;
    let s = js::math_max(0.0, js::math_min(1.0, ss));
    let l = js::math_max(0.0, js::math_min(1.0, ll));
    let hue2rgb = |p: f64, q: f64, mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    Rgba::new(
        math_round(hue2rgb(p, q, h + 1.0 / 3.0) * 255.0),
        math_round(hue2rgb(p, q, h) * 255.0),
        math_round(hue2rgb(p, q, h - 1.0 / 3.0) * 255.0),
        alpha,
    )
}

/// Whether a color parses to an effectively transparent value.
pub fn is_transparent_css(value: &str) -> bool {
    let text = js::to_lower_case(js::trim(value));
    if text.is_empty() || text == "transparent" {
        return true;
    }
    match parse_design_color(&text) {
        Some(color) => color.alpha_or_one() <= 0.05,
        None => false,
    }
}

pub fn colors_close(a: &Rgba, b: &Rgba) -> bool {
    math_max3((a.r - b.r).abs(), (a.g - b.g).abs(), (a.b - b.b).abs()) <= DESIGN_COLOR_TOLERANCE
}

/// Apply the shared CSS color allowlist semantics to a set of tokens.
pub fn is_allowed_color_raw<'a>(
    raw: &str,
    has_colors: bool,
    allowed_colors: impl IntoIterator<Item = &'a Rgba>,
) -> bool {
    if !has_colors {
        return true;
    }
    let text = js::to_lower_case(js::trim(raw));
    if text.is_empty()
        || text == "transparent"
        || text == "currentcolor"
        || text == "inherit"
        || text == "initial"
        || text.contains("var(")
    {
        return true;
    }
    let Some(parsed) = parse_design_color(&text) else {
        return true;
    };
    if parsed.alpha_or_one() <= 0.05 {
        return true;
    }
    allowed_colors
        .into_iter()
        .any(|allowed| colors_close(&parsed, allowed))
}

/// JS extractRadiusTokens; the trailing close-paren trim handles var()
/// fallback values that the static source scan can observe before resolution.
pub fn radius_tokens(value: &str) -> Vec<String> {
    let replaced = SLASH_WS_RE.replace_all(value, " ");
    WS_RUN_RE
        .split(&replaced)
        .map(|token| js::trim(token).trim_end_matches(')').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

/// Apply the shared CSS radius allowlist semantics to a set of tokens.
pub fn is_allowed_radius_raw(
    raw: &str,
    has_radii: bool,
    allowed_radii_px: impl IntoIterator<Item = f64>,
    has_pill_radius: bool,
) -> bool {
    if !has_radii {
        return true;
    }
    let text = js::to_lower_case(js::trim(raw));
    if text.is_empty() || text == "0" || text == "none" || text == "initial" || text == "inherit" {
        return true;
    }
    if text.contains("var(") || text.contains('%') {
        return true;
    }
    let Some(px) = resolve_length_px(Some(&text), 16.0) else {
        return true;
    };
    if !px.is_finite() || px <= DESIGN_RADIUS_TOLERANCE_PX {
        return true;
    }
    if has_pill_radius && px >= 99.0 {
        return true;
    }
    allowed_radii_px
        .into_iter()
        .any(|allowed| (allowed - px).abs() <= DESIGN_RADIUS_TOLERANCE_PX)
}

/// The design-system checks that read computed style but do not use layout.
/// Both DOM adapters collect facts; findings and dedupe rules stay here.
pub fn check_computed_element(
    style: &ComputedElementStyle,
    tokens: &DesignSystemTokens<'_>,
    seen: &mut DesignSystemSeen,
) -> Vec<DesignSystemFinding> {
    let mut findings = Vec::new();

    if tokens.has_fonts && style.has_direct_text {
        let font = computed_primary_font(&style.font_family);
        if !font.is_empty()
            && !tokens.allowed_fonts.iter().any(|allowed| allowed == &font)
            && !seen.fonts.contains(&font)
        {
            seen.fonts.push(font.clone());
            findings.push(DesignSystemFinding {
                type_: "design-system-font".to_string(),
                detail: format!(
                    "{}{} uses {}; not declared in DESIGN.md typography",
                    style.tag, style.sample_text, font
                ),
                ignore_value: font,
            });
        }
    }

    if tokens.has_colors {
        let mut color_checks: Vec<(String, &str)> = Vec::new();
        if style.has_direct_text {
            color_checks.push(("text color".to_string(), &style.color));
        }
        if !is_transparent_css(&style.background_color) {
            color_checks.push(("background".to_string(), &style.background_color));
        }
        for (index, side) in ["Top", "Right", "Bottom", "Left"].iter().enumerate() {
            if style.border_widths[index] > 0.0 {
                color_checks.push((
                    format!("border-{}", side.to_ascii_lowercase()),
                    &style.border_colors[index],
                ));
            }
        }
        if style.outline_width > 0.0 {
            color_checks.push(("outline".to_string(), &style.outline_color));
        }

        for (kind, raw) in color_checks {
            let label = css_color_label(raw);
            if is_allowed_color_raw(&label, true, tokens.allowed_colors) {
                continue;
            }
            let key = format!("{kind}:{label}");
            if seen.colors.contains(&key) {
                continue;
            }
            seen.colors.push(key);
            findings.push(DesignSystemFinding {
                type_: "design-system-color".to_string(),
                detail: format!(
                    "{kind} {label} on {}{} is outside DESIGN.md colors",
                    style.tag, style.sample_text
                ),
                ignore_value: label,
            });
        }
    }

    if tokens.has_radii {
        for token in radius_tokens(&style.border_radius) {
            if is_allowed_radius_raw(
                &token,
                true,
                tokens.allowed_radii_px.iter().copied(),
                tokens.has_pill_radius,
            ) || seen.radii.contains(&token)
            {
                continue;
            }
            seen.radii.push(token.clone());
            findings.push(DesignSystemFinding {
                type_: "design-system-radius".to_string(),
                detail: format!(
                    "border-radius {token} on {}{} is outside the DESIGN.md rounded scale",
                    style.tag, style.sample_text
                ),
                ignore_value: token,
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computed_font_stack_accepts_css_plus_signs_inside_family_names() {
        assert_eq!(computed_primary_font("'A + B', sans-serif"), "a b");
    }

    #[test]
    fn radius_tokens_keep_the_shared_fallback_paren_semantics() {
        assert_eq!(radius_tokens("8px 3px / 2px)"), ["8px", "3px", "2px"]);
    }

    #[test]
    fn design_color_parser_handles_hsl_and_transparency() {
        assert_eq!(
            parse_design_color("hsl(120, 50%, 50%)"),
            parse_any_color(Some("hsl(120, 50%, 50%)"))
        );
        assert!(is_transparent_css("rgba(1, 2, 3, 0.01)"));
    }
}
