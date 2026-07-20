use super::plan::RenderPlan;
use super::raster::EncodingParallelBudget;
use super::svg_pipeline::{svg_metadata, svg_output_policy};
use crate::cli::{RenderFormat, SvgPipelineKind};
use crate::config::renderer_for;
use crate::error::CliError;
use crate::io::write_output;
use merman::render::{HeadlessRenderer, ResvgCompatibleSvg, SvgPipeline};

pub(super) struct RenderRequest<'a> {
    pub(super) plan: &'a RenderPlan,
    pub(super) renderer: HeadlessRenderer,
    pub(super) encoding_parallel_budget: Option<EncodingParallelBudget>,
    pipeline: SvgPipeline,
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
    let markdown_input = plan.is_mmdc_markdown_input();
    let encoding_parallel_budget = (markdown_input && plan.format.requires_svg_encoding())
        .then(|| EncodingParallelBudget::new(plan.raster.encoding_parallel_budget_bytes()));
    let request = RenderRequest::new(&plan, renderer, encoding_parallel_budget);

    if markdown_input {
        request.render_markdown(&text)
    } else {
        request.render(&text)
    }
}

impl<'a> RenderRequest<'a> {
    pub(super) fn new(
        plan: &'a RenderPlan,
        renderer: HeadlessRenderer,
        encoding_parallel_budget: Option<EncodingParallelBudget>,
    ) -> Self {
        let kind = if plan.format.requires_svg_encoding() {
            SvgPipelineKind::ResvgSafe
        } else {
            plan.svg_pipeline.unwrap_or(SvgPipelineKind::Parity)
        };
        let pipeline =
            svg_output_policy(kind, plan.background.as_deref(), plan.css.as_deref()).pipeline();
        Self {
            plan,
            renderer,
            encoding_parallel_budget,
            pipeline,
        }
    }

    fn render(&self, text: &str) -> Result<(), CliError> {
        let artifact = self.render_artifact(text)?;
        write_output(self.plan.output.as_ref(), &artifact.bytes)
    }

    pub(super) fn render_artifact(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        if self.plan.format.is_text() {
            return self.render_text(text);
        }

        if text.trim_start().starts_with("<svg") && self.plan.format.requires_svg_encoding() {
            let svg = self.prepare_raw_svg_for_encoding(text)?;
            return self.encode_prepared_svg(&svg);
        }

        match self.plan.format {
            RenderFormat::Svg => {
                let Some(svg) = self
                    .renderer
                    .render_svg_with_pipeline_sync(text, self.postprocess_pipeline())?
                else {
                    return Err(CliError::NoDiagram);
                };
                Ok(RenderedArtifact::from_svg(svg))
            }
            RenderFormat::Ascii | RenderFormat::Unicode => unreachable!("handled above"),
            RenderFormat::Png | RenderFormat::Jpeg | RenderFormat::Pdf => {
                let Some(svg) = self
                    .renderer
                    .render_resvg_compatible_svg_with_pipeline_sync(
                        text,
                        self.postprocess_pipeline(),
                    )?
                else {
                    return Err(CliError::NoDiagram);
                };
                self.encode_prepared_svg(&svg)
            }
        }
    }

    pub(super) const fn postprocess_pipeline(&self) -> &SvgPipeline {
        &self.pipeline
    }

    fn prepare_raw_svg_for_encoding(&self, svg: &str) -> Result<ResvgCompatibleSvg, CliError> {
        let session = self
            .renderer
            .environment()
            .begin_session()
            .map_err(merman::render::HeadlessError::from)?;
        Ok(self
            .pipeline
            .process_resvg_compatible(svg, &session)
            .map_err(merman::render::HeadlessError::from)?)
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
