//! SVG renderers for Mermaid-parity diagrams.
//!
//! Public API is re-exported from the parity-focused renderer implementation.
//!
//! This module is named `parity` to reflect intent: upstream Mermaid is treated as the spec, and
//! SVG output is gated by DOM parity checks.

#![forbid(unsafe_code)]

mod css_identifier;
mod fallback;
mod icon_registry;
mod parity;
mod pipeline;
pub(crate) mod scanner;

pub(crate) use css_identifier::escape_css_identifier;
pub(crate) use parity::FlowchartEdgeStylePlan;
#[cfg(feature = "layout-cytoscape")]
pub(crate) use parity::render_architecture_family_artifact;
pub(crate) use parity::theme as render_theme;
#[cfg(test)]
pub(crate) use parity::write_flowchart_svg_label_plan_for_test;
pub(crate) use parity::{RootThemeAppliedSvg, render_builtin_family_artifact};
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
    SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor, StripForeignObjectPostprocessor,
    SvgFinalizationReport, SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext,
    SvgPostprocessMetadata, SvgPostprocessor, SvgReferencePlan, SvgResourceClosure,
    SvgResourceFingerprint, finalize_resvg_svg,
};
