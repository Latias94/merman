#![forbid(unsafe_code)]
//! Shared Mermaid markup safety, URL normalization, CSS structure, and color operations.
//!
//! Source ingestion and output owners use the same algorithms and retain their own admission,
//! operation budgets, and error routing. This package does not parse Mermaid source or render SVG.

#[cfg(feature = "html")]
mod generated;
#[cfg(feature = "html")]
pub mod sanitize;
pub mod style;
#[cfg(feature = "color")]
pub mod theme_color;
#[cfg(feature = "url")]
pub mod url;
