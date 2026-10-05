//! Shared project and workspace path facts.
//!
//! Context selection and DESIGN.md inheritance have different stopping
//! rules, but both use the same workspace declarations, glob semantics,
//! marker names, and ignored discovery directories. Keep those facts here so
//! each resolver applies them to its own result contract.

use serde_json::Value;

pub const PROJECT_MARKER_FILES: [&str; 2] = [".git", "package.json"];
pub const MONOREPO_MARKER_FILES: [&str; 4] =
    ["pnpm-workspace.yaml", "turbo.json", "nx.json", "lerna.json"];
pub const MONOREPO_FALLBACK_PROJECT_DIRS: [&str; 2] = ["apps", "packages"];
pub const WORKSPACE_DISCOVERY_IGNORED_DIRS: [&str; 12] = [
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".cache",
    "coverage",
    "vendor",
    "vendors",
];

/// True when a directory marks an independent project boundary.
pub fn has_project_marker(dir: &str) -> bool {
    PROJECT_MARKER_FILES.iter().any(|marker| {
        let path = crate::jsp::join(&[dir, marker]);
        std::path::Path::new(&path).exists()
    })
}

/// True when a directory or linked worktree has a `.git` entry.
pub fn has_git_boundary(dir: &str) -> bool {
    let path = crate::jsp::join(&[dir, ".git"]);
    std::path::Path::new(&path).exists()
}

/// Directories excluded while discovering workspace candidates.
pub fn is_ignored_workspace_discovery_dir(name: &str) -> bool {
    name.starts_with('.') || WORKSPACE_DISCOVERY_IGNORED_DIRS.contains(&name)
}

/// Workspace patterns from package.json, pnpm-workspace.yaml, and lerna.json.
/// Package then pnpm then Lerna preserves the context resolver's precedence.
pub fn read_workspace_patterns(root: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let package_path = crate::jsp::join(&[root, "package.json"]);
    if let Some(package) = read_json(&package_path) {
        if let Some(workspaces) = package.get("workspaces") {
            if workspaces.is_array() {
                patterns.extend(value_strings(workspaces));
            } else if let Some(entries) = workspaces.get("packages") {
                if entries.is_array() {
                    patterns.extend(value_strings(entries));
                }
            }
        }
    }
    patterns.extend(read_pnpm_workspace_patterns(root));
    if let Some(lerna) = read_json(&crate::jsp::join(&[root, "lerna.json"])) {
        if let Some(entries) = lerna.get("packages").filter(|value| value.is_array()) {
            patterns.extend(value_strings(entries));
        }
    }
    patterns
        .into_iter()
        .filter(|pattern| !pattern.is_empty())
        .collect()
}

fn value_strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| match entry {
            Value::String(value) => value.clone(),
            Value::Null => "null".to_string(),
            other => other.to_string(),
        })
        .collect()
}

fn read_json(path: &str) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_pnpm_workspace_patterns(root: &str) -> Vec<String> {
    let path = crate::jsp::join(&[root, "pnpm-workspace.yaml"]);
    let Some(bytes) = std::fs::read(path).ok() else {
        return Vec::new();
    };
    let body = String::from_utf8_lossy(&bytes);
    let mut patterns = Vec::new();
    let mut in_packages = false;
    for line in body.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let without_comment = strip_yaml_inline_comment(line);
        let trimmed = js_trim(&without_comment);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(list) = trimmed
            .strip_prefix("packages:")
            .and_then(|tail| tail.trim().strip_prefix('['))
            .and_then(|tail| tail.strip_suffix(']'))
        {
            patterns.extend(parse_yaml_flow_list(list));
            in_packages = false;
            continue;
        }
        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }
        if in_packages && is_yaml_key(trimmed) {
            break;
        }
        if in_packages {
            if let Some(item) = trimmed.strip_prefix('-') {
                let value = unquote_yaml_value(item);
                if !value.is_empty() {
                    patterns.push(value);
                }
            }
        }
    }
    patterns
}

