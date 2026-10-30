//! Configuration and parsing that retain presentation authority.

pub use crate::config::presentation::SourcePresentationConfig;

pub mod diagram;
pub(crate) mod engine;
pub use engine::{
    MermaidEngine, ParseOptions, SourcePresentationMetadata, ThemeCompatibilityPlan,
    ThemeCompatibilityPlanError,
};
