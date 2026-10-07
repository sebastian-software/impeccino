//! Shared rule predicates, CSS scanners and text checks.

pub mod css_scan;
pub mod html_patterns;
pub mod measures;
pub mod quality;
pub mod rules;
pub mod text_rules;

#[cfg(feature = "vectors")]
pub mod vectors_a;
#[cfg(feature = "vectors")]
pub mod vectors_b;
