//! Portable, validated diagram-theme inputs and host admission policy.

mod admission;
mod assets;

pub use admission::{
    FontEmbeddingRequirement, FontSource, FontSourcePolicy, HostMeasurementFallback,
    HostMeasurementFallbackPolicy, TextLayoutCapability, ThemeAdmissionError, ThemeAdmissionPolicy,
    ThemeCapability, ThemePortabilityRequirement, ThemeRequirements, TrustedThemeLane,
    TrustedThemeLanes,
};
pub use assets::{
    FontAsset, FontAssetFingerprint, FontAssetIdError, FontAssetSpec, FontCatalog,
    FontCatalogError, FontCatalogFingerprint, FontCatalogSpec, FontContainer,
    FontEmbeddingPermissions, FontFaceMetadata, FontFamilyAlias, FontStyle, GenericFontFamily,
};
