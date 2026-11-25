//! Syntax parsers for Mermaid graph diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-er")]
pub mod er;
#[cfg(feature = "diagram-state")]
pub mod state;
#[cfg(any(feature = "diagram-er", feature = "diagram-state"))]
use merman_core_preprocess::scan;
