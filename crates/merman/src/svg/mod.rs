//! SVG target-local adapters and host-service types.
//!
//! Source parsing, operation ownership, cancellation, deadlines, and target dispatch live in
//! [`crate::Renderer`]. This module deliberately exposes only the SVG adapter vocabulary needed
//! to construct a typed [`crate::SvgRequest`]; it does not contain a second source-to-SVG
//! orchestration path.

pub use crate::SvgEnvironment;
pub use merman_render::environment::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId,
    TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfile, TextMeasurementProfileIdentity,
    TextMeasurementReport, TextMeasurementResultKind, TextMeasurementRoute, TextMeasurementSource,
    TextMeasurementSummary, validate_host_text_measurement,
};
pub use merman_render::family::{RenderCapabilityPlan, RenderFamilyKind};
#[cfg(feature = "math")]
pub use merman_render::math::RatexMathRenderer;
pub use merman_render::math::{MathRenderer, NoopMathRenderer};
pub use merman_render::presentation::{
    HostTheme, HostThemeAppearance, HostThemePreset, Presentation, PresentationAspectApplicability,
    PresentationAspectDescriptor, PresentationAspectResolution, PresentationAspectState,
    PresentationError, PresentationProfile, PresentationProfileDescriptor,
    PresentationRenderPolicy, ResolvedPresentation, ThemeRole, presentation_profile_descriptors,
    theme_preset_descriptors,
};
pub use merman_render::resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, ClassComplexity, FlowchartComplexity,
    GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, MindmapComplexity, RenderResourceLimitId,
    RenderResourcePolicy, RenderResourceProfile, RenderResourceProfileDescriptor,
    ResourceLimitCause, ResourceLimitDescriptor, ResourceLimitExceeded, ResourceLimitId,
    ResourceLimitOverride, ResourceLimitOverrideError, ResourceLimitPhase,
    SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID, WASM_RESVG_TREE_DEPTH_HARD_CAP, resource_limit_descriptors,
    resource_profile_descriptors,
};
pub use merman_render::svg::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor, IconPack,
    IconRegistry, IconRegistryBuildError, IconRegistryBuildErrorKind, IconRegistryBuilder,
    IconRegistryResourceLimitDescriptor, IconRegistryResourceLimitId, ResvgCompatibleSvg,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor, SvgDebugOptions, SvgOutputPolicy,
    SvgPipeline, SvgPipelinePreset, SvgPostprocessContext, SvgPostprocessMetadata,
    SvgPostprocessor, SvgRenderOptions, foreign_object_label_fallback_svg_text,
    icon_registry_resource_limit_descriptors,
};
pub use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle,
    VendoredFontMetricsTextMeasurer, WrapMode,
};
pub use merman_render::{
    Error as RenderError, LayoutOptions, RenderCapability, RenderCapabilityPolicy,
    Result as RenderResult, layout_cytoscape_available, layout_elk_available, math_available,
};

/// Binary export APIs accept only terminally validated [`ResvgCompatibleSvg`] values.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub mod export {
    pub use merman_export::*;
}

/// Converts arbitrary text into a conservative SVG `id` token.
pub fn sanitize_svg_id(raw: &str) -> String {
    merman_render::svg::sanitize_svg_id(raw)
}

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
    fn sanitize_svg_id_keeps_allowed_punctuation() {
        assert_eq!(sanitize_svg_id("a:b.c_d"), "a:b.c_d");
    }

    #[test]
    fn sanitize_svg_id_m_is_reserved_for_untitled() {
        assert_eq!(sanitize_svg_id("m"), "m-untitled");
        assert_eq!(sanitize_svg_id("m-"), "m-untitled");
        assert_eq!(sanitize_svg_id("m--"), "m-untitled");
    }
}
