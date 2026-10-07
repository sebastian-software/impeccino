//! Shared file-scan scope facts used by the directory scanner and hooks.
//!
//! The CLI and hook scan source files and server-side templates. The hook
//! keeps tighter per-event byte ceilings than the
//! CLI; those are latency limits, not another extension or generated-file
//! policy.

use once_cell::sync::Lazy;
use regex::Regex;

/// Extensions accepted by directory scans and import resolution. Compound
/// suffixes must remain ordered after simple suffixes for imports.
pub const SCANNABLE_EXTENSIONS: &[&str] = &[
    ".html",
    ".htm",
    ".css",
    ".scss",
    ".sass",
    ".less",
    ".jsx",
    ".tsx",
    ".js",
    ".ts",
    ".vue",
    ".svelte",
    ".astro",
    ".blade.php",
    ".twig",
    ".html.erb",
    ".erb",
    ".hbs",
    ".handlebars",
];

/// HTML extensions that the source scanner sends to the static HTML engine.
pub const HTML_EXTENSIONS: &[&str] = &[".html", ".htm"];

/// Server-side template suffixes both scanners send to the static HTML engine.
/// The hook installs these as its fixed default extension entries.
pub const TEMPLATE_EXTENSIONS: &[&str] = &[
    ".blade.php",
    ".twig",
    ".html.erb",
    ".erb",
    ".hbs",
    ".handlebars",
];

/// Ordinary source extensions accepted by the edit hook. `.blade.php` is
/// supplied by [`TEMPLATE_EXTENSIONS`] so it can select the HTML engine.
pub const HOOK_SOURCE_EXTENSIONS: &[&str] = &[
    ".tsx", ".jsx", ".html", ".htm", ".vue", ".svelte", ".astro", ".css", ".scss", ".sass",
    ".less", ".ts", ".js",
];

/// Directories whose contents are generated or dependency output.
/// This list is also reflected in [`is_generated_path`] for hook candidates.
pub const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "__pycache__",
    "out",
    "generated",
    "coverage",
    ".next",
    ".cache",
];

/// Hidden source directories intentionally included in directory scans.
pub const HIDDEN_SOURCE_DIRS: &[&str] = &[".vitepress", ".vuepress", ".storybook"];

static GENERATED_PATH_RE: Lazy<Regex> = Lazy::new(|| {
    let skipped_dirs = SKIP_DIRS
        .iter()
        .map(|dir| regex::escape(dir))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(
        r"(?i)(?:\.generated\.[a-z]+$|\.d\.ts$|\.min\.[a-z]+$|[/\\](?:{skipped_dirs})[/\\]|[/\\]?[^/\\]+\.lock(?:\.json)?$)"
    ))
    .expect("generated path pattern")
});

/// True when a basename has an accepted scan suffix. Compound suffixes such
/// as `.blade.php` are checked after the host path's ordinary extension.
pub fn has_scannable_extension(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    let ext = crate::jsp::extname(&lower);
    SCANNABLE_EXTENSIONS.contains(&ext.as_str())
        || SCANNABLE_EXTENSIONS
            .iter()
            .any(|suffix| suffix[1..].contains('.') && lower.ends_with(suffix))
}

/// True when the path ends in an HTML file suffix handled by the static HTML
/// engine on the source-scan path.
pub fn is_html_path(file_path: &str) -> bool {
    let lower = file_path.to_ascii_lowercase();
    HTML_EXTENSIONS.contains(&crate::jsp::extname(&lower).as_str())
        || TEMPLATE_EXTENSIONS
            .iter()
            .any(|suffix| lower.ends_with(suffix))
}

/// Shared generated/dependency path filter for hooks and directory scans.
/// Explicit CLI file arguments remain an intentional user-selected override.
pub fn is_generated_path(path: &str) -> bool {
    GENERATED_PATH_RE.is_match(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_and_template_suffixes_have_one_shared_definition() {
        assert!(has_scannable_extension("A.HTML"));
        assert!(has_scannable_extension("a.blade.php"));
        assert!(!has_scannable_extension("a.php"));
        for suffix in TEMPLATE_EXTENSIONS {
            assert!(has_scannable_extension(&format!("view{suffix}")));
            assert!(is_html_path(&format!("/project/view{suffix}")));
            assert!(SCANNABLE_EXTENSIONS.contains(suffix));
        }
        for suffix in HOOK_SOURCE_EXTENSIONS {
            assert!(SCANNABLE_EXTENSIONS.contains(suffix));
        }
    }

    #[test]
    fn generated_filter_covers_directory_and_file_suffixes() {
        for dir in SKIP_DIRS {
            assert!(
                is_generated_path(&format!("/x/{dir}/bundle.css")),
                "skip directory should match generated filter: {dir}"
            );
        }
        for path in [
            "/x/out/bundle.css",
            "/x/coverage/report.ts",
            "/x/generated/schema.ts",
            "/x/node_modules/pkg/index.js",
            "/x/__pycache__/cache.js",
            "/x/component.min.css",
            "/x/types.d.ts",
            "/x/tokens.generated.css",
            "/x/project.lock.json",
        ] {
            assert!(is_generated_path(path), "expected generated: {path}");
        }
        for path in [
            "/x/src/component.css",
            "/x/src/generated-utils.ts",
            "/x/src/building.css",
            "/x/locksmith.css",
        ] {
            assert!(!is_generated_path(path), "unexpected generated: {path}");
        }
    }
}
