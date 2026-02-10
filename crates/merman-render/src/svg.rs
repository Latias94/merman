//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from the parity-focused renderer implementation.
//!
//! Historically this lived under a `legacy` module name while we split a single large renderer
//! into smaller per-diagram units without changing output behavior. The name `parity` better
//! reflects the intent: upstream Mermaid is treated as the spec, and SVG output is gated by DOM
//! parity checks.

#![forbid(unsafe_code)]

mod legacy;

pub use legacy::*;

/// Public alias for the parity-focused renderer.
///
/// The underlying implementation is still organized under an internal `legacy` module while we
/// keep splitting it up without changing behavior, but `parity` better describes the intent.
pub mod parity {
    pub use super::legacy::*;
}
