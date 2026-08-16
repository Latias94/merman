#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod authoring;
mod canonical_json;
mod finite;
mod spec;
mod version;
mod wire;

pub use authoring::{
    ThemeAuthoringTypographyV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeTokensV1,
};
pub use canonical_json::{CanonicalJsonError, CanonicalJsonErrorKind};
pub use spec::{
    DiagramThemeSpecWireV1, MermaidThemeCompatibilityWireV1, MermaidThemeValueWireV1,
    ThemeAssetsWireV1, ThemeCanvasLayerWireV1, ThemeCanvasSpecWireV1, ThemeEffectEntryWireV1,
    ThemeEffectPrimitiveWireV1, ThemeFontAliasWireV1, ThemeFontAssetWireV1,
    ThemeGenericFamilyWireV1, ThemeRequirementsWireV1, ThemeTextStyleWireV1,
    ThemeTypographySpecWireV1,
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
