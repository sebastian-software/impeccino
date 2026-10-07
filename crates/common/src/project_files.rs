//! Where Impeccino keeps its state (docs/adr/0020-project-state-is-top-level-files.md).
//!
//! Project state is PRODUCT.md, DESIGN.md (including detector metadata),
//! and SURFACES.md (surface briefs). There is no config file: project-wide detector
//! decisions live in DESIGN.md, and the project's own ignore rules keep
//! files out. Runtime state nobody reads as a document (the hook's session
//! cache, the staleness throttle, the launcher's engine cache) lives in a
//! per-user cache directory instead of the project.

/// Every persisted surface brief, one section per surface.
pub const SURFACES_FILE: &str = "SURFACES.md";
/// Legacy detector metadata next to DESIGN.md; read until explicitly migrated.
pub const DESIGN_SIDECAR_FILE: &str = "DESIGN.json";
/// The retired project-state directory. Nothing reads it; boot and doctor
/// report a leftover one so the user can move its files by hand.
pub const LEGACY_STATE_DIR: &str = ".impeccino";
/// Product/design documents recognized at a project or workspace root.
pub const PRODUCT_NAMES: [&str; 3] = ["PRODUCT.md", "Product.md", "product.md"];
pub const DESIGN_NAMES: [&str; 3] = ["DESIGN.md", "Design.md", "design.md"];
/// Fallback directories searched for project context documents.
pub const CONTEXT_FALLBACK_DIRS: [&str; 2] = [".agents/context", "docs"];

/// Host home precedence, with empty values treated as unset.
pub fn home_dir(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    let (first, second) = if cfg!(windows) {
        ("USERPROFILE", "HOME")
    } else {
        ("HOME", "USERPROFILE")
    };
    get(first)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| get(second).filter(|value| !value.trim().is_empty()))
}

/// The per-user cache directory, `<cache>/impeccino`:
///
/// - Windows: `%LOCALAPPDATA%\impeccino`, else
///   `%USERPROFILE%\AppData\Local\impeccino`.
/// - Everywhere else: `$XDG_CACHE_HOME/impeccino` when that is an absolute
///   path (the XDG spec ignores a relative one), else `~/.cache/impeccino`.
///
/// `get` looks up an environment variable; empty values count as unset.
/// `None` when no home directory can be determined.
pub fn user_cache_dir(get: impl Fn(&str) -> Option<String>) -> Option<String> {
    let var = |k: &str| get(k).filter(|v| !v.trim().is_empty());
    if cfg!(windows) {
        if let Some(local) = var("LOCALAPPDATA") {
            return Some(crate::jsp::join(&[&local, "impeccino"]));
        }
        let home = home_dir(&get)?;
        return Some(crate::jsp::join(&[&home, "AppData", "Local", "impeccino"]));
    }
    if let Some(xdg) = var("XDG_CACHE_HOME") {
        if crate::jsp::is_absolute(&xdg) {
            return Some(crate::jsp::join(&[&xdg, "impeccino"]));
        }
    }
    let home = home_dir(&get)?;
    Some(crate::jsp::join(&[&home, ".cache", "impeccino"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    #[cfg(not(windows))]
    #[test]
    fn xdg_cache_home_wins_when_absolute() {
        assert_eq!(
            user_cache_dir(lookup(&[("XDG_CACHE_HOME", "/xdg"), ("HOME", "/home/u")])).as_deref(),
            Some("/xdg/impeccino")
        );
        assert_eq!(
            user_cache_dir(lookup(&[
                ("XDG_CACHE_HOME", "relative"),
                ("HOME", "/home/u")
            ]))
            .as_deref(),
            Some("/home/u/.cache/impeccino")
        );
        assert_eq!(
            user_cache_dir(lookup(&[("XDG_CACHE_HOME", ""), ("HOME", "/home/u")])).as_deref(),
            Some("/home/u/.cache/impeccino")
        );
        assert_eq!(user_cache_dir(lookup(&[])), None);
    }

    #[cfg(windows)]
    #[test]
    fn local_app_data_wins_on_windows() {
        assert_eq!(
            user_cache_dir(lookup(&[
                ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
                ("USERPROFILE", r"C:\Users\u")
            ]))
            .as_deref(),
            Some(r"C:\Users\u\AppData\Local\impeccino")
        );
        assert_eq!(
            user_cache_dir(lookup(&[("USERPROFILE", r"C:\Users\u")])).as_deref(),
            Some(r"C:\Users\u\AppData\Local\impeccino")
        );
    }
}
