//! Immutable presentation configuration and authored settings.

#[cfg(feature = "compat-json")]
mod builtin;
#[cfg(feature = "compat-json")]
pub mod compat;
mod config;
mod context;
mod diagram;
mod error;
mod input;
#[cfg(test)]
mod input_tests;
mod metadata;
#[cfg(test)]
mod tests;
mod theme;
mod validation;

#[cfg(feature = "compat-json")]
pub use builtin::BuiltinRenderSemantic;
pub use config::{FrozenPresentationConfigParts, PresentationConfig};
pub use context::PresentationContext;
pub use diagram::{CustomJsonProvenance, CustomJsonRenderModel, RenderSemanticModel};
pub use error::PresentationConfigError;
pub use input::{AuthoredPresentationMetadata, PresentationDataError, PresentationDiagram};
pub use metadata::PresentationMetadata;
pub use theme::{MAX_THEME_COLOR_ITERATIONS, ThemeEvaluationLimitExceeded, theme_color_iterations};
