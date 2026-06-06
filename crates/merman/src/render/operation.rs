use super::{
    LayoutOptions, Result, SvgPipeline, SvgPostprocessMetadata, SvgRenderOptions,
    apply_svg_pipeline_with_metadata,
};

pub(super) struct HeadlessRenderOperation<'a> {
    engine: &'a merman_core::Engine,
    text: &'a str,
    parse_options: merman_core::ParseOptions,
    layout_options: &'a LayoutOptions,
    svg_options: &'a SvgRenderOptions,
}

impl<'a> HeadlessRenderOperation<'a> {
    pub(super) fn new(
        engine: &'a merman_core::Engine,
        text: &'a str,
        parse_options: merman_core::ParseOptions,
        layout_options: &'a LayoutOptions,
        svg_options: &'a SvgRenderOptions,
    ) -> Self {
        Self {
            engine,
            text,
            parse_options,
            layout_options,
            svg_options,
        }
    }

    pub(super) fn render_svg(&self) -> Result<Option<String>> {
        Ok(self.render_svg_parts()?.map(RenderedSvgParts::into_svg))
    }

    pub(super) fn render_svg_with_pipeline(
        &self,
        pipeline: &SvgPipeline,
    ) -> Result<Option<String>> {
        let Some(parts) = self.render_svg_parts()? else {
            return Ok(None);
        };

        Ok(Some(parts.into_pipeline_svg(pipeline)?))
    }

    fn render_svg_parts(&self) -> Result<Option<RenderedSvgParts>> {
        let Some(parsed) = self
            .engine
            .parse_diagram_for_render_model_sync(self.text, self.parse_options)?
        else {
            return Ok(None);
        };

        let layout = merman_render::layout_parsed_render_layout_only(&parsed, self.layout_options)?;
        let svg = merman_render::svg::render_layout_svg_parts_for_render_model_with_config(
            &layout,
            &parsed.model,
            &parsed.meta.effective_config,
            parsed.meta.title.as_deref(),
            self.layout_options.text_measurer.as_ref(),
            self.svg_options,
        )?;

        Ok(Some(RenderedSvgParts {
            svg,
            diagram_type: parsed.meta.diagram_type,
            diagram_title: parsed.meta.title,
        }))
    }
}

struct RenderedSvgParts {
    svg: String,
    diagram_type: String,
    diagram_title: Option<String>,
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
        } = self;
        let metadata = SvgPostprocessMetadata::from_svg(&svg)
            .with_diagram_type(diagram_type)
            .with_optional_diagram_title(diagram_title);

        apply_svg_pipeline_with_metadata(&svg, pipeline, &metadata)
    }
}
