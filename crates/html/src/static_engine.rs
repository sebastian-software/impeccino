//! Static HTML engine adapter with governing design-system resolution.

use std::path::Path;

use impeccino_core::findings::Finding;
use impeccino_core::js;
use impeccino_detect::design_system::{
    check_source_design_system, make_design_finding, merge_design_system_findings, DesignSystem,
    STATIC_DESIGN_SKIP_TAGS,
};
use impeccino_detect::detect_text::run_text_content_analyzers;
use impeccino_detect::engines::{EngineError, HtmlEngine, ScanOptions};

use crate::background::sv;
use crate::dom::{StaticDocument, StaticElement};
use crate::engine::{detect_html, DesignSystemHook, DetectHtmlOptions};
use crate::quality::pf0;

/// The static HTML engine as `impeccino detect` sees it.
///
/// `static_rule_pack` is the rule pack's static-document hook, set by
/// whichever binary builds `Engines`; the `impeccino` binary leaves it
/// `None`. The pack's engine-wide hooks travel on `ScanOptions` instead,
/// because `detect` owns those options and cannot name this crate's trait.
#[derive(Debug, Default, Clone, Copy)]
pub struct StaticHtmlEngine {
    pub static_rule_pack: Option<&'static dyn crate::engine::StaticRulePack>,
}

impl HtmlEngine for StaticHtmlEngine {
    fn detect_html(
        &self,
        path: &str,
        options: &ScanOptions,
        stderr: &mut dyn std::io::Write,
    ) -> Result<Vec<Finding>, EngineError> {
        // The JS DEGRADED notice fires only when its parser modules fail to
        // import; the port links them in. The stderr sink carries the
        // unreadable-linked-stylesheet notices (issue pbakaus/impeccable#652).
        let stderr_cell = std::cell::RefCell::new(stderr);
        let warn = |msg: &str| {
            let _ = stderr_cell.borrow_mut().write_all(msg.as_bytes());
        };
        let analyzers = move |content: &str, file_path: &str| -> Vec<Finding> {
            run_text_content_analyzers(content, file_path)
        };
        let hook = options
            .design_system
            .as_deref()
            .map(|ds| DetectDesignSystemHook { design_system: ds });
        let html_options = DetectHtmlOptions {
            inline_ignores_disabled: !options.inline_ignores,
            design_system: hook.as_ref().map(|h| h as &dyn DesignSystemHook),
            text_content_analyzers: Some(&analyzers),
            warn: Some(&warn),
            static_rule_pack: self.static_rule_pack,
            rule_pack: options.rule_pack,
        };
        detect_html(Path::new(path), &html_options).map_err(|e| {
            EngineError::new(match e {
                // JS `fs.readFileSync` rejection surfaced by `detectCli`'s catch.
                crate::engine::HtmlEngineError::Read { path, source } => match source.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("ENOENT: no such file or directory, open '{path}'")
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        format!("EACCES: permission denied, open '{path}'")
                    }
                    _ => format!("{source}, open '{path}'"),
                },
            })
        })
    }
}

/// [`DesignSystemHook`] over a loaded `DesignSystem`.
pub struct DetectDesignSystemHook<'a> {
    pub design_system: &'a DesignSystem,
}

impl DesignSystemHook for DetectDesignSystemHook<'_> {
    fn check_source(&self, html: &str, file_path: &str) -> Vec<Finding> {
        check_source_design_system(html, file_path, Some(self.design_system))
    }

    fn collect_static(&self, doc: &StaticDocument, file_path: &str) -> Vec<Finding> {
        collect_static_design_system_findings(doc, file_path, self.design_system)
    }

    fn merge(&self, static_findings: Vec<Finding>, source_findings: Vec<Finding>) -> Vec<Finding> {
        merge_design_system_findings(vec![static_findings, source_findings])
    }
}

fn should_skip_static_design_element(el: &StaticElement<'_>) -> bool {
    let tag = el.tag_lower();
    if STATIC_DESIGN_SKIP_TAGS.contains(&tag.as_str()) {
        return true;
    }
    let mut current = Some(*el);
    while let Some(cur) = current {
        if cur.get_attribute("hidden").is_some() || cur.get_attribute("aria-hidden") == Some("true")
        {
            return true;
        }
        let style = cur.style();
        let display = js::to_lower_case(sv(style, "display"));
        let visibility = js::to_lower_case(sv(style, "visibility"));
        if display == "none" || visibility == "hidden" || visibility == "collapse" {
            return true;
        }
        current = cur.parent_element();
    }
    false
}

