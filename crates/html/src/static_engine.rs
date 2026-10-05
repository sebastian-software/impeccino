//! The `impeccino detect` adapter for the static HTML engine: implements
//! `impeccino_detect::engines::HtmlEngine` over [`crate::engine::detect_html`],
//! wiring the three pieces `detectHtml` borrows from other JS modules:
//!
//! - the design-system trio (`checkSourceDesignSystem` and
//!   `mergeDesignSystemFindings` from the `detect` crate, plus the DOM-backed
//!   `collectStaticDesignSystemFindings` ported here, JS
//!   `cli/engine/design-system.mjs`),
//! - `runTextContentAnalyzers` (regex engine, `detect` crate).
//!
//! Dependency direction: html depends on detect, never the reverse; the
//! `cli` binary registers [`StaticHtmlEngine`] in `Engines`.

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
        // unreadable-linked-stylesheet notices (issue #652).
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

/// JS: design-system.mjs#shouldSkipStaticDesignElement
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

/// JS: design-system.mjs#collectStaticDesignSystemFindings
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
}
