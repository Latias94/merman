//! Syntax parsers for Mermaid graph diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-class")]
pub mod class;
#[cfg(feature = "diagram-er")]
pub mod er;
#[cfg(feature = "diagram-git-graph")]
#[doc(hidden)]
pub mod git_graph;
#[cfg(feature = "diagram-state")]
pub mod state;
#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-state"
))]
use merman_core_preprocess::scan;
