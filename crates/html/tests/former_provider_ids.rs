use impeccino_html::{detect_html_source, DetectHtmlOptions};
use std::path::Path;

#[test]
fn quality_checks_include_former_provider_owned_ids() {
    let html = r#"<html><body>
      <img id="claude-widget" src="hero.png" alt="Hero" style="opacity: 0">
      <img id="cic-widget" src="banner.png" alt="Banner" style="opacity: 0">
    </body></html>"#;

    let findings = detect_html_source(html, Path::new("page.html"), &DetectHtmlOptions::default());

    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding.antipattern == "buried-raster")
            .count(),
        2,
        "former provider ids should not exempt page elements: {findings:?}"
    );
}
