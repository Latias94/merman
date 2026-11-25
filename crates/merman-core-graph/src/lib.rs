//! Syntax parsers for Mermaid graph diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-er")]
pub mod er;
#[cfg(feature = "diagram-er")]
use merman_core_preprocess::scan;
