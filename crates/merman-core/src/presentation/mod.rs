//! Configuration and parsing that retain presentation authority.

pub use crate::config::presentation::PresentationConfig;

pub mod diagram;
pub(crate) mod engine;
pub use engine::{MermaidEngine, ParseOptions, PresentationMetadata};
