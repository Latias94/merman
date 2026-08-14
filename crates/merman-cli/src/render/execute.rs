#[cfg(feature = "svg")]
use super::prepare::{PreparedGraphicalOutput, PreparedGraphicalRender, PreparedGraphicalSource};
use super::prepare::{PreparedSingleOutput, PreparedWorkflow};
#[cfg(feature = "markdown")]
use super::svg_pipeline::svg_metadata;
use crate::error::CliError;
use crate::io::write_output;
#[cfg(feature = "markdown")]
use crate::resources::CheckedBytes;
use crate::runtime::ExecutionContext;
#[cfg(feature = "svg")]
use crate::runtime::SharedWriter;

pub(crate) fn execute_render(
    prepared: PreparedWorkflow,
    context: &mut ExecutionContext,
) -> Result<(), CliError> {
    match prepared {
        PreparedWorkflow::Single(single) => {
            let artifact = match &single.output {
                #[cfg(feature = "ascii")]
                PreparedSingleOutput::Text(text) => {
                    let permit = text.admission.acquire_controlled(&single.control)?;
                    let request = merman::AsciiRequest {
                        options: text.options,
                        resources: text.resources,
                    };
                    let output = text
                        .renderer
                        .renderer
                        .render(text.renderer.request(
                            &single.source,
                            merman::RenderTarget::Ascii(request),
                            single.control.clone(),
                        ))
                        .map_err(|error| map_ascii_render_error(error, text.resources))?;
                    let merman::RenderOutput::Ascii(Some(rendered)) = output else {
                        return Err(CliError::NoDiagram);
                    };
                    ExecutedArtifact::text(rendered.into_bytes(), permit)
                }
                #[cfg(feature = "svg")]
                PreparedSingleOutput::Graphical { renderer, .. } => {
                    execute_graphical(renderer, &single.source, &single.control, &context.stderr)?
                }
            };
            let destination = match &single.output {
                #[cfg(feature = "ascii")]
                PreparedSingleOutput::Text(text) => &text.destination,
                #[cfg(feature = "svg")]
                PreparedSingleOutput::Graphical { destination, .. } => destination,
            };
            write_output(
                destination,
                &artifact.bytes,
                &single.control,
                &single.publications,
                &context.stdout,
                context.publication.as_mut(),
            )
        }
        #[cfg(feature = "markdown")]
        PreparedWorkflow::Markdown(batch) => crate::batch::execute(*batch, context),
    }
}

#[cfg(feature = "ascii")]
fn map_ascii_render_error(
    error: merman::RenderError,
    resources: merman::ascii::AsciiResourcePolicy,
) -> CliError {
    match error {
        merman::RenderError::Parse(error) => {
            CliError::Ascii(merman::ascii::AsciiDiagnostic::from(error))
        }
        merman::RenderError::Ascii(error) => {
            CliError::Ascii(merman::ascii::AsciiDiagnostic::from(error))
        }
        merman::RenderError::RuntimePolicy(error) => {
            CliError::Ascii(merman::ascii::AsciiDiagnostic::from(error))
        }
        merman::RenderError::ResourceLimitExceeded(error) => {
            if merman::ascii::AsciiResourceLimitId::from_stable_id(error.id).is_none() {
                return CliError::Render(merman::RenderError::ResourceLimitExceeded(error));
            }
            CliError::ascii_resource(error, resources.profile())
        }
        other => CliError::Render(other),
    }
}

