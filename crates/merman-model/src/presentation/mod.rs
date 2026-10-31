//! Immutable presentation configuration and declared theme ownership.
//!
//! These values describe output data. They retain no source parser, detector journal, theme
//! evaluator, initialization cache, or operation control, and do not certify source history.

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
mod ownership;
mod paint;
mod recipe;
#[cfg(test)]
mod tests;
mod theme;
mod validation;

#[cfg(feature = "compat-json")]
pub use builtin::BuiltinRenderSemantic;
pub use config::{FrozenPresentationConfigParts, PalettePaintAuthority, PresentationConfig};
pub use context::PresentationContext;
pub use diagram::{CustomJsonProvenance, CustomJsonRenderModel, RenderSemanticModel};
pub use error::PresentationConfigError;
pub use input::{AuthoredPresentationMetadata, PresentationDataError, PresentationDiagram};
pub use metadata::PresentationMetadata;
pub use ownership::{
    ThemeCompatibilityConsumptionDisposition, ThemeCompatibilityEvidence, ThemeCompatibilityField,
    ThemeCompatibilityFieldConsumption, ThemeCompatibilityFieldKind,
    ThemeCompatibilityReconciliation,
};
pub use paint::{
    ErPaintInput, ErPaintInputs, FamilyPaintInputs, GitGraphPaintInput, GitGraphPaintInputs,
    PaintInputMaskError,
};
pub use recipe::{ThemeCompatibilityRecipe, ThemeCompatibilityRecipeError};

pub use theme::{MAX_THEME_COLOR_ITERATIONS, ThemeEvaluationLimitExceeded, theme_color_iterations};
