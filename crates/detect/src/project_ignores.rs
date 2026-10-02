//! Which project files the detector leaves alone, read from the project's own
//! git metadata instead of a tool config (docs/adr/0020).
//!
//! - `.gitignore` files, at the repository root and in nested directories,
//!   plus `.git/info/exclude`. A file git ignores is not project source.
//! - `.gitattributes` entries that mark a path `linguist-generated` or
//!   `linguist-vendored`: generated and vendored files are checked in but
//!   not authored here, so their design findings belong to someone else.
//!
//! Both only apply inside a git repository (a directory with `.git`), the way
//! git itself reads them. The rules of every directory from the repository
//! root down to a path apply, deeper files first, last matching line wins.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;

use crate::jsp;

/// The parsed rules of one directory.
#[derive(Default)]
struct DirRules {
    ignore: Option<Gitignore>,
    generated: Option<Gitignore>,
}

/// A memo of per-directory git rules for one scan or one hook run.
#[derive(Default)]
pub struct ProjectIgnores {
    dirs: RefCell<HashMap<String, Rc<DirRules>>>,
    roots: RefCell<HashMap<String, Option<String>>>,
}

fn build(dir: &str, lines: &[String]) -> Option<Gitignore> {
    if lines.is_empty() {
        return None;
    }
    let mut b = GitignoreBuilder::new(dir);
    for line in lines {
        let _ = b.add_line(None, line);
    }
    b.build().ok().filter(|g| !g.is_empty())
}

fn read_lines(path: &str) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect(),
        Err(_) => Vec::new(),
    }
}

/// `.gitattributes` lines that set or unset `linguist-generated` /
/// `linguist-vendored`, rewritten as gitignore lines (unset becomes `!`), so
/// one matcher answers "is this path marked generated or vendored".
fn attribute_lines(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in read_lines(path) {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut parts = t.split_whitespace();
        let Some(pattern) = parts.next() else { continue };
        if pattern.starts_with('!') || pattern.starts_with('"') {
            // Negative patterns are not allowed in .gitattributes; quoted
            // patterns are rare enough to leave out.
            continue;
        }
        let mut state: Option<bool> = None;
        for attr in parts {
            let (name, set) = if let Some(n) = attr.strip_prefix('-') {
                (n, false)
            } else if let Some(n) = attr.strip_prefix('!') {
                (n, false)
            } else if let Some((n, v)) = attr.split_once('=') {
                (n, !matches!(v, "false" | "0"))
            } else {
                (attr, true)
            };
            if name == "linguist-generated" || name == "linguist-vendored" {
                // Either attribute set marks the path; an explicit unset of
                // one only clears what this line itself set.
                state = Some(state.unwrap_or(false) || set);
            }
        }
        match state {
            Some(true) => out.push(pattern.to_string()),
            Some(false) => out.push(format!("!{pattern}")),
            None => {}
        }
    }
    out
}

impl ProjectIgnores {
    pub fn new() -> Self {
        Self::default()
    }

    fn rules(&self, dir: &str, is_root: bool) -> Rc<DirRules> {
        if let Some(r) = self.dirs.borrow().get(dir) {
            return r.clone();
        }
        let mut ignore_lines = read_lines(&jsp::join(&[dir, ".gitignore"]));
        if is_root {
            ignore_lines.extend(read_lines(&jsp::join(&[dir, ".git", "info", "exclude"])));
        }
        let rules = Rc::new(DirRules {
            ignore: build(dir, &ignore_lines),
            generated: build(dir, &attribute_lines(&jsp::join(&[dir, ".gitattributes"]))),
        });
        self.dirs.borrow_mut().insert(dir.to_string(), rules.clone());
        rules
    }

    /// The repository root that contains `dir`, if any.
    fn repo_root(&self, dir: &str) -> Option<String> {
        if let Some(hit) = self.roots.borrow().get(dir) {
            return hit.clone();
        }
        let mut cur = dir.to_string();
        let found = loop {
            if Path::new(&jsp::join(&[&cur, ".git"])).exists() {
                break Some(cur.clone());
            }
            let parent = jsp::dirname(&cur);
            if parent == cur {
                break None;
            }
            cur = parent;
        };
        self.roots.borrow_mut().insert(dir.to_string(), found.clone());
        found
    }

