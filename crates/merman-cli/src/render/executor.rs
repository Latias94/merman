use super::plan::RenderPlan;
use super::svg_pipeline::{svg_metadata, svg_pipeline_from_kind, svg_postprocess_pipeline};
use crate::cli::{RenderFormat, SvgPipelineKind};
use crate::config::renderer_for;
use crate::error::CliError;
use crate::io::write_output;
use merman::render::{HeadlessRenderer, SvgPipeline};

pub(super) struct RenderRequest<'a> {
    pub(super) plan: &'a RenderPlan,
    pub(super) renderer: HeadlessRenderer,
}

pub(super) struct RenderedArtifact {
    pub(super) bytes: Vec<u8>,
    pub(super) title: Option<String>,
    pub(super) desc: Option<String>,
}

pub(crate) fn run_render(plan: RenderPlan) -> Result<(), CliError> {
    plan.warn_for_accepted_compat_options();
    let text = crate::io::read_input(plan.input.as_deref(), plan.quiet)?;

    let renderer = renderer_for(&plan.parse, &plan.render, plan.icon_registry.clone())?;
    let request = RenderRequest {
        plan: &plan,
        renderer,
    };

    if plan.is_mmdc_markdown_input() {
        request.render_markdown(&text)
    } else {
        request.render(&text)
    }
}

impl<'a> RenderRequest<'a> {
    fn render(&self, text: &str) -> Result<(), CliError> {
        let artifact = self.render_artifact(text)?;
        write_output(self.plan.output.as_ref(), &artifact.bytes)
    }

    pub(super) fn render_artifact(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        if self.plan.format.is_text() {
            return self.render_text(text);
        }

        if text.trim_start().starts_with("<svg") && self.plan.format.is_raster() {
            let svg = self.postprocess_raw_svg_for_raster(text)?;
            return self.rasterize_prepared_svg(&svg);
        }

        let pipeline = self.postprocess_pipeline();
        let Some(svg) = self
            .renderer
            .render_svg_with_pipeline_sync(text, &pipeline)?
        else {
            return Err(CliError::NoDiagram);
        };

        match self.plan.format {
            RenderFormat::Svg => Ok(RenderedArtifact::from_svg(svg)),
            RenderFormat::Ascii | RenderFormat::Unicode => unreachable!("handled above"),
            RenderFormat::Png | RenderFormat::Jpeg | RenderFormat::Pdf => {
                self.rasterize_prepared_svg(&svg)
            }
        }
    }

    pub(super) fn postprocess_pipeline(&self) -> SvgPipeline {
        let pipeline = if self.plan.format.is_raster() {
            SvgPipeline::resvg_safe()
        } else {
            svg_pipeline_from_kind(self.plan.svg_pipeline.unwrap_or(SvgPipelineKind::Parity))
        };
        svg_postprocess_pipeline(
            pipeline,
            self.plan.background.as_deref(),
            self.plan.css.as_deref(),
        )
    }

    fn raw_svg_raster_pipeline(&self) -> SvgPipeline {
        svg_postprocess_pipeline(
            SvgPipeline::resvg_safe(),
            self.plan.background.as_deref(),
            self.plan.css.as_deref(),
        )
    }

    fn postprocess_raw_svg_for_raster(&self, svg: &str) -> Result<String, CliError> {
        let pipeline = self.raw_svg_raster_pipeline();
        let session = self
            .renderer
            .environment()
            .begin_session()
            .map_err(merman::render::HeadlessError::from)?;
        Ok(merman::render::apply_svg_pipeline(
            svg, &pipeline, &session,
        )?)
    }

    #[cfg(feature = "ascii")]
    fn render_text(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        let options = match self.plan.format {
            RenderFormat::Ascii => merman::ascii::AsciiRenderOptions::ascii(),
            RenderFormat::Unicode => merman::ascii::AsciiRenderOptions::unicode(),
            _ => {
                return Err(CliError::InvalidOutput(
                    "text output requested for a non-text format".to_string(),
                ));
            }
        };
        let options = self.plan.apply_text_options(options)?;
        let Some(rendered) = merman::ascii::render_ascii_sync(
            self.renderer.engine(),
            text,
            self.renderer.parse_options(),
            &options,
        )?
        else {
            return Err(CliError::NoDiagram);
        };
        Ok(RenderedArtifact {
            bytes: rendered.into_bytes(),
            title: None,
            desc: None,
        })
    }

    #[cfg(not(feature = "ascii"))]
    fn render_text(&self, _text: &str) -> Result<RenderedArtifact, CliError> {
        let _ = &self.plan.text;
        Err(CliError::InvalidOutput(
            "ASCII/Unicode output requires building merman-cli with --features ascii.".to_string(),
        ))
    }

    pub(super) fn info(&self, message: &str) {
        if !self.plan.quiet {
            eprintln!("{message}");
        }
    }
}

impl RenderedArtifact {
    fn from_svg(svg: String) -> Self {
        let (title, desc) = svg_metadata(&svg);
        Self {
            bytes: svg.into_bytes(),
            title,
            desc,
        }
    }
}
