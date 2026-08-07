use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::diagrams::mindmap::MindmapDiagramRenderModel;
use merman_core::diagrams::zenuml::ZenumlDiagramRenderModel;
use merman_core::models::class_diagram::ClassDiagram;
pub use merman_core::resources::{
    ClassComplexity, FlowchartComplexity, MindmapComplexity, ModelComplexity,
    RESOURCE_PROFILE_DESCRIPTORS, ResourceProfile as RenderResourceProfile,
    ResourceProfileDescriptor as RenderResourceProfileDescriptor, SequenceComplexity,
    ZenumlComplexity,
};
use merman_core::resources::{
    InputResourceLimitExceeded, InputResourceLimitId, InputResourceLimitPhase, InputResourcePolicy,
    InputResourcePolicyRestrictionError,
};
use merman_core::{ParsedDiagramRender, RenderSemanticModel};

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

#[cfg(not(target_arch = "wasm32"))]
pub const MAX_RESVG_TREE_DEPTH: usize = 256;
pub const SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID: &str = "svg_backend_tree_depth";
/// Maximum resolved usvg nodes accepted independently of caller-selected policies.
///
/// This matches usvg's own parser capability while rejecting expansion before it can allocate the
/// upstream parser's full tree.
pub const MAX_RESVG_TREE_NODES: usize = 1_000_000;
pub const SVG_BACKEND_TREE_NODES_HARD_CAP_ID: &str = "svg_backend_tree_nodes";

#[cfg(target_arch = "wasm32")]
pub const MAX_RESVG_TREE_DEPTH: usize = 64;

// Backend capability for recursively owned typed/compatibility trees, not a Mermaid syntax limit.
// It remains active when policy budgets are disabled because increasing it is not stack-safe.
#[cfg(not(target_arch = "wasm32"))]
const MAX_RECURSIVE_MODEL_TREE_DEPTH: usize = merman_core::MAX_DIAGRAM_NESTING_DEPTH;

#[cfg(target_arch = "wasm32")]
const MAX_RECURSIVE_MODEL_TREE_DEPTH: usize = 64;

pub const RESOURCE_PROFILE_COUNT: usize = merman_core::resources::RESOURCE_PROFILE_COUNT;
const RENDER_RESOURCE_LIMIT_COUNT: usize = 3;
const THEME_RESOURCE_LIMIT_COUNT: usize = 20;
pub const RESOURCE_LIMIT_COUNT: usize = merman_core::resources::INPUT_RESOURCE_LIMIT_COUNT
    + RENDER_RESOURCE_LIMIT_COUNT
    + THEME_RESOURCE_LIMIT_COUNT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceLimitPhase {
    Source,
    LayoutModel,
    ThemeInput,
    FontDecode,
    FontCatalog,
    SvgOutput,
    SvgPostprocess,
}

/// Stable reason why a resource check failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceLimitCause {
    /// The requested work exceeded the configured policy ceiling.
    Ceiling,
    /// Computing cumulative work overflowed the platform counter.
    ArithmeticOverflow,
}

impl ResourceLimitCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ceiling => "ceiling",
            Self::ArithmeticOverflow => "arithmetic_overflow",
        }
    }
}

impl std::fmt::Display for ResourceLimitCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ResourceLimitPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::LayoutModel => "layout_model",
            Self::ThemeInput => "theme_input",
            Self::FontDecode => "font_decode",
            Self::FontCatalog => "font_catalog",
            Self::SvgOutput => "svg_output",
            Self::SvgPostprocess => "svg_postprocess",
        }
    }
}

impl std::fmt::Display for ResourceLimitPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderResourceLimitId {
    MaxSvgBytes,
    MaxSvgElements,
    MaxLayoutWorkUnits,
}

