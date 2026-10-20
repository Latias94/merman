//! General typed visual-theme authoring, retained bindings, and concrete compilation.
//! Copy a preset or construct a ThemeDesignV1, then compile its ThemeRecipeV1 through the caller-owned compiler.

pub use merman_core::DiagramFamilyId;
pub use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, ThemeCompileError,
    ThemeDesignCompileError, ThemeDesignInspection, ThemeDesignReferenceUse,
    ThemeDesignValueDomain, ThemePreset, ThemePresetDescriptor, ThemePresetParseError,
    ThemePresetQualifiedCell, ThemeTarget, ThemeVariant, describe_theme_support,
    inspect_theme_design, materialize_theme_design, materialize_theme_design_json_with_bindings,
    materialize_theme_design_json_with_resource_policy,
    materialize_theme_design_with_resource_policy, theme_preset_descriptors,
};
pub use merman_theme_contract::{
    CanonicalJsonError, CanonicalJsonErrorKind, DiagramThemeSpecWireV1, MaterializedThemeWireV1,
    SpecifiedWireV1, THEME_SUPPORT_SCHEMA_VERSION_V1, ThemeCanvasPaintObjectWireV1,
    ThemeCanvasPaintWireV1, ThemeCapabilityDescriptorV1, ThemeDesignCanvasLayerV1,
    ThemeDesignCanvasV1, ThemeDesignEffectV1, ThemeDesignGradientStopV1, ThemeDesignPaintObjectV1,
    ThemeDesignPaintV1, ThemeDesignRuleV1, ThemeDesignShadowV1, ThemeDesignStrokeV1,
    ThemeDesignStyleV1, ThemeDesignTextStyleV1, ThemeDesignTokenV1, ThemeDesignTokenValueV1,
    ThemeDesignTypographyV1, ThemeDesignV1, ThemeGradientStopWireV1, ThemeInsetsWireV1,
    ThemeLengthWireV1, ThemeLineHeightWireV1, ThemeLinearGradientRepetitionWireV1,
    ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1, ThemeOrdinalCycleWireV1,
    ThemeOrdinalSelectorWireV1, ThemePresetFamilyDesignV1, ThemeRadialGradientRepetitionWireV1,
    ThemeRecipeV1, ThemeReferenceV1, ThemeRuleFacetV1, ThemeRuleSetWireV1,
    ThemeShadowCompositionV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1,
    ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeSupportSubjectV1, ThemeTextStylePatchWireV1,
    ThemeValueV1,
};
