#[cfg(feature = "ratex-math")]
pub use merman_render::math::RatexMathRenderer;
pub use merman_render::math::{MathRenderer, NoopMathRenderer};
pub use merman_render::model::LayoutedDiagram;
pub use merman_render::svg::{
    CssOverridePolicy, CssOverridePostprocessor, DropNativeDuplicateFallbacksPostprocessor,
    ForeignObjectFallbackPostprocessor, IconRegistry, IconRegistryError, IconSvg,
    RootBackgroundPostprocessor, SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor,
    ScopedCssPostprocessor, StripForeignObjectPostprocessor, SvgPipeline, SvgPipelinePreset,
    SvgPostprocessContext, SvgPostprocessMetadata, SvgPostprocessor, SvgRenderOptions,
    foreign_object_label_fallback_svg_text, resvg_safe_svg,
};
pub use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, VendoredFontMetricsTextMeasurer,
};
pub use merman_render::{
    Error as RenderError, LayoutOptions, Result as RenderResult, layout_parsed,
};

mod operation;

#[cfg(feature = "raster")]
pub mod raster;

#[derive(Debug, thiserror::Error)]
pub enum HeadlessError {
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    Render(#[from] merman_render::Error),
}

pub type Result<T> = std::result::Result<T, HeadlessError>;

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
    let raw = raw.trim();
    if raw.is_empty() {
        return "m-untitled".to_string();
    }

    let mut iter = raw.chars();
    let Some(first_raw) = iter.next() else {
        return "m-untitled".to_string();
    };

    fn sanitize_char(ch: char) -> char {
        let ok = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ':' || ch == '.';
        if ok { ch } else { '-' }
    }

    let first = sanitize_char(first_raw);
    let mut out = String::with_capacity(raw.len() + 2);
    let mut prev_dash = false;

    if !first.is_ascii_alphabetic() {
        out.push('m');
        if first != '-' {
            out.push('-');
            prev_dash = true;
        }
    }

    let push_sanitized = |ch: char, out: &mut String, prev_dash: &mut bool| {
        if ch == '-' {
            if *prev_dash {
                return;
            }
            *prev_dash = true;
        } else {
            *prev_dash = false;
        }
        out.push(ch);
    };

