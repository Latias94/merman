//! Syntax parsers for structured Mermaid diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-packet")]
pub mod packet;

#[cfg(feature = "diagram-railroad")]
pub mod railroad;