impl RenderResourceLimitId {
    pub const ALL: [Self; RENDER_RESOURCE_LIMIT_COUNT] = [
        Self::MaxSvgBytes,
        Self::MaxSvgElements,
        Self::MaxLayoutWorkUnits,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}

impl ThemeResourceLimitId {
    pub const ALL: [Self; THEME_RESOURCE_LIMIT_COUNT] = [
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
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceLimitId {
    Input(InputResourceLimitId),
    Render(RenderResourceLimitId),
    Theme(ThemeResourceLimitId),
}

#[allow(non_upper_case_globals)]
impl ResourceLimitId {
    pub const MaxSourceBytes: Self = Self::Input(InputResourceLimitId::MaxSourceBytes);
    pub const MaxModelItems: Self = Self::Input(InputResourceLimitId::MaxModelItems);
    pub const MaxModelTextBytes: Self = Self::Input(InputResourceLimitId::MaxModelTextBytes);
    pub const MaxModelNestingDepth: Self = Self::Input(InputResourceLimitId::MaxModelNestingDepth);
    pub const MaxSvgBytes: Self = Self::Render(RenderResourceLimitId::MaxSvgBytes);
    pub const MaxSvgElements: Self = Self::Render(RenderResourceLimitId::MaxSvgElements);
    pub const MaxLayoutWorkUnits: Self = Self::Render(RenderResourceLimitId::MaxLayoutWorkUnits);
    pub const MaxThemeEncodedBytes: Self = Self::Theme(ThemeResourceLimitId::MaxThemeEncodedBytes);
    pub const MaxThemeBase64Bytes: Self = Self::Theme(ThemeResourceLimitId::MaxThemeBase64Bytes);
    pub const MaxFontAssetCompressedBytes: Self =
        Self::Theme(ThemeResourceLimitId::MaxFontAssetCompressedBytes);
    pub const MaxFontAssetDecodedBytes: Self =
        Self::Theme(ThemeResourceLimitId::MaxFontAssetDecodedBytes);
    pub const MaxFontCatalogDecodedBytes: Self =
        Self::Theme(ThemeResourceLimitId::MaxFontCatalogDecodedBytes);
    pub const MaxFontAssets: Self = Self::Theme(ThemeResourceLimitId::MaxFontAssets);
    pub const MaxFontFaces: Self = Self::Theme(ThemeResourceLimitId::MaxFontFaces);
    pub const MaxFontTables: Self = Self::Theme(ThemeResourceLimitId::MaxFontTables);
    pub const MaxFontAliases: Self = Self::Theme(ThemeResourceLimitId::MaxFontAliases);
    pub const MaxFontDecodedExpansionRatio: Self =
        Self::Theme(ThemeResourceLimitId::MaxFontDecodedExpansionRatio);
    pub const ThemeEncodedBytesHardCap: Self =
        Self::Theme(ThemeResourceLimitId::ThemeEncodedBytesHardCap);
    pub const ThemeBase64BytesHardCap: Self =
        Self::Theme(ThemeResourceLimitId::ThemeBase64BytesHardCap);
    pub const FontAssetCompressedBytesHardCap: Self =
        Self::Theme(ThemeResourceLimitId::FontAssetCompressedBytesHardCap);
    pub const FontAssetDecodedBytesHardCap: Self =
        Self::Theme(ThemeResourceLimitId::FontAssetDecodedBytesHardCap);
    pub const FontCatalogDecodedBytesHardCap: Self =
        Self::Theme(ThemeResourceLimitId::FontCatalogDecodedBytesHardCap);
    pub const FontAssetsHardCap: Self = Self::Theme(ThemeResourceLimitId::FontAssetsHardCap);
    pub const FontFacesHardCap: Self = Self::Theme(ThemeResourceLimitId::FontFacesHardCap);
    pub const FontTablesHardCap: Self = Self::Theme(ThemeResourceLimitId::FontTablesHardCap);
    pub const FontAliasesHardCap: Self = Self::Theme(ThemeResourceLimitId::FontAliasesHardCap);
    pub const FontDecodedExpansionRatioHardCap: Self =
        Self::Theme(ThemeResourceLimitId::FontDecodedExpansionRatioHardCap);

    pub const ALL: [Self; RESOURCE_LIMIT_COUNT] = [
        Self::MaxSourceBytes,
        Self::MaxModelItems,
        Self::MaxModelTextBytes,
        Self::MaxModelNestingDepth,
        Self::MaxLayoutWorkUnits,
        Self::MaxSvgBytes,
        Self::MaxSvgElements,
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
    ];

    pub fn from_stable_id(id: &str) -> Option<Self> {
        InputResourceLimitId::from_stable_id(id)
            .map(Self::Input)
            .or_else(|| {
                RENDER_RESOURCE_LIMIT_DESCRIPTORS
                    .iter()
                    .find(|descriptor| descriptor.stable_id == id)
                    .map(|descriptor| descriptor.id)
            })
            .or_else(|| {
                THEME_RESOURCE_LIMIT_DESCRIPTORS
                    .iter()
                    .find(|descriptor| descriptor.stable_id == id)
                    .map(|descriptor| descriptor.id)
            })
    }

    pub const fn descriptor(self) -> ResourceLimitDescriptor {
        match self {
            Self::Input(id) => input_descriptor(id),
            Self::Render(id) => RENDER_RESOURCE_LIMIT_DESCRIPTORS[id.index()],
            Self::Theme(id) => THEME_RESOURCE_LIMIT_DESCRIPTORS[id.index()],
        }
    }

    pub const fn as_str(self) -> &'static str {
        self.descriptor().stable_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceLimitDescriptor {
    pub id: ResourceLimitId,
    pub stable_id: &'static str,
    pub phase: ResourceLimitPhase,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
}

const fn input_descriptor(id: InputResourceLimitId) -> ResourceLimitDescriptor {
    let descriptor = id.descriptor();
    ResourceLimitDescriptor {
        id: ResourceLimitId::Input(id),
        stable_id: descriptor.stable_id,
        phase: match descriptor.phase {
            InputResourceLimitPhase::Source => ResourceLimitPhase::Source,
            InputResourceLimitPhase::Model => ResourceLimitPhase::LayoutModel,
        },
        description: descriptor.description,
        overridable: descriptor.overridable,
        hard_cap: false,
        minimum_value: descriptor.minimum_value,
    }
}

const RENDER_RESOURCE_LIMIT_DESCRIPTORS: [ResourceLimitDescriptor; RENDER_RESOURCE_LIMIT_COUNT] = [
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxSvgBytes,
        stable_id: "max_svg_bytes",
        phase: ResourceLimitPhase::SvgOutput,
        description: "Maximum serialized SVG bytes",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxSvgElements,
        stable_id: "max_svg_elements",
        phase: ResourceLimitPhase::SvgPostprocess,
        description: "Maximum SVG element count",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxLayoutWorkUnits,
        stable_id: "max_layout_work_units",
        phase: ResourceLimitPhase::LayoutModel,
        description: "Maximum family-accounted derived layout and render geometry work units",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
];

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

const THEME_RESOURCE_LIMIT_DESCRIPTORS: [ResourceLimitDescriptor; THEME_RESOURCE_LIMIT_COUNT] = [
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxThemeEncodedBytes,
        stable_id: "max_theme_encoded_bytes",
        phase: ResourceLimitPhase::ThemeInput,
        description: "Maximum encoded theme JSON bytes before typed decoding",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxThemeBase64Bytes,
        stable_id: "max_theme_base64_bytes",
        phase: ResourceLimitPhase::ThemeInput,
        description: "Maximum aggregate canonical base64 font payload bytes",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontAssetCompressedBytes,
        stable_id: "max_font_asset_compressed_bytes",
        phase: ResourceLimitPhase::FontDecode,
        description: "Maximum compressed bytes for one font asset",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontAssetDecodedBytes,
        stable_id: "max_font_asset_decoded_bytes",
        phase: ResourceLimitPhase::FontDecode,
        description: "Maximum canonical SFNT bytes for one font asset",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontCatalogDecodedBytes,
        stable_id: "max_font_catalog_decoded_bytes",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Maximum aggregate canonical SFNT bytes retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontAssets,
        stable_id: "max_font_assets",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Maximum font assets retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontFaces,
        stable_id: "max_font_faces",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Maximum faces retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontTables,
        stable_id: "max_font_tables",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Maximum aggregate SFNT table records retained by one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontAliases,
        stable_id: "max_font_aliases",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Maximum family aliases and generic-family mappings in one font catalog",
        overridable: true,
        hard_cap: false,
        minimum_value: 0,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::MaxFontDecodedExpansionRatio,
        stable_id: "max_font_decoded_expansion_ratio",
        phase: ResourceLimitPhase::FontDecode,
        description: "Maximum decoded-to-compressed font byte ratio",
        overridable: true,
        hard_cap: false,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::ThemeEncodedBytesHardCap,
        stable_id: "theme_encoded_bytes_hard_cap",
        phase: ResourceLimitPhase::ThemeInput,
        description: "Non-overridable implementation cap for encoded theme input bytes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::ThemeBase64BytesHardCap,
        stable_id: "theme_base64_bytes_hard_cap",
        phase: ResourceLimitPhase::ThemeInput,
        description: "Non-overridable implementation cap for aggregate base64 font payload bytes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontAssetCompressedBytesHardCap,
        stable_id: "font_asset_compressed_bytes_hard_cap",
        phase: ResourceLimitPhase::FontDecode,
        description: "Non-overridable implementation cap for one compressed font asset",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontAssetDecodedBytesHardCap,
        stable_id: "font_asset_decoded_bytes_hard_cap",
        phase: ResourceLimitPhase::FontDecode,
        description: "Non-overridable implementation cap for one canonical SFNT asset",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontCatalogDecodedBytesHardCap,
        stable_id: "font_catalog_decoded_bytes_hard_cap",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for retained catalog font bytes",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontAssetsHardCap,
        stable_id: "font_assets_hard_cap",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for catalog asset count",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontFacesHardCap,
        stable_id: "font_faces_hard_cap",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for catalog face count",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontTablesHardCap,
        stable_id: "font_tables_hard_cap",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for catalog table records",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontAliasesHardCap,
        stable_id: "font_aliases_hard_cap",
        phase: ResourceLimitPhase::FontCatalog,
        description: "Non-overridable implementation cap for aliases and generic mappings",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
    ResourceLimitDescriptor {
        id: ResourceLimitId::FontDecodedExpansionRatioHardCap,
        stable_id: "font_decoded_expansion_ratio_hard_cap",
        phase: ResourceLimitPhase::FontDecode,
        description: "Non-overridable implementation cap for font decode expansion ratio",
        overridable: false,
        hard_cap: true,
        minimum_value: 1,
    },
];

pub static RESOURCE_LIMIT_DESCRIPTORS: [ResourceLimitDescriptor; RESOURCE_LIMIT_COUNT] = [
    input_descriptor(InputResourceLimitId::MaxSourceBytes),
    input_descriptor(InputResourceLimitId::MaxModelItems),
    input_descriptor(InputResourceLimitId::MaxModelTextBytes),
    input_descriptor(InputResourceLimitId::MaxModelNestingDepth),
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[2],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[0],
    RENDER_RESOURCE_LIMIT_DESCRIPTORS[1],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[0],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[1],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[2],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[3],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[4],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[5],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[6],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[7],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[8],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[9],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[10],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[11],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[12],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[13],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[14],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[15],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[16],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[17],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[18],
    THEME_RESOURCE_LIMIT_DESCRIPTORS[19],
];

const RENDER_PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT];
    RENDER_RESOURCE_LIMIT_COUNT] = [
    [Some(24 * MIB), Some(12 * MIB), Some(128 * MIB), None],
    [Some(250_000), Some(125_000), Some(1_000_000), None],
    // A policy budget, not a Mermaid limit. Families charge deterministic
    // units for derived geometry and inspected layout candidates. The interactive
    // ceiling admits the repository's normal large public fixtures with calibration
    // headroom while the constrained profile remains the untrusted-input boundary.
    [Some(800_000), Some(125_000), Some(1_000_000), None],
];

const THEME_PROFILE_VALUES: [[Option<usize>; RESOURCE_PROFILE_COUNT]; THEME_RESOURCE_LIMIT_COUNT] = [
    [Some(2 * MIB), Some(512 * KIB), Some(8 * MIB), None],
    [Some(2 * MIB), Some(512 * KIB), Some(8 * MIB), None],
    [Some(4 * MIB), Some(MIB), Some(8 * MIB), None],
    [Some(16 * MIB), Some(4 * MIB), Some(32 * MIB), None],
    [Some(32 * MIB), Some(8 * MIB), Some(64 * MIB), None],
    [Some(16), Some(8), Some(32), None],
    [Some(32), Some(16), Some(128), None],
    [Some(2_048), Some(512), Some(4_096), None],
    [Some(128), Some(64), Some(256), None],
    [Some(32), Some(16), Some(64), None],
    [Some(MAX_THEME_ENCODED_BYTES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_THEME_BASE64_BYTES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_ASSET_COMPRESSED_BYTES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_ASSET_DECODED_BYTES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_CATALOG_DECODED_BYTES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_ASSETS_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_FACES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_TABLES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_ALIASES_HARD_CAP); RESOURCE_PROFILE_COUNT],
    [Some(MAX_FONT_DECODED_EXPANSION_RATIO_HARD_CAP); RESOURCE_PROFILE_COUNT],
];

pub const GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    merman_core::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE;
pub const CLI_DEFAULT_RESOURCE_PROFILE: RenderResourceProfile =
    merman_core::resources::CLI_DEFAULT_RESOURCE_PROFILE;

pub const fn resource_profile_descriptors() -> &'static [RenderResourceProfileDescriptor] {
    &merman_core::resources::RESOURCE_PROFILE_DESCRIPTORS
}

pub const fn resource_limit_descriptors() -> &'static [ResourceLimitDescriptor] {
    &RESOURCE_LIMIT_DESCRIPTORS
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResourceLimitOverrideError {
    #[error("resource limit id `{0}` is not part of resource contract schema 1")]
    UnknownLimit(String),
    #[error("resource limit `{0}` is a hard implementation capability and cannot be overridden")]
    HardCap(&'static str),
    #[error("resource limit `{0}` must be a positive integer")]
    NonPositive(&'static str),
    #[error("resource limit `{id}` must be at least {minimum}")]
    BelowMinimum { id: &'static str, minimum: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "resource policy would loosen `{}`: requested {:?}, ceiling {:?}",
    .id.as_str(),
    .requested,
    .ceiling
)]
pub struct ResourcePolicyRestrictionError {
    pub id: ResourceLimitId,
    pub requested: Option<usize>,
    pub ceiling: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderResourcePolicy {
    input: InputResourcePolicy,
    render_base_values: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
    render_effective_values: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
    render_explicit_overrides: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
    theme_base_values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    theme_effective_values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    theme_explicit_overrides: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
}

impl Default for RenderResourcePolicy {
    fn default() -> Self {
        Self::interactive()
    }
}

impl RenderResourcePolicy {
    pub const fn profile(self) -> RenderResourceProfile {
        self.input.profile()
    }

    pub const fn interactive() -> Self {
        Self::for_profile(RenderResourceProfile::Interactive)
    }

    pub const fn constrained() -> Self {
        Self::for_profile(RenderResourceProfile::Constrained)
    }

    pub const fn trusted_native() -> Self {
        Self::for_profile(RenderResourceProfile::TrustedNative)
    }

    pub const fn unbounded_for_trusted_input() -> Self {
        Self::for_profile(RenderResourceProfile::UnboundedForTrustedInput)
    }

    pub const fn for_profile(profile: RenderResourceProfile) -> Self {
        let mut render_values = [None; RENDER_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < RENDER_RESOURCE_LIMIT_COUNT {
            render_values[index] = RENDER_PROFILE_VALUES[index][profile as usize];
            index += 1;
        }
        let mut theme_values = [None; THEME_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < THEME_RESOURCE_LIMIT_COUNT {
            theme_values[index] = THEME_PROFILE_VALUES[index][profile as usize];
            index += 1;
        }
        Self {
            input: InputResourcePolicy::for_profile(profile),
            render_base_values: render_values,
            render_effective_values: render_values,
            render_explicit_overrides: [None; RENDER_RESOURCE_LIMIT_COUNT],
            theme_base_values: theme_values,
            theme_effective_values: theme_values,
            theme_explicit_overrides: [None; THEME_RESOURCE_LIMIT_COUNT],
        }
    }

    pub const fn input_policy(&self) -> &InputResourcePolicy {
        &self.input
    }

    pub const fn value(self, id: ResourceLimitId) -> Option<usize> {
        match id {
            ResourceLimitId::Input(id) => self.input.value(id),
            ResourceLimitId::Render(id) => self.render_effective_values[id.index()],
            ResourceLimitId::Theme(id) => self.theme_effective_values[id.index()],
        }
    }

    pub const fn base_value(self, id: ResourceLimitId) -> Option<usize> {
        match id {
            ResourceLimitId::Input(id) => self.input.base_value(id),
            ResourceLimitId::Render(id) => self.render_base_values[id.index()],
            ResourceLimitId::Theme(id) => self.theme_base_values[id.index()],
        }
    }

    pub const fn explicit_override(self, id: ResourceLimitId) -> Option<usize> {
        match id {
            ResourceLimitId::Input(id) => self.input.explicit_override(id),
            ResourceLimitId::Render(id) => self.render_explicit_overrides[id.index()],
            ResourceLimitId::Theme(id) => self.theme_explicit_overrides[id.index()],
        }
    }

    pub fn explicit_overrides(&self) -> impl Iterator<Item = (ResourceLimitId, usize)> + '_ {
        ResourceLimitId::ALL
            .into_iter()
            .filter_map(|id| self.explicit_override(id).map(|value| (id, value)))
    }

    pub fn apply_override(
        &mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<(), ResourceLimitOverrideError> {
        let id = ResourceLimitId::from_stable_id(stable_id)
            .ok_or_else(|| ResourceLimitOverrideError::UnknownLimit(stable_id.to_string()))?;
        self.apply_limit(id, value)
    }

    pub fn apply_limit(
        &mut self,
        id: ResourceLimitId,
        value: usize,
    ) -> Result<(), ResourceLimitOverrideError> {
        match id {
            ResourceLimitId::Input(id) => {
                self.input
                    .apply_limit(id, value)
                    .map_err(|error| match error {
                        merman_core::resources::InputResourceLimitOverrideError::UnknownLimit(
                            id,
                        ) => ResourceLimitOverrideError::UnknownLimit(id),
                        merman_core::resources::InputResourceLimitOverrideError::NonPositive(
                            id,
                        ) => ResourceLimitOverrideError::NonPositive(id),
                    })
            }
            ResourceLimitId::Render(id) => {
                let descriptor = RENDER_RESOURCE_LIMIT_DESCRIPTORS[id.index()];
                if descriptor.hard_cap || !descriptor.overridable {
                    return Err(ResourceLimitOverrideError::HardCap(descriptor.stable_id));
                }
                if value == 0 {
                    return Err(ResourceLimitOverrideError::NonPositive(
                        descriptor.stable_id,
                    ));
                }
                self.render_effective_values[id.index()] = Some(value);
                self.render_explicit_overrides[id.index()] = Some(value);
                Ok(())
            }
            ResourceLimitId::Theme(id) => {
                let descriptor = THEME_RESOURCE_LIMIT_DESCRIPTORS[id.index()];
                if descriptor.hard_cap || !descriptor.overridable {
                    return Err(ResourceLimitOverrideError::HardCap(descriptor.stable_id));
                }
                if value < descriptor.minimum_value {
                    return Err(ResourceLimitOverrideError::BelowMinimum {
                        id: descriptor.stable_id,
                        minimum: descriptor.minimum_value,
                    });
                }
                self.theme_effective_values[id.index()] = Some(value);
                self.theme_explicit_overrides[id.index()] = Some(value);
                Ok(())
            }
        }
    }

    /// Returns the pointwise minimum of two policies while preserving this policy's host profile.
    pub fn meet(&self, restriction: &Self) -> Self {
        let input = self.input.meet(&restriction.input);
        let mut render_effective_values = [None; RENDER_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < RENDER_RESOURCE_LIMIT_COUNT {
            render_effective_values[index] = minimum_ceiling(
                self.render_effective_values[index],
                restriction.render_effective_values[index],
            );
            index += 1;
        }
        let mut theme_effective_values = [None; THEME_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < THEME_RESOURCE_LIMIT_COUNT {
            theme_effective_values[index] = minimum_ceiling(
                self.theme_effective_values[index],
                restriction.theme_effective_values[index],
            );
            index += 1;
        }
        self.with_effective_values(input, render_effective_values, theme_effective_values)
    }

    /// Applies a request policy only when every requested ceiling is at least as strict.
    pub fn restrict_with(
        &self,
        restriction: &Self,
    ) -> Result<Self, ResourcePolicyRestrictionError> {
        self.input
            .restrict_with(&restriction.input)
            .map_err(map_input_restriction_error)?;
        for id in ResourceLimitId::ALL {
            if matches!(id, ResourceLimitId::Input(_)) {
                continue;
            }
            let ceiling = self.value(id);
            let requested = restriction.value(id);
            if loosens_ceiling(ceiling, requested) {
                return Err(ResourcePolicyRestrictionError {
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
        input: InputResourcePolicy,
        render_effective_values: [Option<usize>; RENDER_RESOURCE_LIMIT_COUNT],
        theme_effective_values: [Option<usize>; THEME_RESOURCE_LIMIT_COUNT],
    ) -> Self {
        let mut render_explicit_overrides = [None; RENDER_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < RENDER_RESOURCE_LIMIT_COUNT {
            if render_effective_values[index] != self.render_base_values[index] {
                render_explicit_overrides[index] = render_effective_values[index];
            }
            index += 1;
        }
        let mut theme_explicit_overrides = [None; THEME_RESOURCE_LIMIT_COUNT];
        let mut index = 0;
        while index < THEME_RESOURCE_LIMIT_COUNT {
            if theme_effective_values[index] != self.theme_base_values[index] {
                theme_explicit_overrides[index] = theme_effective_values[index];
            }
            index += 1;
        }
        Self {
            input,
            render_base_values: self.render_base_values,
            render_effective_values,
            render_explicit_overrides,
            theme_base_values: self.theme_base_values,
            theme_effective_values,
            theme_explicit_overrides,
        }
    }

    pub fn with_override(
        mut self,
        stable_id: &str,
        value: usize,
    ) -> Result<Self, ResourceLimitOverrideError> {
        self.apply_override(stable_id, value)?;
        Ok(self)
    }

    pub fn with_limit(
        mut self,
        id: ResourceLimitId,
        value: usize,
    ) -> Result<Self, ResourceLimitOverrideError> {
        self.apply_limit(id, value)?;
        Ok(self)
    }

    fn check_render_limit(
        &self,
        phase: ResourceLimitPhase,
        id: RenderResourceLimitId,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        let Some(max) = self.render_effective_values[id.index()] else {
            return Ok(());
        };
        if actual <= max {
            return Ok(());
        }
        let limit = ResourceLimitId::Render(id);
        Err(ResourceLimitExceeded {
            cause: ResourceLimitCause::Ceiling,
            phase,
            limit: limit.as_str(),
            actual,
            max,
            profile: self.profile(),
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        })
    }

    fn check_theme_limit(
        &self,
        id: ThemeResourceLimitId,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        let descriptor = THEME_RESOURCE_LIMIT_DESCRIPTORS[id.index()];
        let Some(max) = self.theme_effective_values[id.index()] else {
            return Ok(());
        };
        if actual <= max {
            return Ok(());
        }
        Err(ResourceLimitExceeded {
            cause: ResourceLimitCause::Ceiling,
            phase: descriptor.phase,
            limit: descriptor.stable_id,
            actual,
            max,
            profile: self.profile(),
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        })
    }

    fn check_theme_limit_with_hard_cap(
        &self,
        policy: ThemeResourceLimitId,
        hard_cap: ThemeResourceLimitId,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit(hard_cap, actual)?;
        self.check_theme_limit(policy, actual)
    }

    pub fn check_theme_encoded_bytes(&self, actual: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxThemeEncodedBytes,
            ThemeResourceLimitId::ThemeEncodedBytesHardCap,
            actual,
        )
    }

    pub fn check_theme_base64_bytes(&self, actual: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxThemeBase64Bytes,
            ThemeResourceLimitId::ThemeBase64BytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_asset_compressed_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAssetCompressedBytes,
            ThemeResourceLimitId::FontAssetCompressedBytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_asset_decoded_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAssetDecodedBytes,
            ThemeResourceLimitId::FontAssetDecodedBytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_catalog_decoded_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontCatalogDecodedBytes,
            ThemeResourceLimitId::FontCatalogDecodedBytesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_asset_count(
        &self,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAssets,
            ThemeResourceLimitId::FontAssetsHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_face_count(&self, actual: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontFaces,
            ThemeResourceLimitId::FontFacesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_table_count(
        &self,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontTables,
            ThemeResourceLimitId::FontTablesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_alias_count(
        &self,
        actual: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontAliases,
            ThemeResourceLimitId::FontAliasesHardCap,
            actual,
        )
    }

    pub(crate) fn check_font_decoded_expansion(
        &self,
        compressed: usize,
        decoded: usize,
    ) -> Result<(), ResourceLimitExceeded> {
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
        self.check_theme_limit_with_hard_cap(
            ThemeResourceLimitId::MaxFontDecodedExpansionRatio,
            ThemeResourceLimitId::FontDecodedExpansionRatioHardCap,
            actual,
        )
    }

    pub fn check_source_bytes(&self, source: &str) -> Result<(), ResourceLimitExceeded> {
        self.input
            .check_source_bytes(source)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_render_model(
        &self,
        model: &RenderSemanticModel,
    ) -> Result<(), ResourceLimitExceeded> {
        let complexity = ModelComplexity::from_render_model(model);
        self.check_render_model_complexity(model, complexity)
    }

    pub fn check_parsed_render(
        &self,
        parsed: &ParsedDiagramRender,
    ) -> Result<(), ResourceLimitExceeded> {
        let mut complexity = ModelComplexity::from_render_model(parsed.model());
        complexity.text_bytes = complexity
            .text_bytes
            .saturating_add(parsed.retained_render_context_bytes());
        self.check_render_model_complexity(parsed.model(), complexity)
    }

    fn check_render_model_complexity(
        &self,
        model: &RenderSemanticModel,
        complexity: ModelComplexity,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_model_complexity(complexity)?;

        if matches!(
            model,
            RenderSemanticModel::Treemap(_) | RenderSemanticModel::Ishikawa(_)
        ) && complexity.nesting_depth > MAX_RECURSIVE_MODEL_TREE_DEPTH
        {
            return Err(ResourceLimitExceeded {
                cause: ResourceLimitCause::Ceiling,
                phase: ResourceLimitPhase::LayoutModel,
                limit: "typed_model_tree_depth",
                actual: complexity.nesting_depth,
                max: MAX_RECURSIVE_MODEL_TREE_DEPTH,
                profile: self.profile(),
                explicit_overrides: self
                    .explicit_overrides()
                    .map(|(id, value)| ResourceLimitOverride { id, value })
                    .collect(),
            });
        }

        Ok(())
    }

    pub fn check_model_complexity(
        &self,
        complexity: ModelComplexity,
    ) -> Result<(), ResourceLimitExceeded> {
        self.input
            .check_model_complexity(complexity)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_svg_bytes(
        &self,
        svg: &str,
        phase: ResourceLimitPhase,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(phase, RenderResourceLimitId::MaxSvgBytes, svg.len())
    }

    pub fn check_svg_structure(
        &self,
        elements: usize,
        tree_depth: usize,
    ) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(
            ResourceLimitPhase::SvgPostprocess,
            RenderResourceLimitId::MaxSvgElements,
            elements,
        )?;
        if elements > MAX_RESVG_TREE_NODES {
            return Err(ResourceLimitExceeded {
                cause: ResourceLimitCause::Ceiling,
                phase: ResourceLimitPhase::SvgPostprocess,
                limit: SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
                actual: elements,
                max: MAX_RESVG_TREE_NODES,
                profile: self.profile(),
                explicit_overrides: self
                    .explicit_overrides()
                    .map(|(id, value)| ResourceLimitOverride { id, value })
                    .collect(),
            });
        }
        if tree_depth <= MAX_RESVG_TREE_DEPTH {
            return Ok(());
        }
        Err(ResourceLimitExceeded {
            cause: ResourceLimitCause::Ceiling,
            phase: ResourceLimitPhase::SvgPostprocess,
            limit: SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
            actual: tree_depth,
            max: MAX_RESVG_TREE_DEPTH,
            profile: self.profile(),
            explicit_overrides: self
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        })
    }

    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartModel,
    ) -> Result<FlowchartComplexity, ResourceLimitExceeded> {
        self.input
            .check_flowchart_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, ResourceLimitExceeded> {
        self.input
            .check_class_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_mindmap_complexity(
        &self,
        model: &MindmapDiagramRenderModel,
    ) -> Result<MindmapComplexity, ResourceLimitExceeded> {
        self.input
            .check_mindmap_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramRenderModel,
    ) -> Result<ZenumlComplexity, ResourceLimitExceeded> {
        self.input
            .check_zenuml_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_sequence_complexity(
        &self,
        model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
    ) -> Result<SequenceComplexity, ResourceLimitExceeded> {
        self.input
            .check_sequence_complexity(model)
            .map_err(|error| ResourceLimitExceeded::from_input(self, error))
    }

    pub fn check_layout_work_units(&self, work_units: usize) -> Result<(), ResourceLimitExceeded> {
        self.check_render_limit(
            ResourceLimitPhase::LayoutModel,
            RenderResourceLimitId::MaxLayoutWorkUnits,
            work_units,
        )
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

fn map_input_restriction_error(
    error: InputResourcePolicyRestrictionError,
) -> ResourcePolicyRestrictionError {
    ResourcePolicyRestrictionError {
        id: ResourceLimitId::Input(error.id),
        requested: error.requested,
        ceiling: error.ceiling,
    }
}

/// One cumulative derived-geometry budget shared by layout and SVG emission.
pub(crate) struct OperationWorkMeter {
    policy: RenderResourcePolicy,
    used: std::sync::atomic::AtomicUsize,
}

impl OperationWorkMeter {
    pub(crate) fn new(policy: RenderResourcePolicy) -> Self {
        Self {
            policy,
            used: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub(crate) const fn policy(&self) -> RenderResourcePolicy {
        self.policy
    }

    /// Checks whether a phase estimate fits without reserving or consuming the estimate.
    pub(crate) fn preflight(&self, additional: usize) -> Result<(), ResourceLimitExceeded> {
        if additional == 0 {
            return Ok(());
        }
        let used = self.used.load(std::sync::atomic::Ordering::Relaxed);
        let next = used
            .checked_add(additional)
            .ok_or_else(|| self.arithmetic_overflow())?;
        self.policy.check_layout_work_units(next)
    }

    /// Charges work atomically. A rejected charge leaves the cumulative usage unchanged.
    pub(crate) fn charge(&self, additional: usize) -> Result<(), ResourceLimitExceeded> {
        if additional == 0 {
            return Ok(());
        }
        let mut used = self.used.load(std::sync::atomic::Ordering::Relaxed);
        loop {
            let next = used
                .checked_add(additional)
                .ok_or_else(|| self.arithmetic_overflow())?;
            self.policy.check_layout_work_units(next)?;
            match self.used.compare_exchange_weak(
                used,
                next,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => return Ok(()),
                Err(actual) => used = actual,
            }
        }
    }

    pub(crate) fn arithmetic_overflow(&self) -> ResourceLimitExceeded {
        let max = self.policy.render_effective_values
            [RenderResourceLimitId::MaxLayoutWorkUnits.index()]
        .unwrap_or(usize::MAX);
        ResourceLimitExceeded {
            cause: ResourceLimitCause::ArithmeticOverflow,
            phase: ResourceLimitPhase::LayoutModel,
            limit: ResourceLimitId::Render(RenderResourceLimitId::MaxLayoutWorkUnits).as_str(),
            actual: usize::MAX,
            max,
            profile: self.policy.profile(),
            explicit_overrides: self
                .policy
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        }
    }

    pub(crate) fn used(&self) -> usize {
        self.used.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceLimitExceeded {
    pub cause: ResourceLimitCause,
    pub phase: ResourceLimitPhase,
    pub limit: &'static str,
    pub actual: usize,
    pub max: usize,
    pub profile: RenderResourceProfile,
    pub explicit_overrides: Vec<ResourceLimitOverride>,
}

impl std::fmt::Display for ResourceLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "resource limit exceeded during {}: {}",
            self.phase, self.limit
        )?;
        if self.cause == ResourceLimitCause::ArithmeticOverflow {
            write!(f, " cause={}", self.cause)?;
        }
        write!(f, " actual={} max={}", self.actual, self.max)
    }
}

impl std::error::Error for ResourceLimitExceeded {}

impl ResourceLimitExceeded {
    fn from_input(policy: &RenderResourcePolicy, error: InputResourceLimitExceeded) -> Self {
        Self {
            cause: ResourceLimitCause::Ceiling,
            phase: match error.phase {
                InputResourceLimitPhase::Source => ResourceLimitPhase::Source,
                InputResourceLimitPhase::Model => ResourceLimitPhase::LayoutModel,
            },
            limit: error.limit,
            actual: error.actual,
            max: error.max,
            profile: error.profile,
            explicit_overrides: policy
                .explicit_overrides()
                .map(|(id, value)| ResourceLimitOverride { id, value })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimitOverride {
    pub id: ResourceLimitId,
    pub value: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph};
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};
    use std::collections::HashSet;

    #[test]
    fn resource_contract_is_complete_unique_and_drives_every_profile() {
        assert_eq!(
            RESOURCE_PROFILE_DESCRIPTORS.len(),
            RenderResourceProfile::ALL.len()
        );
        assert_eq!(RESOURCE_LIMIT_DESCRIPTORS.len(), RESOURCE_LIMIT_COUNT);

        let profile_ids = RESOURCE_PROFILE_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.id)
            .collect::<HashSet<_>>();
        assert_eq!(profile_ids.len(), RESOURCE_PROFILE_DESCRIPTORS.len());
        assert_eq!(
            RESOURCE_PROFILE_DESCRIPTORS
                .iter()
                .filter(|descriptor| descriptor.recommended_binding_default)
                .map(|descriptor| descriptor.profile)
                .collect::<Vec<_>>(),
            vec![GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE]
        );
        for profile in RenderResourceProfile::ALL {
            let descriptor = profile.descriptor();
            assert_eq!(RenderResourceProfile::from_id(descriptor.id), Some(profile));
            let policy = RenderResourcePolicy::for_profile(profile);
            for limit in RESOURCE_LIMIT_DESCRIPTORS {
                assert_eq!(policy.profile(), profile);
                if limit.hard_cap {
                    assert!(!limit.overridable);
                    assert!(policy.value(limit.id).is_some());
                }
            }
        }

        let limit_ids = RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.stable_id)
            .collect::<HashSet<_>>();
        assert_eq!(limit_ids.len(), RESOURCE_LIMIT_DESCRIPTORS.len());
        for descriptor in RESOURCE_LIMIT_DESCRIPTORS {
            assert_eq!(
                ResourceLimitId::from_stable_id(descriptor.stable_id),
                Some(descriptor.id)
            );
            assert_eq!(descriptor.id.descriptor(), descriptor);
        }
    }

    #[test]
    fn resource_overrides_fail_closed_for_unknown_and_internal_ids() {
        let mut limits = RenderResourcePolicy::interactive();
        assert!(matches!(
            limits.apply_override("future_limit", 1),
            Err(ResourceLimitOverrideError::UnknownLimit(_))
        ));
        assert_eq!(
            limits.apply_override("max_svg_tree_depth", 1),
            Err(ResourceLimitOverrideError::UnknownLimit(
                "max_svg_tree_depth".to_string()
            ))
        );
        assert_eq!(
            limits.apply_override("max_svg_elements", 0),
            Err(ResourceLimitOverrideError::NonPositive("max_svg_elements"))
        );
        limits.apply_override("max_svg_elements", 7).unwrap();
        assert_eq!(limits.value(ResourceLimitId::MaxSvgElements), Some(7));
    }

    #[test]
    fn theme_resource_overrides_follow_descriptor_minimums_and_preserve_hard_caps() {
        let mut policy = RenderResourcePolicy::interactive();
        for id in ThemeResourceLimitId::ALL {
            let descriptor = ResourceLimitId::Theme(id).descriptor();
            if descriptor.hard_cap {
                assert_eq!(
                    policy.apply_limit(descriptor.id, descriptor.minimum_value),
                    Err(ResourceLimitOverrideError::HardCap(descriptor.stable_id))
                );
            } else if descriptor.minimum_value == 0 {
                policy.apply_limit(descriptor.id, 0).unwrap();
                assert_eq!(policy.value(descriptor.id), Some(0));
            } else {
                assert_eq!(
                    policy.apply_limit(descriptor.id, 0),
                    Err(ResourceLimitOverrideError::BelowMinimum {
                        id: descriptor.stable_id,
                        minimum: descriptor.minimum_value,
                    })
                );
            }
        }
    }

    #[test]
    fn resource_policy_meet_and_restriction_are_pointwise_monotonic_for_every_public_limit() {
        for descriptor in RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .copied()
            .filter(|descriptor| descriptor.overridable)
        {
            let host = RenderResourcePolicy::interactive();
            let ceiling = host
                .value(descriptor.id)
                .expect("interactive profile must bound every overridable limit");
            let stricter_value = descriptor.minimum_value.max(ceiling / 2);
            let stricter = host.with_limit(descriptor.id, stricter_value).unwrap();

            let met = host.meet(&stricter);
            assert_eq!(met.profile(), host.profile());
            assert_eq!(met.value(descriptor.id), Some(stricter_value));
            let restricted = host.restrict_with(&stricter).unwrap();
            assert_eq!(restricted.value(descriptor.id), Some(stricter_value));

            let looser_value = ceiling.checked_add(1).unwrap();
            let looser = host.with_limit(descriptor.id, looser_value).unwrap();
            assert_eq!(
                host.restrict_with(&looser),
                Err(ResourcePolicyRestrictionError {
                    id: descriptor.id,
                    requested: Some(looser_value),
                    ceiling: Some(ceiling),
                })
            );
        }

        let host = RenderResourcePolicy::unbounded_for_trusted_input();
        let restricted = host
            .restrict_with(&RenderResourcePolicy::constrained())
            .unwrap();
        assert_eq!(restricted.profile(), host.profile());
        for descriptor in RESOURCE_LIMIT_DESCRIPTORS {
            assert_eq!(
                restricted.value(descriptor.id),
                minimum_ceiling(
                    host.value(descriptor.id),
                    RenderResourcePolicy::constrained().value(descriptor.id)
                )
            );
        }
    }

    #[test]
    fn unbounded_policy_retains_every_theme_and_font_hard_cap() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
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
        ];

        assert_eq!(
            checks.map(|error| error.limit),
            expected,
            "unbounded profiles may disable policy ceilings but not implementation capabilities"
        );
    }

    #[test]
    fn zero_theme_limits_can_disable_custom_font_ingestion() {
        let policy = RenderResourcePolicy::interactive()
            .with_limit(ResourceLimitId::MaxFontAssets, 0)
            .unwrap()
            .with_limit(ResourceLimitId::MaxFontCatalogDecodedBytes, 0)
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

    #[test]
    fn resolved_svg_node_hard_cap_remains_active_for_unbounded_policy() {
        let error = RenderResourcePolicy::unbounded_for_trusted_input()
            .check_svg_structure(MAX_RESVG_TREE_NODES + 1, 0)
            .unwrap_err();

        assert_eq!(error.limit, SVG_BACKEND_TREE_NODES_HARD_CAP_ID);
        assert_eq!(error.actual, MAX_RESVG_TREE_NODES + 1);
        assert_eq!(error.max, MAX_RESVG_TREE_NODES);
    }

    #[test]
    fn source_limit_reports_structured_error() {
        let err = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSourceBytes, 4)
            .unwrap()
            .with_limit(ResourceLimitId::MaxSvgBytes, 123)
            .unwrap()
            .check_source_bytes("12345")
            .unwrap_err();

        assert_eq!(err.phase, ResourceLimitPhase::Source);
        assert_eq!(err.limit, "max_source_bytes");
        assert_eq!(err.actual, 5);
        assert_eq!(err.max, 4);
        assert_eq!(
            err.explicit_overrides,
            vec![
                ResourceLimitOverride {
                    id: ResourceLimitId::MaxSourceBytes,
                    value: 4,
                },
                ResourceLimitOverride {
                    id: ResourceLimitId::MaxSvgBytes,
                    value: 123,
                },
            ]
        );
    }

    #[test]
    fn derived_layout_work_limits_report_the_owned_phase_and_metric() {
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 7)
            .unwrap();

        let error = limits.check_layout_work_units(8).unwrap_err();
        assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.cause, ResourceLimitCause::Ceiling);
        assert_eq!(error.actual, 8);
        assert_eq!(error.max, 7);
    }

    #[test]
    fn operation_work_meter_preflight_does_not_consume_budget() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 7)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);

        meter.preflight(7).unwrap();
        meter.preflight(7).unwrap();
        assert_eq!(meter.used(), 0);
        meter.charge(7).unwrap();
        assert_eq!(meter.used(), 7);
    }

    #[test]
    fn operation_work_meter_rejected_charge_does_not_advance() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 10)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        meter.charge(8).unwrap();

        let error = meter.charge(usize::MAX).unwrap_err();
        assert_eq!(error.cause, ResourceLimitCause::ArithmeticOverflow);
        assert_eq!(error.actual, usize::MAX);
        assert_eq!(error.max, 10);
        assert_eq!(meter.used(), 8);
        meter.charge(2).unwrap();
        assert_eq!(meter.used(), 10);
    }

    #[test]
    fn operation_work_meter_overflow_fails_under_unlimited_policy() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        meter.charge(usize::MAX).unwrap();

        let preflight_error = meter.preflight(1).unwrap_err();
        assert_eq!(
            preflight_error.cause,
            ResourceLimitCause::ArithmeticOverflow
        );
        assert_eq!(preflight_error.limit, "max_layout_work_units");
        assert_eq!(preflight_error.max, usize::MAX);
        assert_eq!(meter.used(), usize::MAX);

        let charge_error = meter.charge(1).unwrap_err();
        assert_eq!(charge_error.cause, ResourceLimitCause::ArithmeticOverflow);
        assert_eq!(meter.used(), usize::MAX);
    }

    #[test]
    fn zenuml_complexity_includes_inline_decorations() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "zenuml\nA->[rocket]B.call()\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model() else {
            panic!("expected ZenUML model");
        };
        let complexity = ZenumlComplexity::from_model(model);

        assert_eq!(complexity.participants, 2);
        assert_eq!(complexity.statements, 1);
        let required = ["rocket", "call()"]
            .into_iter()
            .map(str::len)
            .sum::<usize>();
        assert!(complexity.label_bytes >= required);
    }

    #[test]
    fn zenuml_uses_the_shared_model_budget() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "zenuml\nA.call() {\n  if(ok) {\n    if(inner) {\n      B.work()\n    }\n  }\n}\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model() else {
            panic!("expected ZenUML model");
        };

        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxModelItems, 1)
            .unwrap();
        let error = limits.check_zenuml_complexity(model).unwrap_err();
        assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(error.limit, "max_model_items");
    }

    #[test]
    fn flowchart_complexity_counts_layout_nodes_and_labels() {
        let model = FlowchartModel {
            keyword: "graph".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: None,
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: vec![FlowNode {
                id: "A".to_string(),
                label: Some("Alpha".to_string()),
                label_type: None,
                layout_shape: None,
                shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            }],
            edges: vec![FlowEdge {
                id: "L-A-B".to_string(),
                from: "A".to_string(),
                to: "B".to_string(),
                label: Some("edge".to_string()),
                label_type: None,
                edge_type: None,
                arrow: "-->".to_string(),
                is_user_defined_id: false,
                stroke: None,
                interpolate: None,
                classes: Vec::new(),
                style: Vec::new(),
                animate: None,
                animation: None,
                length: 1,
            }],
            subgraphs: vec![FlowSubgraph {
                id: "cluster".to_string(),
                title: "Cluster".to_string(),
                dir: None,
                has_explicit_dir: false,
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: vec!["A".to_string()],
            }],
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        };

        let complexity = FlowchartComplexity::from_model(&model);
        assert_eq!(complexity.nodes, 2);
        assert_eq!(complexity.edges, 1);
        assert_eq!(complexity.subgraphs, 1);
        assert!(complexity.label_bytes >= "AlphaedgeCluster".len());
    }
}
