//! impeccino-core: the rule logic of the impeccino detector engine, ported
//! from the JS `cli/engine` with byte-for-byte behavioral parity. The
//! `check_*` / `scan_*` functions and the heuristics behind them live here.
//!
//! Everything they are written against lives in `impeccino-foundation`: JS
//! number and string semantics, colour maths, the rule registry, inline
//! ignores, the DOM probe trait, and the plain-data input and output types.
//! This crate re-exports those modules for its own convenience, so
//! `crate::js`, `crate::color`, `crate::browser::dom` and friends keep
//! resolving inside it, and so consumers can name everything through
//! `impeccino_core::`. No filesystem, process, or network access lives here.

pub mod browser;
pub mod checks;

pub use impeccino_foundation::{
    color, constants, fdlibm_trig, findings, fonts, inline_ignores, js, js_ext_a, js_ext_b, page,
    registry, rule_pack,
};

#[cfg(any(test, feature = "vectors"))]
pub mod vectors;
