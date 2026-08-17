//! Versioned, target-independent visual diagram-theme authoring.
//!
//! This is an alpha design-validation surface. [`ThemeMaterializer`] expands a compact
//! [`ThemeDefinitionV1`] into one complete wire recipe; [`DiagramThemeCompiler`] remains the only
//! semantic compiler. Rendering consumes only the compiled [`DiagramTheme`]. Terminal/ASCII
//! styling is a separate concern and is not part of this visual contract.

pub use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, MaterializedTheme, ThemeCompileError,
    ThemeMaterializationDigest, ThemeMaterializationError, ThemeMaterializer, ThemePreset,
    ThemePresetDescriptor, ThemePresetParseError, theme_preset_descriptors,
};
pub use merman_theme_contract::{
    CanonicalJsonError, CanonicalJsonErrorKind, DiagramThemeSpecWireV1, SpecifiedWireV1,
    ThemeAuthoringTypographyV1, ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1,
    ThemeColorTokenV1, ThemeDefinitionV1, ThemeGradientStopWireV1, ThemeInsetsWireV1,
    ThemeLengthWireV1, ThemeLineHeightWireV1, ThemeLinearGradientRepetitionWireV1,
    ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1, ThemeRadialGradientRepetitionWireV1,
    ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeTextStylePatchWireV1,
    ThemeTokensV1,
};
