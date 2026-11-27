//! Rendering primitives for Mermaid graph diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-c4")]
#[doc(hidden)]
pub mod c4;

#[cfg(feature = "diagram-class")]
#[doc(hidden)]
pub mod class;

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

#[cfg(all(test, any(feature = "diagram-class", feature = "diagram-usecase")))]
mod test_support;

#[cfg(feature = "diagram-usecase")]
#[doc(hidden)]
pub mod usecase;
