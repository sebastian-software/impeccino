//! The supported DESIGN.md YAML subset, shared by doctor and the detector.
//! Permissive reading retains the existing nested-map/scalar interpretation.
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(is_yaml_whitespace)
}

pub(crate) fn is_yaml_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

pub fn parse_frontmatter(md: &str) -> Option<Map<String, Value>> {
    let lines: Vec<&str> = md
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    if trim(lines.first().copied().unwrap_or("")) != "---" {
        return None;
    }
    let mut end = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if trim(line) == "---" {
            end = Some(i);
            break;
        }
    }
    let end = end?;
    Some(parse_yaml_subset(&lines[1..end].join("\n")))
}

/// `parseYamlSubset`: (nested maps and scalars only).
pub fn parse_yaml_subset(yaml: &str) -> Map<String, Value> {
    parse_mapping(yaml, 0).values
}

#[derive(Default)]
pub(crate) struct Mapping {
    pub values: Map<String, Value>,
    pub locations: BTreeMap<String, usize>,
    pub columns: BTreeMap<String, usize>,
    pub problems: Vec<(usize, String, &'static str)>,
}

pub(crate) fn pointer(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| format!("/{}", part.replace('~', "~0").replace('/', "~1")))
        .collect()
}

pub(crate) fn parse_mapping(yaml: &str, line_offset: usize) -> Mapping {
    // Build a tree of paths, then materialize: JS mutates nested objects by
    // reference; here we track the key path of each open mapping.
    let mut root = Map::new();
    let mut mapping = Mapping::default();
    let mut stack: Vec<(i64, Vec<String>)> = vec![(-1, vec![])];
    for (index, raw) in yaml
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .enumerate()
    {
        if trim(raw).is_empty() || trim(raw).starts_with('#') {
            continue;
        }
        let indent_bytes = raw.len() - raw.trim_start_matches(is_yaml_whitespace).len();
        let indent = raw[..indent_bytes]
            .chars()
            .map(|c| c.len_utf16())
            .sum::<usize>() as i64;
        let content = &raw[indent_bytes..];
        let Some(colon_idx) = find_top_level_colon(content) else {
            mapping.problems.push((
                index + line_offset + 1,
                pointer(&stack.last().unwrap().1),
                "Unsupported YAML syntax; use nested mappings and scalar token values",
            ));
            continue;
        };
        while stack.len() > 1 && stack.last().map(|s| s.0 >= indent).unwrap_or(false) {
            stack.pop();
        }
        let key = unquote_yaml_key(trim(&content[..colon_idx])).to_string();
        let rest = strip_inline_yaml_comment(trim(&content[colon_idx + 1..]));
        let parent_path = stack.last().map(|s| s.1.clone()).unwrap_or_default();
        let mut entry_path = parent_path.clone();
        entry_path.push(key.clone());
        let entry_pointer = pointer(&entry_path);
        mapping
            .locations
            .insert(entry_pointer.clone(), index + line_offset + 1);
        mapping.columns.insert(
            entry_pointer.clone(),
            raw[..indent_bytes].chars().count() + 1,
        );
        let parent = get_path_mut(&mut root, &parent_path);
        if parent.contains_key(&key) {
            mapping.problems.push((
                index + line_offset + 1,
                entry_pointer,
                "Duplicate mapping key; retain one authoritative value",
            ));
        }
        if rest.is_empty() {
            parent.insert(key.clone(), Value::Object(Map::new()));
            let mut path = parent_path;
            path.push(key);
            stack.push((indent, path));
        } else {
            parent.insert(key, parse_scalar(&rest));
        }
    }
    mapping.values = root;
    mapping
}

fn get_path_mut<'a>(
    root: &'a mut Map<String, Value>,
    path: &[String],
) -> &'a mut Map<String, Value> {
    let mut cur = root;
    for key in path {
        // A key on the stack was inserted as an object; a later scalar write to
        // the same key at the parent level would already have popped the stack.
        let entry = cur
            .entry(key.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cur = entry.as_object_mut().unwrap();
    }
    cur
}

