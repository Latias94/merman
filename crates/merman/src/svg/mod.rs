//! SVG target-local adapters and host-service types.
//!
//! Source parsing, operation ownership, cancellation, deadlines, and target dispatch live in
//! [`crate::Renderer`]. This module deliberately exposes only the SVG adapter vocabulary needed
//! to construct a typed [`crate::SvgRequest`]; it does not contain a second source-to-SVG
//! orchestration path.

pub use crate::SvgEnvironment;
pub use merman_model::family::DiagramFamilyId;
pub use merman_render::environment::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
    TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfile, TextMeasurementProfileIdentity,
    TextMeasurementReport, TextMeasurementResultKind, TextMeasurementRoute, TextMeasurementSource,
    TextMeasurementSummary, validate_host_text_measurement,
};
pub use merman_render::family::RenderCapabilityPlan;
pub use merman_render::resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, RenderResourceLimitId,
    RenderResourcePolicy, RenderResourceProfile, RenderResourceProfileDescriptor,
    ResourceLimitCause, ResourceLimitDescriptor, ResourceLimitExceeded, ResourceLimitId,
    ResourceLimitOverride, ResourceLimitOverrideError, ResourceLimitPhase,
    ResourcePolicyRestrictionError, SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
    WASM_RESVG_TREE_DEPTH_HARD_CAP, resource_limit_descriptors, resource_profile_descriptors,
};
pub use merman_render::svg::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor, IconPack,
    IconRegistry, IconRegistryBuildError, IconRegistryBuildErrorKind, IconRegistryBuilder,
    IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId, ResvgCompatibleSvg,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor, SvgDebugOptions,
    SvgFinalizationReport, SvgGeneration, SvgOutputPolicy, SvgOutputProfile, SvgPipeline,
    SvgPipelinePreset, SvgPostprocessContext, SvgPostprocessMetadata, SvgPostprocessor,
    SvgRenderOptions, SvgResourceClosure, finalize_resvg_svg,
    foreign_object_label_fallback_svg_text, icon_registry_resource_limit_descriptors,
};
pub use merman_render::text::{
    DeterministicTextMeasurer, FontCatalog, FontCatalogFingerprint, FontSource, FontSourcePolicy,
    FontSourcePolicyError, FontStack, FontStackError, FontStyle, GenericFontFamily, TextMeasurer,
    TextMetrics, TextStyle, WrapMode,
};
pub use merman_render::theme::ThemePortabilityRequirement;
pub use merman_render::{
    Error as RenderError, LayoutAdaptation, LayoutAdaptationObservation, LayoutOptions,
    RenderCapability, RenderCapabilityPolicy, Result as RenderResult, ShoalLayoutFrame,
    layout_cytoscape_available, layout_dagre_available, layout_elk_available,
    layout_shoal_available, math_available,
};

/// Reports whether this facade includes Mermaid Grid for any compiled family.
/// Consult the operation's capability plan for session admission.
pub const fn layout_grid_available() -> bool {
    cfg!(all(
        feature = "layout-grid",
        any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow",
            feature = "diagram-er",
            feature = "diagram-mindmap"
        )
    )) && merman_render::layout_grid_available()
}

/// Reports whether this facade includes Mermaid Layered for any compiled family.
/// Consult the operation's capability plan for session admission.
pub const fn layout_layered_available() -> bool {
    cfg!(all(
        feature = "layout-layered",
        any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        )
    )) && merman_render::layout_layered_available()
}

/// Binary export APIs accept only terminally validated [`ResvgCompatibleSvg`] values.
#[cfg(any(
    feature = "native-fonts",
    feature = "png",
    feature = "jpeg",
    feature = "pdf"
))]
pub mod export {
    pub use merman_export::*;
}

/// Converts arbitrary text into a conservative SVG `id` token.
pub fn sanitize_svg_id(raw: &str) -> String {
    merman_render::svg::sanitize_svg_id(raw)
}

#[cfg(feature = "diagram-class")]
pub use merman_render::ClassComplexity;

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub use merman_render::FlowchartComplexity;

#[cfg(feature = "diagram-mindmap")]
pub use merman_render::MindmapComplexity;

#[cfg(test)]
mod sanitize_svg_id_tests {
    use super::sanitize_svg_id;

    #[test]
    fn sanitize_svg_id_empty_is_untitled() {
        assert_eq!(sanitize_svg_id(""), "m-untitled");
        assert_eq!(sanitize_svg_id("   "), "m-untitled");
    }

    #[test]
    fn sanitize_svg_id_trims_and_replaces() {
        assert_eq!(sanitize_svg_id(" my diagram "), "my-diagram");
        assert_eq!(sanitize_svg_id("a b\tc"), "a-b-c");
    }

    #[test]
    fn sanitize_svg_id_prefixes_when_needed() {
        assert_eq!(sanitize_svg_id("1a"), "m-1a");
        assert_eq!(sanitize_svg_id("_a"), "m-_a");
        assert_eq!(sanitize_svg_id("-a"), "m-a");
    }

    #[test]
    fn sanitize_svg_id_collapses_and_trims_dashes() {
        assert_eq!(sanitize_svg_id("a----b"), "a-b");
        assert_eq!(sanitize_svg_id("abc--"), "abc");
        assert_eq!(sanitize_svg_id("--"), "m-untitled");
        assert_eq!(sanitize_svg_id("-"), "m-untitled");
    }

    #[test]
    fn sanitize_svg_id_keeps_css_safe_punctuation() {
        assert_eq!(sanitize_svg_id("a:b.c_d"), "a-b-c_d");
    }

    #[test]
    fn sanitize_svg_id_m_is_reserved_for_untitled() {
        assert_eq!(sanitize_svg_id("m"), "m-untitled");
        assert_eq!(sanitize_svg_id("m-"), "m-untitled");
        assert_eq!(sanitize_svg_id("m--"), "m-untitled");
    }
}