#[cfg(feature = "svg")]
pub(crate) fn execute_graphical(
    prepared: &PreparedGraphicalRender,
    source: &str,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
) -> Result<ExecutedArtifact, CliError> {
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    let _ = stderr;
    let permit = prepared.admission.acquire_controlled(control)?;
    match &prepared.source {
        PreparedGraphicalSource::Mermaid(renderer) => {
            let mut svg = renderer.svg.clone();
            svg.pipeline = Some(prepared.pipeline.clone());
            let target = prepared.output.target(svg);
            let output =
                renderer
                    .renderer
                    .render(renderer.request(source, target, control.clone()))?;
            match (&prepared.output, output) {
                (PreparedGraphicalOutput::Svg, merman::RenderOutput::Svg(Some(svg))) => {
                    let (svg, _evidence) = svg.into_parts();
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
                (PreparedGraphicalOutput::Svg, merman::RenderOutput::Svg(None)) => {
                    Err(CliError::NoDiagram)
                }
                #[cfg(feature = "png")]
                (PreparedGraphicalOutput::Png { .. }, merman::RenderOutput::Png(Some(output))) => {
                    report_raster_plan(prepared.quiet, output.plan, stderr);
                    Ok(ExecutedArtifact {
                        bytes: output.bytes,
                        _permit: Some(permit),
                        #[cfg(feature = "markdown")]
                        title: None,
                        #[cfg(feature = "markdown")]
                        desc: None,
                    })
                }
                #[cfg(feature = "png")]
                (PreparedGraphicalOutput::Png { .. }, merman::RenderOutput::Png(None)) => {
                    Err(CliError::NoDiagram)
                }
                #[cfg(feature = "jpeg")]
                (
                    PreparedGraphicalOutput::Jpeg { .. },
                    merman::RenderOutput::Jpeg(Some(output)),
                ) => {
                    report_raster_plan(prepared.quiet, output.plan, stderr);
                    Ok(ExecutedArtifact {
                        bytes: output.bytes,
                        _permit: Some(permit),
                        #[cfg(feature = "markdown")]
                        title: None,
                        #[cfg(feature = "markdown")]
                        desc: None,
                    })
                }
                #[cfg(feature = "jpeg")]
                (PreparedGraphicalOutput::Jpeg { .. }, merman::RenderOutput::Jpeg(None)) => {
                    Err(CliError::NoDiagram)
                }
                #[cfg(feature = "pdf")]
                (PreparedGraphicalOutput::Pdf { .. }, merman::RenderOutput::Pdf(Some(output))) => {
                    report_pdf_filter_plan(prepared.quiet, output.plan, stderr);
                    Ok(ExecutedArtifact {
                        bytes: output.bytes,
                        _permit: Some(permit),
                        #[cfg(feature = "markdown")]
                        title: None,
                        #[cfg(feature = "markdown")]
                        desc: None,
                    })
                }
                #[cfg(feature = "pdf")]
                (PreparedGraphicalOutput::Pdf { .. }, merman::RenderOutput::Pdf(None)) => {
                    Err(CliError::NoDiagram)
                }
                (_, _) => Err(CliError::InvalidOutput(
                    "typed renderer returned an unexpected output target".to_string(),
                )),
            }
        }
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        PreparedGraphicalSource::RawSvg(environment) => {
            let session = environment
                .begin_session_with_control(control.clone())
                .map_err(|error| CliError::Render(merman::RenderError::RuntimePolicy(error)))?;
            let svg = prepared
                .pipeline
                .process_resvg_compatible(source, &session)
                .map_err(|error| CliError::Render(merman::RenderError::Svg(error)))?;
            execute_encoded_svg(prepared, svg, permit, control, stderr)
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn execute_encoded_svg(
    prepared: &PreparedGraphicalRender,
    svg: merman::svg::ResvgCompatibleSvg,
    permit: super::admission::BackendPermit,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
) -> Result<ExecutedArtifact, CliError> {
    #[cfg(feature = "markdown")]
    let metadata = svg_metadata(svg.as_str());
    match &prepared.output {
        PreparedGraphicalOutput::Svg => Err(CliError::InvalidOutput(
            "SVG output entered the encoded backend".to_string(),
        )),
        #[cfg(feature = "png")]
        PreparedGraphicalOutput::Png { options } => {
            let prepared_raster =
                merman::svg::export::prepare_raster_controlled(&svg, options, control.clone())
                    .map_err(|error| CliError::Render(merman::RenderError::Export(error)))?;
            let actual_weight = super::admission::actual_raster_weight(
                prepared_raster.plan(),
                prepared_raster.embedded_image_plan(),
                8,
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_raster_plan(prepared.quiet, prepared_raster.plan(), stderr);
            Ok(ExecutedArtifact {
                bytes: prepared_raster
                    .encode_png()
                    .map_err(|error| CliError::Render(merman::RenderError::Export(error)))?,
                _permit: Some(permit),
                #[cfg(feature = "markdown")]
                title: metadata.0,
                #[cfg(feature = "markdown")]
                desc: metadata.1,
            })
        }
        #[cfg(feature = "jpeg")]
        PreparedGraphicalOutput::Jpeg { options } => {
            let prepared_raster =
                merman::svg::export::prepare_raster_controlled(&svg, options, control.clone())
                    .map_err(|error| CliError::Render(merman::RenderError::Export(error)))?;
            let actual_weight = super::admission::actual_raster_weight(
                prepared_raster.plan(),
                prepared_raster.embedded_image_plan(),
                10,
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_raster_plan(prepared.quiet, prepared_raster.plan(), stderr);
            Ok(ExecutedArtifact {
                bytes: prepared_raster
                    .encode_jpeg()
                    .map_err(|error| CliError::Render(merman::RenderError::Export(error)))?,
                _permit: Some(permit),
                #[cfg(feature = "markdown")]
                title: metadata.0,
                #[cfg(feature = "markdown")]
                desc: metadata.1,
            })
        }
        #[cfg(feature = "pdf")]
        PreparedGraphicalOutput::Pdf { options } => {
            let prepared_pdf =
                merman::svg::export::prepare_pdf_controlled(&svg, options, control.clone())
                    .map_err(|error| CliError::Render(merman::RenderError::Export(error)))?;
            let actual_weight = super::admission::actual_pdf_weight(
                prepared_pdf.filter_plan(),
                prepared_pdf.embedded_image_plan(),
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_pdf_filter_plan(prepared.quiet, prepared_pdf.filter_plan(), stderr);
            Ok(ExecutedArtifact {
                bytes: prepared_pdf
                    .encode()
                    .map_err(|error| CliError::Render(merman::RenderError::Export(error)))?,
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
fn report_raster_plan(quiet: bool, plan: merman::svg::export::RasterPlan, stderr: &SharedWriter) {
    if plan.limited {
        crate::diagnostics::DiagnosticSink::new(quiet, stderr).info(format!(
            "Raster output was constrained from {:.0}x{:.0} to {}x{} pixels.",
            plan.requested_width_px, plan.requested_height_px, plan.width_px, plan.height_px
        ));
    }
}

#[cfg(feature = "pdf")]
fn report_pdf_filter_plan(
    quiet: bool,
    plan: merman::svg::export::PdfFilterImagePlan,
    stderr: &SharedWriter,
) {
    if plan.limited {
        crate::diagnostics::DiagnosticSink::new(quiet, stderr).info(format!(
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
        control: &merman::OperationControl,
    ) -> Result<ExecutedMetadata, CliError> {
        // Until this charge succeeds, the bytes remain backend working memory
        // covered by the live permit rather than staged publication memory.
        let bytes = u64::try_from(self.bytes.len())
            .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
        staged_bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_add(bytes)?;
        slot.write_bytes_controlled(&self.bytes, control)?;
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
