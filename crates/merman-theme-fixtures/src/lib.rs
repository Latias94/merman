//! Validated, source-backed evidence for portable diagram theme development.
//!
//! This publish-false crate owns one fixed reference corpus. Its public values are validated,
//! immutable projections; JSON wire records remain private to the loader.

mod catalog;
mod corpus;
mod error;
mod evidence;
mod io;
mod model;
mod source_compatibility;
mod theme_input;
mod wire;

pub use catalog::ThemeFixtureCatalog;
pub use error::CatalogError;
pub use model::*;