fn find_top_level_colon(s: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    let mut prev: Option<char> = None;
    for (i, ch) in s.char_indices() {
        if let Some(q) = in_quote {
            if ch == q && prev != Some('\\') {
                in_quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
        } else if ch == ':' {
            return Some(i);
        }
        prev = Some(ch);
    }
    None
}

pub fn unquote_yaml_key(key: &str) -> &str {
    if (key.starts_with('"') && key.ends_with('"'))
        || (key.starts_with('\'') && key.ends_with('\''))
    {
        if key.chars().count() >= 2 {
            return &key[1..key.len() - 1];
        }
        // JS slice(1, -1) on a one-char string yields ''.
        return "";
    }
    key
}

fn strip_inline_yaml_comment(s: &str) -> String {
    let mut in_quote: Option<char> = None;
    let mut prev: Option<char> = None;
    for (i, ch) in s.char_indices() {
        if let Some(q) = in_quote {
            if ch == q && prev != Some('\\') {
                in_quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
        } else if ch == '#' && i > 0 && prev.map(is_yaml_whitespace).unwrap_or(false) {
            return s[..i].trim_end_matches(is_yaml_whitespace).to_string();
        }
        prev = Some(ch);
    }
    s.to_string()
}

fn yaml_simple_escape(c: char) -> Option<char> {
    Some(match c {
        '0' => '\0',
        'a' => '\x07',
        'b' => '\x08',
        't' => '\t',
        'n' => '\n',
        'v' => '\x0B',
        'f' => '\x0C',
        'r' => '\r',
        'e' => '\x1b',
        ' ' => ' ',
        '"' => '"',
        '/' => '/',
        '\\' => '\\',
        'N' => '\u{85}',
        '_' => '\u{a0}',
        'L' => '\u{2028}',
        'P' => '\u{2029}',
        _ => return None,
    })
}

fn unescape_yaml_double_quoted(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch != '\\' || i == chars.len() - 1 {
            out.push(ch);
            i += 1;
            continue;
        }
        let next = chars[i + 1];
        if let Some(mapped) = yaml_simple_escape(next) {
            out.push(mapped);
            i += 2;
            continue;
        }
        let hex_len = match next {
            'x' => Some(2),
            'u' => Some(4),
            'U' => Some(8),
            _ => None,
        };
        if let Some(hex_len) = hex_len {
            let hex: String = chars[(i + 2).min(chars.len())..(i + 2 + hex_len).min(chars.len())]
                .iter()
                .collect();
            let code_point: i64 =
                if hex.chars().count() == hex_len && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    i64::from_str_radix(&hex, 16).unwrap_or(-1)
                } else {
                    -1
                };
            if (0..=0x10ffff).contains(&code_point) {
                // String.fromCodePoint: a surrogate code point has no char
                // representation; substitute U+FFFD as the closest thing.
                out.push(char::from_u32(code_point as u32).unwrap_or('\u{fffd}'));
                i += 2 + hex_len;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

static INT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^-?[0-9]+$").unwrap());
static FLOAT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^-?[0-9]*\\.[0-9]+$").unwrap());

fn parse_scalar(raw: &str) -> Value {
    let s = trim(raw);
    let n = s.chars().count();
    if n >= 2 && s.starts_with('"') && s.ends_with('"') {
        return Value::String(unescape_yaml_double_quoted(&s[1..s.len() - 1]));
    }
    if n >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return Value::String(s[1..s.len() - 1].replace("''", "'"));
    }
    if s == "true" {
        return Value::Bool(true);
    }
    if s == "false" {
        return Value::Bool(false);
    }
    if s == "null" || s == "~" {
        return Value::Null;
    }
    if INT_RE.is_match(s) || FLOAT_RE.is_match(s) {
        let v = s.parse::<f64>().unwrap_or(f64::NAN);
        return serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::String(s.to_string()));
    }
    Value::String(s.to_string())
}