fn is_yaml_key(line: &str) -> bool {
    let Some((key, _)) = line.split_once(':') else {
        return false;
    };
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn strip_yaml_inline_comment(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut quote: Option<char> = None;
    for index in 0..chars.len() {
        let ch = chars[index];
        if (ch == '"' || ch == '\'') && (index == 0 || chars[index - 1] != '\\') {
            quote = if quote == Some(ch) {
                None
            } else {
                quote.or(Some(ch))
            };
            continue;
        }
        if ch == '#' && quote.is_none() {
            return chars[..index].iter().collect();
        }
    }
    line.to_string()
}

fn parse_yaml_flow_list(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut items = Vec::new();
    let mut quote: Option<char> = None;
    let mut current = String::new();
    for index in 0..chars.len() {
        let ch = chars[index];
        if (ch == '"' || ch == '\'') && (index == 0 || chars[index - 1] != '\\') {
            quote = if quote == Some(ch) {
                None
            } else {
                quote.or(Some(ch))
            };
            current.push(ch);
            continue;
        }
        if ch == ',' && quote.is_none() {
            let value = unquote_yaml_value(&current);
            if !value.is_empty() {
                items.push(value);
            }
            current.clear();
            continue;
        }
        current.push(ch);
    }
    let value = unquote_yaml_value(&current);
    if !value.is_empty() {
        items.push(value);
    }
    items
}

fn unquote_yaml_value(value: &str) -> String {
    strip_one_quote_each_end(js_trim(value))
}

fn strip_one_quote_each_end(value: &str) -> String {
    let mut value = value;
    if value.starts_with('\'') || value.starts_with('"') {
        value = &value[1..];
    }
    if value.ends_with('\'') || value.ends_with('"') {
        value = &value[..value.len() - 1];
    }
    value.to_string()
}

/// The JS String.prototype.trim behavior used for YAML values.
fn js_trim(value: &str) -> &str {
    value.trim_matches(|ch: char| ch.is_whitespace() || ch == '\u{FEFF}')
}

/// Normalize the package-manager glob syntax shared by both root resolvers.
pub fn normalize_workspace_pattern(value: &str) -> String {
    let value = strip_one_quote_each_end(js_trim(value));
    let value = value.strip_prefix("./").unwrap_or(&value);
    value.trim_end_matches('/').to_string()
}

/// Match a workspace glob segment. Only `*` is a wildcard, matching any
/// characters inside the segment.
pub fn segment_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut pi, mut vi, mut star, mut retry) = (0, 0, None, 0);
    while vi < value.len() {
        if pi < pattern.len() && pattern[pi] == value[vi] {
            pi += 1;
            vi += 1;
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star = Some(pi);
            pi += 1;
            retry = vi;
        } else if let Some(star_index) = star {
            pi = star_index + 1;
            retry += 1;
            vi = retry;
        } else {
            return false;
        }
    }
    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}

/// Match a slash-separated workspace glob against path segments.
pub fn match_glob_segments(pattern: &[&str], path: &[&str]) -> bool {
    fn match_from(pattern: &[&str], path: &[&str], pi: usize, si: usize) -> bool {
        if pi == pattern.len() {
            return si == path.len();
        }
        if pattern[pi] == "**" {
            if pi + 1 == pattern.len() {
                return true;
            }
            return (si..=path.len()).any(|next| match_from(pattern, path, pi + 1, next));
        }
        si < path.len()
            && segment_matches(pattern[pi], path[si])
            && match_from(pattern, path, pi + 1, si + 1)
    }
    match_from(pattern, path, 0, 0)
}

