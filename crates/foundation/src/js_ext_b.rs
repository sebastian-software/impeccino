//! JS-semantics helpers needed by the group-B checks port that `js.rs` does
//! not carry (kept separate so parallel work does not collide).

use regex::Regex;
use std::sync::LazyLock as Lazy;

static WHITESPACE_RUN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!("{}+", crate::js::WS)).expect("WHITESPACE_RUN_RE"));

/// JS `String.prototype.length`: UTF-16 code units, not chars or bytes.
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

/// JS `s.replace(/\s+/g, ' ')`.
pub fn collapse_whitespace(s: &str) -> String {
    WHITESPACE_RUN_RE.replace_all(s, " ").into_owned()
}

/// JS `cleanInlineText(el)`: direct text nodes joined with a space,
/// whitespace collapsed, and trimmed.
pub fn clean_inline_text<I, S>(parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let joined = parts
        .into_iter()
        .map(|part| part.as_ref().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    crate::js::trim(&collapse_whitespace(&joined)).to_string()
}

/// JS `(el.textContent || '').replace(/\s+/g, ' ').trim()`.
pub fn collapsed_text_content(text: &str) -> String {
    crate::js::trim(&collapse_whitespace(text)).to_string()
}

/// Limit a display snippet by UTF-16 units without splitting a Unicode scalar.
/// A supplementary character that crosses the limit is omitted entirely.
pub fn slice_utf16_prefix(s: &str, end: usize) -> String {
    let mut units = 0;
    s.chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= end
        })
        .collect()
}

/// JS truthiness of a number: `0`, `-0`, and `NaN` are falsy.
pub fn num_truthy(n: f64) -> bool {
    n != 0.0 && !n.is_nan()
}

/// JS `String.prototype.split(sep)` with a regex separator that never
/// matches empty text: the pieces between matches, keeping empty pieces at
/// the ends exactly as JS does.
pub fn split_regex<'a>(re: &regex::Regex, s: &'a str) -> Vec<&'a str> {
    re.split(s).collect()
}

/// JS SameValueZero, the equality `Set` uses (NaN equals NaN, +0 equals -0).
pub fn same_value_zero(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_cases() {
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(utf16_len("a😀"), 3);
        assert_eq!(slice_utf16_prefix("a😀b", 2), "a");
        assert_eq!(slice_utf16_prefix("abc", 60), "abc");
        assert_eq!(slice_utf16_prefix("abcd", 2), "ab");
    }

    #[test]
    fn direct_text_cleaning_joins_nodes_before_collapsing_whitespace() {
        assert_eq!(clean_inline_text(["  Fea ", " \t tures  "]), "Fea tures");
        assert_eq!(collapsed_text_content(" A \n  B "), "A B");
    }
}
