use super::{
    LayoutOptions, LayoutedDiagram, Result, SvgPipeline, SvgPostprocessMetadata, SvgRenderOptions,
    apply_svg_pipeline_with_metadata,
};
use merman_render::{RenderResourceLimits, ResourceLimitExceeded, ResourceLimitPhase};

#[cfg(feature = "raster")]
use super::raster;

pub(super) struct HeadlessOperation<'a> {
    engine: &'a merman_core::Engine,
    text: &'a str,
    parse_options: merman_core::ParseOptions,
    layout_options: &'a LayoutOptions,
}

#[cfg(feature = "raster")]
enum HeadlessRasterOutput<'a> {
    Png(&'a raster::RasterOptions),
    Jpeg(&'a raster::RasterOptions),
    Pdf,
}

impl<'a> HeadlessOperation<'a> {
    pub(super) fn new(
        engine: &'a merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
    ) -> Self {
        Self {
            engine,
            text,
            parse_options,
            layout_options,
        }
    }

    pub(super) fn layout_diagram(&self) -> Result<Option<LayoutedDiagram>> {
        self.layout_options
            .resource_limits
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
        )?))
    }

    pub(super) fn render_svg(&self, svg_options: &SvgRenderOptions) -> Result<Option<String>> {
        Ok(self
            .render_svg_parts(svg_options)?
            .map(RenderedSvgParts::into_svg))
    }

    pub(super) fn render_svg_with_pipeline(
        &self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        let Some(parts) = self.render_svg_parts(svg_options)? else {
            return Ok(None);
        };

        Ok(Some(parts.into_pipeline_svg(pipeline)?))
    }

    #[cfg(feature = "raster")]
    pub(super) fn render_png(
        &self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        self.render_raster(svg_options, pipeline, HeadlessRasterOutput::Png(raster))
    }

    #[cfg(feature = "raster")]
    pub(super) fn render_jpeg(
        &self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
        raster: &raster::RasterOptions,
    ) -> raster::Result<Option<Vec<u8>>> {
        self.render_raster(svg_options, pipeline, HeadlessRasterOutput::Jpeg(raster))
    }

    #[cfg(feature = "raster")]
    pub(super) fn render_pdf(
        &self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
    ) -> raster::Result<Option<Vec<u8>>> {
        self.render_raster(svg_options, pipeline, HeadlessRasterOutput::Pdf)
    }

    #[cfg(feature = "raster")]
    fn render_raster(
        &self,
        svg_options: &SvgRenderOptions,
        pipeline: &SvgPipeline,
        output: HeadlessRasterOutput<'_>,
    ) -> raster::Result<Option<Vec<u8>>> {
        let Some(svg) = self.render_svg_with_pipeline(svg_options, pipeline)? else {
            return Ok(None);
        };

        let bytes = match output {
            HeadlessRasterOutput::Png(raster) => raster::svg_to_png(&svg, raster)?,
            HeadlessRasterOutput::Jpeg(raster) => raster::svg_to_jpeg(&svg, raster)?,
            HeadlessRasterOutput::Pdf => raster::svg_to_pdf(&svg)?,
        };
        Ok(Some(bytes))
    }

    fn render_svg_parts(&self, svg_options: &SvgRenderOptions) -> Result<Option<RenderedSvgParts>> {
        self.layout_options
            .resource_limits
            .check_source_bytes(self.text)
            .map_err(resource_limit_error)?;
        let Some(parsed) = self
            .engine
            .parse_diagram_for_render_model_sync(self.text, self.parse_options)?
        else {
            return Ok(None);
        };

        let layout = merman_render::layout_parsed_render_layout_only(&parsed, self.layout_options)?;
        let svg = merman_render::svg::render_layout_svg_parts_for_render_model_with_metadata(
            &layout,
            &parsed.model,
            &parsed.meta.effective_config,
            &parsed.meta.diagram_type,
            parsed.meta.title.as_deref(),
            self.layout_options.text_measurer.as_ref(),
            svg_options,
        )?;
        self.layout_options
            .resource_limits
            .check_svg_bytes(&svg, ResourceLimitPhase::SvgOutput)
            .map_err(resource_limit_error)?;

        Ok(Some(RenderedSvgParts {
            svg,
            diagram_type: parsed.meta.diagram_type,
            diagram_title: parsed.meta.title,
            resource_limits: self.layout_options.resource_limits,
        }))
    }
}

struct RenderedSvgParts {
    svg: String,
    diagram_type: String,
    diagram_title: Option<String>,
    resource_limits: RenderResourceLimits,
}

impl RenderedSvgParts {
    fn into_svg(self) -> String {
        self.svg
    }

    fn into_pipeline_svg(self, pipeline: &SvgPipeline) -> Result<String> {
        let Self {
            svg,
            diagram_type,
            diagram_title,
            resource_limits,
        } = self;
        let metadata = SvgPostprocessMetadata::from_svg(&svg)
            .with_diagram_type(diagram_type)
            .with_optional_diagram_title(diagram_title);

        let out = apply_svg_pipeline_with_metadata(&svg, pipeline, &metadata)?;
        resource_limits
            .check_svg_bytes(&out, ResourceLimitPhase::SvgPostprocess)
            .map_err(resource_limit_error)?;
        Ok(out)
    }
}

fn resource_limit_error(err: ResourceLimitExceeded) -> super::HeadlessError {
    merman_render::Error::ResourceLimitExceeded(err).into()
}
