//! Syntax and semantic model construction for Mermaid sequence diagrams.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-sequence")]
pub mod sequence;

#[cfg(feature = "diagram-zenuml")]
#[doc(hidden)]
pub mod zenuml;
