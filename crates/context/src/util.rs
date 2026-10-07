//! Small helpers shared by every verb: JS-flavoured string/number semantics,
//! best-effort fs reads, JSON conveniences.

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;

pub type Env = HashMap<String, String>;

/// JS `String.prototype.trim()`: WhiteSpace + LineTerminator, incl. U+FEFF.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
}

/// JS `str.length` (UTF-16 code units).
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// JS `Number.prototype.toFixed(digits)` (round-half-up on the exact decimal
/// expansion, which is what ties pick under "the larger n").
pub fn to_fixed(v: f64, digits: usize) -> String {
    impeccino_core::js::to_fixed(v, digits)
}

pub fn js_number_to_string(v: f64) -> String {
    impeccino_core::js::number_to_string(v)
}

/// A JS number as a JSON value: integral values print without `.0`.
pub fn js_num(v: f64) -> Value {
    if v.is_finite() && v.fract() == 0.0 && v.abs() < 9007199254740992.0 {
        Value::from(v as i64)
    } else if v.is_finite() {
        Value::from(v)
    } else {
        Value::Null
    }
}

pub fn exists(p: &str) -> bool {
    Path::new(p).exists()
}

pub fn is_dir(p: &str) -> bool {
    Path::new(p).is_dir()
}

pub fn is_file(p: &str) -> bool {
    Path::new(p).is_file()
}

/// `fs.readFileSync(p, 'utf-8')` or null.
pub fn safe_read(p: &str) -> Option<String> {
    std::fs::read(p)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `JSON.parse(fs.readFileSync(p))` or null.
pub fn read_json(p: &str) -> Option<Value> {
    let text = safe_read(p)?;
    serde_json::from_str::<Value>(&text).ok()
}

/// `readdirSync` names in directory order (Node returns them sorted by the
/// OS; on macOS/Linux this is not guaranteed sorted, so callers that sort do
/// so explicitly). We sort by byte order for determinism where JS output
/// depends on order without sorting.
pub fn read_dir_names(p: &str) -> Option<Vec<String>> {
    let rd = std::fs::read_dir(p).ok()?;
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Some(names)
}

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
}

/// `readdirSync(p, { withFileTypes: true })`, sorted by name.
pub fn read_dir_entries(p: &str) -> Option<Vec<DirEntry>> {
    let rd = std::fs::read_dir(p).ok()?;
    let mut out: Vec<DirEntry> = rd
        .filter_map(|e| e.ok())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            // Node's Dirent reports the link type, not the target's; follow
            // symlinks like Node does when the entry is a symlink? Node's
            // withFileTypes reports isSymbolicLink() for links, so isDirectory()
            // is false for symlinked dirs. Mirror that.
            let ft = e.file_type().ok();
            let (is_dir, is_file) = match ft {
                Some(t) if t.is_symlink() => (false, false),
                Some(t) => (t.is_dir(), t.is_file()),
                None => (false, false),
            };
            DirEntry {
                name,
                is_dir,
                is_file,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Some(out)
}

pub fn mtime_ms(p: &str) -> Option<f64> {
    let md = std::fs::metadata(p).ok()?;
    let t = md.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_secs_f64() * 1000.0)
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// `new Date().toISOString()`
pub fn iso_now() -> String {
    iso_from_ms(now_ms())
}

pub fn iso_from_ms(ms: f64) -> String {
    let ms_i = ms.floor() as i64;
    let secs = ms_i.div_euclid(1000);
    let millis = ms_i.rem_euclid(1000);
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y,
        m,
        d,
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60,
        millis
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `JSON.stringify(v, null, 2)`
pub fn json_pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| "null".into())
}

/// `JSON.stringify(v)`
pub fn json_compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

pub fn obj() -> Map<String, Value> {
    Map::new()
}

pub fn opt_str(s: Option<&str>) -> Value {
    match s {
        Some(v) => Value::String(v.to_string()),
        None => Value::Null,
    }
}

pub fn opt_string(s: &Option<String>) -> Value {
    match s {
        Some(v) => Value::String(v.clone()),
        None => Value::Null,
    }
}

/// `os.homedir()` as Node computes it on posix: $HOME first.
pub fn homedir(env: &Env) -> String {
    impeccino_common::project_files::home_dir(|key| env.get(key).cloned())
        .or_else(|| impeccino_common::project_files::home_dir(|key| std::env::var(key).ok()))
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "C:\\".to_string()
            } else {
                "/".to_string()
            }
        })
}

/// hook-lib `truthy()`: /^(1|true|yes|on)$/i on the trimmed value.
pub fn truthy_env(env: &Env, key: &str) -> bool {
    match env.get(key) {
        Some(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

/// Node's ENOENT-style error message for a failed read.
pub fn node_read_error(p: &str, err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => format!("ENOENT: no such file or directory, open '{}'", p),
        std::io::ErrorKind::PermissionDenied => format!("EACCES: permission denied, open '{}'", p),
        _ => {
            if err.raw_os_error() == Some(21) {
                "EISDIR: illegal operation on a directory, read".to_string()
            } else {
                format!("{}", err)
            }
        }
    }
}

/// The per-user cache directory (`<cache>/impeccino`, see
/// `impeccino_common::project_files::user_cache_dir`), read from the verb's
/// environment. Falls back to `<homedir>/.cache/impeccino` when the env
/// names no home, the way `homedir` falls back to the process home.
pub fn user_cache_dir(env: &Env) -> String {
    impeccino_common::project_files::user_cache_dir(|k| env.get(k).cloned())
        .unwrap_or_else(|| crate::jsp::join(&[&homedir(env), ".cache", "impeccino"]))
}

/// `Number(str)`
pub fn js_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    if t == "Infinity" || t == "+Infinity" {
        return f64::INFINITY;
    }
    if t == "-Infinity" {
        return f64::NEG_INFINITY;
    }
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return i64::from_str_radix(h, 16)
            .map(|v| v as f64)
            .unwrap_or(f64::NAN);
    }
    // Reject things Rust accepts but JS doesn't (e.g. "nan", "inf")
    if t.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

fn js_number_value_string(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    js_number_to_string(n.as_f64().unwrap_or(0.0))
}

/// `String(value)` for a JSON value: arrays join with ',', objects read
/// "[object Object]" (used where JS coerces sidecar and config fields).
pub fn js_string_value(v: &Value) -> String {
    match v {
        Value::Array(a) => a
            .iter()
            .map(|e| match e {
                Value::Null => String::new(),
                Value::String(s) => s.clone(),
                Value::Bool(b) => b.to_string(),
                Value::Number(n) => js_number_value_string(n),
                other => js_string_value(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => js_number_value_string(n),
        Value::Null => "null".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed() {
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(0.1234, 3), "0.123");
        assert_eq!(to_fixed(1.0005, 3), "1.000"); // 1.0005 is below the tie in binary
        assert_eq!(to_fixed(22.5, 0), "23");
        assert_eq!(to_fixed(0.65, 3), "0.650");
        assert_eq!(to_fixed(359.99, 1), "360.0");
    }
    #[test]
    fn iso() {
        assert_eq!(iso_from_ms(0.0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_from_ms(1778610600123.0), "2026-05-12T18:30:00.123Z");
    }
}
