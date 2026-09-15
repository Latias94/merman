#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod authoring;
mod canonical_json;
mod facet;
mod finite;
mod materialization_error;
mod materialized;
mod preset_catalog;
mod preset_export;
mod spec;
mod support;
mod version;
mod wire;

pub use authoring::{
    ThemeAuthoringTypographyV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1,
};
pub use canonical_json::{CanonicalJsonError, CanonicalJsonErrorKind};
pub use facet::ThemeRuleFacetV1;
pub use materialization_error::{ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1};
pub use materialized::MaterializedThemeWireV1;
pub use preset_catalog::{
    THEME_PRESET_CATALOG_SCHEMA_VERSION_V1, ThemePresetMetadataV1, ThemePresetQualifiedCellV1,
};
pub use preset_export::PresetExportV1;
pub use spec::{
    DiagramThemeSpecWireV1, MermaidThemeCompatibilityWireV1, MermaidThemeValueWireV1,
    ThemeAssetsWireV1, ThemeCanvasLayerWireV1, ThemeCanvasSpecWireV1, ThemeEffectEntryWireV1,
    ThemeEffectPrimitiveWireV1, ThemeFontAliasWireV1, ThemeFontAssetWireV1,
    ThemeGenericFamilyWireV1, ThemeRequirementsWireV1, ThemeTextStyleWireV1,
    ThemeTypographySpecWireV1,
};
pub use support::{
    MAX_THEME_SUPPORT_REASON_IDS_V1, THEME_SUPPORT_SCHEMA_VERSION_V1, ThemeCapabilityDescriptorV1,
    ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeSupportSubjectV1, ThemeSupportUnknownSubjectV1,
};
pub use version::{
    ThemeContractVersion, ThemeContractVersionError, authoring_version_registry,
    resolve_authoring_version,
};
pub use wire::{
    SpecifiedWireV1, ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1, ThemeGradientStopWireV1,
    ThemeInsetsWireV1, ThemeLengthWireV1, ThemeLineHeightWireV1,
    ThemeLinearGradientRepetitionWireV1, ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRadialGradientRepetitionWireV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1,
    ThemeStylePatchWireV1, ThemeTextStylePatchWireV1,
};
