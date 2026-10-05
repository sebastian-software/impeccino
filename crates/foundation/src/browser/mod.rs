//! Shared types for rendered-page checks: the [`dom::Dom`] interface, the
//! serialized-page snapshot and selector engine, test fake, and plain-data
//! inputs and outputs. The checks themselves live in `impeccino-core`.
//!
//! - `dom`: the [`dom::Dom`] trait, `ElId`, `Rect`, shared helpers.
//! - `snapshot`: [`snapshot::SnapshotDom`], the adapter over the page snapshot
//!   captured by the CLI; `selector`: the browser-flavored selector engine.
//! - `fake_dom`: a table-driven fake for unit tests (test builds only).
//! - `visual`: the plain-data plans and rects of the visual-contrast
//!   subsystem.

pub mod dom;
#[cfg(any(test, feature = "fake-dom"))]
pub mod fake_dom;

pub mod selector;
pub mod snapshot;
pub mod visual;

use serde::{Deserialize, Serialize};

pub use dom::{Dom, ElId, Rect};

/// The `{ type, detail, severity?, ignoreValue? }` shape page scans carry
/// (`checkElement*DOM(el).map(f => ({ type: f.id, detail: f.snippet }))`).
/// Field order matches the JS object literal so serialized JSON is byte-equal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserFinding {
    #[serde(rename = "type")]
    pub type_: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(
        default,
        rename = "ignoreValue",
        skip_serializing_if = "Option::is_none"
    )]
    pub ignore_value: Option<String>,
}

impl BrowserFinding {
    pub fn new(type_: impl Into<String>, detail: impl Into<String>) -> Self {
        BrowserFinding {
            type_: type_.into(),
            detail: detail.into(),
            severity: None,
            ignore_value: None,
        }
    }
    /// `{ type: f.id, detail: f.snippet }` from a Section 3 hit.
    pub fn from_hit(hit: &crate::rules::types::RuleHit) -> Self {
        BrowserFinding::new(hit.id.clone(), hit.snippet.clone())
    }
    /// `{ type: f.id, detail: f.snippet }` from a measures Finding.
    pub fn from_measure(f: &crate::css::measures::Finding) -> Self {
        BrowserFinding::new(f.id.clone(), f.snippet.clone())
    }
}

/// A finding attributed to an element (`{ el, type, detail }` from the
/// page-level checks that name their own target). `el == None` means "the
/// check attributes to document.body" (JS `f.el || document.body`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElFinding {
    pub el: Option<ElId>,
    pub finding: BrowserFinding,
}

/// One entry of the driver's group map: `{ el, findings }` in insertion order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingGroup {
    pub el: ElId,
    pub findings: Vec<BrowserFinding>,
}

/// Design tokens passed into the rendered-page checks.
#[derive(Debug, Clone, Default)]
pub struct DesignSystemConfig {
    pub declared_selectors: Vec<String>,
    pub has_fonts: bool,
    pub allowed_fonts: Vec<String>,
    pub has_colors: bool,
    pub allowed_colors: Vec<crate::color::Rgba>,
    pub has_radii: bool,
    pub allowed_radii: Vec<f64>,
    pub has_pill_radius: bool,
}

/// Options for measuring one rendered page.
#[derive(Debug, Clone, Default)]
pub struct BrowserConfig {
    pub design_system: Option<DesignSystemConfig>,
}

/// JS: checks.mjs#measureHiddenTextDOM() result.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenTextMeasure {
    #[serde(with = "crate::js::json_number")]
    pub total_chars: f64,
    #[serde(with = "crate::js::json_number")]
    pub hidden_chars: f64,
    pub hidden_samples: Vec<String>,
}

/// The result of `collectBrowserFindings()`: the group map in insertion
/// order and the page-level list (banner content).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectResult {
    pub groups: Vec<FindingGroup>,
    pub page_level: Vec<BrowserFinding>,
}
