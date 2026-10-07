//! CSS list splitting, property normalization and computed value resolution.

use super::checks_shim::{resolve_length_px, resolve_var_refs, CustomProps};
use super::defaults::{
    static_default_style, static_named_color, static_prop_map, STATIC_NAMED_COLORS,
};
use impeccino_core::color::{parse_any_color, Rgba, CSS_NAMED_COLORS};
use impeccino_core::js;
use once_cell::sync::Lazy;
use regex::Regex;

/// A computed / partially-computed style: an ordered map of camelCase
/// property name to value string. `parentStyle` and `values` in the JS are
/// plain objects read with `style[prop]`; a missing key reads as undefined
/// (`None` here), an empty string is present but falsy.
pub type StyleValues = indexmap::IndexMap<String, String>;

/// JS `{ ...STATIC_DEFAULT_STYLE }`: a fresh style map holding every default,
/// in table order (the base `computeNode` fills before applying winners).
pub fn make_default_style() -> StyleValues {
    super::defaults::STATIC_DEFAULT_STYLE
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// JS `style?.[prop]` on an optional style.
pub fn style_get<'a>(style: Option<&'a StyleValues>, prop: &str) -> Option<&'a str> {
    style.and_then(|s| s.get(prop).map(|v| v.as_str()))
}

// ─── splitCssList / splitCssTokens ──────────────────────────────────────────

/// Split on top-level commas (outside quotes and parens/brackets), trimming
/// each part and dropping an empty tail.
pub fn split_css_list(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut parts: Vec<String> = Vec::new();
    let mut depth: i64 = 0;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    for i in 0..chars.len() {
        let ch = chars[i];
        if let Some(q) = quote {
            if ch == q && (i == 0 || chars[i - 1] != '\\') {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == '(' || ch == '[' {
            depth += 1;
        } else if ch == ')' || ch == ']' {
            depth = std::cmp::max(0, depth - 1);
        } else if ch == ',' && depth == 0 {
            let piece: String = chars[start..i].iter().collect();
            parts.push(js::trim(&piece).to_string());
            start = i + 1;
        }
    }
    let tail: String = chars[start.min(chars.len())..].iter().collect();
    let tail = js::trim(&tail);
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }
    parts
}

/// Split on top-level whitespace (outside quotes and parens).
pub fn split_css_tokens(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut depth: i64 = 0;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    for i in 0..chars.len() {
        let ch = chars[i];
        if let Some(q) = quote {
            current.push(ch);
            if ch == q && (i == 0 || chars[i - 1] != '\\') {
                quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            current.push(ch);
            continue;
        }
        if ch == '(' {
            depth += 1;
            current.push(ch);
            continue;
        }
        if ch == ')' {
            depth = std::cmp::max(0, depth - 1);
            current.push(ch);
            continue;
        }
        if js::is_js_whitespace(ch) && depth == 0 {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

// ─── cssPropToCamel ─────────────────────────────────────────────────────────

static DASH_LOWER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"-([a-z])").expect("DASH_LOWER_RE"));

pub fn css_prop_to_camel(prop: &str) -> String {
    if prop.is_empty() {
        return prop.to_string();
    }
    if let Some(mapped) = static_prop_map(prop) {
        return mapped.to_string();
    }
    DASH_LOWER_RE
        .replace_all(prop, |m: &regex::Captures| m[1].to_ascii_uppercase())
        .into_owned()
}

// ─── Colors ─────────────────────────────────────────────────────────────────

pub fn static_color_to_css(c: Option<&Rgba>) -> String {
    let Some(c) = c else {
        return String::new();
    };
    let n = js::number_to_string;
    match c.a {
        Some(a) if a < 1.0 => {
            let rounded = js::string_to_number(&js::to_fixed(a, 3));
            format!("rgba({}, {}, {}, {})", n(c.r), n(c.g), n(c.b), n(rounded))
        }
        _ => format!("rgb({}, {}, {})", n(c.r), n(c.g), n(c.b)),
    }
}

pub fn parse_static_color(value: &str) -> Option<Rgba> {
    if let Some(parsed) = parse_any_color(Some(value)) {
        return Some(parsed);
    }
    let key = js::to_lower_case(js::trim(value));
    static_named_color(&key)
}

/// JS `NAMED_COLOR_TOKENS`: every shared + static named color, longest
/// first (stable), joined with `|`.
static NAMED_COLOR_TOKENS: Lazy<String> = Lazy::new(|| {
    let mut names: Vec<&str> = CSS_NAMED_COLORS
        .iter()
        .map(|(n, _)| *n)
        .chain(STATIC_NAMED_COLORS.iter().map(|(n, _)| *n))
        .collect();
    // JS Array.prototype.sort is stable; sort_by is stable too.
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    names.join("|")
});

/// JS `STATIC_COLOR_TOKEN_RE`.
static STATIC_COLOR_TOKEN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"(?i)(?:rgba?\([^)]+\)|oklch\([^)]+\)|oklab\([^)]+\)|lch\([^)]+\)|lab\([^)]+\)|hsla?\([^)]+\)|hwb\([^)]+\)|#[0-9a-f]{{3,8}}(?-u:\b)|(?-u:\b)(?:{})(?-u:\b))",
        *NAMED_COLOR_TOKENS
    ))
    .expect("STATIC_COLOR_TOKEN_RE")
});

