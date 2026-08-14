//! Package identity for the Tree-sitter Mermaid language.
//!
//! The generated language entry point is added only after the deterministic ABI-14 pipeline is
//! proven. Until then this crate deliberately exposes identities without claiming a parser.

#![forbid(unsafe_code)]

pub use tree_sitter::Language;

/// Independently versioned language package release.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Tree-sitter language symbol used by generated bindings.
pub const LANGUAGE_SYMBOL: &str = "mermaid";
/// Tree-sitter generated language ABI selected by this package.
pub const LANGUAGE_ABI: u32 = 14;
/// Experimental public CST schema version.
pub const NODE_SCHEMA_VERSION: u32 = 1;
/// Experimental public query schema version.
pub const QUERY_SCHEMA_VERSION: u32 = 1;
/// Exact Rust Tree-sitter runtime line validated by the package.
pub const TREE_SITTER_RUST_RUNTIME_VERSION: &str = "0.26.12";
