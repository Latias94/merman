use super::{
    LayoutOptions, LayoutedDiagram, Result, SvgDebugOptions, SvgPipeline, SvgPostprocessMetadata,
    SvgRenderOptions, apply_svg_pipeline_with_metadata,
};
use merman_render::{
    ResourceLimitExceeded, ResourceLimitPhase,
    environment::{RenderEnvironment, RenderOperationReport, RenderSession},
    model::LayoutDiagram,
};

#[cfg(feature = "raster")]
use super::raster;

pub(super) struct HeadlessOperation<'a> {
    engine: merman_core::Engine,
    text: &'a str,
    parse_options: merman_core::ParseOptions,
    layout_options: &'a LayoutOptions,
    session: RenderSession,
}

/// A canonical typed parse that has not started layout yet.
///
/// Callers can inspect metadata and the family-owned semantic model to apply admission policy.
/// Continuing to layout consumes this stage, so the same parsed model cannot start layout twice.
pub struct PreparedSemantic {
    parsed: merman_core::ParsedDiagramRender,
    layout_options: LayoutOptions,
    session: RenderSession,
}

impl PreparedSemantic {
    /// Returns preprocessed metadata for admission and diagnostics.
    pub fn metadata(&self) -> &merman_core::ParseMetadata {
        &self.parsed.meta
    }

    /// Returns the family-owned typed semantic variant name without exposing mutable pairing.
    pub fn semantic_kind(&self) -> &'static str {
        self.parsed.model.kind()
    }

    /// Runs layout exactly once and advances to the pre-SVG render stage.
    pub fn continue_layout(self) -> Result<PreparedRender> {
        let Self {
            parsed,
            layout_options,
            session,
        } = self;
        let layout =
            merman_render::layout_parsed_render_layout_only(&parsed, &layout_options, &session)?;

        Ok(PreparedRender {
            parsed,
            layout,
            session,
        })
    }
}

/// A canonical typed render that has completed parsing and layout exactly once.
///
/// The artifact exposes borrowed semantic, metadata, and layout views so callers can collect
/// diagnostics or derive request policy before rendering. SVG rendering consumes the artifact,
/// which prevents the prepared parse/layout result from being rendered more than once.
///
/// ```compile_fail
/// use merman::render::{LayoutOptions, SvgRenderOptions, prepare_render_sync};
/// use merman::{Engine, ParseOptions};
///
/// let prepared = prepare_render_sync(
///     &Engine::new(),
///     "info",
///     ParseOptions::strict(),
///     &LayoutOptions::headless_svg_defaults(),
/// )
/// .unwrap()
/// .unwrap();
/// let options = SvgRenderOptions::default();
/// let _first = prepared.render_svg(&options);
/// let _second = prepared.render_svg(&options); // `prepared` was consumed above.
/// ```
pub struct PreparedRender {
    parsed: merman_core::ParsedDiagramRender,
    layout: LayoutDiagram,
    session: RenderSession,
}

/// Complete SVG output plus immutable evidence from the operation-owned session.
pub struct RenderedSvg {
    svg: String,
    report: RenderOperationReport,
}

impl RenderedSvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn report(&self) -> &RenderOperationReport {
        &self.report
    }

    pub fn into_svg(self) -> String {
        self.svg
    }

    pub fn into_parts(self) -> (String, RenderOperationReport) {
        (self.svg, self.report)
    }
}

impl PreparedRender {
    /// Returns preprocessed metadata for diagnostics and request policy.
    pub fn metadata(&self) -> &merman_core::ParseMetadata {
        &self.parsed.meta
    }

