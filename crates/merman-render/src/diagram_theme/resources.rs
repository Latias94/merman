use merman_core::resources::ResourceProfile;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

pub const THEME_RESOURCE_LIMIT_COUNT: usize = 36;

pub const MAX_THEME_ENCODED_BYTES_HARD_CAP: usize = 64 * MIB;
pub const MAX_THEME_BASE64_BYTES_HARD_CAP: usize = 64 * MIB;
pub const MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP: usize = 16 * MIB;
pub const MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP: usize = 64 * MIB;
pub const MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP: usize = 128 * MIB;
pub const MAX_FONT_ASSETS_HARD_CAP: usize = 64;
pub const MAX_FONT_FACES_HARD_CAP: usize = 256;
pub const MAX_FONT_TABLES_HARD_CAP: usize = 8_192;
pub const MAX_FONT_ALIASES_HARD_CAP: usize = 1_024;
pub const MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP: usize = 100;
pub const MAX_EFFECT_GRAPHS_HARD_CAP: usize = 32;
pub const MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP: usize = 256;
pub const MAX_EFFECT_BINDINGS_HARD_CAP: usize = 64;
pub const MAX_EFFECT_OFFSET_MAGNITUDE_HARD_CAP: usize = 65_536;
pub const MAX_EFFECT_FILTER_REGION_MAGNITUDE_HARD_CAP: usize = 65_536;
pub const MAX_EFFECT_BLUR_MAGNITUDE_HARD_CAP: usize = 8_192;
pub const MAX_EFFECT_DISPLACEMENT_SCALE_HARD_CAP: usize = 65_536;
pub const MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeResourceLimitPhase {
    ThemeInput,
    FontDecode,
    FontCatalog,
    EffectCompile,
    EffectMaterialize,
}

impl ThemeResourceLimitPhase {
    pub const ALL: &'static [Self] = &[
        Self::ThemeInput,
        Self::FontDecode,
        Self::FontCatalog,
        Self::EffectCompile,
        Self::EffectMaterialize,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThemeInput => "theme_input",
            Self::FontDecode => "font_decode",
            Self::FontCatalog => "font_catalog",
            Self::EffectCompile => "effect_compile",
            Self::EffectMaterialize => "effect_materialize",
        }
    }
}

impl std::fmt::Display for ThemeResourceLimitPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeResourceLimitId {
    MaxThemeEncodedBytes,
    MaxThemeBase64Bytes,
    MaxFontAssetCompressedBytes,
    MaxFontAssetDecodedBytes,
    MaxFontCatalogDecodedBytes,
    MaxFontAssets,
    MaxFontFaces,
    MaxFontTables,
    MaxFontAliases,
    MaxFontDecodedExpansionRatio,
    ThemeEncodedBytesHardCap,
    ThemeBase64BytesHardCap,
    FontAssetCompressedBytesHardCap,
    FontAssetDecodedBytesHardCap,
    FontCatalogDecodedBytesHardCap,
    FontAssetsHardCap,
    FontFacesHardCap,
    FontTablesHardCap,
    FontAliasesHardCap,
    FontDecodedExpansionRatioHardCap,
    MaxEffectGraphs,
    MaxEffectPrimitivesPerGraph,
    MaxEffectBindings,
    MaxEffectOffsetMagnitude,
    MaxEffectFilterRegionMagnitude,
    MaxEffectBlurMagnitude,
    MaxEffectDisplacementScale,
    MaxEffectTurbulenceOctaves,
    EffectGraphsHardCap,
    EffectPrimitivesPerGraphHardCap,
    EffectBindingsHardCap,
    EffectOffsetMagnitudeHardCap,
    EffectFilterRegionMagnitudeHardCap,
    EffectBlurMagnitudeHardCap,
    EffectDisplacementScaleHardCap,
    EffectTurbulenceOctavesHardCap,
}

