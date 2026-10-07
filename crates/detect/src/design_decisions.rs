//! Project-wide detector decisions read from DESIGN.md (docs/adr/0020).
//!
//! Impeccino has no config file. What used to be detector ignores is
//! derived from the design document the project already keeps:
//!
//! - **Waivers.** An `<!-- impeccino-disable <rule-id> [-- reason] -->`
//!   comment anywhere in DESIGN.md turns that rule off for every file the
//!   document governs. Same syntax as the in-file waiver; only HTML comments
//!   outside code fences count, so prose that explains the syntax waives
//!   nothing, and a bare `impeccino-disable` (all rules) is not honored at
//!   project scope.
//! - **Declared values.** A font family DESIGN.md declares (frontmatter
//!   `typography`) is a deliberate choice, so
//!   `overused-font` does not fire for it.

use std::collections::HashMap;
use std::rc::Rc;

use impeccino_core::findings::Finding;
use impeccino_core::inline_ignores::parse_design_waivers;

use crate::config::{filter_detection_findings, DetectionConfig, IgnoreValueEntry};
use crate::design_system::{find_design_root, load_design_system_for_cwd, resolve_design_md_path};
use crate::util::read_text;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DesignDecisions {
    /// The DESIGN.md these decisions come from.
    pub source: Option<String>,
    /// Rule ids waived project-wide, lowercased, in document order.
    pub waived_rules: Vec<String>,
    /// Font families the design system declares, normalized like the
    /// detector's font values.
    pub declared_fonts: Vec<String>,
}

impl DesignDecisions {
    /// Decisions recorded in the DESIGN.md that lives in `dir` (or its
    /// `.agents/context` / `docs` fallbacks).
    pub fn load_for_dir(dir: &str) -> DesignDecisions {
        let Some(md) = resolve_design_md_path(dir) else {
            return DesignDecisions::default();
        };
        let text = read_text(&md.path).unwrap_or_default();
        let declared_fonts = load_design_system_for_cwd(dir)
            .map(|ds| ds.allowed_fonts.clone())
            .unwrap_or_default();
        DesignDecisions {
            source: Some(md.path),
            waived_rules: parse_design_waivers(&text),
            declared_fonts,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.waived_rules.is_empty() && self.declared_fonts.is_empty()
    }

    /// The decisions as a detector filter.
    pub fn to_detection_config(&self) -> DetectionConfig {
        DetectionConfig {
            ignore_rules: self.waived_rules.clone(),
            ignore_values: self
                .declared_fonts
                .iter()
                .map(|font| IgnoreValueEntry {
                    rule: "overused-font".to_string(),
                    value: font.clone(),
                })
                .collect(),
        }
    }

    /// Drop the findings these decisions settle.
    pub fn apply(&self, findings: Vec<Finding>) -> Vec<Finding> {
        if self.is_empty() {
            return findings;
        }
        filter_detection_findings(findings, &self.to_detection_config())
    }
}

/// Memo of decisions keyed by design root, for a scan over many files.
#[derive(Default)]
pub struct DesignDecisionsCache {
    by_root: HashMap<String, Rc<DesignDecisions>>,
}

impl DesignDecisionsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The decisions of the DESIGN.md that governs `start_dir` (the same
    /// design root the design-system rules resolve).
    pub fn for_dir(&mut self, start_dir: &str, cwd: &str, home: &str) -> Rc<DesignDecisions> {
        let key = match find_design_root(start_dir, cwd, home) {
            Some(root) if root.has_design => root.dir,
            _ => return Rc::new(DesignDecisions::default()),
        };
        self.by_root
            .entry(key.clone())
            .or_insert_with(|| Rc::new(DesignDecisions::load_for_dir(&key)))
            .clone()
    }
}
