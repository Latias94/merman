//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from `legacy` for now while we incrementally split the module into
//! smaller per-diagram units without changing behavior.

#![forbid(unsafe_code)]

mod legacy;

pub use legacy::*;