impl ThemeResourceLimitId {
    pub const ALL: &'static [Self] = &[
        Self::MaxThemeEncodedBytes,
        Self::MaxThemeBase64Bytes,
        Self::MaxFontAssetCompressedBytes,
        Self::MaxFontAssetDecodedBytes,
        Self::MaxFontCatalogDecodedBytes,
        Self::MaxFontAssets,
        Self::MaxFontFaces,
        Self::MaxFontTables,
        Self::MaxFontAliases,
        Self::MaxFontDecodedExpansionRatio,
        Self::ThemeEncodedBytesHardCap,
        Self::ThemeBase64BytesHardCap,
        Self::FontAssetCompressedBytesHardCap,
        Self::FontAssetDecodedBytesHardCap,
        Self::FontCatalogDecodedBytesHardCap,
        Self::FontAssetsHardCap,
        Self::FontFacesHardCap,
        Self::FontTablesHardCap,
        Self::FontAliasesHardCap,
        Self::FontDecodedExpansionRatioHardCap,
        Self::MaxEffectGraphs,
        Self::MaxEffectPrimitivesPerGraph,
        Self::MaxEffectBindings,
        Self::MaxEffectOffsetMagnitude,
        Self::MaxEffectFilterRegionMagnitude,
        Self::MaxEffectBlurMagnitude,
        Self::MaxEffectDisplacementScale,
        Self::MaxEffectTurbulenceOctaves,
        Self::EffectGraphsHardCap,
        Self::EffectPrimitivesPerGraphHardCap,
        Self::EffectBindingsHardCap,
        Self::EffectOffsetMagnitudeHardCap,
        Self::EffectFilterRegionMagnitudeHardCap,
        Self::EffectBlurMagnitudeHardCap,
        Self::EffectDisplacementScaleHardCap,
        Self::EffectTurbulenceOctavesHardCap,
    ];

    const fn index(self) -> usize {
        self as usize
    }

    pub fn from_stable_id(id: &str) -> Option<Self> {
        THEME_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.stable_id == id)
            .map(|descriptor| descriptor.id)
    }

    pub const fn descriptor(self) -> ThemeResourceLimitDescriptor {
        THEME_RESOURCE_LIMIT_DESCRIPTORS[self.index()]
    }

    pub const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ThemeResourceLimitDescriptor {
    pub id: ThemeResourceLimitId,
    pub stable_id: &'static str,
    pub phase: ThemeResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
}