    push_sanitized(first, &mut out, &mut prev_dash);
    for ch in iter {
        push_sanitized(sanitize_char(ch), &mut out, &mut prev_dash);
    }

    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() || out == "m" {
        return "m-untitled".to_string();
    }
    out
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

/// Synchronous layout helper (executor-free).
pub fn layout_diagram_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<LayoutedDiagram>> {
    let Some(parsed) = engine.parse_diagram_sync(text, parse_options)? else {
        return Ok(None);
    };
    Ok(Some(merman_render::layout_parsed(&parsed, layout_options)?))
}

/// Returns layout defaults intended for UI integrations that render headless SVG.
///
/// This is a convenience wrapper around [`LayoutOptions::headless_svg_defaults`].
pub fn headless_layout_options() -> LayoutOptions {
    LayoutOptions::headless_svg_defaults()
}

pub async fn layout_diagram(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
) -> Result<Option<LayoutedDiagram>> {
    // This async API is runtime-agnostic: layout is CPU-bound and does not perform I/O.
    // It executes synchronously and does not yield.
    layout_diagram_sync(engine, text, parse_options, layout_options)
}

pub fn render_layouted_svg(
    diagram: &LayoutedDiagram,
    measurer: &dyn TextMeasurer,
    svg_options: &SvgRenderOptions,
) -> Result<String> {
    Ok(merman_render::svg::render_layouted_svg(
        diagram,
        measurer,
        svg_options,
    )?)
}

/// Synchronous SVG render helper (executor-free).
pub fn render_svg_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<String>> {
    operation::HeadlessRenderOperation::new(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
    )
    .render_svg()
}

pub fn apply_svg_pipeline(svg: &str, pipeline: &SvgPipeline) -> Result<String> {
    Ok(pipeline.process_to_string(svg)?)
}

pub fn apply_svg_pipeline_with_metadata(
    svg: &str,
    pipeline: &SvgPipeline,
    metadata: &SvgPostprocessMetadata,
) -> Result<String> {
    Ok(pipeline.process_to_string_with_metadata(svg, metadata)?)
}

pub fn svg_readable(svg: &str) -> Result<String> {
    apply_svg_pipeline(svg, &SvgPipeline::readable())
}

pub fn svg_resvg_safe(svg: &str) -> Result<String> {
    apply_svg_pipeline(svg, &SvgPipeline::resvg_safe())
}

pub fn render_svg_with_pipeline_sync(
    engine: &merman_core::Engine,
    text: &str,
    parse_options: merman_core::ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    pipeline: &SvgPipeline,
) -> Result<Option<String>> {
    operation::HeadlessRenderOperation::new(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
    )
    .render_svg_with_pipeline(pipeline)
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

    #[test]
    fn render_svg_sync_applies_scoped_theme_css_once() {
        let renderer = HeadlessRenderer::new().with_diagram_id("theme-css");
        let source = r##"%%{init: {"themeCSS": ".node rect { fill: #123456; } @media (max-width: 600px) { text { fill: #654321; } }"}}%%
flowchart TD
  A[Hello] --> B[World]
"##;

        let svg = renderer.render_svg_sync(source).unwrap().unwrap();

        assert_eq!(
            svg.matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
        assert!(svg.contains("#theme-css .node rect { fill: #123456; }"));
        assert!(svg.contains("@media (max-width: 600px) {"));
        assert!(svg.contains("#theme-css text { fill: #654321; }"));
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
    fn headless_renderer_fixed_time_controls_semantic_parse() {
        let renderer = HeadlessRenderer::new()
            .with_fixed_today(Some(
                chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid fixed today"),
            ))
            .with_fixed_local_offset_minutes(Some(0));
        let parsed = renderer
            .parse_diagram_sync(
                r#"gantt
dateFormat MM-DD
section Demo
Missing year: id1,03-01,1d
Missing ref: id2,after missing,1d
"#,
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            task_by_id(&parsed.model, "id1")["startTime"].as_i64(),
            Some(1_772_323_200_000)
        );
        assert_eq!(
            task_by_id(&parsed.model, "id2")["startTime"].as_i64(),
            Some(1_771_113_600_000)
        );
    }
}

/// Convenience wrapper that bundles a [`merman_core::Engine`] and common options for headless rendering.
///
/// This is intended for UI integrations where passing 4-5 separate parameters per call is
/// noisy. It stays runtime-agnostic: all work is CPU-bound and does not perform I/O.
#[derive(Clone)]
pub struct HeadlessRenderer {
    pub engine: merman_core::Engine,
    pub parse: merman_core::ParseOptions,
    pub layout: LayoutOptions,
    pub svg: SvgRenderOptions,
}

impl Default for HeadlessRenderer {
    fn default() -> Self {
        Self {
            engine: merman_core::Engine::new(),
            parse: merman_core::ParseOptions::default(),
            layout: LayoutOptions::headless_svg_defaults(),
            svg: SvgRenderOptions::default(),
        }
    }
}

impl HeadlessRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_site_config(mut self, site_config: merman_core::MermaidConfig) -> Self {
        self.engine = self.engine.with_site_config(site_config);
        self
    }

    pub fn with_fixed_today(mut self, today: Option<chrono::NaiveDate>) -> Self {
        self.engine = self.engine.with_fixed_today(today);
        self
    }

    pub fn with_fixed_local_offset_minutes(mut self, offset_minutes: Option<i32>) -> Self {
        self.engine = self.engine.with_fixed_local_offset_minutes(offset_minutes);
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

    pub fn with_svg_options(mut self, svg: SvgRenderOptions) -> Self {
        self.svg = svg;
        self
    }

    pub fn with_diagram_id(mut self, diagram_id: &str) -> Self {
        self.svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        self
    }

    pub fn with_text_measurer(
        mut self,
        measurer: std::sync::Arc<dyn TextMeasurer + Send + Sync>,
    ) -> Self {
        self.layout = self.layout.with_text_measurer(measurer);
        self
    }

    pub fn with_math_renderer(
        mut self,
        renderer: std::sync::Arc<dyn MathRenderer + Send + Sync>,
    ) -> Self {
        self.layout = self.layout.with_math_renderer(renderer.clone());
        self.svg.math_renderer = Some(renderer);
        self
    }

    pub fn with_vendored_text_measurer(self) -> Self {
        self.with_text_measurer(std::sync::Arc::new(
            VendoredFontMetricsTextMeasurer::default(),
        ))
    }

    pub fn with_deterministic_text_measurer(self) -> Self {
        self.with_text_measurer(std::sync::Arc::new(DeterministicTextMeasurer::default()))
    }

    pub fn parse_metadata_sync(&self, text: &str) -> Result<Option<merman_core::ParseMetadata>> {
        Ok(self.engine.parse_metadata_sync(text, self.parse)?)
    }

    pub fn parse_diagram_sync(&self, text: &str) -> Result<Option<merman_core::ParsedDiagram>> {
        Ok(self.engine.parse_diagram_sync(text, self.parse)?)
    }

    pub fn layout_diagram_sync(&self, text: &str) -> Result<Option<LayoutedDiagram>> {
        layout_diagram_sync(&self.engine, text, self.parse, &self.layout)
    }

    pub fn render_layouted_svg_sync(&self, diagram: &LayoutedDiagram) -> Result<String> {
        render_layouted_svg(diagram, self.layout.text_measurer.as_ref(), &self.svg)
    }

    pub fn render_layouted_svg_sync_with(
        &self,
        diagram: &LayoutedDiagram,
        svg: &SvgRenderOptions,
    ) -> Result<String> {
        render_layouted_svg(diagram, self.layout.text_measurer.as_ref(), svg)
    }

    pub fn render_svg_sync(&self, text: &str) -> Result<Option<String>> {
        render_svg_sync(&self.engine, text, self.parse, &self.layout, &self.svg)
    }

    pub fn render_svg_with_pipeline_sync(
        &self,
        text: &str,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        render_svg_with_pipeline_sync(
            &self.engine,
            text,
            self.parse,
            &self.layout,
            &self.svg,
            pipeline,
        )
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
        self.render_svg_with_pipeline_sync(text, &SvgPipeline::resvg_safe())
    }

    #[cfg(feature = "raster")]
    pub fn render_png_sync(
        &self,
        text: &str,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        raster::render_png_sync(
            &self.engine,
            text,
            self.parse,
            &self.layout,
            &self.svg,
            raster,
        )
    }

    #[cfg(feature = "raster")]
    pub fn render_jpeg_sync(
        &self,
        text: &str,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        raster::render_jpeg_sync(
            &self.engine,
            text,
            self.parse,
            &self.layout,
            &self.svg,
            raster,
        )
    }

    #[cfg(feature = "raster")]
    pub fn render_pdf_sync(&self, text: &str) -> raster::Result<Option<Vec<u8>>> {
        raster::render_pdf_sync(&self.engine, text, self.parse, &self.layout, &self.svg)
    }
}
