//! Rendered-page rules over the shared [`dom::Dom`] interface.
//! The CLI captures measurements through agent-browser and supplies a snapshot.
//!
//! Shared DOM types, snapshots, selectors and test fakes live in foundation.
//! `background` resolves painted surfaces; `element_checks` and `page_checks`
//! run element and layout rules; `quality` and `text_collectors` adapt shared
//! text predicates; `visual` plans contrast samples; `driver` groups findings.
//! Number and string compatibility helpers keep the frozen vectors stable.

pub use impeccino_foundation::browser::dom;
pub use impeccino_foundation::browser::selector;

#[cfg(any(test, feature = "fake-dom"))]
pub use impeccino_foundation::browser::fake_dom;

pub mod background;
pub mod driver;
pub mod element_checks;
pub mod page_checks;
pub mod quality;
pub mod snapshot;
pub mod text_collectors;
pub mod visual;

pub use dom::{Dom, ElId, Rect};
pub use impeccino_foundation::browser::{
    BrowserConfig, BrowserFinding, DesignSystemConfig, ElFinding, FindingGroup,
};
