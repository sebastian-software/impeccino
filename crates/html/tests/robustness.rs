//! The engine must never panic on any input: truncated fixtures, garbage,
//! and pathological nesting.

use impeccino_html::cascade::csstree::parse_stylesheet;
use impeccino_html::select::Selector;
use impeccino_html::{detect_html_source, DetectHtmlOptions};
use std::path::{Path, PathBuf};

/// The repo root: this workspace is the public repo. `IMPECCINO_PUBLIC_REPO`
/// overrides it for an out-of-tree checkout.
fn repo_root() -> PathBuf {
    if let Ok(p) = std::env::var("IMPECCINO_PUBLIC_REPO") {
        return PathBuf::from(p);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> Option<PathBuf> {
    let p = repo_root().join("tests/fixtures/antipatterns");
    p.is_dir().then_some(p)
}

fn scan(html: &str) -> usize {
    let opts = DetectHtmlOptions::default();
    detect_html_source(html, Path::new("/nonexistent/dir/x.html"), &opts).len()
}

#[test]
fn truncated_fixtures_do_not_panic() {
    let Some(dir) = fixtures() else {
        eprintln!("fixtures not found; skipping");
        return;
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "html"))
        .collect();
    files.sort();
    for f in files.iter().take(12) {
        let src = String::from_utf8_lossy(&std::fs::read(f).unwrap()).into_owned();
        let n = src.len();
        let mut cut = 0usize;
        while cut < n {
            let mut end = cut;
            while end < n && !src.is_char_boundary(end) {
                end += 1;
            }
            let _ = scan(&src[..end]);
            cut += (n / 23).max(7);
        }
    }
}

#[test]
fn garbage_and_edge_inputs_do_not_panic() {
    let cases = [
        "",
        "<",
        "<!DOCTYPE html>",
        "<html",
        "<style>",
        "<style>a{color:</style>",
        "<style>:root{--a:var(--a)} p{color:var(--a)}</style><p>x</p>",
        "<div style=\"color\">x</div>",
        "<div style=\"background: ; border: 5px solid; font-size: em\">text longer than twenty chars</div>",
        "<template><p>t</p></template><table><tr><td>a</td></table>",
        "<link rel=stylesheet href=''><link rel=stylesheet href='?x'><link rel=stylesheet href='//cdn/x.css'>",
        "<p>\u{FEFF}\u{2028}\u{200B}\u{1F600}</p><h1 style='letter-spacing:-1em'>\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}</h1>",
        "<html><body data-impeccino-ignore><h1>x</h1><h3>y</h3></body></html>",
        "<svg><linearGradient id='g'></linearGradient><foreignObject><div>hi there</div></foreignObject></svg>",
        "<style>a::before{content:'';width:20px;height:2px;background:#f00} .x:hover{color:red} .y:focus{color:blue} @media(min-width:1px){.z{color:red}} @keyframes k{from{color:red}}</style><a>l</a>",
        "<img><img src=''><img src='#'>",
    ];
    for c in cases {
        let _ = scan(c);
    }
    let deep = format!(
        "<html><body>{}{}</body></html>",
        "<div style='padding:1px'>".repeat(800),
        "</div>".repeat(800)
    );
    let _ = scan(&deep);
    let wide = format!(
        "<html><body>{}</body></html>",
        "<p style='color:#777'>text of some length here</p>".repeat(1000)
    );
    let _ = scan(&wide);
    let noise: String = (0..4000u32)
        .map(|i| char::from_u32(0x20 + (i * 7919) % 0x2fff).unwrap_or(' '))
        .collect();
    let _ = scan(&format!(
        "<html><body><p>{noise}</p><style>{noise}</style></body></html>"
    ));
}

#[test]
fn deeply_nested_css_is_rejected_before_recursive_parsing() {
    let calc = format!(
        "a{{width:{}1px{}}}",
        "calc(".repeat(300),
        ")".repeat(300)
    );
    let media = format!(
        "{}a{{color:red}}{}",
        "@media all{".repeat(300),
        "}".repeat(300)
    );
    // The outer rule block contributes one level, so 63 calc() calls reach
    // the exact accepted depth of 64.
    let at_limit = format!(
        "a{{width:{}1px{}}}",
        "calc(".repeat(63),
        ")".repeat(63)
    );
    let over_limit = format!(
        "a{{width:{}1px{}}}",
        "calc(".repeat(64),
        ")".repeat(64)
    );
    let escaped = format!("a{}{{color:red;}}", r"\(".repeat(300));

    assert!(parse_stylesheet(&calc).is_err());
    assert!(parse_stylesheet(&media).is_err());
    assert!(parse_stylesheet(&at_limit).is_ok());
    assert!(parse_stylesheet(&over_limit).is_err());
    assert!(parse_stylesheet(&escaped).is_ok());
}

#[test]
fn original_twenty_thousand_calc_input_returns_without_aborting() {
    let html = format!(
        "<style>a{{width:{}1px{}}}</style><p>x</p>",
        "calc(".repeat(20_000),
        ")".repeat(20_000)
    );

    assert_eq!(scan(&html), 0);
}

#[test]
fn bad_string_newline_does_not_hide_deep_css_from_the_guard() {
    let calc = format!(
        "a{{content:\"\n;width:{}1px{}}}",
        "calc(".repeat(65),
        ")".repeat(65)
    );
    assert!(parse_stylesheet(&calc).is_err());

    let html = format!(
        "<style>a{{content:\"\n;width:{}1px{}}}</style><p>x</p>",
        "calc(".repeat(20_000),
        ")".repeat(20_000)
    );
    assert_eq!(scan(&html), 0);
}

#[test]
fn nesting_in_css_comments_and_strings_does_not_consume_the_depth_limit() {
    let inert = "([{".repeat(300);
    let css = format!(
        "/*{inert}*/ a::before {{ content: \"{inert}\"; color: red; }}"
    );
    let continued_string = format!(
        "a::before {{ content: \"continued\\\n{inert}\"; color: red; }}"
    );

    assert!(parse_stylesheet(&css).is_ok());
    assert!(parse_stylesheet(&continued_string).is_ok());
}

#[test]
fn deeply_nested_selector_is_rejected_before_selector_parsing() {
    let selector = format!(
        "{}x{}",
        ":not(".repeat(300),
        ")".repeat(300)
    );

    assert!(Selector::parse(&selector).is_err());

    let at_limit = format!("{}x{}", ":not(".repeat(64), ")".repeat(64));
    let over_limit = format!("{}x{}", ":not(".repeat(65), ")".repeat(65));
    assert!(Selector::parse(&at_limit).is_ok());
    assert!(Selector::parse(&over_limit).is_err());
}
