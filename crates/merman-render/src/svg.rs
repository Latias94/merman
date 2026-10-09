//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from the parity-focused renderer implementation.
//!
//! This module is named `parity` to reflect intent: upstream Mermaid is treated as the spec, and
//! SVG output is gated by DOM parity checks.

#![forbid(unsafe_code)]

/// Suffix reserved for renderer-owned paths whose SVG paint channel differs from their semantic
/// facet. Native exporters may use this marker only after accepting a sealed renderer artifact.
#[doc(hidden)]
pub const RENDERER_SEMANTIC_FILL_PATH_SUFFIX: &str = "-merman-fill-paint";

/// Suffix reserved for renderer-owned paths whose stroke channel proves both semantic fill and
/// stroke facets. Native exporters may use this marker only after accepting a sealed renderer
/// artifact.
#[doc(hidden)]
pub const RENDERER_SEMANTIC_FILL_AND_STROKE_PATH_SUFFIX: &str = "-merman-fill-stroke-paint";

/// Suffix reserved for renderer-owned paths or groups whose native SVG paint channel is the
/// semantic fill channel. Native exporters may use this marker only after accepting a sealed
/// renderer artifact.
#[doc(hidden)]
pub const RENDERER_SEMANTIC_NATIVE_PAINT_SUFFIX: &str = "-merman-native-paint";

mod css_identifier;
mod fallback;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        unused_imports,
        reason = "Shared rendering utilities have different callers in each diagram selection."
    )
)]
mod icon_registry;
mod parity;
pub(crate) use parity::BaseEdgeMarkerKind;
pub(crate) use parity::PreparedCommonCss;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-usecase"
))]
pub(crate) use parity::PreparedCommonNeoCss;
pub(crate) use parity::PreparedLookDefs;
pub(crate) use parity::cssom_color_value;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use parity::{
    FlowchartRenderConfig, flowchart_node_label_fill_config_override,
    prepare_flowchart_render_config,
};
#[cfg(any(feature = "diagram-er", feature = "diagram-requirement"))]
pub(crate) use parity::{escape_attr, escape_xml};
mod pipeline;
pub(crate) mod scanner;

pub(crate) use css_identifier::escape_css_identifier;
pub(crate) use fallback::{
    FALLBACK_BACKGROUND_FILL_DATA_ATTR, FALLBACK_OCCURRENCE_DATA_ATTR,
    PREPARED_TEXT_LABEL_DATA_ATTR,
};
pub(crate) use icon_registry::IconCurrentColorUse;
#[cfg(all(feature = "layout-cytoscape", feature = "diagram-architecture"))]
pub(crate) use parity::render_architecture_family_artifact;
pub(crate) use parity::theme as render_theme;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use parity::{FlowchartEdgeStylePlan, FlowchartNodeLayoutView, FlowchartPreparedNodes};
pub(crate) use parity::{RootThemeAppliedSvg, render_builtin_family_artifact};
pub(crate) use pipeline::SvgPostprocessExecution;
pub(crate) use pipeline::partition_prepared_text_label_ids;

pub use fallback::foreign_object_label_fallback_svg_text;
pub use icon_registry::{
    IconPack, IconRegistry, IconRegistryBuildError, IconRegistryBuildErrorKind,
    IconRegistryBuilder, IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId,
    icon_registry_resource_limit_descriptors,
};
pub use parity::*;
pub use pipeline::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    ResvgCompatibleSvg, RootBackgroundPostprocessor, SanitizeCssPostprocessor,
    SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor, StandaloneSvgArtifact,
    StandaloneSvgTerminalStatus, StripForeignObjectPostprocessor, SvgFinalizationReport,
    SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext, SvgPostprocessMetadata,
    SvgPostprocessor, SvgReferencePlan, SvgResourceClosure, SvgResourceFingerprint,
    finalize_resvg_svg, rebase_svg_ids, validate_static_inline_svg,
    validate_static_inline_svg_admission,
};

#[cfg(feature = "diagram-block")]
pub(crate) use parity::block_edge_path_data;
