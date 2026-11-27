//! Rendering primitives for Mermaid graph diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-er")]
#[doc(hidden)]
pub mod er;

#[cfg(feature = "diagram-git-graph")]
#[doc(hidden)]
pub mod gitgraph;