    /// Returns the family-owned typed semantic variant name without exposing mutable pairing.
    pub fn semantic_kind(&self) -> &'static str {
        self.parsed.model.kind()
    }

    /// Returns the typed layout produced from this artifact's semantic model.
    pub fn layout(&self) -> &LayoutDiagram {
        &self.layout
    }

    /// Renders SVG once from the prepared semantic model and layout.
    pub fn render_svg(self, svg_options: &SvgRenderOptions) -> Result<String> {
        self.render_svg_with_debug(svg_options, &SvgDebugOptions::default())
    }

    pub fn render_svg_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<String> {
        self.render_svg_report_with_debug(svg_options, debug_options)
            .map(RenderedSvg::into_svg)
    }

    pub fn render_svg_report(self, svg_options: &SvgRenderOptions) -> Result<RenderedSvg> {
        self.render_svg_report_with_debug(svg_options, &SvgDebugOptions::default())
    }

    pub fn render_svg_report_with_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<RenderedSvg> {
        self.render_svg_parts(svg_options, debug_options)
            .map(RenderedSvgParts::into_rendered_svg)
    }

    /// Renders SVG once and applies the supplied postprocessing pipeline.
    pub fn render_svg_with_pipeline(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<String> {
        self.render_svg_with_pipeline_and_debug(svg_options, &SvgDebugOptions::default(), pipeline)
    }

    pub fn render_svg_with_pipeline_and_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<String> {
        self.render_svg_with_pipeline_report_and_debug(svg_options, debug_options, pipeline)
            .map(RenderedSvg::into_svg)
    }

    pub fn render_svg_with_pipeline_report(
        self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedSvg> {
        self.render_svg_with_pipeline_report_and_debug(
            svg_options,
            &SvgDebugOptions::default(),
            pipeline,
        )
    }

    pub fn render_svg_with_pipeline_report_and_debug(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<RenderedSvg> {
        self.render_svg_parts(svg_options, debug_options)?
            .into_pipeline_svg(pipeline)
    }

    fn render_svg_parts(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<RenderedSvgParts> {
        let Self {
            parsed,
            layout,
            session,
        } = self;
        let svg =
            merman_render::svg::render_layout_svg_parts_for_render_model_with_metadata_and_debug(
                &layout,
                &parsed.model,
                &parsed.meta.effective_config,
                &parsed.meta.diagram_type,
                parsed.meta.title.as_deref(),
                &session,
                svg_options,
                debug_options,
            )?;
        session
            .resource_limits()
            .check_svg_bytes(&svg, ResourceLimitPhase::SvgOutput)
            .map_err(resource_limit_error)?;

        Ok(RenderedSvgParts {
            svg,
            diagram_type: parsed.meta.diagram_type,
            diagram_title: parsed.meta.title,
            session,
        })
    }
}

#[cfg(feature = "raster")]
enum HeadlessRasterOutput<'a> {
    Png(&'a raster::RasterOptions),
    Jpeg(&'a raster::RasterOptions),
    Pdf,
}

impl<'a> HeadlessOperation<'a> {
    pub(super) fn new(
        engine: &merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
        environment: &RenderEnvironment,
    ) -> Result<Self> {
        let session = environment.begin_session()?;
        let snapshot = session.time();
        let engine = engine
            .clone()
            .with_fixed_today(Some(snapshot.local_date()))
            .with_fixed_local_offset_minutes(Some(snapshot.local_offset_minutes()));
        Ok(Self {
            engine,
            text,
            parse_options,
            layout_options,
            session,
        })
    }

    pub(super) fn layout_diagram(&self) -> Result<Option<LayoutedDiagram>> {
        self.session
            .resource_limits()
            .check_source_bytes(self.text)
            .map_err(resource_limit_error)?;
        let Some(parsed) = self
            .engine
            .parse_diagram_sync(self.text, self.parse_options)?
        else {
            return Ok(None);
        };

        Ok(Some(merman_render::layout_parsed(
            &parsed,
            self.layout_options,
            &self.session,
        )?))
    }

    pub(super) fn prepare_render(self) -> Result<Option<PreparedRender>> {
        let Some(semantic) = self.prepare_semantic()? else {
            return Ok(None);
        };

        Ok(Some(semantic.continue_layout()?))
    }

    pub(super) fn prepare_semantic(self) -> Result<Option<PreparedSemantic>> {
        self.session
            .resource_limits()
            .check_source_bytes(self.text)
            .map_err(resource_limit_error)?;
        let Some(parsed) = self
            .engine
            .parse_diagram_for_render_model_sync(self.text, self.parse_options)?
        else {
            return Ok(None);
        };

        Ok(Some(PreparedSemantic {
            parsed,
            layout_options: self.layout_options.clone(),
            session: self.session,
        }))
    }

    pub(super) fn render_svg(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
    ) -> Result<Option<String>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };

        Ok(Some(
            prepared.render_svg_with_debug(svg_options, debug_options)?,
        ))
    }

    pub(super) fn render_svg_with_pipeline(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        let Some(prepared) = self.prepare_render()? else {
            return Ok(None);
        };

        Ok(Some(prepared.render_svg_with_pipeline_and_debug(
            svg_options,
            debug_options,
            pipeline,
        )?))
    }

    #[cfg(feature = "raster")]
    pub(super) fn render_png(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        self.render_raster(
            svg_options,
            debug_options,
            pipeline,
            HeadlessRasterOutput::Png(raster),
        )
    }

    #[cfg(feature = "raster")]
    pub(super) fn render_jpeg(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        self.render_raster(
            svg_options,
            debug_options,
            pipeline,
            HeadlessRasterOutput::Jpeg(raster),
        )
    }

    #[cfg(feature = "raster")]
    pub(super) fn render_pdf(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
    ) -> raster::Result<Option<Vec<u8>>> {
        self.render_raster(
            svg_options,
            debug_options,
            pipeline,
            HeadlessRasterOutput::Pdf,
        )
    }

    #[cfg(feature = "raster")]
    fn render_raster(
        self,
        svg_options: &SvgRenderOptions,
        debug_options: &SvgDebugOptions,
        pipeline: &SvgPipeline,
        output: HeadlessRasterOutput<'_>,
    ) -> raster::Result<Option<Vec<u8>>> {
        let Some(svg) = self.render_svg_with_pipeline(svg_options, debug_options, pipeline)? else {
            return Ok(None);
        };

        let bytes = match output {
            HeadlessRasterOutput::Png(raster) => raster::svg_to_png(&svg, raster)?,
            HeadlessRasterOutput::Jpeg(raster) => raster::svg_to_jpeg(&svg, raster)?,
            HeadlessRasterOutput::Pdf => raster::svg_to_pdf(&svg)?,
        };
        Ok(Some(bytes))
    }
}

struct RenderedSvgParts {
    svg: String,
    diagram_type: String,
    diagram_title: Option<String>,
    session: RenderSession,
}

impl RenderedSvgParts {
    fn into_rendered_svg(self) -> RenderedSvg {
        let report = self.session.report();
        RenderedSvg {
            svg: self.svg,
            report,
        }
    }

    fn into_pipeline_svg(self, pipeline: &SvgPipeline) -> Result<RenderedSvg> {
        let Self {
            svg,
            diagram_type,
            diagram_title,
            session,
        } = self;
        let metadata = SvgPostprocessMetadata::from_svg(&svg)
            .with_diagram_type(diagram_type)
            .with_optional_diagram_title(diagram_title);

        let out = apply_svg_pipeline_with_metadata(&svg, pipeline, &metadata, &session)?;
        session
            .resource_limits()
            .check_svg_bytes(&out, ResourceLimitPhase::SvgPostprocess)
            .map_err(resource_limit_error)?;
        let report = session.report();
        Ok(RenderedSvg { svg: out, report })
    }
}

pub(super) fn resource_limit_error(err: ResourceLimitExceeded) -> super::HeadlessError {
    merman_render::Error::ResourceLimitExceeded(err).into()
}