/// True when the path belongs to the workspace declaration at `root`.
/// Ordinary `*` patterns own a package at the declared depth and nested
/// directories below that package; a `**` pattern is matched directly.
pub fn workspace_owns_path(root: &str, target: &str) -> bool {
    let relative = crate::jsp::relative("/", root, target);
    if relative.is_empty() || relative.starts_with("..") || crate::jsp::is_absolute(&relative) {
        return false;
    }
    let path: Vec<&str> = relative
        .split(crate::jsp::SEP_CHAR)
        .filter(|segment| !segment.is_empty())
        .collect();
    let raw_patterns = read_workspace_patterns(root);
    let patterns: Vec<String> = raw_patterns
        .iter()
        .map(|pattern| normalize_workspace_pattern(pattern))
        .filter(|pattern| !pattern.is_empty())
        .collect();
    let matches_pattern = |pattern: &str| {
        let segments: Vec<&str> = pattern
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.is_empty() {
            return false;
        }
        if segments.contains(&"**") {
            return match_glob_segments(&segments, &path);
        }
        path.len() >= segments.len()
            && segments
                .iter()
                .zip(&path)
                .all(|(pattern, value)| segment_matches(pattern, value))
    };
    let excluded = patterns
        .iter()
        .filter_map(|pattern| pattern.strip_prefix('!'))
        .any(matches_pattern);
    if excluded {
        return false;
    }
    let included = patterns
        .iter()
        .filter(|pattern| !pattern.starts_with('!'))
        .any(|pattern| {
            let segments: Vec<&str> = pattern
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect();
            if segments.contains(&"**") {
                return match_glob_segments(&segments, &path);
            }
            if segments.is_empty()
                || path.len() < segments.len()
                || !segments
                    .iter()
                    .zip(&path)
                    .all(|(pattern, value)| segment_matches(pattern, value))
            {
                return false;
            }
            if path.len() == segments.len() {
                return true;
            }
            let ancestor = crate::jsp::join(
                &std::iter::once(root)
                    .chain(path[..segments.len()].iter().copied())
                    .collect::<Vec<_>>(),
            );
            std::path::Path::new(&crate::jsp::join(&[&ancestor, "package.json"])).exists()
        });
    if included {
        return true;
    }
    if patterns.iter().any(|pattern| !pattern.starts_with('!')) {
        return false;
    }
    path.len() >= 2 && MONOREPO_FALLBACK_PROJECT_DIRS.contains(&path[0])
}

/// True when a directory declares a workspace or a marker file and fallback
/// `apps/` or `packages/` tree contains an actual candidate directory.
pub fn is_monorepo_root(dir: &str) -> bool {
    if read_workspace_patterns(dir)
        .iter()
        .any(|pattern| !normalize_workspace_pattern(pattern).starts_with('!'))
    {
        return true;
    }
    if !MONOREPO_MARKER_FILES
        .iter()
        .any(|file| std::path::Path::new(&crate::jsp::join(&[dir, file])).exists())
    {
        return false;
    }
    MONOREPO_FALLBACK_PROJECT_DIRS.iter().any(|name| {
        std::fs::read_dir(crate::jsp::join(&[dir, name]))
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
                        && !is_ignored_workspace_discovery_dir(&entry.file_name().to_string_lossy())
                })
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_segment_and_path_matching_are_shared() {
        assert!(segment_matches("app-*", "app-web"));
        assert!(!segment_matches("app-*", "app"));
        assert!(match_glob_segments(
            &["apps", "**", "web"],
            &["apps", "web"]
        ));
        assert!(match_glob_segments(
            &["apps", "**", "web"],
            &["apps", "one", "web"]
        ));
        assert!(!match_glob_segments(
            &["apps", "**", "web"],
            &["apps", "one", "api"]
        ));
    }

    #[test]
    fn workspace_declarations_share_yaml_and_json_parsing() {
        let root = std::env::temp_dir().join(format!(
            "impeccino-workspace-patterns-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"workspaces":{"packages":["apps/*","packages/*"]}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - 'apps/*' # app packages\n  - \"components/**\"\n",
        )
        .unwrap();
        std::fs::write(root.join("lerna.json"), r#"{"packages":["modules/*"]}"#).unwrap();

        let root_text = root.to_string_lossy();
        assert_eq!(
            read_workspace_patterns(&root_text),
            vec![
                "apps/*",
                "packages/*",
                "apps/*",
                "components/**",
                "modules/*"
            ]
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn marker_only_monorepos_ignore_hidden_and_dependency_directories() {
        let root = std::env::temp_dir().join(format!(
            "impeccino-marker-workspace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("apps/.hidden")).unwrap();
        std::fs::create_dir_all(root.join("packages/node_modules")).unwrap();
        std::fs::write(root.join("turbo.json"), "{}").unwrap();
        let root_text = root.to_string_lossy();
        assert!(!is_monorepo_root(&root_text));
        std::fs::create_dir_all(root.join("apps/web")).unwrap();
        assert!(is_monorepo_root(&root_text));
        std::fs::remove_dir_all(root).unwrap();
    }
}
