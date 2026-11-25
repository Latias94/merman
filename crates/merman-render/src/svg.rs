//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from the parity-focused renderer implementation.
//!
//! This module is named `parity` to reflect intent: upstream Mermaid is treated as the spec, and
//! SVG output is gated by DOM parity checks.

#![forbid(unsafe_code)]

mod fallback;
pub(crate) use fallback::measure_styled_html_label;
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
#[cfg(all(feature = "diagram-class", feature = "layout-shoal"))]
pub(crate) use parity::class_canonical_marker;
pub(crate) use parity::cssom_color_value;
#[cfg(all(feature = "diagram-er", feature = "layout-shoal"))]
pub(crate) use parity::er_canonical_marker;
#[cfg(any(
    feature = "diagram-er",
    feature = "diagram-requirement",
    feature = "diagram-pie",
    feature = "diagram-treemap",
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-zenuml",
    feature = "diagram-kanban",
))]
pub(crate) use parity::escape_attr;
#[cfg(any(feature = "diagram-er", feature = "diagram-requirement"))]
pub(crate) use parity::escape_xml;
#[cfg(all(feature = "diagram-state", feature = "layout-shoal"))]
pub(crate) use parity::state_canonical_marker;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use parity::{FlowchartRenderConfig, prepare_flowchart_render_config};
mod pipeline;

pub use fallback::foreign_object_label_fallback_svg_text;
pub use icon_registry::{
    IconPack, IconRegistry, IconRegistryBuildError, IconRegistryBuildErrorKind,
    IconRegistryBuilder, IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId,
    icon_registry_resource_limit_descriptors,
};
pub(crate) use merman_render_core::svg_css::escape_css_identifier;
#[cfg(all(
    feature = "diagram-architecture",
    any(feature = "layout-cytoscape", feature = "layout-shoal")
))]
pub(crate) use parity::render_architecture_family_artifact;
pub(crate) use parity::theme as render_theme;
pub use parity::*;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use parity::{FlowchartEdgeStylePlan, FlowchartNodeLayoutView, FlowchartPreparedNodes};
pub(crate) use parity::{edge_geometry_family_artifact, render_builtin_family_artifact};
pub(crate) use pipeline::SvgPostprocessExecution;
pub use pipeline::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    ResvgCompatibleSvg, RootBackgroundPostprocessor, SanitizeCssPostprocessor,
    SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor, StandaloneSvgArtifact,
    StandaloneSvgTerminalStatus, StripForeignObjectPostprocessor, SvgFinalizationReport,
    SvgGeneration, SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext,
    SvgPostprocessMetadata, SvgPostprocessor, SvgReferencePlan, SvgResourceClosure,
    finalize_resvg_svg, rebase_svg_ids, validate_static_inline_svg,
    validate_static_inline_svg_admission,
};