pub static THEME_RESOURCE_LIMIT_DESCRIPTORS: [ThemeResourceLimitDescriptor;
    THEME_RESOURCE_LIMIT_COUNT] = [
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxThemeEncodedBytes,
        stable_id: "max_theme_encoded_bytes",
        phase: ThemeResourceLimitPhase::ThemeInput,
        description: "Maximum encoded theme JSON bytes before typed decoding",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxThemeBase64Bytes,
        stable_id: "max_theme_base64_bytes",
        phase: ThemeResourceLimitPhase::ThemeInput,
        description: "Maximum aggregate canonical base64 font payload bytes",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontAssetCompressedBytes,
        stable_id: "max_font_asset_compressed_bytes",
        phase: ThemeResourceLimitPhase::FontDecode,
        description: "Maximum compressed bytes for one font asset",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontAssetDecodedBytes,
        stable_id: "max_font_asset_decoded_bytes",
        phase: ThemeResourceLimitPhase::FontDecode,
        description: "Maximum canonical SFNT bytes for one font asset",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontCatalogDecodedBytes,
        stable_id: "max_font_catalog_decoded_bytes",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Maximum aggregate canonical SFNT bytes retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontAssets,
        stable_id: "max_font_assets",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Maximum font assets retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontFaces,
        stable_id: "max_font_faces",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Maximum faces retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontTables,
        stable_id: "max_font_tables",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Maximum aggregate SFNT table records retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontAliases,
        stable_id: "max_font_aliases",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Maximum family aliases and generic-family mappings in one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxFontDecodedExpansionRatio,
        stable_id: "max_font_decoded_expansion_ratio",
        phase: ThemeResourceLimitPhase::FontDecode,
        description: "Maximum decoded-to-compressed font byte ratio",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::ThemeEncodedBytesHardCap,
        stable_id: "theme_encoded_bytes_hard_cap",
        phase: ThemeResourceLimitPhase::ThemeInput,
        description: "Non-overridable implementation cap for encoded theme input bytes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::ThemeBase64BytesHardCap,
        stable_id: "theme_base64_bytes_hard_cap",
        phase: ThemeResourceLimitPhase::ThemeInput,
        description: "Non-overridable implementation cap for aggregate base64 font payload bytes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontAssetCompressedBytesHardCap,
        stable_id: "font_asset_compressed_bytes_hard_cap",
        phase: ThemeResourceLimitPhase::FontDecode,
        description: "Non-overridable implementation cap for one compressed font asset",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontAssetDecodedBytesHardCap,
        stable_id: "font_asset_decoded_bytes_hard_cap",
        phase: ThemeResourceLimitPhase::FontDecode,
        description: "Non-overridable implementation cap for one canonical SFNT asset",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontCatalogDecodedBytesHardCap,
        stable_id: "font_catalog_decoded_bytes_hard_cap",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for retained catalog font bytes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontAssetsHardCap,
        stable_id: "font_assets_hard_cap",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for catalog asset count",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontFacesHardCap,
        stable_id: "font_faces_hard_cap",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for catalog face count",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontTablesHardCap,
        stable_id: "font_tables_hard_cap",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for catalog table records",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontAliasesHardCap,
        stable_id: "font_aliases_hard_cap",
        phase: ThemeResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for aliases and generic mappings",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::FontDecodedExpansionRatioHardCap,
        stable_id: "font_decoded_expansion_ratio_hard_cap",
        phase: ThemeResourceLimitPhase::FontDecode,
        description: "Non-overridable implementation cap for font decode expansion ratio",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectGraphs,
        stable_id: "max_effect_graphs",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum effect graphs compiled by one theme",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectPrimitivesPerGraph,
        stable_id: "max_effect_primitives_per_graph",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum effect primitives compiled in one effect graph",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectBindings,
        stable_id: "max_effect_bindings",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum semantic effect bindings compiled by one theme",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectOffsetMagnitude,
        stable_id: "max_effect_offset_magnitude",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum absolute drop-shadow offset in a compiled effect graph",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectFilterRegionMagnitude,
        stable_id: "max_effect_filter_region_magnitude",
        phase: ThemeResourceLimitPhase::EffectMaterialize,
        description: "Maximum absolute coordinate or extent in a materialized terminal filter region",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectBlurMagnitude,
        stable_id: "max_effect_blur_magnitude",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum blur, spread, or standard-deviation magnitude in a compiled effect graph",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectDisplacementScale,
        stable_id: "max_effect_displacement_scale",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum displacement scale in a compiled effect graph",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::MaxEffectTurbulenceOctaves,
        stable_id: "max_effect_turbulence_octaves",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Maximum turbulence octaves in a compiled effect graph",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectGraphsHardCap,
        stable_id: "effect_graphs_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for effect graph count",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectPrimitivesPerGraphHardCap,
        stable_id: "effect_primitives_per_graph_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for primitives in one effect graph",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectBindingsHardCap,
        stable_id: "effect_bindings_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for semantic effect binding count",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectOffsetMagnitudeHardCap,
        stable_id: "effect_offset_magnitude_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for absolute drop-shadow offsets",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectFilterRegionMagnitudeHardCap,
        stable_id: "effect_filter_region_magnitude_hard_cap",
        phase: ThemeResourceLimitPhase::EffectMaterialize,
        description: "Non-overridable implementation cap for materialized terminal filter regions",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectBlurMagnitudeHardCap,
        stable_id: "effect_blur_magnitude_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for effect blur magnitudes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectDisplacementScaleHardCap,
        stable_id: "effect_displacement_scale_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for displacement scale",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ThemeResourceLimitDescriptor {
        id: ThemeResourceLimitId::EffectTurbulenceOctavesHardCap,
        stable_id: "effect_turbulence_octaves_hard_cap",
        phase: ThemeResourceLimitPhase::EffectCompile,
        description: "Non-overridable implementation cap for turbulence octaves",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
];

const INTERACTIVE_VALUES: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT] = [
    Some(2 * MIB),
    Some(2 * MIB),
    Some(4 * MIB),
    Some(16 * MIB),
    Some(32 * MIB),
    Some(16),
    Some(32),
    Some(2_048),
    Some(128),
    Some(32),
    Some(MAX_THEME_ENCODED_BYTES_HARD_CAP),
    Some(MAX_THEME_BASE64_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSETS_HARD_CAP),
    Some(MAX_FONT_FACES_HARD_CAP),
    Some(MAX_FONT_TABLES_HARD_CAP),
    Some(MAX_FONT_ALIASES_HARD_CAP),
    Some(MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP),
    Some(16),
    Some(64),
    Some(32),
    Some(4_096),
    Some(4_096),
    Some(1_024),
    Some(4_096),
    Some(8),
    Some(MAX_EFFECT_GRAPHS_HARD_CAP),
    Some(MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP),
    Some(MAX_EFFECT_BINDINGS_HARD_CAP),
    Some(MAX_EFFECT_OFFSET_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_FILTER_REGION_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_BLUR_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_DISPLACEMENT_SCALE_HARD_CAP),
    Some(MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP),
];

const CONSTRAINED_VALUES: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT] = [
    Some(512 * KIB),
    Some(512 * KIB),
    Some(MIB),
    Some(4 * MIB),
    Some(8 * MIB),
    Some(8),
    Some(16),
    Some(512),
    Some(64),
    Some(16),
    Some(MAX_THEME_ENCODED_BYTES_HARD_CAP),
    Some(MAX_THEME_BASE64_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSETS_HARD_CAP),
    Some(MAX_FONT_FACES_HARD_CAP),
    Some(MAX_FONT_TABLES_HARD_CAP),
    Some(MAX_FONT_ALIASES_HARD_CAP),
    Some(MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP),
    Some(8),
    Some(32),
    Some(16),
    Some(1_024),
    Some(1_024),
    Some(256),
    Some(1_024),
    Some(4),
    Some(MAX_EFFECT_GRAPHS_HARD_CAP),
    Some(MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP),
    Some(MAX_EFFECT_BINDINGS_HARD_CAP),
    Some(MAX_EFFECT_OFFSET_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_FILTER_REGION_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_BLUR_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_DISPLACEMENT_SCALE_HARD_CAP),
    Some(MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP),
];

