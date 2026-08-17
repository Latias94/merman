//! Versioned, target-independent visual diagram-theme authoring.
//!
//! [`compile_theme_definition`] is the normal Rust entry point: it expands a compact
//! [`ThemeDefinitionV1`] and delegates the resulting complete recipe to a caller-owned
//! [`DiagramThemeCompiler`]. The materializer and complete-spec wire remain available for callers
//! that need to inspect or edit the intermediate representation. Rendering consumes only the
//! compiled [`DiagramTheme`]. Terminal/ASCII styling is a separate concern and is not part of this
//! visual contract.

mod authoring;

pub use authoring::{ThemeDefinitionBuilderV1, ThemeRuleBuilderV1};
pub use merman_core::DiagramFamilyId;
pub use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemeCompileError,
    ThemeDefinitionCompileError, ThemeMaterializer, ThemePreset, ThemePresetDescriptor,
    ThemePresetParseError, ThemeTarget, ThemeVariant, compile_theme_definition,
    compile_theme_definition_json, describe_theme_support, theme_preset_descriptors,
};
pub use merman_theme_contract::{
    CanonicalJsonError, CanonicalJsonErrorKind, DiagramThemeSpecWireV1, MaterializedThemeWireV1,
    SpecifiedWireV1, ThemeAuthoringTypographyV1, ThemeCanvasPaintObjectWireV1,
    ThemeCanvasPaintWireV1, ThemeCapabilityDescriptorV1, ThemeColorTokenV1, ThemeDefinitionV1,
    ThemeGradientStopWireV1, ThemeInsetsWireV1, ThemeLengthWireV1, ThemeLineHeightWireV1,
    ThemeLinearGradientRepetitionWireV1, ThemeMaterializationDiagnosticV1,
    ThemeMaterializationErrorV1, ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRadialGradientRepetitionWireV1, ThemeRuleFacetV1, ThemeRuleSetWireV1,
    ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeTextStylePatchWireV1, ThemeTokensV1,
};
