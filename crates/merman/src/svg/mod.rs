//! Public typed headless-render boundaries.
//!
//! The render facade keeps semantic data, layout, SVG output, and operation evidence in one
//! consuming pipeline. A fresh session report is not proof that a completed SVG operation ran,
//! and callers cannot construct a completed report themselves:
//!
//! ```compile_fail
//! use merman::svg::{RenderEnvironment, RenderOperationReport};
//!
//! fn retain_completed(_: &RenderOperationReport) {}
//! let session = RenderEnvironment::deterministic().begin_session().unwrap();
//! retain_completed(session.report());
//! ```
//!
//! The completion fields are private even though the report itself is observable:
//!
//! ```compile_fail
//! use merman::svg::{RenderEnvironment, RenderExecutionPath, RenderOperationReport};
//!
//! let session = RenderEnvironment::deterministic().begin_session().unwrap();
//! let _forged = RenderOperationReport {
//!     execution_path: RenderExecutionPath::HeadlessOperationTyped,
//!     session: session.report(),
//! };
//! ```
//!
//! Prepared semantic/layout artifacts are linear: rendering consumes the artifact, so a caller
//! cannot accidentally render the same parse/layout pair twice:
//!
//! ```compile_fail
//! use merman::svg::{LayoutOptions, SvgRenderOptions, prepare_render_sync};
//! use merman::{Engine, ParseOptions};
//!
//! let prepared = prepare_render_sync(
//!     &Engine::new(),
//!     "info",
//!     ParseOptions::strict(),
//!     &LayoutOptions::headless_svg_defaults(),
//! )?.unwrap();
//! let options = SvgRenderOptions::default();
//! let _first = prepared.render_svg(&options);
//! let _second = prepared.render_svg(&options);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub use merman_core::runtime::{
    OperationContext, RuntimePolicy, RuntimePolicyError, RuntimeValueSource,
};
pub use merman_render::environment::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId, RenderEnvironment,
    RenderSession, RenderSessionReport, TEXT_MEASUREMENT_PROTOCOL_VERSION,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity, TextMeasurementReport, TextMeasurementResultKind,
    TextMeasurementRoute, TextMeasurementSource, TextMeasurementSummary,
};
pub use merman_render::family::RenderFamilyKind;
#[cfg(feature = "math")]
pub use merman_render::math::RatexMathRenderer;
pub use merman_render::math::{MathRenderer, NoopMathRenderer};
pub use merman_render::resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, ClassComplexity, FlowchartComplexity,
    GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, MindmapComplexity, RESOURCE_CONTRACT_SCHEMA_VERSION,
    RenderResourceLimitId, RenderResourcePolicy, RenderResourceProfile,
    RenderResourceProfileDescriptor, ResourceLimitDescriptor, ResourceLimitExceeded,
    ResourceLimitId, ResourceLimitOverride, ResourceLimitOverrideError, ResourceLimitPhase,
    resource_limit_descriptors, resource_profile_descriptors,
};
pub use merman_render::svg::{
    CompiledHostTheme, CssOverridePolicy, CssOverridePostprocessor,
    ForeignObjectFallbackPostprocessor, HostThemeAppearance, HostThemeOutput,
    HostThemePipelinePreset, HostThemePreset, HostThemeProfile, HostThemeProfileBuilder,
    HostThemeRoles, HostThemeRootBackground, IconRegistry, IconRegistryError, IconSvg,
    ResvgCompatibleSvg, RootBackgroundPostprocessor, SanitizeCssPostprocessor,
    SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor, StripForeignObjectPostprocessor,
    SvgDebugOptions, SvgOutputPolicy, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext,
    SvgPostprocessMetadata, SvgPostprocessor, SvgRenderOptions, finalize_resvg_svg,
    foreign_object_label_fallback_svg_text, supported_host_theme_presets,
};
pub use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle,
    VendoredFontMetricsTextMeasurer, WrapMode,
};
pub use merman_render::{
    Error as RenderError, LayoutOptions, RenderCapability, Result as RenderResult,
    layout_cytoscape_available, layout_elk_available, math_available,
};

mod operation;
pub use operation::{
    PreparedRender, PreparedSemantic, RenderExecutionPath, RenderOperationReport, RenderedSvg,
};

/// Binary export APIs are isolated from Mermaid parsing and layout. They accept only terminally
/// validated [`ResvgCompatibleSvg`] values, so arbitrary SVG cannot bypass the render pipeline.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub mod export {
    pub use merman_export::*;
}