const TRUSTED_NATIVE_VALUES: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT] = [
    Some(8 * MIB),
    Some(8 * MIB),
    Some(8 * MIB),
    Some(32 * MIB),
    Some(64 * MIB),
    Some(32),
    Some(128),
    Some(4_096),
    Some(256),
    Some(64),
    Some(MAX_THEME_ENCODED_BYTES_HARD_CAP),
    Some(MAX_THEME_BASE64_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSETS_HARD_CAP),
    Some(MAX_FONT_FACES_HARD_CAP),
    Some(MAX_FONT_TABLES_HARD_CAP),
    Some(MAX_FONT_ALIASES_HARD_CAP),
    Some(MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP),
    Some(32),
    Some(256),
    Some(64),
    Some(16_384),
    Some(16_384),
    Some(4_096),
    Some(16_384),
    Some(8),
    Some(MAX_EFFECT_GRAPHS_HARD_CAP),
    Some(MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP),
    Some(MAX_EFFECT_BINDINGS_HARD_CAP),
    Some(MAX_EFFECT_OFFSET_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_FILTER_REGION_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_BLUR_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_DISPLACEMENT_SCALE_HARD_CAP),
    Some(MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP),
];

const UNBOUNDED_VALUES: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT] = [
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(MAX_THEME_ENCODED_BYTES_HARD_CAP),
    Some(MAX_THEME_BASE64_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP),
    Some(MAX_FONT_ASSETS_HARD_CAP),
    Some(MAX_FONT_FACES_HARD_CAP),
    Some(MAX_FONT_TABLES_HARD_CAP),
    Some(MAX_FONT_ALIASES_HARD_CAP),
    Some(MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(MAX_EFFECT_GRAPHS_HARD_CAP),
    Some(MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP),
    Some(MAX_EFFECT_BINDINGS_HARD_CAP),
    Some(MAX_EFFECT_OFFSET_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_FILTER_REGION_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_BLUR_MAGNITUDE_HARD_CAP),
    Some(MAX_EFFECT_DISPLACEMENT_SCALE_HARD_CAP),
    Some(MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP),
];

pub const fn theme_resource_limit_descriptors() -> &'static [ThemeResourceLimitDescriptor] {
    &THEME_RESOURCE_LIMIT_DESCRIPTORS
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThemeResourceLimitOverrideError {
    #[error("theme resource limit id `{0}` is not part of the theme resource contract")]
    UnknownLimit(String),
    #[error(
        "theme resource limit `{0}` is a hard implementation capability and cannot be overridden"
    )]
    HardCap(&'static str),
    #[error("theme resource limit `{id}` must be at least {minimum}")]
    BelowMinimum { id: &'static str, minimum: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "theme resource policy would loosen `{}`: requested {:?}, ceiling {:?}",
    .id.as_str(),
    .requested,
    .ceiling
)]
pub struct ThemeResourcePolicyRestrictionError {
    pub id: ThemeResourceLimitId,
    pub requested: Option<usize>,
    pub ceiling: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeResourceLimitOverride {
    pub id: ThemeResourceLimitId,
    pub value: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("theme resource limit exceeded during {phase}: {limit} actual={actual} max={max}")]
#[non_exhaustive]
pub struct ThemeResourceLimitExceeded {
    pub phase: ThemeResourceLimitPhase,
    pub limit: &'static str,
    pub actual: usize,
    pub max: usize,
    pub profile: Option<ResourceProfile>,
    pub explicit_overrides: Vec<ThemeResourceLimitOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeResourcePolicy {
    profile: Option<ResourceProfile>,
    base_values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    effective_values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    explicit_overrides: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
}

impl Default for ThemeResourcePolicy {
    fn default() -> Self {
        Self::interactive()
    }
}

impl ThemeResourcePolicy {
    pub const fn for_profile(profile: ResourceProfile) -> Self {
        match profile {
            ResourceProfile::Interactive => Self::from_values(profile, INTERACTIVE_VALUES),
            ResourceProfile::Constrained => Self::from_values(profile, CONSTRAINED_VALUES),
            ResourceProfile::TrustedNative => Self::from_values(profile, TRUSTED_NATIVE_VALUES),
            ResourceProfile::UnboundedForTrustedInput => {
                Self::from_values(profile, UNBOUNDED_VALUES)
            }
        }
    }

    pub const fn interactive() -> Self {
        Self::for_profile(ResourceProfile::Interactive)
    }

    pub const fn constrained() -> Self {
        Self::for_profile(ResourceProfile::Constrained)
    }

    pub const fn trusted_native() -> Self {
        Self::for_profile(ResourceProfile::TrustedNative)
    }

    pub const fn unbounded_for_trusted_input() -> Self {
        Self::for_profile(ResourceProfile::UnboundedForTrustedInput)
    }

