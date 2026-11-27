//! Rendering primitives for Mermaid graph diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-er")]
#[doc(hidden)]
pub mod er;

#[cfg(feature = "diagram-git-graph")]
#[doc(hidden)]
pub mod gitgraph;

#[cfg(feature = "diagram-requirement")]
#[doc(hidden)]
pub mod requirement;

#[cfg(feature = "diagram-state")]
#[doc(hidden)]
pub mod state;
