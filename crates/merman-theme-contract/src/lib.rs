#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod authoring;
mod canonical_json;
mod version;

pub use authoring::{
    SpecifiedWireV1, ThemeAuthoringTypographyV1, ThemeCanvasPaintObjectWireV1,
    ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeDefinitionV1, ThemeGradientStopWireV1,
    ThemeInsetsWireV1, ThemeLengthWireV1, ThemeLineHeightWireV1,
    ThemeLinearGradientRepetitionWireV1, ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRadialGradientRepetitionWireV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1,
    ThemeStylePatchWireV1, ThemeTextStylePatchWireV1, ThemeTokensV1,
};
pub use canonical_json::{CanonicalJsonError, CanonicalJsonErrorKind};
pub use version::{
    ThemeContractVersion, ThemeContractVersionError, authoring_version_registry,
    resolve_authoring_version,
};