    const fn from_values(
        profile: ResourceProfile,
        values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    ) -> Self {
        Self {
            profile: Some(profile),
            base_values: values,
            effective_values: values,
            explicit_overrides: [None; THEME_RESOURCE_LIMIT_COUNT],
        }
    }

    /// Returns the selected standard profile when the policy still has unambiguous provenance.
    pub const fn profile(&self) -> Option<ResourceProfile> {
        self.profile
    }

    pub const fn value(&self, id: ThemeResourceLimitId) -> Option<usize> {
        self.effective_values[id.index()]
    }

    pub const fn base_value(&self, id: ThemeResourceLimitId) -> Option<usize> {
        self.base_values[id.index()]
    }

    pub const fn explicit_override(&self, id: ThemeResourceLimitId) -> Option<usize> {
        self.explicit_overrides[id.index()]
    }

    pub fn explicit_overrides(&self) -> impl Iterator<Item = (ThemeResourceLimitId, usize)> + '_ {
        ThemeResourceLimitId::ALL
            .iter()
            .copied()
            .filter_map(|id| self.explicit_override(id).map(|value| (id, value)))
    }

    pub fn apply_override(
        &mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<(), ThemeResourceLimitOverrideError> {
        let id = ThemeResourceLimitId::from_stable_id(stable_id)
            .ok_or_else(|| ThemeResourceLimitOverrideError::UnknownLimit(stable_id.to_string()))?;
        self.apply_limit(id, value)
    }

    pub fn apply_limit(
        &mut self,
        id: ThemeResourceLimitId,
        value: usize,
    ) -> Result<(), ThemeResourceLimitOverrideError> {
        let descriptor = id.descriptor();
        if descriptor.hard_cap || !descriptor.overridable {
            return Err(ThemeResourceLimitOverrideError::HardCap(
                descriptor.stable_id,
            ));
        }
        if value < descriptor.minimum_value {
            return Err(ThemeResourceLimitOverrideError::BelowMinimum {
                id: descriptor.stable_id,
                minimum: descriptor.minimum_value,
            });
        }
        self.effective_values[id.index()] = Some(value);
        self.explicit_overrides[id.index()] = Some(value);
        Ok(())
    }

    pub fn with_override(
        mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<Self, ThemeResourceLimitOverrideError> {
        self.apply_override(stable_id, value)?;
        Ok(self)
    }

    pub fn with_limit(
        mut self,
        id: ThemeResourceLimitId,
        value: usize,
    ) -> Result<Self, ThemeResourceLimitOverrideError> {
        self.apply_limit(id, value)?;
        Ok(self)
    }

    /// Returns the pointwise minimum while preserving this host policy's base values.
    ///
    /// Profile provenance follows the strictly tighter operand. Crossing custom restrictions do
    /// not pretend to be one of the standard profiles.
    pub fn meet(&self, restriction: &Self) -> Self {
        let mut effective_values = [None; THEME_RESOURCE_LIMIT_COUNT];
        let mut restriction_tightens = false;
        let mut host_tightens = false;
        let mut index = 0;
        while index < THEME_RESOURCE_LIMIT_COUNT {
            restriction_tightens |= loosens_ceiling(
                restriction.effective_values[index],
                self.effective_values[index],
            );
            host_tightens |= loosens_ceiling(
                self.effective_values[index],
                restriction.effective_values[index],
            );
            effective_values[index] = minimum_ceiling(
                self.effective_values[index],
                restriction.effective_values[index],
            );
            index += 1;
        }
        let profile = match (restriction_tightens, host_tightens) {
            (true, false) => restriction.profile,
            (false, true) | (false, false) => self.profile,
            (true, true) => None,
        };
        self.with_effective_values(profile, effective_values)
    }

    /// Applies a request policy only when every requested ceiling is at least as strict.
    pub fn restrict_with(
        &self,
        restriction: &Self,
    ) -> Result<Self, ThemeResourcePolicyRestrictionError> {
        for id in ThemeResourceLimitId::ALL.iter().copied() {
            let ceiling = self.value(id);
            let requested = restriction.value(id);
            if loosens_ceiling(ceiling, requested) {
                return Err(ThemeResourcePolicyRestrictionError {
                    id,
                    requested,
                    ceiling,
                });
            }
        }
        Ok(self.meet(restriction))
    }

    fn with_effective_values(
        &self,
        profile: Option<ResourceProfile>,
        effective_values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    ) -> Self {
        let mut explicit_overrides = [None; THEME_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < THEME_RESOURCE_LIMIT_COUNT {
            if effective_values[index] != self.base_values[index] {
                explicit_overrides[index] = effective_values[index];
            }
            index += 1;
        }
        Self {
            profile,
            base_values: self.base_values,
            effective_values,
            explicit_overrides,
        }
    }

    fn check_limit(
        &self,
        id: ThemeResourceLimitId,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        let Some(max) = self.value(id) else {
            return Ok(());
        };
        if actual <= max {
            return Ok(());
        }
        let descriptor = id.descriptor();
        Err(ThemeResourceLimitExceeded {
            phase: descriptor.phase,
            limit: descriptor.stable_id,
            actual,
            max,
            profile: self.profile,
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ThemeResourceLimitOverride { id, value })
                .collect(),
        })
    }

    fn check_limit_with_hard_cap(
        &self,
        policy: ThemeResourceLimitId,
        hard_cap: ThemeResourceLimitId,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit(hard_cap, actual)?;
        self.check_limit(policy, actual)
    }

    pub fn check_theme_encoded_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxThemeEncodedBytes,
            ThemeResourceLimitId::ThemeEncodedBytesHardCap,
            actual,
        )
    }