#[derive(Debug, thiserror::Error)]
pub enum HeadlessError {
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    Render(#[from] merman_render::Error),
    #[error(transparent)]
    RuntimePolicy(#[from] RuntimePolicyError),
}

pub type Result<T> = std::result::Result<T, HeadlessError>;

/// Failure from a complete Mermaid-source-to-binary-output operation.
///
/// SVG construction remains owned by this crate; byte encoding remains owned by
/// [`merman_export`]. The two causes stay distinct for hosts that need to decide whether a
/// failure is input/layout-related or backend-related.
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error(transparent)]
    Headless(#[from] HeadlessError),
    #[error(transparent)]
    Export(#[from] merman_export::ExportError),
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub type OutputResult<T> = std::result::Result<T, OutputError>;

fn default_render_environment() -> RenderEnvironment {
    RenderEnvironment::deterministic()
}

fn engine_with_session_context(
    engine: &merman_core::Engine,
    session: &merman_render::environment::RenderSession,
) -> merman_core::Engine {
    engine
        .clone()
        .with_operation_context(session.operation_context().clone())
}

/// Converts an arbitrary string into a conservative SVG `id` token suitable for embedding
/// multiple Mermaid diagrams in the same UI tree.
///
/// Mermaid uses the root `<svg id="...">` value as a prefix for internal ids like
/// `chart-title-<id>` and marker ids under `<defs>`. If you inline multiple SVGs with the same
/// id, those internal ids may collide.
///
/// This helper:
/// - trims whitespace
/// - replaces unsupported characters with `-`
/// - ensures the id starts with an ASCII letter by prefixing `m-` when needed
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

/// Parses and lays out one diagram as compatibility JSON (executor-free).
pub fn layout_json_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<serde_json::Value>> {
    let environment = default_render_environment();
    operation::HeadlessOperation::new(engine, text, parse_options, layout_options, &environment)?
        .layout_json()
}

/// Returns layout defaults intended for UI integrations that render headless SVG.
///
/// This is a convenience wrapper around [`LayoutOptions::headless_svg_defaults`].
pub fn headless_layout_options() -> LayoutOptions {
    LayoutOptions::headless_svg_defaults()
}

pub async fn layout_json(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<serde_json::Value>> {
    // This async API is runtime-agnostic: layout is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    layout_json_sync(engine, text, parse_options, layout_options)
}

/// Parses and lays out one diagram through the canonical typed render path.
///
/// The returned artifact exposes metadata and compatibility layout JSON while keeping its typed
/// semantic/layout pair opaque. Rendering consumes the same artifact that completed layout.
pub fn prepare_render_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<PreparedRender>> {
    let environment = default_render_environment();
    operation::HeadlessOperation::new(engine, text, parse_options, layout_options, &environment)?
        .prepare_render()
}

/// Parses one diagram through the canonical typed path without starting layout.
///
/// This stage lets callers apply admission or skip policy from the exact metadata and semantic
/// model that would otherwise continue into layout.
pub fn prepare_semantic_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<PreparedSemantic>> {
    let environment = default_render_environment();
    operation::HeadlessOperation::new(engine, text, parse_options, layout_options, &environment)?
        .prepare_semantic()
}

/// Synchronous SVG render helper (executor-free).
pub fn render_svg_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    let environment = default_render_environment();
    operation::HeadlessOperation::new(engine, text, parse_options, layout_options, &environment)?
        .render_svg(svg_options, &SvgDebugOptions::default())
}

pub fn apply_svg_pipeline(
    svg: &str,
    pipeline: &SvgPipeline,
    session: &merman_render::environment::RenderSession,
) -> Result<String> {
    Ok(pipeline.process_to_string(svg, session)?)
}

pub fn apply_svg_pipeline_with_metadata(
    svg: &str,
    pipeline: &SvgPipeline,
    metadata: &SvgPostprocessMetadata,
    session: &merman_render::environment::RenderSession,
) -> Result<String> {
    Ok(pipeline.process_to_string_with_metadata(svg, metadata, session)?)
}

pub fn svg_readable(
    svg: &str,
    session: &merman_render::environment::RenderSession,
) -> Result<String> {
    apply_svg_pipeline(svg, &SvgPipeline::readable(), session)
}

pub fn svg_resvg_safe(
    svg: &str,
    session: &merman_render::environment::RenderSession,
) -> Result<String> {
    apply_svg_pipeline(svg, &SvgPipeline::resvg_safe(), session)
}

pub fn render_svg_with_pipeline_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    pipeline: &SvgPipeline,
) -> Result<Option<String>> {
    let environment = default_render_environment();
    operation::HeadlessOperation::new(engine, text, parse_options, layout_options, &environment)?
        .render_svg_with_pipeline(svg_options, &SvgDebugOptions::default(), pipeline)
}

/// Synchronous SVG render helper that applies a best-effort readability fallback for
/// `<foreignObject>` labels.
///
/// This is intended for raster outputs and UI previews where `<foreignObject>` is not
/// supported. It does not aim for upstream Mermaid DOM parity.
pub fn render_svg_readable_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    render_svg_with_pipeline_sync(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
        &SvgPipeline::readable(),
    )
}

pub fn render_svg_resvg_safe_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    render_svg_with_pipeline_sync(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
        &SvgPipeline::resvg_safe(),
    )
}

