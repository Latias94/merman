#[cfg(feature = "svg")]
use super::prepare::{PreparedGraphicalOutput, PreparedGraphicalRender, PreparedGraphicalSource};
use super::prepare::{PreparedRender, PreparedSingleOutput};
#[cfg(feature = "markdown")]
use super::svg_pipeline::svg_metadata;
use crate::error::CliError;
use crate::io::write_output;
#[cfg(feature = "markdown")]
use crate::resources::CheckedBytes;

pub(crate) fn execute_render(prepared: PreparedRender) -> Result<(), CliError> {
    match prepared {
        PreparedRender::Single(single) => {
            let artifact = match &single.output {
                #[cfg(feature = "ascii")]
                PreparedSingleOutput::Text {
                    renderer,
                    admission,
                    ..
                } => {
                    let permit = admission.acquire()?;
                    let Some(rendered) = renderer.render_ascii_sync(&single.source)? else {
                        return Err(CliError::NoDiagram);
                    };
                    ExecutedArtifact::text(rendered.into_bytes(), permit)
                }
                #[cfg(feature = "svg")]
                PreparedSingleOutput::Graphical { renderer, .. } => {
                    execute_graphical(renderer, &single.source)?
                }
            };
            let destination = match &single.output {
                #[cfg(feature = "ascii")]
                PreparedSingleOutput::Text { destination, .. } => destination,
                #[cfg(feature = "svg")]
                PreparedSingleOutput::Graphical { destination, .. } => destination,
            };
            write_output(destination, &artifact.bytes, &single.publications)
        }
        #[cfg(feature = "markdown")]
        PreparedRender::Markdown(batch) => crate::batch::execute(batch),
    }
}