    pub fn check_theme_base64_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxThemeBase64Bytes,
            ThemeResourceLimitId::ThemeBase64BytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_asset_compressed_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAssetCompressedBytes,
            ThemeResourceLimitId::FontAssetCompressedBytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_asset_decoded_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAssetDecodedBytes,
            ThemeResourceLimitId::FontAssetDecodedBytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_catalog_decoded_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontCatalogDecodedBytes,
            ThemeResourceLimitId::FontCatalogDecodedBytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_asset_count(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAssets,
            ThemeResourceLimitId::FontAssetsHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_face_count(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontFaces,
            ThemeResourceLimitId::FontFacesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_table_count(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontTables,
            ThemeResourceLimitId::FontTablesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_alias_count(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAliases,
            ThemeResourceLimitId::FontAliasesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_decoded_expansion(
        &self,
        compressed: usize,
        decoded: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        let actual = if decoded == 0 {
            0
        } else if compressed == 0 {
            usize::MAX
        } else {
            decoded
                .saturating_add(compressed.saturating_sub(1))
                .checked_div(compressed)
                .unwrap_or(usize::MAX)
        };
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontDecodedExpansionRatio,
            ThemeResourceLimitId::FontDecodedExpansionRatioHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_graph_count(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxEffectGraphs,
            ThemeResourceLimitId::EffectGraphsHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_primitives_per_graph(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxEffectPrimitivesPerGraph,
            ThemeResourceLimitId::EffectPrimitivesPerGraphHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_binding_count(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxEffectBindings,
            ThemeResourceLimitId::EffectBindingsHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_offset_magnitude(
        &self,
        actual: f32,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_effect_magnitude(
            ThemeResourceLimitId::MaxEffectOffsetMagnitude,
            ThemeResourceLimitId::EffectOffsetMagnitudeHardCap,
            actual,
        )
    }

    pub(crate) fn check_materialized_filter_region_magnitude(
        &self,
        actual: f32,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_effect_magnitude(
            ThemeResourceLimitId::MaxEffectFilterRegionMagnitude,
            ThemeResourceLimitId::EffectFilterRegionMagnitudeHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_blur_magnitude(
        &self,
        actual: f32,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_effect_magnitude(
            ThemeResourceLimitId::MaxEffectBlurMagnitude,
            ThemeResourceLimitId::EffectBlurMagnitudeHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_displacement_scale(
        &self,
        actual: f32,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_effect_magnitude(
            ThemeResourceLimitId::MaxEffectDisplacementScale,
            ThemeResourceLimitId::EffectDisplacementScaleHardCap,
            actual,
        )
    }

    pub(crate) fn check_effect_turbulence_octaves(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(
            ThemeResourceLimitId::MaxEffectTurbulenceOctaves,
            ThemeResourceLimitId::EffectTurbulenceOctavesHardCap,
            actual,
        )
    }

    fn check_effect_magnitude(
        &self,
        policy: ThemeResourceLimitId,
        hard_cap: ThemeResourceLimitId,
        actual: f32,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.check_limit_with_hard_cap(policy, hard_cap, effect_magnitude_units(actual))
    }
}

fn effect_magnitude_units(actual: f32) -> usize {
    if !actual.is_finite() || actual < 0.0 {
        usize::MAX
    } else {
        actual.ceil() as usize
    }
}

const fn minimum_ceiling(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if left < right { left } else { right }),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

const fn loosens_ceiling(ceiling: Option<usize>, requested: Option<usize>) -> bool {
    match (ceiling, requested) {
        (Some(_), None) => true,
        (Some(ceiling), Some(requested)) => requested > ceiling,
        (None, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn owner_local_descriptor_contract_is_complete_unique_and_isolated() {
        assert_eq!(
            THEME_RESOURCE_LIMIT_DESCRIPTORS.len(),
            THEME_RESOURCE_LIMIT_COUNT
        );
        let ids = THEME_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.stable_id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), THEME_RESOURCE_LIMIT_COUNT);

        for descriptor in THEME_RESOURCE_LIMIT_DESCRIPTORS {
            assert_eq!(
                ThemeResourceLimitId::from_stable_id(descriptor.stable_id),
                Some(descriptor.id)
            );
            assert_eq!(descriptor.id.descriptor(), descriptor);
        }
        assert_eq!(ThemeResourceLimitId::from_stable_id("max_svg_bytes"), None);
    }

    #[test]
    fn profile_constructors_preserve_the_established_theme_budget_matrix() {
        let profiles = [
            ThemeResourcePolicy::interactive(),
            ThemeResourcePolicy::constrained(),
            ThemeResourcePolicy::trusted_native(),
            ThemeResourcePolicy::unbounded_for_trusted_input(),
        ];
        for (profile, policy) in ResourceProfile::ALL.into_iter().zip(profiles.iter()) {
            assert_eq!(ThemeResourcePolicy::for_profile(profile), *policy);
        }
        let expected = [
            (
                ThemeResourceLimitId::MaxThemeEncodedBytes,
                [Some(2 * MIB), Some(512 * KIB), Some(8 * MIB), None],
            ),
            (
                ThemeResourceLimitId::MaxThemeBase64Bytes,
                [Some(2 * MIB), Some(512 * KIB), Some(8 * MIB), None],
            ),
            (
                ThemeResourceLimitId::MaxFontAssetCompressedBytes,
                [Some(4 * MIB), Some(MIB), Some(8 * MIB), None],
            ),
            (
                ThemeResourceLimitId::MaxFontAssetDecodedBytes,
                [Some(16 * MIB), Some(4 * MIB), Some(32 * MIB), None],
            ),
            (
                ThemeResourceLimitId::MaxFontCatalogDecodedBytes,
                [Some(32 * MIB), Some(8 * MIB), Some(64 * MIB), None],
            ),
            (
                ThemeResourceLimitId::MaxFontAssets,
                [Some(16), Some(8), Some(32), None],
            ),
            (
                ThemeResourceLimitId::MaxFontFaces,
                [Some(32), Some(16), Some(128), None],
            ),
            (
                ThemeResourceLimitId::MaxFontTables,
                [Some(2_048), Some(512), Some(4_096), None],
            ),
            (
                ThemeResourceLimitId::MaxFontAliases,
                [Some(128), Some(64), Some(256), None],
            ),
            (
                ThemeResourceLimitId::MaxFontDecodedExpansionRatio,
                [Some(32), Some(16), Some(64), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectGraphs,
                [Some(16), Some(8), Some(32), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectPrimitivesPerGraph,
                [Some(64), Some(32), Some(256), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectBindings,
                [Some(32), Some(16), Some(64), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectOffsetMagnitude,
                [Some(4_096), Some(1_024), Some(16_384), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectFilterRegionMagnitude,
                [Some(4_096), Some(1_024), Some(16_384), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectBlurMagnitude,
                [Some(1_024), Some(256), Some(4_096), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectDisplacementScale,
                [Some(4_096), Some(1_024), Some(16_384), None],
            ),
            (
                ThemeResourceLimitId::MaxEffectTurbulenceOctaves,
                [Some(8), Some(4), Some(8), None],
            ),
        ];

        for (id, values) in expected {
            assert_eq!(profiles.each_ref().map(|profile| profile.value(id)), values);
        }
    }

    #[test]
    fn overrides_follow_minimums_and_preserve_hard_caps() {
        let mut policy = ThemeResourcePolicy::interactive();
        for descriptor in THEME_RESOURCE_LIMIT_DESCRIPTORS {
            if descriptor.hard_cap {
                assert_eq!(
                    policy.apply_limit(descriptor.id, descriptor.minimum_value),
                    Err(ThemeResourceLimitOverrideError::HardCap(
                        descriptor.stable_id
                    ))
                );
            } else if descriptor.minimum_value == 0 {
                policy.apply_limit(descriptor.id, 0).unwrap();
                assert_eq!(policy.value(descriptor.id), Some(0));
            } else {
                assert_eq!(
                    policy.apply_limit(descriptor.id, 0),
                    Err(ThemeResourceLimitOverrideError::BelowMinimum {
                        id: descriptor.stable_id,
                        minimum: descriptor.minimum_value,
                    })
                );
            }
        }
    }

    #[test]
    fn meet_and_restriction_are_pointwise_monotonic() {
        for descriptor in THEME_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .copied()
            .filter(|descriptor| descriptor.overridable)
        {
            let host = ThemeResourcePolicy::interactive();
            let ceiling = host
                .value(descriptor.id)
                .expect("interactive policy must bound every overridable theme limit");
            let stricter_value = descriptor.minimum_value.max(ceiling / 2);
            let stricter = host
                .clone()
                .with_limit(descriptor.id, stricter_value)
                .unwrap();

            assert_eq!(
                host.meet(&stricter).value(descriptor.id),
                Some(stricter_value)
            );
            assert_eq!(
                host.restrict_with(&stricter).unwrap().value(descriptor.id),
                Some(stricter_value)
            );

            let looser_value = ceiling.checked_add(1).unwrap();
            let looser = host
                .clone()
                .with_limit(descriptor.id, looser_value)
                .unwrap();
            assert_eq!(
                host.restrict_with(&looser),
                Err(ThemeResourcePolicyRestrictionError {
                    id: descriptor.id,
                    requested: Some(looser_value),
                    ceiling: Some(ceiling),
                })
            );
        }

        let host = ThemeResourcePolicy::unbounded_for_trusted_input();
        let restricted = host
            .restrict_with(&ThemeResourcePolicy::constrained())
            .unwrap();
        assert_eq!(restricted.profile(), Some(ResourceProfile::Constrained));
        for descriptor in THEME_RESOURCE_LIMIT_DESCRIPTORS {
            assert_eq!(
                restricted.value(descriptor.id),
                minimum_ceiling(
                    host.value(descriptor.id),
                    ThemeResourcePolicy::constrained().value(descriptor.id)
                )
            );
        }
    }

    #[test]
    fn crossing_custom_restrictions_do_not_claim_a_standard_profile() {
        let first = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxEffectGraphs, 1)
            .unwrap();
        let second = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxEffectBindings, 1)
            .unwrap();

        assert_eq!(first.meet(&second).profile(), None);
    }

    #[test]
    fn unbounded_policy_retains_every_implementation_hard_cap() {
        let policy = ThemeResourcePolicy::unbounded_for_trusted_input();
        let checks = [
            policy
                .check_theme_encoded_bytes(MAX_THEME_ENCODED_BYTES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_theme_base64_bytes(MAX_THEME_BASE64_BYTES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_asset_compressed_bytes(MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_asset_decoded_bytes(MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_catalog_decoded_bytes(MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_asset_count(MAX_FONT_ASSETS_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_face_count(MAX_FONT_FACES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_table_count(MAX_FONT_TABLES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_alias_count(MAX_FONT_ALIASES_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_font_decoded_expansion(1, MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_effect_graph_count(MAX_EFFECT_GRAPHS_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_effect_primitives_per_graph(MAX_EFFECT_PRIMITIVES_PER_GRAPH_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_effect_binding_count(MAX_EFFECT_BINDINGS_HARD_CAP + 1)
                .unwrap_err(),
            policy
                .check_effect_offset_magnitude((MAX_EFFECT_OFFSET_MAGNITUDE_HARD_CAP + 1) as f32)
                .unwrap_err(),
            policy
                .check_materialized_filter_region_magnitude(
                    (MAX_EFFECT_FILTER_REGION_MAGNITUDE_HARD_CAP + 1) as f32,
                )
                .unwrap_err(),
            policy
                .check_effect_blur_magnitude((MAX_EFFECT_BLUR_MAGNITUDE_HARD_CAP + 1) as f32)
                .unwrap_err(),
            policy
                .check_effect_displacement_scale(
                    (MAX_EFFECT_DISPLACEMENT_SCALE_HARD_CAP + 1) as f32,
                )
                .unwrap_err(),
            policy
                .check_effect_turbulence_octaves(MAX_EFFECT_TURBULENCE_OCTAVES_HARD_CAP + 1)
                .unwrap_err(),
        ];
        let expected = [
            "theme_encoded_bytes_hard_cap",
            "theme_base64_bytes_hard_cap",
            "font_asset_compressed_bytes_hard_cap",
            "font_asset_decoded_bytes_hard_cap",
            "font_catalog_decoded_bytes_hard_cap",
            "font_assets_hard_cap",
            "font_faces_hard_cap",
            "font_tables_hard_cap",
            "font_aliases_hard_cap",
            "font_decoded_expansion_ratio_hard_cap",
            "effect_graphs_hard_cap",
            "effect_primitives_per_graph_hard_cap",
            "effect_bindings_hard_cap",
            "effect_offset_magnitude_hard_cap",
            "effect_filter_region_magnitude_hard_cap",
            "effect_blur_magnitude_hard_cap",
            "effect_displacement_scale_hard_cap",
            "effect_turbulence_octaves_hard_cap",
        ];

        assert_eq!(checks.map(|error| error.limit), expected);
    }

    #[test]
    fn zero_limits_can_disable_custom_font_ingestion() {
        let policy = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxFontAssets, 0)
            .unwrap()
            .with_limit(ThemeResourceLimitId::MaxFontCatalogDecodedBytes, 0)
            .unwrap();

        policy.check_font_asset_count(0).unwrap();
        policy.check_font_catalog_decoded_bytes(0).unwrap();
        assert_eq!(
            policy.check_font_asset_count(1).unwrap_err().limit,
            "max_font_assets"
        );
        assert_eq!(
            policy
                .check_font_catalog_decoded_bytes(1)
                .unwrap_err()
                .limit,
            "max_font_catalog_decoded_bytes"
        );
    }
}