static VAR_HEAD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^var\(").expect("VAR_HEAD_RE"));
static COLOR_MIX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)color-mix\(").expect("COLOR_MIX_RE"));

pub fn extract_static_color(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let raw = js::trim(value);
    if VAR_HEAD_RE.is_match(raw) {
        return raw.to_string();
    }
    // color-mix(...) needs balanced-paren capture (its arguments regularly
    // contain nested var()/oklch() calls AND the keyword `transparent`, which
    // the flat regex below would otherwise pluck out of the middle of the
    // expression and report as the whole color).
    if let Some(m) = COLOR_MIX_RE.find(raw) {
        let mix_start = m.start();
        let bytes = raw.as_bytes();
        // JS: raw.indexOf('(', mixStart) — the `(` right after `color-mix`.
        let mut i = match raw[mix_start..].find('(') {
            Some(off) => mix_start + off,
            None => raw.len(),
        };
        let mut depth: i64 = 0;
        while i < bytes.len() {
            if bytes[i] == b'(' {
                depth += 1;
            } else if bytes[i] == b')' {
                depth -= 1;
                if depth == 0 {
                    return raw[mix_start..=i].to_string();
                }
            }
            i += 1;
        }
        return String::new();
    }
    match STATIC_COLOR_TOKEN_RE.find(raw) {
        Some(m) => m.as_str().to_string(),
        None => String::new(),
    }
}

static MODERN_BORDER_PROP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^border[A-Z][a-z]+Color$").expect("MODERN_BORDER_PROP_RE"));
static MODERN_COLOR_FN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?:oklch|oklab|lch|lab|hsl|hwb)\(").expect("MODERN_COLOR_FN_RE")
});
static COLOR_TAIL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)color$").expect("COLOR_TAIL_RE"));

/// JS `parseFloat(style?.fontSize) || 16`.
fn font_size_base(style: Option<&StyleValues>) -> f64 {
    let n = match style_get(style, "fontSize") {
        Some(v) => js::parse_float(v),
        None => f64::NAN,
    };
    if n.is_nan() || n == 0.0 {
        16.0
    } else {
        n
    }
}

/// JS `parseFloat(currentStyle?.fontSize || parentStyle?.fontSize) || 16`.
fn font_size_base2(current: Option<&StyleValues>, parent: Option<&StyleValues>) -> f64 {
    let v = match style_get(current, "fontSize") {
        Some(v) if !v.is_empty() => Some(v),
        _ => style_get(parent, "fontSize"),
    };
    let n = match v {
        Some(v) => js::parse_float(v),
        None => f64::NAN,
    };
    if n.is_nan() || n == 0.0 {
        16.0
    } else {
        n
    }
}

pub fn normalize_static_css_value(
    prop: &str,
    value: &str,
    custom_props: &CustomProps,
    parent_style: Option<&StyleValues>,
    current_style: Option<&StyleValues>,
) -> String {
    let mut resolved = resolve_var_refs(js::trim(value), custom_props);
    if resolved == "inherit" {
        if let Some(v) = style_get(parent_style, prop) {
            if !v.is_empty() {
                return v.to_string();
            }
        }
        if let Some(d) = static_default_style(prop) {
            if !d.is_empty() {
                return d.to_string();
            }
        }
        return String::new();
    }
    let is_modern_border_color =
        MODERN_BORDER_PROP_RE.is_match(prop) && MODERN_COLOR_FN_RE.is_match(&resolved);
    if !is_modern_border_color
        && (COLOR_TAIL_RE.is_match(prop) || prop == "color" || prop == "backgroundColor")
    {
        if let Some(parsed) = parse_static_color(&resolved) {
            resolved = static_color_to_css(Some(&parsed));
        }
    }
    if prop == "fontSize" {
        let base = font_size_base(parent_style);
        if let Some(px) = resolve_length_px(&resolved, base) {
            resolved = format!("{}px", js::number_to_string(px));
        }
    }
    if prop == "letterSpacing" {
        let base = font_size_base2(current_style, parent_style);
        if let Some(px) = resolve_length_px(&resolved, base) {
            resolved = format!("{}px", js::number_to_string(px));
        }
    }
    if prop == "lineHeight" && resolved != "normal" {
        // Unitless line-height inherits its multiplier. Relative lengths such
        // as em and percent compute against this element's font size here and
        // then inherit as that computed length.
        let is_unitless = resolved
            .parse::<f64>()
            .is_ok_and(|number| number.is_finite());
        if !is_unitless {
            let base = font_size_base2(current_style, parent_style);
            if let Some(px) = resolve_length_px(&resolved, base) {
                resolved = format!("{}px", js::number_to_string(px));
            }
        }
    }
    resolved
}