pub async fn prepare_render(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<PreparedRender>> {
    // This async API is runtime-agnostic: parsing and layout are CPU-bound and do not perform I/O.
    // It executes synchronously and does not yield.
    prepare_render_sync(engine, text, parse_options, layout_options)
}

pub async fn prepare_semantic(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<PreparedSemantic>> {
    // This async API is runtime-agnostic: parsing is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    prepare_semantic_sync(engine, text, parse_options, layout_options)
}

pub async fn render_svg(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    // This async API is runtime-agnostic: rendering is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    render_svg_sync(engine, text, parse_options, layout_options, svg_options)
}

pub async fn render_svg_readable(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    // This async API is runtime-agnostic: rendering is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    render_svg_readable_sync(engine, text, parse_options, layout_options, svg_options)
}

pub async fn render_svg_with_pipeline(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    pipeline: &SvgPipeline,
) -> Result<Option<String>> {
    // This async API is runtime-agnostic: rendering is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    render_svg_with_pipeline_sync(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
        pipeline,
    )
}

pub async fn render_svg_resvg_safe(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    // This async API is runtime-agnostic: rendering is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    render_svg_resvg_safe_sync(engine, text, parse_options, layout_options, svg_options)
}

#[cfg(test)]
mod svg_pipeline_tests {
    use super::*;
    use serde_json::{Value, json};
    use std::borrow::Cow;

    fn root_style_property_is(svg: &str, property: &str, expected: &str) -> bool {
        let Ok(document) = roxmltree::Document::parse(svg) else {
            return false;
        };
        document
            .root_element()
            .attribute("style")
            .is_some_and(|style| {
                style.split(';').map(str::trim).any(|declaration| {
                    declaration.split_once(':').is_some_and(|(name, value)| {
                        name.trim() == property && value.trim() == expected
                    })
                })
            })
    }

    fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
        model["tasks"]
            .as_array()
            .expect("Gantt tasks should be an array")
            .iter()
            .find(|task| task["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
    }

    #[test]
    fn readable_helper_routes_through_readable_pipeline() {
        let engine = merman_core::Engine::new();
        let layout = LayoutOptions::headless_svg_defaults();
        let svg = SvgRenderOptions::default();
        let source = "flowchart TD\nA[Hello] --> B[World]";

        let helper = render_svg_readable_sync(
            &engine,
            source,
            merman_core::ParseOptions::default(),
            &layout,
            &svg,
        )
        .unwrap()
        .unwrap();
        let pipeline = render_svg_with_pipeline_sync(
            &engine,
            source,
            merman_core::ParseOptions::default(),
            &layout,
            &svg,
            &SvgPipeline::readable(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(helper, pipeline);
        assert!(pipeline.contains("data-merman-foreignobject"));
    }

    #[test]
    fn default_svg_helper_stays_parity_without_pipeline_cleanup() {
        let engine = merman_core::Engine::new();
        let layout = LayoutOptions::headless_svg_defaults();
        let svg = SvgRenderOptions::default();
        let source = "flowchart TD\nA[Hello] --> B[World]";

        let default_svg = render_svg_sync(
            &engine,
            source,
            merman_core::ParseOptions::default(),
            &layout,
            &svg,
        )
        .unwrap()
        .unwrap();
        let parity_pipeline = render_svg_with_pipeline_sync(
            &engine,
            source,
            merman_core::ParseOptions::default(),
            &layout,
            &svg,
            &SvgPipeline::parity(),
        )
        .unwrap()
        .unwrap();
        let readable = render_svg_readable_sync(
            &engine,
            source,
            merman_core::ParseOptions::default(),
            &layout,
            &svg,
        )
        .unwrap()
        .unwrap();

        assert_eq!(default_svg, parity_pipeline);
        assert_ne!(default_svg, readable);
    }

    #[test]
    fn layout_json_helpers_share_the_complete_typed_operation_projection() {
        let renderer = HeadlessRenderer::new().with_lenient_parsing();
        for (source, diagram_type, layout_key) in [
            (
                "flowchart TD\nA[Hello] --> B[World]",
                "flowchart-v2",
                "FlowchartV2",
            ),
            (
                "stateDiagram-v2\n[*] --> Active",
                "stateDiagram",
                "StateDiagramV2",
            ),
            ("classDiagram\nclass Animal", "class", "ClassDiagramV2"),
        ] {
            let free_layout =
                layout_json_sync(&renderer.engine, source, renderer.parse, &renderer.layout)
                    .unwrap()
                    .unwrap();
            let renderer_layout = renderer.layout_json_sync(source).unwrap().unwrap();

            assert_eq!(free_layout, renderer_layout);
            assert_eq!(
                renderer_layout["meta"]["diagram_type"],
                serde_json::json!(diagram_type)
            );
            assert!(renderer_layout["semantic"].is_object());
            assert!(renderer_layout["layout"][layout_key].is_object());
        }
        assert!(
            renderer
                .layout_json_sync("not a mermaid diagram")
                .unwrap()
                .is_none()
        );
    }

    struct MetadataComment;

    impl SvgPostprocessor for MetadataComment {
        fn name(&self) -> &'static str {
            "metadata-comment"
        }

        fn process<'a>(
            &self,
            svg: Cow<'a, str>,
            ctx: &SvgPostprocessContext<'_>,
        ) -> RenderResult<Cow<'a, str>> {
            Ok(Cow::Owned(format!(
                "{}<!--type={};title={};id={}-->",
                svg,
                ctx.diagram_type().unwrap_or(""),
                ctx.diagram_title().unwrap_or(""),
                ctx.svg_id().unwrap_or("")
            )))
        }
    }

    #[cfg(feature = "png")]
    struct FailingRasterPass;

    #[cfg(feature = "png")]
    impl SvgPostprocessor for FailingRasterPass {
        fn name(&self) -> &'static str {
            "failing-raster-pass"
        }

        fn process<'a>(
            &self,
            _svg: Cow<'a, str>,
            _ctx: &SvgPostprocessContext<'_>,
        ) -> RenderResult<Cow<'a, str>> {
            Err(RenderError::InvalidModel {
                message: "raster pipeline marker".to_string(),
            })
        }
    }

    #[test]
    fn render_svg_with_pipeline_passes_parsed_metadata() {
        let renderer = HeadlessRenderer::new().with_diagram_id("host-style");
        let source = "---\ntitle: Host Pipeline\n---\nflowchart TD\nA --> B";
        let pipeline = SvgPipeline::parity().with_postprocessor(MetadataComment);

        let svg = renderer
            .render_svg_with_pipeline_sync(source, &pipeline)
            .unwrap()
            .unwrap();

        assert!(svg.contains("type=flowchart"));
        assert!(svg.contains("title=Host Pipeline"));
        assert!(svg.contains("id=host-style"));
    }

    #[cfg(not(feature = "math"))]
    fn assert_missing_math_capability(source: &str, expected_diagram_type: &str) {
        let renderer = HeadlessRenderer::new();
        let error = renderer
            .render_svg_sync(source)
            .expect_err("math labels must not silently render as plain text without math");
        assert_eq!(
            error.to_string(),
            format!(
                "compiled renderer lacks capability `math` required by diagram `{expected_diagram_type}`"
            )
        );

        let HeadlessError::Render(RenderError::MissingCapability {
            capability,
            diagram_type,
        }) = error
        else {
            panic!("expected a missing math capability error");
        };
        assert_eq!(capability, RenderCapability::Math);
        assert_eq!(diagram_type, expected_diagram_type);
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn flowchart_math_label_requires_the_compiled_math_capability() {
        assert_missing_math_capability("flowchart TD\nA[\"$$x^2$$\"] --> B[Done]", "flowchart-v2");
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn sequence_math_label_requires_the_compiled_math_capability() {
        assert_missing_math_capability(
            "sequenceDiagram\nA->>B: $$x^2$$\nNote right of B: $$x^2$$",
            "sequence",
        );
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn wrapped_sequence_actor_math_label_requires_the_compiled_math_capability() {
        assert_missing_math_capability(
            "%%{wrap}%%\nsequenceDiagram\nparticipant A as $$x^2$$\nA->>A: done",
            "sequence",
        );
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn sequence_loop_math_label_requires_the_compiled_math_capability() {
        assert_missing_math_capability(
            "sequenceDiagram\nloop $$x^2$$\nA->>A: done\nend",
            "sequence",
        );
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn sequence_section_math_label_requires_the_compiled_math_capability() {
        assert_missing_math_capability(
            "sequenceDiagram\nalt first\nA->>B: done\nelse $$x^2$$\nB->>A: done\nend",
            "sequence",
        );
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn unclosed_math_delimiters_remain_literal_without_math() {
        let svg = HeadlessRenderer::new()
            .render_svg_sync("flowchart TD\nA[\"literal $$\"]")
            .expect("an unclosed delimiter is not a math capability request")
            .expect("flowchart should be detected");

        assert!(svg.contains("literal $$"));
    }

    #[cfg(feature = "math")]
    #[test]
    fn compiled_math_capability_is_installed_in_the_default_renderer() {
        for source in [
            "flowchart TD\nA[\"$$x^2$$\"] --> B[Done]",
            "sequenceDiagram\nA->>B: $$x^2$$\nNote right of B: $$x^2$$",
            "sequenceDiagram\nloop $$x^2$$\nA->>A: done\nend",
            "sequenceDiagram\nalt $$x^2$$\nA->>B: done\nelse $$y^2$$\nB->>A: done\nend",
        ] {
            let svg = HeadlessRenderer::new()
                .render_svg_sync(source)
                .expect("compiled math renderer should be available")
                .expect("diagram should be detected");

            assert!(!svg.contains("$$"), "math source leaked into SVG: {svg}");
            assert!(
                svg.contains("<foreignObject"),
                "math label was not emitted through the HTML math path: {svg}"
            );
        }
    }

    #[cfg(feature = "math")]
    #[test]
    fn wrapped_sequence_actor_uses_the_compiled_math_renderer() {
        let svg = HeadlessRenderer::new()
            .render_svg_sync("%%{wrap}%%\nsequenceDiagram\nparticipant A as $$x^2$$\nA->>A: done")
            .expect("compiled math renderer should be available")
            .expect("diagram should be detected");

        // Mermaid intentionally retains a plain `<text>` candidate inside the actor `<switch>`.
        // The foreignObject proves wrapped actors select the math candidate instead of falling
        // through to plain-only rendering.
        assert!(svg.contains("<switch><foreignObject"), "{svg}");
        assert!(svg.matches("<foreignObject").count() >= 2, "{svg}");
        assert!(
            svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox="),
            "{svg}"
        );
    }

    #[test]
    fn renderer_owned_readable_pipeline_records_postprocess_measurements_in_final_report() {
        let source = "flowchart TD\nA@{ shape: stadium, label: 'This is a long label that wraps in the pipeline' }";
        let renderer = HeadlessRenderer::new()
            .with_environment(RenderEnvironment::deterministic())
            .with_diagram_id("pipeline-report");
        let plain = renderer.render_svg_report_sync(source).unwrap().unwrap();
        let readable_renderer = renderer.clone().with_svg_pipeline(SvgPipeline::readable());
        let readable = readable_renderer
            .render_svg_report_sync(source)
            .unwrap()
            .unwrap();

        let wrapped_count = |rendered: &RenderedSvg| {
            rendered
                .report()
                .measurement()
                .entries()
                .iter()
                .filter(|entry| {
                    entry.provenance().phase == TextMeasurementPhase::Wrap
                        && entry.provenance().operation == TextMeasurementOperation::Wrapped
                })
                .map(TextMeasurementSummary::count)
                .sum::<u64>()
        };

        assert!(readable.svg().contains("data-merman-foreignobject"));
        assert!(wrapped_count(&readable) > wrapped_count(&plain));
        assert_eq!(
            readable_renderer.render_svg_sync(source).unwrap().unwrap(),
            readable.svg(),
            "the string API must only project the report-producing pipeline implementation"
        );
    }

    #[test]
    fn render_svg_rejects_source_over_resource_limit_before_parse() {
        let renderer = HeadlessRenderer::new().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSourceBytes, 4)
                .unwrap(),
        );

        let err = renderer
            .render_svg_sync("flowchart TD\nA --> B")
            .unwrap_err();

        let HeadlessError::Render(RenderError::ResourceLimitExceeded(limit)) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::Source);
        assert_eq!(limit.limit, "max_source_bytes");
    }

    #[test]
    fn render_svg_rejects_mindmap_cardinality_before_layout() {
        let renderer = HeadlessRenderer::new().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxModelItems, 2)
                .unwrap(),
        );

        let error = renderer
            .render_svg_sync("mindmap\n  Root\n    First child\n    Second child\n")
            .unwrap_err();

        let HeadlessError::Render(RenderError::ResourceLimitExceeded(limit)) = error else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
        assert_eq!(limit.actual, 5);
        assert_eq!(limit.max, 2);
    }

    #[test]
    fn parse_only_entry_points_use_the_environment_source_limit() {
        let renderer = HeadlessRenderer::new().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSourceBytes, 4)
                .unwrap(),
        );
        let source = "flowchart TD\nA --> B";

        for err in [
            renderer.parse_metadata_sync(source).unwrap_err(),
            renderer.parse_diagram_sync(source).unwrap_err(),
        ] {
            let HeadlessError::Render(RenderError::ResourceLimitExceeded(limit)) = err else {
                panic!("expected resource limit error");
            };
            assert_eq!(limit.phase, ResourceLimitPhase::Source);
            assert_eq!(limit.limit, "max_source_bytes");
        }
    }

    #[test]
    fn render_svg_with_pipeline_rejects_expanded_svg_over_resource_limit() {
        struct AppendingPass;

        impl SvgPostprocessor for AppendingPass {
            fn name(&self) -> &'static str {
                "append"
            }

            fn process<'a>(
                &self,
                svg: Cow<'a, str>,
                _ctx: &SvgPostprocessContext<'_>,
            ) -> RenderResult<Cow<'a, str>> {
                Ok(Cow::Owned(format!("{svg}{}", "x".repeat(128 * 1024))))
            }
        }

        let renderer = HeadlessRenderer::new().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, 64 * 1024)
                .unwrap(),
        );
        let pipeline = SvgPipeline::parity().with_postprocessor(AppendingPass);

        let err = renderer
            .render_svg_with_pipeline_sync("flowchart TD\nA --> B", &pipeline)
            .unwrap_err();

        let HeadlessError::Render(RenderError::ResourceLimitExceeded(limit)) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::SvgPostprocess);
        assert_eq!(limit.limit, "max_svg_bytes");
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_png_sync_uses_renderer_owned_pipeline_before_encoding() {
        let renderer = HeadlessRenderer::new()
            .with_svg_pipeline(SvgPipeline::parity().with_postprocessor(FailingRasterPass));

        let err = renderer
            .render_png_sync(
                "flowchart TD\n  A[Pipeline] --> B[Raster]",
                &merman_export::RasterOptions::default(),
            )
            .unwrap_err();

        let message = err.to_string();
        assert!(message.contains("failing-raster-pass"), "{message}");
        assert!(message.contains("raster pipeline marker"), "{message}");
    }

    #[cfg(feature = "png")]
    #[test]
    fn render_png_sync_encodes_a_sealed_svg() {
        let bytes = HeadlessRenderer::new()
            .render_png_sync(
                "flowchart TD\n  A[PNG] --> B[Export]",
                &merman_export::RasterOptions::default(),
            )
            .expect("PNG render should succeed")
            .expect("diagram should be detected");

        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[cfg(feature = "jpeg")]
    #[test]
    fn render_jpeg_sync_encodes_a_sealed_svg() {
        let bytes = HeadlessRenderer::new()
            .render_jpeg_sync(
                "flowchart TD\n  A[JPEG] --> B[Export]",
                &merman_export::RasterOptions::default(),
            )
            .expect("JPEG render should succeed")
            .expect("diagram should be detected");

        assert!(bytes.starts_with(b"\xff\xd8\xff"));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn render_pdf_sync_encodes_a_sealed_svg() {
        let bytes = HeadlessRenderer::new()
            .render_pdf_sync("flowchart TD\n  A[PDF] --> B[Export]")
            .expect("PDF render should succeed")
            .expect("diagram should be detected");

        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn render_svg_sync_merges_site_config_scoped_theme_css_once() {
        let renderer = HeadlessRenderer::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "themeCSS": ".node rect { fill: #123456; } @media (max-width: 600px) { text { fill: #654321; } }"
            })))
            .with_diagram_id("theme-css");
        let source = r##"flowchart TD
  A[Hello] --> B[World]