///
/// Font-size design-system checks are source-scan-only (see
/// `checkSourceDesignSystem`); computed font-size cascades and clamp() ramps
/// resolve to off-ramp px in the browser.
pub fn collect_static_design_system_findings(
    doc: &StaticDocument,
    file_path: &str,
    ds: &DesignSystem,
) -> Vec<Finding> {
    if !ds.present {
        return vec![];
    }
    let allowed_colors: Vec<_> = ds
        .allowed_color_keys
        .iter()
        .map(|(_, entry)| entry.color)
        .collect();
    let allowed_radii_px: Vec<_> = ds.allowed_radii.iter().map(|entry| entry.px).collect();
    let tokens = impeccino_core::design_system::DesignSystemTokens {
        has_fonts: ds.has_fonts,
        allowed_fonts: &ds.allowed_fonts,
        has_colors: ds.has_colors,
        allowed_colors: &allowed_colors,
        has_radii: ds.has_radii,
        allowed_radii_px: &allowed_radii_px,
        has_pill_radius: ds.has_pill_radius,
    };
    let mut seen = impeccino_core::design_system::DesignSystemSeen::default();
    let mut findings = Vec::new();

    for el in doc.query_selector_all("*") {
        if should_skip_static_design_element(&el) {
            continue;
        }
        let style = el.style();
        let computed = impeccino_core::design_system::ComputedElementStyle {
            tag: el.tag_lower(),
            sample_text: impeccino_core::design_system::sample_text(&el.text_content(), 40),
            has_direct_text: el.has_direct_text_longer_than(0),
            font_family: sv(style, "fontFamily").to_string(),
            color: sv(style, "color").to_string(),
            background_color: sv(style, "backgroundColor").to_string(),
            border_widths: ["Top", "Right", "Bottom", "Left"]
                .map(|side| pf0(sv(style, &format!("border{side}Width")))),
            border_colors: ["Top", "Right", "Bottom", "Left"]
                .map(|side| sv(style, &format!("border{side}Color")).to_string()),
            outline_width: pf0(sv(style, "outlineWidth")),
            outline_color: sv(style, "outlineColor").to_string(),
            border_radius: sv(style, "borderRadius").to_string(),
        };
        for finding in
            impeccino_core::design_system::check_computed_element(&computed, &tokens, &mut seen)
        {
            findings.push(make_design_finding(
                &finding.type_,
                file_path,
                &finding.detail,
                0.0,
                &finding.ignore_value,
            ));
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use impeccino_core::browser::driver::{check_element_design_system_dom, DesignSeen};
    use impeccino_core::browser::fake_dom::FakeDom;
    use impeccino_core::browser::DesignSystemConfig;

    #[test]
    fn computed_design_system_findings_match_across_static_and_rendered_dom() {
        let html = "<p style=\"font-family: 'A + B', sans-serif; color: rgb(200, 10, 10); border-radius: 3px 8px / 2px\">Example</p>";
        let mut doc = StaticDocument::parse(html);
        crate::cascade::build_static_style_map(&mut doc, "");

        let design = DesignSystem {
            present: true,
            has_fonts: true,
            allowed_fonts: vec!["inter".to_string()],
            has_colors: true,
            allowed_color_keys: vec![(
                "10,20,30".to_string(),
                impeccino_detect::design_system::AllowedColor {
                    color: impeccino_core::color::Rgba::new(10.0, 20.0, 30.0, 1.0),
                    labels: vec!["ink".to_string()],
                },
            )],
            has_radii: true,
            allowed_radii: vec![impeccino_detect::design_system::AllowedRadius {
                name: "medium".to_string(),
                value: "8px".to_string(),
                px: 8.0,
            }],
            ..DesignSystem::default()
        };
        let static_findings = collect_static_design_system_findings(&doc, "page.html", &design);
        let static_rules: Vec<_> = static_findings
            .iter()
            .filter_map(|finding| {
                Some((
                    finding.antipattern.as_str(),
                    finding.snippet.as_str(),
                    finding.extras.get("ignoreValue")?.as_str()?,
                ))
            })
            .collect();

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let p = dom.add(Some(body), "p");
        dom.add_text(p, "Example");
        dom.set_styles(
            p,
            &[
                ("fontFamily", "'A + B', sans-serif"),
                ("color", "rgb(200, 10, 10)"),
                ("borderRadius", "3px 8px / 2px"),
            ],
        );
        dom.el_mut(p).check_visibility = Some(true);
        let rendered_design = DesignSystemConfig {
            has_fonts: true,
            allowed_fonts: vec!["inter".to_string()],
            has_colors: true,
            allowed_colors: vec![impeccino_core::color::Rgba::new(10.0, 20.0, 30.0, 1.0)],
            has_radii: true,
            allowed_radii: vec![8.0],
            ..DesignSystemConfig::default()
        };
        let rendered_findings = check_element_design_system_dom(
            &dom,
            p,
            Some(&rendered_design),
            &mut DesignSeen::default(),
        );
        let rendered_rules: Vec<_> = rendered_findings
            .iter()
            .filter_map(|finding| {
                Some((
                    finding.type_.as_str(),
                    finding.detail.as_str(),
                    finding.ignore_value.as_deref()?,
                ))
            })
            .collect();

        assert_eq!(static_rules, rendered_rules);
        assert_eq!(
            static_rules,
            vec![
                (
                    "design-system-font",
                    "p \"Example\" uses a b; not declared in DESIGN.md typography",
                    "a b",
                ),
                (
                    "design-system-color",
                    "text color rgb(200, 10, 10) on p \"Example\" is outside DESIGN.md colors",
                    "rgb(200, 10, 10)",
                ),
                (
                    "design-system-radius",
                    "border-radius 3px on p \"Example\" is outside the DESIGN.md rounded scale",
                    "3px",
                ),
                (
                    "design-system-radius",
                    "border-radius 2px on p \"Example\" is outside the DESIGN.md rounded scale",
                    "2px",
                ),
            ]
        );
    }

    #[test]
    fn kicker_candidate_text_matches_across_static_and_rendered_dom() {
        let html = "<section><p style=\"font-size:12px; letter-spacing:1.2px; text-transform:uppercase; font-variant:normal; font-variant-caps:normal\"> Fea <span>ignored</span> tures </p><h2 style=\"font-size:32px\">Build better pages</h2></section>";
        let mut doc = StaticDocument::parse(html);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_candidates = crate::adapters::collect_kicker_candidates(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let section = dom.add(Some(body), "section");
        let kicker = dom.add(Some(section), "p");
        dom.add_text(kicker, " Fea ");
        let span = dom.add(Some(kicker), "span");
        dom.add_text(span, "ignored");
        dom.add_text(kicker, " tures ");
        dom.set_styles(
            kicker,
            &[
                ("fontSize", "12px"),
                ("letterSpacing", "1.2px"),
                ("textTransform", "uppercase"),
                ("fontVariant", "normal"),
                ("fontVariantCaps", "normal"),
            ],
        );
        let heading = dom.add(Some(section), "h2");
        dom.add_text(heading, "Build better pages");
        dom.set_style(heading, "fontSize", "32px");
        let rendered_candidates =
            impeccino_core::browser::text_collectors::collect_kicker_candidates(&dom);

        assert_eq!(static_candidates, rendered_candidates);
        assert_eq!(static_candidates.len(), 1);
        assert_eq!(static_candidates[0].kicker_text, "Fea tures");

        let static_hits = crate::adapters::check_kicker_above_heading_from_doc(&doc);
        let rendered_hits =
            impeccino_core::browser::text_collectors::check_kicker_above_heading_dom(&dom, None);
        let rendered_rules: Vec<_> = rendered_hits
            .iter()
            .map(|finding| {
                (
                    finding.finding.type_.as_str(),
                    finding.finding.detail.as_str(),
                )
            })
            .collect();
        let static_rules: Vec<_> = static_hits
            .iter()
            .map(|finding| (finding.id.as_str(), finding.snippet.as_str()))
            .collect();
        assert_eq!(static_rules, rendered_rules);
    }

    #[test]
    fn numbered_section_label_findings_match_across_static_and_rendered_dom() {
        let html = "<section><span style=\"font-size:11px; letter-spacing:1px; font-weight:700; font-family:monospace; text-transform:none; color:rgb(0, 0, 0)\">01</span><h2 style=\"font-size:28px\">Section number 1</h2></section><section><span style=\"font-size:11px; letter-spacing:1px; font-weight:700; font-family:monospace; text-transform:none; color:rgb(0, 0, 0)\">02</span><h2 style=\"font-size:28px\">Section number 2</h2></section>";
        let mut doc = StaticDocument::parse(html);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_findings = crate::adapters::check_numbered_section_labels_from_doc(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        for (index, label_text) in ["01", "02"].iter().enumerate() {
            let section = dom.add(Some(body), "section");
            let label = dom.add(Some(section), "span");
            dom.add_text(label, label_text);
            dom.set_styles(
                label,
                &[
                    ("fontSize", "11px"),
                    ("letterSpacing", "1px"),
                    ("fontWeight", "700"),
                    ("fontFamily", "monospace"),
                    ("textTransform", "none"),
                    ("color", "rgb(0, 0, 0)"),
                ],
            );
            let heading = dom.add(Some(section), "h2");
            dom.add_text(heading, &format!("Section number {}", index + 1));
            dom.set_style(heading, "fontSize", "28px");
        }
        let rendered_findings =
            impeccino_core::browser::text_collectors::check_numbered_section_labels_dom(&dom);
        let rendered_rules: Vec<_> = rendered_findings
            .iter()
            .map(|finding| (finding.id.as_str(), finding.snippet.as_str()))
            .collect();
        let static_rules: Vec<_> = static_findings
            .iter()
            .map(|finding| (finding.id.as_str(), finding.snippet.as_str()))
            .collect();

        assert_eq!(static_rules, rendered_rules);
        assert_eq!(
            static_rules,
            vec![
                (
                    "numbered-section-labels",
                    "tiny numbered label \"01\" beside h2 \"Section number 1\" (2 on page)"
                ),
                (
                    "numbered-section-labels",
                    "tiny numbered label \"02\" beside h2 \"Section number 2\" (2 on page)"
                ),
            ]
        );
    }

    #[test]
    fn layout_independent_quality_findings_match_across_static_and_rendered_dom() {
        struct Case {
            id: &'static str,
            tag: &'static str,
            text: &'static str,
            css: &'static str,
            browser_styles: &'static [(&'static str, &'static str)],
        }

        let cases = [
            Case {
                id: "tight-leading",
                tag: "p",
                text: "A long paragraph gives tight leading enough body copy to evaluate.",
                css: "font-size:16px; line-height:18px",
                browser_styles: &[("fontSize", "16px"), ("lineHeight", "18px")],
            },
            Case {
                id: "justified-text",
                tag: "p",
                text: "Justified body copy without automatic hyphenation.",
                css: "text-align:justify; hyphens:manual",
                browser_styles: &[("textAlign", "justify"), ("hyphens", "manual")],
            },
            Case {
                id: "tiny-text",
                tag: "p",
                text: "Small body text should be reported consistently.",
                css: "font-size:10px",
                browser_styles: &[("fontSize", "10px")],
            },
            Case {
                id: "undersized-ui-text",
                tag: "button",
                text: "Continue",
                css: "font-size:10px",
                browser_styles: &[("fontSize", "10px")],
            },
            Case {
                id: "all-caps-body",
                tag: "p",
                text: "THIS BODY COPY IS ALL CAPS AND LONG ENOUGH TO FLAG",
                css: "text-transform:uppercase",
                browser_styles: &[("textTransform", "uppercase")],
            },
            Case {
                id: "wide-tracking",
                tag: "p",
                text: "Tracked body copy has enough characters for the rule.",
                css: "letter-spacing:1.2px",
                browser_styles: &[("letterSpacing", "1.2px")],
            },
            Case {
                id: "extreme-negative-tracking",
                tag: "p",
                text: "Crushed body copy has enough characters for the rule.",
                css: "letter-spacing:-1px",
                browser_styles: &[("letterSpacing", "-1px")],
            },
            Case {
                id: "buried-raster",
                tag: "div",
                text: "Cover artwork",
                css: "opacity:0.1; background-image:url(texture.png)",
                browser_styles: &[("opacity", "0.1"), ("backgroundImage", "url(texture.png)")],
            },
        ];

        for case in cases {
            let html = format!(
                "<{} id=\"target\" style=\"{}\">{}</{}>",
                case.tag, case.css, case.text, case.tag
            );
            let mut doc = StaticDocument::parse(&html);
            crate::cascade::build_static_style_map(&mut doc, "");
            let static_element = doc.query_selector("#target").unwrap();
            let static_hits = crate::quality::check_element_quality(
                &static_element,
                static_element.style(),
                &static_element.tag_lower(),
            );

            let mut dom = FakeDom::new();
            let (_html, body) = dom.with_page();
            let rendered_element = dom.add(Some(body), case.tag);
            dom.add_text(rendered_element, case.text);
            dom.set_attr(rendered_element, "id", "target");
            dom.set_styles(rendered_element, case.browser_styles);
            let rendered_hits =
                impeccino_core::browser::quality::check_element_quality_dom(&dom, rendered_element);

            assert_eq!(
                static_hits, rendered_hits,
                "{} differs for static and rendered DOM",
                case.id
            );
            assert!(
                static_hits.iter().any(|hit| hit.id == case.id),
                "{} should be reported: {static_hits:?}",
                case.id
            );
        }
    }

    #[test]
    fn skipped_heading_findings_match_across_static_and_rendered_dom() {
        let html = "<h1>Overview</h1><h3>Details</h3>";
        let doc = StaticDocument::parse(html);
        let static_hits = crate::quality::check_page_quality_from_doc(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let h1 = dom.add(Some(body), "h1");
        dom.add_text(h1, "Overview");
        let h3 = dom.add(Some(body), "h3");
        dom.add_text(h3, "Details");
        let rendered_hits = impeccino_core::browser::quality::check_page_quality_from_doc(&dom);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(
            static_hits,
            vec![impeccino_core::checks::rules::RuleHit::new(
                "skipped-heading",
                "<h1> \"Overview\" followed by <h3> \"Details\" (missing h2)".to_string(),
            )]
        );
    }

    #[test]
    fn repeated_text_visibility_matches_across_static_and_rendered_dom() {
        let html = "<section id=\"cards\" class=\"cards\" style=\"box-shadow:0 1px 3px rgba(0,0,0,.2); border-radius:8px\"><div class=\"first\">Shared label</div><div class=\"second\">Shared label</div><div class=\"hidden\" style=\"opacity:0\">Shared label</div></section>";
        let mut doc = StaticDocument::parse(html);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_hits = crate::page::check_repeated_container_text_from_doc(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let cards = dom.add(Some(body), "section");
        dom.set_attr(cards, "id", "cards");
        dom.set_attr(cards, "class", "cards");
        dom.set_styles(
            cards,
            &[
                ("boxShadow", "0 1px 3px rgba(0,0,0,.2)"),
                ("borderRadius", "8px"),
            ],
        );
        for (class_name, hidden) in [("first", false), ("second", false), ("hidden", true)] {
            let item = dom.add(Some(cards), "div");
            dom.set_attr(item, "class", class_name);
            if hidden {
                dom.set_style(item, "opacity", "0");
            }
            dom.add_text(item, "Shared label");
        }
        let rendered_hits =
            impeccino_core::browser::text_collectors::check_repeated_container_text_dom(&dom);

        assert_eq!(static_hits, rendered_hits);
        assert!(static_hits.is_empty());
    }

    #[test]
    fn repeated_container_text_findings_match_across_static_and_rendered_dom() {
        let html = "<section id=\"cards\" class=\"cards\" style=\"box-shadow:0 1px 3px rgba(0,0,0,.2); border-radius:8px\"><p class=\"item one\">Shared navigation</p><p class=\"item two\">Shared navigation</p><p class=\"item three\">Shared navigation</p></section>";
        let mut doc = StaticDocument::parse(html);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_hits = crate::page::check_repeated_container_text_from_doc(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let cards = dom.add(Some(body), "section");
        dom.set_attr(cards, "id", "cards");
        dom.set_attr(cards, "class", "cards");
        dom.set_styles(
            cards,
            &[
                ("boxShadow", "0 1px 3px rgba(0,0,0,.2)"),
                ("borderRadius", "8px"),
            ],
        );
        for class_name in ["item one", "item two", "item three"] {
            let item = dom.add(Some(cards), "p");
            dom.set_attr(item, "class", class_name);
            dom.add_text(item, "Shared navigation");
        }
        let rendered_hits =
            impeccino_core::browser::text_collectors::check_repeated_container_text_dom(&dom);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(
            static_hits,
            vec![impeccino_core::checks::rules::RuleHit::new(
                "repeated-container-text",
                "\"Shared navigation\" rendered 3× in distinct spots inside section.cards"
                    .to_string(),
            )]
        );
    }

    #[test]
    fn em_dash_page_findings_match_for_shared_visible_text() {
        let html_source = "<html><body>a — b — c — d — e — f — g — h — i</body></html>";
        let static_hits =
            impeccino_detect::detect_text::run_text_content_analyzers(html_source, "fixture.html");
        let static_rules: Vec<_> = static_hits
            .iter()
            .filter(|finding| finding.antipattern == "em-dash-overuse")
            .map(|finding| (finding.antipattern.as_str(), finding.snippet.as_str()))
            .collect();

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        dom.add_text(body, "a — b — c — d — e — f — g — h — i");
        let rendered_hits =
            impeccino_core::browser::text_collectors::check_em_dash_overuse_dom(&dom);
        let rendered_rules: Vec<_> = rendered_hits
            .iter()
            .map(|finding| (finding.id.as_str(), finding.snippet.as_str()))
            .collect();

        assert_eq!(static_rules, rendered_rules);
        assert_eq!(static_rules.len(), 1);
    }

    #[test]
    fn typography_page_findings_match_across_static_and_rendered_dom() {
        let paragraphs = (0..20)
            .map(|index| {
                format!(
                    "<p style=\"font-family:Inter, sans-serif; font-size:16px\">Body copy {index}</p>"
                )
            })
            .collect::<String>();
        let html_source = format!(
            "<html><body>{paragraphs}<h2 style=\"font-family:Inter, sans-serif; font-size:17px\">Subheading</h2><h1 style=\"font-family:Inter, sans-serif; font-size:18px\">Heading</h1></body></html>"
        );
        let mut doc = StaticDocument::parse(&html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_hits = crate::page::check_static_page_typography(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        for index in 0..20 {
            let paragraph = dom.add(Some(body), "p");
            dom.add_text(paragraph, &format!("Body copy {index}"));
            dom.set_styles(
                paragraph,
                &[("fontFamily", "Inter, sans-serif"), ("fontSize", "16px")],
            );
        }
        let h2 = dom.add(Some(body), "h2");
        dom.add_text(h2, "Subheading");
        dom.set_styles(
            h2,
            &[("fontFamily", "Inter, sans-serif"), ("fontSize", "17px")],
        );
        let h1 = dom.add(Some(body), "h1");
        dom.add_text(h1, "Heading");
        dom.set_styles(
            h1,
            &[("fontFamily", "Inter, sans-serif"), ("fontSize", "18px")],
        );
        let rendered_hits = impeccino_core::browser::page_checks::check_typography(&dom);
        let rendered_rules: Vec<_> = rendered_hits
            .iter()
            .map(|finding| (finding.type_.as_str(), finding.detail.as_str()))
            .collect();
        let static_rules: Vec<_> = static_hits
            .iter()
            .map(|finding| (finding.id.as_str(), finding.snippet.as_str()))
            .collect();

        assert_eq!(static_rules, rendered_rules);
        assert_eq!(static_rules.len(), 2);
        assert_eq!(static_rules[0].0, "overused-font");
        assert_eq!(static_rules[1].0, "flat-type-hierarchy");
    }

    #[test]
    fn content_visibility_hidden_typography_matches_across_static_and_rendered_dom() {
        let html_source = "<html><body><p style=\"font-size:16px\">Body copy</p><h2 style=\"font-size:16px\">Visible subheading</h2><h1 style=\"font-size:18px;content-visibility:hidden\">Skipped heading</h1></body></html>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_hits = crate::page::check_static_page_typography(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        for (tag, text, font_size, content_visibility) in [
            ("p", "Body copy", "16px", "visible"),
            ("h2", "Visible subheading", "16px", "visible"),
            ("h1", "Skipped heading", "18px", "hidden"),
        ] {
            let element = dom.add(Some(body), tag);
            dom.add_text(element, text);
            dom.set_styles(
                element,
                &[
                    ("fontFamily", "Inter, sans-serif"),
                    ("fontSize", font_size),
                    ("contentVisibility", content_visibility),
                ],
            );
        }
        let rendered_hits = impeccino_core::browser::page_checks::check_typography(&dom);
        let static_rules: Vec<_> = static_hits
            .iter()
            .map(|finding| (finding.id.as_str(), finding.snippet.as_str()))
            .collect();
        let rendered_rules: Vec<_> = rendered_hits
            .iter()
            .map(|finding| (finding.type_.as_str(), finding.detail.as_str()))
            .collect();

        assert_eq!(static_rules, rendered_rules);
        assert!(!static_rules
            .iter()
            .any(|(id, _)| *id == "flat-type-hierarchy"));
    }

    #[test]
    fn cream_palette_findings_match_across_static_and_rendered_dom() {
        let html_source =
            "<html><body style=\"background-color:rgb(245, 239, 220)\">Warm page</body></html>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let static_hits = crate::page::check_cream_palette(&doc);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        dom.set_style(body, "backgroundColor", "rgb(245, 239, 220)");
        dom.add_text(body, "Warm page");
        let rendered_hits = impeccino_core::browser::page_checks::check_cream_palette(&dom);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(
            static_hits,
            vec![impeccino_core::checks::rules::RuleHit::new(
                "cream-palette",
                "cream/beige page background rgb(245, 239, 220)".to_string(),
            )]
        );
    }

    #[test]
    fn tab_and_status_context_predicates_match_across_static_and_rendered_dom() {
        for (class_name, role, expected_tab, expected_status) in [
            ("active", "", true, false),
            ("ſelected", "", false, false),
            ("", "alert", false, true),
        ] {
            let role_attribute = if role.is_empty() {
                String::new()
            } else {
                format!(" role=\"{role}\"")
            };
            let html_source = format!(
                "<div class=\"{class_name}\"{role_attribute}><button id=\"target\">Action</button></div>"
            );
            let doc = StaticDocument::parse(&html_source);
            let target = doc.query_selector("#target").unwrap();

            let mut dom = FakeDom::new();
            let (_html, body) = dom.with_page();
            let wrapper = dom.add(Some(body), "div");
            dom.set_attr(wrapper, "class", class_name);
            if !role.is_empty() {
                dom.set_attr(wrapper, "role", role);
                dom.add_selector(
                    wrapper,
                    impeccino_core::checks::text_rules::STATUS_CONTEXT_SELECTOR,
                );
            }
            let rendered_target = dom.add(Some(wrapper), "button");
            dom.add_text(rendered_target, "Action");

            assert_eq!(
                crate::adapters::is_tab_context_element(&target),
                impeccino_core::browser::element_checks::is_tab_context_element(
                    &dom,
                    rendered_target
                )
            );
            assert_eq!(
                crate::adapters::is_status_context_element(&target),
                impeccino_core::browser::element_checks::is_status_context_element(
                    &dom,
                    rendered_target
                ),
                "class {class_name:?}, role {role:?}"
            );
            assert_eq!(
                crate::adapters::is_tab_context_element(&target),
                expected_tab
            );
            assert_eq!(
                crate::adapters::is_status_context_element(&target),
                expected_status
            );
        }
    }

    #[test]
    fn motion_class_tokens_match_across_static_and_rendered_dom() {
        let html_source = "<div id=\"target\" class=\"animate-bounce\"></div>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let target = doc.query_selector("#target").unwrap();
        let static_hits = crate::adapters::check_element_motion(
            &target.tag_lower(),
            target.class_name(),
            target.style(),
        );

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let target = dom.add(Some(body), "div");
        dom.set_attr(target, "class", "animate-bounce");
        let rendered_hits =
            impeccino_core::browser::element_checks::check_element_motion_dom(&dom, target);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(
            static_hits,
            vec![impeccino_core::checks::rules::RuleHit::new(
                "bounce-easing",
                "animate-bounce (Tailwind)".to_string(),
            )]
        );
    }

    #[test]
    fn visible_border_and_color_findings_match_across_static_and_rendered_dom() {
        let html_source = "<div id=\"border\" style=\"border-left-width:3px;border-left-color:rgb(255,0,0)\"></div><p id=\"color\" style=\"font-size:16px;font-weight:400;color:rgb(200,200,200);background-color:rgb(255,255,255)\">Readable body copy</p>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let border = doc.query_selector("#border").unwrap();
        let border_style = border.style();
        let static_border_hits = crate::adapters::check_element_borders(
            &border.tag_lower(),
            border_style,
            crate::background::resolve_border_radius_px(
                border_style,
                crate::quality::pf0(crate::background::sv(border_style, "width")),
            ),
            &border,
        );
        let color = doc.query_selector("#color").unwrap();
        let static_color_hits =
            crate::adapters::check_element_colors(&color, color.style(), &color.tag_lower(), None);

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let rendered_border = dom.add(Some(body), "div");
        dom.set_rect(rendered_border, 0.0, 0.0, 300.0, 80.0);
        dom.set_styles(
            rendered_border,
            &[
                ("borderLeftWidth", "3px"),
                ("borderLeftColor", "rgb(255, 0, 0)"),
            ],
        );
        let rendered_border_hits =
            impeccino_core::browser::element_checks::check_element_borders_dom(
                &dom,
                rendered_border,
            );
        let rendered_color = dom.add(Some(body), "p");
        dom.add_text(rendered_color, "Readable body copy");
        dom.set_rect(rendered_color, 0.0, 100.0, 300.0, 32.0);
        dom.set_styles(
            rendered_color,
            &[
                ("fontSize", "16px"),
                ("fontWeight", "400"),
                ("color", "rgb(200, 200, 200)"),
                ("backgroundColor", "rgb(255, 255, 255)"),
            ],
        );
        let rendered_color_hits =
            impeccino_core::browser::element_checks::check_element_colors_dom(&dom, rendered_color);

        assert_eq!(static_border_hits, rendered_border_hits);
        assert_eq!(static_color_hits, rendered_color_hits);
        assert_eq!(static_border_hits.len(), 1);
        assert_eq!(static_border_hits[0].id, "side-tab");
        assert!(static_color_hits
            .iter()
            .any(|finding| finding.id == "low-contrast"));
    }

    #[test]
    fn dark_glow_findings_match_across_static_and_rendered_dom() {
        let html_source = "<body style=\"background-color:rgb(0, 0, 0)\"><div id=\"target\" style=\"box-shadow:rgb(59, 130, 246) 0px 4px 20px 0px\"></div></body>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let target = doc.query_selector("#target").unwrap();
        let parent = target.parent_element().unwrap_or(target);
        let static_hits = crate::adapters::check_element_glow(
            target.style(),
            crate::background::resolve_background(&parent, None),
        );

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        dom.set_style(body, "backgroundColor", "rgb(0, 0, 0)");
        let target = dom.add(Some(body), "div");
        dom.set_style(target, "boxShadow", "rgb(59, 130, 246) 0px 4px 20px 0px");
        let rendered_hits =
            impeccino_core::browser::element_checks::check_element_glow_dom(&dom, target);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(static_hits.len(), 1);
        assert_eq!(static_hits[0].id, "dark-glow");
    }

    #[test]
    fn hero_eyebrow_findings_match_across_static_and_rendered_dom() {
        let html_source = "<p style=\"font-size:12px;letter-spacing:2px;text-transform:uppercase\">FEATURES</p><h1 id=\"target\" style=\"font-size:48px\">Build better interfaces with care</h1>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let target = doc.query_selector("#target").unwrap();
        let static_hits = crate::adapters::check_element_hero_eyebrow(
            &target,
            target.style(),
            &target.tag_lower(),
        );

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let eyebrow = dom.add(Some(body), "p");
        dom.add_text(eyebrow, "FEATURES");
        dom.set_styles(
            eyebrow,
            &[
                ("fontSize", "12px"),
                ("letterSpacing", "2px"),
                ("textTransform", "uppercase"),
            ],
        );
        let target = dom.add(Some(body), "h1");
        dom.add_text(target, "Build better interfaces with care");
        dom.set_style(target, "fontSize", "48px");
        let rendered_hits =
            impeccino_core::browser::element_checks::check_element_hero_eyebrow_dom(&dom, target);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(static_hits.len(), 1);
        assert_eq!(static_hits[0].id, "hero-eyebrow-chip");
    }

    #[test]
    fn thin_border_wide_shadow_findings_match_across_static_and_rendered_dom() {
        let html_source = "<div id=\"target\" style=\"border-top-width:1px;border-bottom-width:1px;border-top-color:rgb(0,0,0);border-bottom-color:rgb(0,0,0);box-shadow:0 0 24px rgba(0,0,0,.2)\"></div>";
        let mut doc = StaticDocument::parse(html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let target = doc.query_selector("#target").unwrap();
        let static_hits = crate::adapters::check_element_gpt_border_shadow(target.style());

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let target = dom.add(Some(body), "div");
        dom.set_styles(
            target,
            &[
                ("borderTopWidth", "1px"),
                ("borderBottomWidth", "1px"),
                ("borderTopColor", "rgb(0, 0, 0)"),
                ("borderBottomColor", "rgb(0, 0, 0)"),
                ("boxShadow", "0 0 24px rgba(0,0,0,.2)"),
            ],
        );
        let rendered_hits =
            impeccino_core::browser::element_checks::check_element_gpt_border_shadow_dom(
                &dom, target,
            );

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(static_hits.len(), 1);
        assert_eq!(static_hits[0].id, "gpt-thin-border-wide-shadow");
    }

    #[test]
    fn italic_heading_snippets_match_at_a_split_utf16_surrogate() {
        let heading_text = format!("{}😀tail", "x".repeat(59));
        let html_source = format!(
            "<h1 id=\"target\" style=\"font-style:italic;font-family:Georgia,serif;font-size:48px\">{heading_text}</h1>"
        );
        let mut doc = StaticDocument::parse(&html_source);
        crate::cascade::build_static_style_map(&mut doc, "");
        let target = doc.query_selector("#target").unwrap();
        let static_hits = crate::adapters::check_element_italic_serif(
            &target,
            target.style(),
            &target.tag_lower(),
        );

        let mut dom = FakeDom::new();
        let (_html, body) = dom.with_page();
        let target = dom.add(Some(body), "h1");
        dom.add_text(target, &heading_text);
        dom.set_styles(
            target,
            &[
                ("fontStyle", "italic"),
                ("fontFamily", "Georgia, serif"),
                ("fontSize", "48px"),
            ],
        );
        let rendered_hits =
            impeccino_core::browser::element_checks::check_element_italic_serif_dom(&dom, target);

        assert_eq!(static_hits, rendered_hits);
        assert_eq!(static_hits.len(), 1);
        assert!(static_hits[0]
            .snippet
            .contains(&format!("{}�", "x".repeat(59))));
    }
}
