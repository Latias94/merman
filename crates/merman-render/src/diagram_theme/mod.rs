//! Portable, validated diagram-theme inputs and host admission policy.

mod admission;
mod assets;
mod resources;

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
pub use resources::{
    MAX_FONT_ALIASES_HARD_CAP, MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP,
    MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP, MAX_FONT_ASSETS_HARD_CAP,
    MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP, MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP,
    MAX_FONT_FACES_HARD_CAP, MAX_FONT_TABLES_HARD_CAP, MAX_THEME_BASE64_BYTES_HARD_CAP,
    MAX_THEME_ENCODED_BYTES_HARD_CAP, THEME_RESOURCE_LIMIT_COUNT, THEME_RESOURCE_LIMIT_DESCRIPTORS,
    ThemeResourceLimitDescriptor, ThemeResourceLimitExceeded, ThemeResourceLimitId,
    ThemeResourceLimitOverride, ThemeResourceLimitOverrideError, ThemeResourceLimitPhase,
    ThemeResourcePolicy, ThemeResourcePolicyRestrictionError, theme_resource_limit_descriptors,
};