"##;

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert_eq!(
            svg.matches("#theme-css .node rect { fill: #123456; }")
                .count(),
            1
        );
        assert!(!svg.contains(r#"data-merman-postprocess="scoped-css""#));
        assert!(svg.contains("@media (max-width: 600px) {"));
        assert!(svg.contains("#theme-css text { fill: #654321; }"));
    }

    #[test]
    fn render_svg_sync_filters_diagram_level_theme_css() {
        let renderer = HeadlessRenderer::new().with_diagram_id("theme-css-filter");
        let source = r##"%%{init: {"themeCSS": ".node rect { outline: 13px solid rgb(1, 2, 3); }"}}%%
flowchart TD
  A[Hello] --> B[World]
"##;

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(!svg.contains("outline: 13px"), "{svg}");
        assert!(
            !svg.contains("data-merman-postprocess=\"scoped-css\""),
            "{svg}"
        );
    }

    #[test]
    fn render_svg_sync_drops_mermaid_unbalanced_css_sentinel() {
        let renderer = HeadlessRenderer::new().with_diagram_id("unbalanced-theme-css");
        let source = r##"%%{init: {"themeCSS": "} * { background: red }"}}%%
flowchart TD
  A[Hello] --> B[World]
"##;

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert_eq!(svg.matches("<style").count(), 1);
        assert!(!svg.contains("Unbalanced CSS"), "{svg}");
        assert!(!svg.contains("background: red"), "{svg}");
    }

    #[test]
    fn render_svg_sync_filters_diagram_level_font_family_css_injection() {
        let renderer = HeadlessRenderer::new().with_diagram_id("font-css-injection");
        let source = r##"%%{init: {"fontFamily": "x;a{b} :not(&){background:green !important} c{d}"}}%%
flowchart TD
  A[Hello] --> B[World]
"##;

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(!svg.contains("background:green"), "{svg}");
        assert!(!svg.contains(":not(&)"), "{svg}");
        assert!(svg.contains(r#"font-family:"trebuchet ms",verdana,arial,sans-serif"#));
    }

    #[test]
    fn render_svg_sync_applies_external_site_theme_to_plain_source() {
        let renderer = HeadlessRenderer::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "theme": "neutral"
            })))
            .with_diagram_id("external-theme");
        let source = "flowchart TD\n  A[Plain source] --> B[External theme]";

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(
            svg.contains("#external-theme .labelBkg{background-color:rgba(255, 255, 255, 0.5);}")
        );
    }

    #[test]
    fn render_svg_sync_applies_external_neo_theme_to_plain_source() {
        let renderer = HeadlessRenderer::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "theme": "neo"
            })))
            .with_diagram_id("external-neo");
        let source = "flowchart TD\n  A[Plain source] --> B[Neo theme]";

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(svg.contains("fill:#ffffff;stroke:#000000;stroke-width:2px;"));
        assert!(
            svg.contains("#external-neo .labelBkg{background-color:rgba(204, 204, 204, 0.5);}")
        );
    }

    #[test]
    fn render_svg_sync_falls_back_for_unknown_external_theme() {
        let renderer = HeadlessRenderer::new()
            .with_site_config(merman_core::MermaidConfig::from_value(json!({
                "theme": "unknown"
            })))
            .with_diagram_id("external-unknown");
        let source = "flowchart TD\n  A[Plain source] --> B[Unknown theme]";

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(svg.contains("fill:#ECECFF;stroke:#9370DB;stroke-width:1px;"));
        assert!(
            svg.contains("#external-unknown .labelBkg{background-color:rgba(232, 232, 232, 0.5);}")
        );
    }

    #[test]
    fn supported_host_theme_presets_are_separate_from_mermaid_themes() {
        assert_eq!(
            supported_host_theme_presets(),
            &[
                "editor-light",
                "editor-dark",
                "one-dark",
                "gruvbox-light",
                "gruvbox-dark",
                "ayu-light",
                "ayu-dark"
            ]
        );
        assert!(merman_core::supported_themes().contains(&"default"));
        assert!(!merman_core::supported_themes().contains(&"one-dark"));
    }

    #[test]
    fn render_svg_with_site_config_is_request_scoped() {
        let renderer = HeadlessRenderer::new().with_diagram_id("request-site-theme");
        let source = "flowchart TD\n  A[Plain source] --> B[Request theme]";

        let themed = renderer
            .render_svg_with_site_config_sync(
                source,
                merman_core::MermaidConfig::from_value(json!({
                    "theme": "neutral"
                })),
            )
            .unwrap()
            .unwrap();
        let plain = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(
            themed.contains(
                "#request-site-theme .labelBkg{background-color:rgba(255, 255, 255, 0.5);}"
            )
        );
        assert!(
            plain.contains(
                "#request-site-theme .labelBkg{background-color:rgba(232, 232, 232, 0.5);}"
            )
        );
    }

    #[test]
    fn host_theme_profile_applies_editor_roles_and_pipeline() {
        let profile = HostThemeProfile::editor_dark();
        let compiled = profile.compile();
        let renderer = HeadlessRenderer::new()
            .with_compiled_host_theme(&compiled)
            .with_diagram_id("host-theme-profile");

        let svg = renderer
            .render_svg_sync(
                r##"%%{init: {"themeCSS": ".node rect { stroke-width: 3px !important; }"}}%%
flowchart TD
  A[Host] --> B[Theme]
"##,
            )
            .unwrap()
            .unwrap();

        assert!(svg.contains("#111827"), "{svg}");
        assert!(svg.contains("#e5e7eb"), "{svg}");
        assert!(svg.contains("#94a3b8"), "{svg}");
        assert!(
            root_style_property_is(&svg, "background-color", "#0f172a"),
            "{svg}"
        );
        assert!(!svg.contains("<foreignObject"), "{svg}");
        assert!(!svg.contains("!important"), "{svg}");
    }

    #[test]
    fn host_theme_profile_raw_theme_variables_win_over_roles() {
        let profile = HostThemeProfile::builder()
            .roles(HostThemeRoles {
                border: Some("#111111".to_string()),
                text: Some("#eeeeee".to_string()),
                ..HostThemeRoles::default()
            })
            .theme_variable("nodeBorder", "#abcdef")
            .build();
        let renderer = HeadlessRenderer::new()
            .with_host_theme(&profile)
            .with_diagram_id("host-theme-override");

        let svg = renderer
            .render_svg_sync("flowchart TD\n  A[Host]")
            .unwrap()
            .unwrap();

        assert!(svg.contains("#abcdef"), "{svg}");
        assert!(svg.contains("#eeeeee"), "{svg}");
    }

    #[test]
    fn render_svg_with_host_theme_is_request_scoped() {
        let renderer = HeadlessRenderer::new().with_diagram_id("request-host-theme");
        let profile = HostThemeProfile::editor_dark();
        let source = "flowchart TD\n  A[Host] --> B[Theme]";

        let themed = renderer
            .render_svg_with_host_theme_sync(source, &profile)
            .unwrap()
            .unwrap();
        let plain = renderer.render_svg_sync(source).unwrap().unwrap();

        assert!(themed.contains("#111827"), "{themed}");
        assert!(
            root_style_property_is(&themed, "background-color", "#0f172a"),
            "{themed}"
        );
        assert!(!themed.contains("<foreignObject"), "{themed}");
        assert!(
            !root_style_property_is(&plain, "background-color", "#0f172a"),
            "{plain}"
        );
        assert!(plain.contains("<foreignObject"), "{plain}");
    }

    #[test]
    fn render_svg_with_compiled_host_theme_reuses_compiled_profile() {
        let renderer = HeadlessRenderer::new().with_diagram_id("request-compiled-theme");
        let compiled = HostThemeProfile::one_dark().compile();
        let source = "flowchart TD\n  A[Compiled] --> B[Theme]";

        let svg = renderer
            .render_svg_with_compiled_host_theme_sync(source, &compiled)
            .unwrap()
            .unwrap();

        assert!(svg.contains("#21252b"), "{svg}");
        assert!(
            root_style_property_is(&svg, "background-color", "#282c34"),
            "{svg}"
        );
        assert!(!svg.contains("<foreignObject"), "{svg}");
    }

    #[test]
    fn headless_renderer_fixed_time_controls_semantic_parse_and_gantt_svg() {
        let source = r#"gantt
dateFormat MM-DD
section Demo
Missing year: id1,03-01,1d
Missing ref: id2,after missing,1d
"#;
        let renderer = HeadlessRenderer::new().with_runtime_policy(
            RuntimePolicy::deterministic().with_fixed_unix_millis(1_771_113_600_000),
        );
        let parsed = renderer.parse_diagram_sync(source).unwrap().unwrap();

        assert_eq!(
            task_by_id(&parsed.model, "id1")["startTime"].as_i64(),
            Some(1_772_323_200_000)
        );
        assert_eq!(
            task_by_id(&parsed.model, "id2")["startTime"].as_i64(),
            Some(1_771_113_600_000)
        );

        let rendered = renderer.render_svg_report_sync(source).unwrap().unwrap();
        assert_eq!(rendered.report().unix_millis(), 1_771_113_600_000);
        fn today_line(svg: &str) -> &str {
            let start = svg.find(r#"<g class="today"><line"#).expect("today marker");
            let end = svg[start..].find("/>").expect("today marker end") + start + 2;
            &svg[start..end]
        }

        let next_month = HeadlessRenderer::new().with_runtime_policy(
            RuntimePolicy::deterministic().with_fixed_unix_millis(1_773_532_800_000),
        );
        let next_svg = next_month.render_svg_sync(source).unwrap().unwrap();
        assert_ne!(today_line(rendered.svg()), today_line(&next_svg));
    }
}