    /// The directories whose rules apply to `path`, deepest first.
    fn chain(&self, path: &str) -> Vec<(String, bool)> {
        let dir = jsp::dirname(path);
        let Some(root) = self.repo_root(&dir) else { return vec![] };
        let mut out = Vec::new();
        let mut cur = dir;
        loop {
            let is_root = cur == root;
            out.push((cur.clone(), is_root));
            if is_root {
                break;
            }
            let parent = jsp::dirname(&cur);
            if parent == cur {
                break;
            }
            cur = parent;
        }
        out
    }

    /// True when git ignores `path` (absolute), or, for a file, when
    /// `.gitattributes` marks it generated or vendored. The `.git` directory
    /// itself is always skipped.
    pub fn is_skipped(&self, path: &str, is_dir: bool) -> bool {
        if jsp::basename(path) == ".git" {
            return true;
        }
        let chain = self.chain(path);
        if chain.is_empty() {
            return false;
        }
        let mut ignored: Option<bool> = None;
        let mut generated: Option<bool> = None;
        for (dir, is_root) in &chain {
            let rules = self.rules(dir, *is_root);
            if ignored.is_none() {
                if let Some(g) = &rules.ignore {
                    match g.matched_path_or_any_parents(path, is_dir) {
                        Match::Ignore(_) => ignored = Some(true),
                        Match::Whitelist(_) => ignored = Some(false),
                        Match::None => {}
                    }
                }
            }
            if !is_dir && generated.is_none() {
                if let Some(g) = &rules.generated {
                    match g.matched_path_or_any_parents(path, false) {
                        Match::Ignore(_) => generated = Some(true),
                        Match::Whitelist(_) => generated = Some(false),
                        Match::None => {}
                    }
                }
            }
            if ignored.is_some() && (is_dir || generated.is_some()) {
                break;
            }
        }
        ignored == Some(true) || generated == Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(String);
    impl Tmp {
        fn new(tag: &str) -> Tmp {
            static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let base = std::env::temp_dir().join(format!(
                "impeccino-ignores-{tag}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&base).unwrap();
            let real = std::fs::canonicalize(&base).unwrap().to_string_lossy().into_owned();
            Tmp(real.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(real))
        }
        fn write(&self, rel: &str, body: &str) -> String {
            let abs = jsp::join(&[&self.0, rel]);
            std::fs::create_dir_all(jsp::dirname(&abs)).unwrap();
            std::fs::write(&abs, body).unwrap();
            abs
        }
        fn path(&self, rel: &str) -> String {
            jsp::join(&[&self.0, rel])
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nested_gitignore_and_attributes_apply_inside_a_repo() {
        let t = Tmp::new("repo");
        std::fs::create_dir_all(t.path(".git/info")).unwrap();
        t.write(".gitignore", "out/\n*.gen.css\n!keep.gen.css\n");
        t.write(".git/info/exclude", "scratch.html\n");
        t.write("src/.gitignore", "local.css\n");
        t.write(".gitattributes", "src/vendor/** linguist-vendored\nsrc/api.css linguist-generated=true\nsrc/vendor/own.css -linguist-vendored\n");
        let p = ProjectIgnores::new();
        assert!(p.is_skipped(&t.path("out"), true));
        assert!(p.is_skipped(&t.path("out/a.css"), false));
        assert!(p.is_skipped(&t.path("src/a.gen.css"), false));
        assert!(!p.is_skipped(&t.path("src/keep.gen.css"), false));
        assert!(p.is_skipped(&t.path("scratch.html"), false));
        assert!(p.is_skipped(&t.path("src/local.css"), false));
        assert!(!p.is_skipped(&t.path("local.css"), false), "a nested .gitignore only covers its own dir");
        assert!(p.is_skipped(&t.path("src/vendor/lib.css"), false));
        assert!(!p.is_skipped(&t.path("src/vendor/own.css"), false));
        assert!(p.is_skipped(&t.path("src/api.css"), false));
        assert!(!p.is_skipped(&t.path("src/app.css"), false));
    }

    #[test]
    fn nothing_applies_outside_a_repo() {
        let t = Tmp::new("norepo");
        t.write(".gitignore", "*.css\n");
        let p = ProjectIgnores::new();
        assert!(!p.is_skipped(&t.path("a.css"), false));
    }
}