#[cfg(feature = "svg")]
pub(crate) fn execute_graphical(
    prepared: &PreparedGraphicalRender,
    source: &str,
) -> Result<ExecutedArtifact, CliError> {
    let permit = prepared.admission.acquire()?;
    match &prepared.source {
        PreparedGraphicalSource::Mermaid(renderer) => match &prepared.output {
            PreparedGraphicalOutput::Svg => {
                let Some(svg) =
                    renderer.render_svg_with_pipeline_sync(source, &prepared.pipeline)?
                else {
                    return Err(CliError::NoDiagram);
                };
                #[cfg(feature = "markdown")]
                let metadata = svg_metadata(&svg);
                Ok(ExecutedArtifact {
                    bytes: svg.into_bytes(),
                    _permit: Some(permit),
                    #[cfg(feature = "markdown")]
                    title: metadata.0,
                    #[cfg(feature = "markdown")]
                    desc: metadata.1,
                })
            }
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            _ => {
                let Some(svg) = renderer
                    .render_resvg_compatible_svg_with_pipeline_sync(source, &prepared.pipeline)?
                else {
                    return Err(CliError::NoDiagram);
                };
                execute_encoded_svg(prepared, svg, permit)
            }
        },
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        PreparedGraphicalSource::RawSvg(environment) => {
            let session = environment
                .begin_session()
                .map_err(merman::svg::HeadlessError::from)?;
            let svg = prepared
                .pipeline
                .process_resvg_compatible(source, &session)
                .map_err(merman::svg::HeadlessError::from)?;
            execute_encoded_svg(prepared, svg, permit)
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn execute_encoded_svg(
    prepared: &PreparedGraphicalRender,
    svg: merman::svg::ResvgCompatibleSvg,
    permit: super::admission::BackendPermit,
) -> Result<ExecutedArtifact, CliError> {
    #[cfg(feature = "markdown")]
    let metadata = svg_metadata(svg.as_str());
    match &prepared.output {
        PreparedGraphicalOutput::Svg => Err(CliError::InvalidOutput(
            "SVG output entered the encoded backend".to_string(),
        )),
        #[cfg(feature = "png")]
        PreparedGraphicalOutput::Png { options } => {
            let prepared_raster = merman::svg::export::prepare_raster(&svg, options)?;
            let actual_weight = super::admission::actual_raster_weight(
                prepared_raster.plan(),
                prepared_raster.embedded_image_plan(),
                8,
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_raster_plan(prepared.quiet, prepared_raster.plan());
            Ok(ExecutedArtifact {
                bytes: prepared_raster.encode_png()?,
                _permit: Some(permit),
                #[cfg(feature = "markdown")]
                title: metadata.0,
                #[cfg(feature = "markdown")]
                desc: metadata.1,
            })
        }
        #[cfg(feature = "jpeg")]
        PreparedGraphicalOutput::Jpeg { options } => {
            let prepared_raster = merman::svg::export::prepare_raster(&svg, options)?;
            let actual_weight = super::admission::actual_raster_weight(
                prepared_raster.plan(),
                prepared_raster.embedded_image_plan(),
                10,
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_raster_plan(prepared.quiet, prepared_raster.plan());
            Ok(ExecutedArtifact {
                bytes: prepared_raster.encode_jpeg()?,
                _permit: Some(permit),
                #[cfg(feature = "markdown")]
                title: metadata.0,
                #[cfg(feature = "markdown")]
                desc: metadata.1,
            })
        }
        #[cfg(feature = "pdf")]
        PreparedGraphicalOutput::Pdf { options } => {
            let prepared_pdf = merman::svg::export::prepare_pdf(&svg, options)?;
            let actual_weight = super::admission::actual_pdf_weight(
                prepared_pdf.filter_plan(),
                prepared_pdf.embedded_image_plan(),
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_pdf_filter_plan(prepared.quiet, prepared_pdf.filter_plan());
            Ok(ExecutedArtifact {
                bytes: prepared_pdf.encode()?,
                _permit: Some(permit),
                #[cfg(feature = "markdown")]
                title: metadata.0,
                #[cfg(feature = "markdown")]
                desc: metadata.1,
            })
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn report_raster_plan(quiet: bool, plan: merman::svg::export::RasterPlan) {
    if plan.limited {
        crate::diagnostics::DiagnosticSink::new(quiet).info(format!(
            "Raster output was constrained from {:.0}x{:.0} to {}x{} pixels.",
            plan.requested_width_px, plan.requested_height_px, plan.width_px, plan.height_px
        ));
    }
}

#[cfg(feature = "pdf")]
fn report_pdf_filter_plan(quiet: bool, plan: merman::svg::export::PdfFilterImagePlan) {
    if plan.limited {
        crate::diagnostics::DiagnosticSink::new(quiet).info(format!(
            "PDF filter sampling was constrained from {} to {} retained image pixels (scale {:.3} -> {:.3}).",
            plan.requested_image_pixels,
            plan.effective_image_pixels,
            plan.requested_scale,
            plan.effective_scale
        ));
    }
}

pub(crate) struct ExecutedArtifact {
    bytes: Vec<u8>,
    _permit: Option<super::admission::BackendPermit>,
    #[cfg(feature = "markdown")]
    title: Option<String>,
    #[cfg(feature = "markdown")]
    desc: Option<String>,
}

impl ExecutedArtifact {
    #[cfg(feature = "ascii")]
    fn text(bytes: Vec<u8>, permit: super::admission::BackendPermit) -> Self {
        Self {
            bytes,
            _permit: Some(permit),
            #[cfg(feature = "markdown")]
            title: None,
            #[cfg(feature = "markdown")]
            desc: None,
        }
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn stage_into(
        self,
        slot: crate::transaction::StageSlot,
        staged_bytes: &std::sync::Mutex<CheckedBytes>,
    ) -> Result<ExecutedMetadata, CliError> {
        // Until this charge succeeds, the bytes remain backend working memory
        // covered by the live permit rather than staged publication memory.
        let bytes = u64::try_from(self.bytes.len())
            .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
        staged_bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_add(bytes)?;
        slot.write_bytes(&self.bytes)?;
        Ok(ExecutedMetadata {
            title: self.title,
            desc: self.desc,
        })
    }
}

#[cfg(feature = "markdown")]
pub(crate) struct ExecutedMetadata {
    pub(crate) title: Option<String>,
    pub(crate) desc: Option<String>,
}