/// Convenience wrapper that bundles a [`merman_core::Engine`] and common options for headless rendering.
///
/// This is intended for UI integrations where passing 4-5 separate parameters per call is
/// noisy. It stays runtime-agnostic: all work is CPU-bound and does not perform I/O.
#[derive(Clone)]
pub struct HeadlessRenderer {
    engine: merman_core::Engine,
    parse: merman_core::ParseOptions,
    environment: RenderEnvironment,
    layout: LayoutOptions,
    svg: SvgRenderOptions,
    svg_debug: SvgDebugOptions,
    /// Optional renderer-owned SVG output pipeline.
    ///
    /// A fresh renderer leaves this unset so `render_svg_sync` keeps the Mermaid-parity SVG
    /// contract. Host theme helpers set this to the compiled profile pipeline so the profile's
    /// output settings travel with the renderer instead of requiring each call to pass them again.
    svg_pipeline: Option<SvgPipeline>,
}

impl Default for HeadlessRenderer {
    fn default() -> Self {
        Self::from_engine_and_environment(merman_core::Engine::new(), default_render_environment())
    }
}

impl HeadlessRenderer {
    /// Creates a renderer from its two operation-scoped runtime owners without constructing
    /// throwaway defaults.
    pub fn from_engine_and_environment(
        engine: merman_core::Engine,
        environment: RenderEnvironment,
    ) -> Self {
        Self {
            engine,
            environment,
            parse: merman_core::ParseOptions::default(),
            layout: LayoutOptions::headless_svg_defaults(),
            svg: SvgRenderOptions::default(),
            svg_debug: SvgDebugOptions::default(),
            svg_pipeline: None,
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a renderer that explicitly captures native clock, timezone, and randomness
    /// adapters for each operation. Timing diagnostics remain an explicit opt-in capability.
    pub fn try_native() -> Result<Self> {
        Ok(Self::from_engine_and_environment(
            merman_core::Engine::new(),
            RenderEnvironment::try_native()?,
        ))
    }

    pub fn with_engine(mut self, engine: merman_core::Engine) -> Self {
        self.engine = engine;
        self
    }

    pub fn engine(&self) -> &merman_core::Engine {
        &self.engine
    }

    pub const fn parse_options(&self) -> merman_core::ParseOptions {
        self.parse
    }

    pub fn environment(&self) -> &RenderEnvironment {
        &self.environment
    }

    pub fn layout_options(&self) -> &LayoutOptions {
        &self.layout
    }

    pub fn svg_options(&self) -> &SvgRenderOptions {
        &self.svg
    }

    pub const fn svg_debug_options(&self) -> &SvgDebugOptions {
        &self.svg_debug
    }

    pub fn svg_pipeline(&self) -> Option<&SvgPipeline> {
        self.svg_pipeline.as_ref()
    }

    pub fn with_environment(mut self, environment: RenderEnvironment) -> Self {
        self.environment = environment;
        self
    }

    pub fn with_site_config(mut self, site_config: merman_core::MermaidConfig) -> Self {
        self.engine = self.engine.with_site_config(site_config);
        self
    }

    pub fn with_host_theme(mut self, profile: &HostThemeProfile) -> Self {
        let compiled = profile.compile();
        let pipeline = compiled.pipeline();
        self.engine = self.engine.with_site_config(compiled.site_config);
        self.svg_pipeline = Some(pipeline);
        self
    }

    pub fn with_compiled_host_theme(mut self, theme: &CompiledHostTheme) -> Self {
        self.engine = self.engine.with_site_config(theme.site_config.clone());
        self.svg_pipeline = Some(theme.pipeline());
        self
    }

    pub fn with_svg_pipeline(mut self, pipeline: SvgPipeline) -> Self {
        self.svg_pipeline = Some(pipeline);
        self
    }

    pub fn with_runtime_policy(mut self, policy: RuntimePolicy) -> Self {
        self.environment = self.environment.with_runtime_policy(policy);
        self
    }

    pub fn with_operation_context(mut self, context: OperationContext) -> Self {
        self.environment = self
            .environment
            .with_runtime_policy(RuntimePolicy::from_operation_context(context));
        self
    }

    pub fn with_parse_options(mut self, parse: merman_core::ParseOptions) -> Self {
        self.parse = parse;
        self
    }

    pub fn with_strict_parsing(self) -> Self {
        self.with_parse_options(merman_core::ParseOptions::strict())
    }

    pub fn with_lenient_parsing(self) -> Self {
        self.with_parse_options(merman_core::ParseOptions::lenient())
    }

    pub fn with_layout_options(mut self, layout: LayoutOptions) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.environment = self.environment.with_resource_policy(policy);
        self
    }

    pub fn with_resource_profile(self, profile: RenderResourceProfile) -> Self {
        self.with_resource_policy(RenderResourcePolicy::for_profile(profile))
    }

    pub fn with_svg_options(mut self, svg: SvgRenderOptions) -> Self {
        self.svg = svg;
        self
    }

    pub fn with_svg_debug_options(mut self, debug: SvgDebugOptions) -> Self {
        self.svg_debug = debug;
        self
    }

    pub fn with_diagram_id(mut self, diagram_id: &str) -> Self {
        self.svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        self
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.environment = self.environment.with_text_measurement_policy(policy);
        self
    }

    pub fn with_math_renderer(
        mut self,
        renderer: std::sync::Arc<dyn MathRenderer + Send + Sync>,
    ) -> Self {
        self.environment = self.environment.with_math_renderer(renderer);
        self
    }

    pub fn with_vendored_text_measurer(self) -> Self {
        self.with_text_measurement_policy(TextMeasurementPolicy::parity())
    }

    pub fn with_deterministic_text_measurer(self) -> Self {
        self.with_text_measurement_policy(TextMeasurementPolicy::deterministic())
    }

    fn engine_for_session(
        &self,
        session: &merman_render::environment::RenderSession,
    ) -> merman_core::Engine {
        engine_with_session_context(&self.engine, session)
    }

    pub fn parse_metadata_sync(&self, text: &str) -> Result<merman_core::ParseMetadata> {
        let session = self.environment.begin_session()?;
        session
            .resource_policy()
            .check_source_bytes(text)
            .map_err(operation::resource_limit_error)?;
        Ok(self
            .engine_for_session(&session)
            .parse_metadata_sync(text)?)
    }

    pub fn parse_diagram_sync(&self, text: &str) -> Result<Option<merman_core::ParsedDiagram>> {
        let session = self.environment.begin_session()?;
        session
            .resource_policy()
            .check_source_bytes(text)
            .map_err(operation::resource_limit_error)?;
        Ok(self
            .engine_for_session(&session)
            .parse_diagram_sync(text, self.parse)?)
    }

    pub fn layout_json_sync(&self, text: &str) -> Result<Option<serde_json::Value>> {
        operation::HeadlessOperation::new(
            &self.engine,
            text,
            self.parse,
            &self.layout,
            &self.environment,
        )?
        .layout_json()
    }

    /// Parses and lays out one typed render artifact for inspection before SVG rendering.
    pub fn prepare_render_sync(&self, text: &str) -> Result<Option<PreparedRender>> {
        operation::HeadlessOperation::new(
            &self.engine,
            text,
            self.parse,
            &self.layout,
            &self.environment,
        )?
        .prepare_render()
    }

    /// Parses one typed semantic stage for admission before layout starts.
    pub fn prepare_semantic_sync(&self, text: &str) -> Result<Option<PreparedSemantic>> {
        operation::HeadlessOperation::new(
            &self.engine,
            text,
            self.parse,
            &self.layout,
            &self.environment,
        )?
        .prepare_semantic()
    }

    pub fn render_svg_sync(&self, text: &str) -> Result<Option<String>> {
        Ok(self
            .render_svg_report_sync(text)?
            .map(RenderedSvg::into_svg))
    }

    pub fn render_svg_report_sync(&self, text: &str) -> Result<Option<RenderedSvg>> {
        let Some(prepared) = self.prepare_render_sync(text)? else {
            return Ok(None);
        };
        let rendered = match &self.svg_pipeline {
            Some(pipeline) => prepared.render_svg_with_pipeline_report_and_debug(
                &self.svg,
                &self.svg_debug,
                pipeline,
            )?,
            None => prepared.render_svg_report_with_debug(&self.svg, &self.svg_debug)?,
        };
        Ok(Some(rendered))
    }

    /// Renders one diagram with additional Mermaid site config defaults.
    ///
    /// The override applies only to this call. Diagram frontmatter and `%%{init}%%` directives
    /// still merge on top of the supplied site config, matching Mermaid's per-diagram config
    /// precedence.
    pub fn render_svg_with_site_config_sync(
        &self,
        text: &str,
        site_config: merman_core::MermaidConfig,
    ) -> Result<Option<String>> {
        self.clone()
            .with_site_config(site_config)
            .render_svg_sync(text)
    }

    /// Renders one diagram with a host/editor theme profile.
    ///
    /// This is the request-level counterpart to [`HeadlessRenderer::with_host_theme`]: it does not
    /// mutate this renderer, and it applies the profile's compiled SVG output pipeline only to this
    /// render call.
    pub fn render_svg_with_host_theme_sync(
        &self,
        text: &str,
        profile: &HostThemeProfile,
    ) -> Result<Option<String>> {
        self.clone().with_host_theme(profile).render_svg_sync(text)
    }

    /// Renders one diagram with a precompiled host/editor theme.
    ///
    /// Prefer this in hot editor paths when the same profile is reused for many diagrams.
    pub fn render_svg_with_compiled_host_theme_sync(
        &self,
        text: &str,
        theme: &CompiledHostTheme,
    ) -> Result<Option<String>> {
        self.clone()
            .with_compiled_host_theme(theme)
            .render_svg_sync(text)
    }

    pub fn render_svg_with_pipeline_sync(
        &self,
        text: &str,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        Ok(self
            .render_svg_with_pipeline_report_sync(text, pipeline)?
            .map(RenderedSvg::into_svg))
    }

    pub fn render_svg_with_pipeline_report_sync(
        &self,
        text: &str,
        pipeline: &SvgPipeline,
    ) -> Result<Option<RenderedSvg>> {
        let Some(prepared) = self.prepare_render_sync(text)? else {
            return Ok(None);
        };
        Ok(Some(prepared.render_svg_with_pipeline_report_and_debug(
            &self.svg,
            &self.svg_debug,
            pipeline,
        )?))
    }

    /// Renders one diagram into the sealed resvg/raster input contract.
    pub fn render_resvg_compatible_svg_with_pipeline_sync(
        &self,
        text: &str,
        pipeline: &SvgPipeline,
    ) -> Result<Option<ResvgCompatibleSvg>> {
        let Some(prepared) = self.prepare_render_sync(text)? else {
            return Ok(None);
        };
        Ok(Some(
            prepared
                .render_resvg_compatible_svg_with_debug(&self.svg, &self.svg_debug, pipeline)?
                .0,
        ))
    }

    /// Renders SVG and applies a best-effort readability fallback for `<foreignObject>` labels.
    ///
    /// Many headless SVG renderers and rasterizers do not fully support HTML inside
    /// `<foreignObject>`. This helper overlays extracted label text as `<text>/<tspan>` so
    /// consumers can still display something readable.
    pub fn render_svg_readable_sync(&self, text: &str) -> Result<Option<String>> {
        self.render_svg_with_pipeline_sync(text, &SvgPipeline::readable())
    }

    pub fn render_svg_resvg_safe_sync(&self, text: &str) -> Result<Option<String>> {
        Ok(self
            .render_resvg_compatible_svg_with_pipeline_sync(text, &SvgPipeline::resvg_safe())?
            .map(ResvgCompatibleSvg::into_string))
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn export_pipeline(&self) -> SvgPipeline {
        self.svg_pipeline
            .clone()
            .unwrap_or_else(SvgPipeline::resvg_safe)
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn render_export_svg_sync(&self, text: &str) -> Result<Option<ResvgCompatibleSvg>> {
        let pipeline = self.export_pipeline();
        self.render_resvg_compatible_svg_with_pipeline_sync(text, &pipeline)
    }

    #[cfg(feature = "png")]
    pub fn render_png_sync(
        &self,
        text: &str,
        options: &merman_export::RasterOptions,
    ) -> OutputResult<Option<Vec<u8>>> {
        let Some(svg) = self.render_export_svg_sync(text)? else {
            return Ok(None);
        };
        Ok(Some(merman_export::svg_to_png(&svg, options)?))
    }

    #[cfg(feature = "jpeg")]
    pub fn render_jpeg_sync(
        &self,
        text: &str,
        options: &merman_export::RasterOptions,
    ) -> OutputResult<Option<Vec<u8>>> {
        let Some(svg) = self.render_export_svg_sync(text)? else {
            return Ok(None);
        };
        Ok(Some(merman_export::svg_to_jpeg(&svg, options)?))
    }

    #[cfg(feature = "pdf")]
    pub fn render_pdf_sync(&self, text: &str) -> OutputResult<Option<Vec<u8>>> {
        self.render_pdf_with_options_sync(text, &merman_export::PdfOptions::default())
    }

    /// Renders vector PDF using PDF-specific page and filter policy.
    #[cfg(feature = "pdf")]
    pub fn render_pdf_with_options_sync(
        &self,
        text: &str,
        options: &merman_export::PdfOptions,
    ) -> OutputResult<Option<Vec<u8>>> {
        let Some(svg) = self.render_export_svg_sync(text)? else {
            return Ok(None);
        };
        Ok(Some(merman_export::svg_to_pdf_with_options(&svg, options)?))
    }
}
