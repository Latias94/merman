#[cfg(feature = "ratex-math")]
pub use merman_render::math::RatexMathRenderer;
pub use merman_render::math::{MathRenderer, NoopMathRenderer};
pub use merman_render::model::LayoutedDiagram;
pub use merman_render::svg::{
    CssOverridePolicy, CssOverridePostprocessor, ForeignObjectFallbackPostprocessor,
    SanitizeCssPostprocessor, SanitizeSvgAttributesPostprocessor, ScopedCssPostprocessor,
    StripForeignObjectPostprocessor, SvgPipeline, SvgPipelinePreset, SvgPostprocessContext,
    SvgPostprocessMetadata, SvgPostprocessor, SvgRenderOptions,
    foreign_object_label_fallback_svg_text, resvg_safe_svg,
};
pub use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, VendoredFontMetricsTextMeasurer,
};
pub use merman_render::{
    Error as RenderError, LayoutOptions, Result as RenderResult, layout_parsed,
};

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
    let Some(parsed) = engine.parse_diagram_for_render_model_sync(text, parse_options)? else {
        return Ok(None);
    };

    let layout = merman_render::layout_parsed_render_layout_only(&parsed, layout_options)?;
    let svg = merman_render::svg::render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        svg_options,
    )?;

    Ok(Some(svg))
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
    let Some(parsed) = engine.parse_diagram_for_render_model_sync(text, parse_options)? else {
        return Ok(None);
    };

    let layout = merman_render::layout_parsed_render_layout_only(&parsed, layout_options)?;
    let svg = merman_render::svg::render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        svg_options,
    )?;
    let metadata = SvgPostprocessMetadata::from_svg(&svg)
        .with_diagram_type(parsed.meta.diagram_type)
        .with_optional_diagram_title(parsed.meta.title);

    Ok(Some(apply_svg_pipeline_with_metadata(
        &svg, pipeline, &metadata,
    )?))
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
    use std::borrow::Cow;

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
}

/// Convenience wrapper that bundles an [`Engine`] and common options for headless rendering.
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

    pub fn render_svg_readable_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
    ) -> Result<Option<String>> {
        self.render_svg_with_pipeline_sync_with_diagram_id(
            text,
            diagram_id,
            &SvgPipeline::readable(),
        )
    }

    pub fn render_svg_resvg_safe_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
    ) -> Result<Option<String>> {
        self.render_svg_with_pipeline_sync_with_diagram_id(
            text,
            diagram_id,
            &SvgPipeline::resvg_safe(),
        )
    }

    pub fn render_svg_sync_with(
        &self,
        text: &str,
        svg: &SvgRenderOptions,
    ) -> Result<Option<String>> {
        render_svg_sync(&self.engine, text, self.parse, &self.layout, svg)
    }

    pub fn render_svg_with_pipeline_sync_with(
        &self,
        text: &str,
        svg: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        render_svg_with_pipeline_sync(&self.engine, text, self.parse, &self.layout, svg, pipeline)
    }

    pub fn render_svg_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
    ) -> Result<Option<String>> {
        let mut svg = self.svg.clone();
        svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        self.render_svg_sync_with(text, &svg)
    }

    pub fn render_svg_with_pipeline_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        let mut svg = self.svg.clone();
        svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        self.render_svg_with_pipeline_sync_with(text, &svg, pipeline)
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
    pub fn render_png_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        let mut svg = self.svg.clone();
        svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        raster::render_png_sync(&self.engine, text, self.parse, &self.layout, &svg, raster)
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
    pub fn render_jpeg_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        let mut svg = self.svg.clone();
        svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        raster::render_jpeg_sync(&self.engine, text, self.parse, &self.layout, &svg, raster)
    }

    #[cfg(feature = "raster")]
    pub fn render_pdf_sync(&self, text: &str) -> raster::Result<Option<Vec<u8>>> {
        raster::render_pdf_sync(&self.engine, text, self.parse, &self.layout, &self.svg)
    }

    #[cfg(feature = "raster")]
    pub fn render_pdf_sync_with_diagram_id(
        &self,
        text: &str,
        diagram_id: &str,
    ) -> raster::Result<Option<Vec<u8>>> {
        let mut svg = self.svg.clone();
        svg.diagram_id = Some(sanitize_svg_id(diagram_id));
        raster::render_pdf_sync(&self.engine, text, self.parse, &self.layout, &svg)
    }
}
