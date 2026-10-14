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
                        viewport: text.viewport,
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
                    let bytes = if text.report {
                        ascii_report_json(&rendered)?
                    } else {
                        rendered.into_bytes()
                    };
                    ExecutedArtifact::text(bytes, permit)
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
fn ascii_report_json(output: &merman::ascii::AsciiOutput) -> Result<Vec<u8>, CliError> {
    if output.encoding != merman::ascii::AsciiOutputEncoding::Plain {
        return Err(CliError::AsciiReportRequiresPlain);
    }
    serde_json::to_vec(&output.report()).map_err(CliError::json_output)
}

#[cfg(feature = "ascii")]
fn map_ascii_render_error(
    error: merman::RenderError,
    resources: merman::ascii::AsciiResourcePolicy,
) -> CliError {
    match error {
        merman::RenderError::Ascii(error) => {
            CliError::Ascii(merman::ascii::AsciiDiagnostic::from(error))
        }
        merman::RenderError::ResourceLimitExceeded(error) => {
            let profile = error
                .provenance
                .as_ref()
                .and_then(|provenance| provenance.profile)
                .unwrap_or_else(|| resources.profile());
            CliError::ascii_resource(error, profile)
        }
        other => CliError::from(other),
    }
}

#[cfg(feature = "svg")]
pub(crate) fn execute_graphical(
    prepared: &PreparedGraphicalRender,
    source: &str,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
) -> Result<ExecutedArtifact, CliError> {
    execute_graphical_with_pipeline(prepared, &prepared.pipeline, source, control, stderr)
}

#[cfg(feature = "rustdoc")]
pub(crate) fn execute_rustdoc_svg_raw(
    prepared: &PreparedGraphicalRender,
    pipeline: &merman::svg::SvgPipeline,
    source: &str,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
) -> Result<Vec<u8>, CliError> {
    execute_graphical_with_pipeline(prepared, pipeline, source, control, stderr)
        .map(ExecutedArtifact::into_bytes)
}

#[cfg(feature = "svg")]
fn execute_graphical_with_pipeline(
    prepared: &PreparedGraphicalRender,
    pipeline: &merman::svg::SvgPipeline,
    source: &str,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
) -> Result<ExecutedArtifact, CliError> {
    #[cfg(not(any(
        feature = "native-fonts",
        feature = "png",
        feature = "jpeg",
        feature = "pdf"
    )))]
    let _ = stderr;
    #[cfg(feature = "native-fonts")]
    let font_context = prepared
        .font_context
        .as_ref()
        .map(|context| context.for_operation(control));
    #[cfg(feature = "native-fonts")]
    let graphical_output = prepared
        .output
        .clone()
        .with_font_context(font_context.clone());
    #[cfg(feature = "native-fonts")]
    let graphical_output = &graphical_output;
    #[cfg(not(feature = "native-fonts"))]
    let graphical_output = &prepared.output;
    #[cfg(feature = "native-fonts")]
    if let Some(context) = &font_context {
        crate::operation::checkpoint(control, merman::OperationPhase::Admission)?;
        context.check().map_err(super::prepare::native_font_error)?;
        let message = if matches!(&prepared.source, PreparedGraphicalSource::Mermaid(_)) {
            "native fonts: shared layout, wrapping, bounds and computed lengths use the selected font context; missing glyphs fall back to deterministic measurement; SVG fonts are not embedded"
        } else {
            "native fonts: raw SVG uses the selected font context for painting; geometry is unchanged"
        };
        crate::diagnostics::DiagnosticSink::new(
            prepared.quiet || !prepared.font_diagnostics,
            stderr,
        )
        .info(message);
    }
    let permit = prepared.admission.acquire_controlled(control)?;
    match &prepared.source {
        PreparedGraphicalSource::Mermaid(renderer) => {
            let mut svg = renderer.svg.clone();
            svg.pipeline = Some(pipeline.clone());
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                svg.environment = svg
                    .environment
                    .with_native_font_context(context.clone())
                    .map_err(super::prepare::native_font_error)?;
            }
            let target = graphical_output.target(svg);
            let output =
                renderer
                    .renderer
                    .render(renderer.request(source, target, control.clone()))?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                crate::operation::checkpoint(control, merman::OperationPhase::Emit)?;
                context.check().map_err(super::prepare::native_font_error)?;
                let faces = context.selected_faces();
                if !faces.is_empty() {
                    crate::diagnostics::DiagnosticSink::new(
                        prepared.quiet || !prepared.font_diagnostics,
                        stderr,
                    )
                    .info(format!(
                        "native measurement faces: {}; context {}",
                        merman::normalize_terminal_diagnostic(&faces.join(", ")),
                        context.identity()
                    ));
                }
            }
            match (graphical_output, output) {
                (PreparedGraphicalOutput::Svg, merman::RenderOutput::Svg(Some(svg))) => {
                    report_fallback_css_residual(
                        prepared.quiet,
                        svg.admission()
                            .reasons()
                            .contains(&merman::TargetAdmissionReason::FallbackCssResidual),
                        stderr,
                    );
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
                (
                    PreparedGraphicalOutput::Png { options },
                    merman::RenderOutput::Document(Some(document)),
                ) => {
                    #[cfg(feature = "markdown")]
                    let metadata = svg_metadata(document.svg());
                    let export = document.prepare_png_export(options, control.clone())?;
                    let report = export.export_report();
                    let actual_weight = super::admission::actual_raster_weight(
                        report.raster(),
                        report.embedded_images(),
                        8,
                    )?;
                    prepared.admission.ensure_actual_weight(actual_weight)?;
                    report_raster_plan(prepared.quiet, report.raster(), stderr);
                    report_export_font_resolution(prepared.quiet, report.fonts(), stderr);
                    report_native_export_quality(
                        prepared.quiet,
                        report.directional_filter_transform_residual(),
                        false,
                        report.fallback_css_residual(),
                        stderr,
                    );
                    let output = export.encode()?;
                    Ok(ExecutedArtifact {
                        bytes: output.into_bytes(),
                        _permit: Some(permit),
                        #[cfg(feature = "markdown")]
                        title: metadata.0,
                        #[cfg(feature = "markdown")]
                        desc: metadata.1,
                    })
                }
                #[cfg(feature = "png")]
                (PreparedGraphicalOutput::Png { .. }, merman::RenderOutput::Document(None)) => {
                    Err(CliError::NoDiagram)
                }
                #[cfg(feature = "jpeg")]
                (
                    PreparedGraphicalOutput::Jpeg { options },
                    merman::RenderOutput::Document(Some(document)),
                ) => {
                    #[cfg(feature = "markdown")]
                    let metadata = svg_metadata(document.svg());
                    let export = document.prepare_jpeg_export(options, control.clone())?;
                    let report = export.export_report();
                    let actual_weight = super::admission::actual_raster_weight(
                        report.raster(),
                        report.embedded_images(),
                        10,
                    )?;
                    prepared.admission.ensure_actual_weight(actual_weight)?;
                    report_raster_plan(prepared.quiet, report.raster(), stderr);
                    report_export_font_resolution(prepared.quiet, report.fonts(), stderr);
                    report_native_export_quality(
                        prepared.quiet,
                        report.directional_filter_transform_residual(),
                        false,
                        report.fallback_css_residual(),
                        stderr,
                    );
                    let output = export.encode()?;
                    Ok(ExecutedArtifact {
                        bytes: output.into_bytes(),
                        _permit: Some(permit),
                        #[cfg(feature = "markdown")]
                        title: metadata.0,
                        #[cfg(feature = "markdown")]
                        desc: metadata.1,
                    })
                }
                #[cfg(feature = "jpeg")]
                (PreparedGraphicalOutput::Jpeg { .. }, merman::RenderOutput::Document(None)) => {
                    Err(CliError::NoDiagram)
                }
                #[cfg(feature = "pdf")]
                (
                    PreparedGraphicalOutput::Pdf { options },
                    merman::RenderOutput::Document(Some(document)),
                ) => {
                    #[cfg(feature = "markdown")]
                    let metadata = svg_metadata(document.svg());
                    let export = document.prepare_pdf_export(options, control.clone())?;
                    let report = export.export_report();
                    let actual_weight = super::admission::actual_pdf_weight(
                        report.filters(),
                        report.embedded_images(),
                    )?;
                    prepared.admission.ensure_actual_weight(actual_weight)?;
                    report_pdf_filter_plan(prepared.quiet, report.filters(), stderr);
                    report_export_font_resolution(prepared.quiet, report.fonts(), stderr);
                    report_native_export_quality(
                        prepared.quiet,
                        report.directional_filter_transform_residual(),
                        report.implicit_viewport_clipping_residual(),
                        report.fallback_css_residual(),
                        stderr,
                    );
                    let output = export.encode()?;
                    Ok(ExecutedArtifact {
                        bytes: output.into_bytes(),
                        _permit: Some(permit),
                        #[cfg(feature = "markdown")]
                        title: metadata.0,
                        #[cfg(feature = "markdown")]
                        desc: metadata.1,
                    })
                }
                #[cfg(feature = "pdf")]
                (PreparedGraphicalOutput::Pdf { .. }, merman::RenderOutput::Document(None)) => {
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
                .map_err(merman::RenderError::from)?;
            let svg = pipeline
                .process_resvg_compatible(source, &session)
                .map_err(merman::RenderError::from)?;
            execute_raw_svg(prepared, graphical_output, svg, permit, control, stderr)
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn execute_raw_svg(
    prepared: &PreparedGraphicalRender,
    output: &PreparedGraphicalOutput,
    svg: merman::svg::ResvgCompatibleSvg,
    permit: super::admission::BackendPermit,
    control: &merman::OperationControl,
    stderr: &SharedWriter,
) -> Result<ExecutedArtifact, CliError> {
    #[cfg(feature = "markdown")]
    let metadata = svg_metadata(svg.as_str());
    match output {
        PreparedGraphicalOutput::Svg => Err(CliError::InvalidOutput(
            "SVG output entered the encoded backend".to_string(),
        )),
        #[cfg(feature = "png")]
        PreparedGraphicalOutput::Png { options } => {
            let prepared_raster =
                merman::svg::export::prepare_raster_controlled(&svg, options, control.clone())
                    .map_err(merman::RenderError::from)?;
            let report =
                prepared_raster.report_for_output(merman::svg::export::RasterOutputKind::Png);
            let actual_weight = super::admission::actual_raster_weight(
                report.raster(),
                report.embedded_images(),
                8,
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_raster_plan(prepared.quiet, report.raster(), stderr);
            report_export_font_resolution(prepared.quiet, report.fonts(), stderr);
            report_native_export_quality(
                prepared.quiet,
                report.directional_filter_transform_residual(),
                false,
                report.fallback_css_residual(),
                stderr,
            );
            Ok(ExecutedArtifact {
                bytes: prepared_raster
                    .encode_png()
                    .map_err(merman::RenderError::from)?,
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
                    .map_err(merman::RenderError::from)?;
            let report =
                prepared_raster.report_for_output(merman::svg::export::RasterOutputKind::Jpeg);
            let actual_weight = super::admission::actual_raster_weight(
                report.raster(),
                report.embedded_images(),
                10,
            )?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_raster_plan(prepared.quiet, report.raster(), stderr);
            report_export_font_resolution(prepared.quiet, report.fonts(), stderr);
            report_native_export_quality(
                prepared.quiet,
                report.directional_filter_transform_residual(),
                false,
                report.fallback_css_residual(),
                stderr,
            );
            Ok(ExecutedArtifact {
                bytes: prepared_raster
                    .encode_jpeg()
                    .map_err(merman::RenderError::from)?,
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
                    .map_err(merman::RenderError::from)?;
            let report = prepared_pdf.report();
            let actual_weight =
                super::admission::actual_pdf_weight(report.filters(), report.embedded_images())?;
            prepared.admission.ensure_actual_weight(actual_weight)?;
            report_pdf_filter_plan(prepared.quiet, report.filters(), stderr);
            report_export_font_resolution(prepared.quiet, report.fonts(), stderr);
            report_native_export_quality(
                prepared.quiet,
                report.directional_filter_transform_residual(),
                report.implicit_viewport_clipping_residual(),
                report.fallback_css_residual(),
                stderr,
            );
            Ok(ExecutedArtifact {
                bytes: prepared_pdf.encode().map_err(merman::RenderError::from)?,
                _permit: Some(permit),
                #[cfg(feature = "markdown")]
                title: metadata.0,
                #[cfg(feature = "markdown")]
                desc: metadata.1,
            })
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn report_native_export_quality(
    quiet: bool,
    directional_filter_transform_residual: bool,
    implicit_viewport_clipping_residual: bool,
    fallback_css_residual: bool,
    stderr: &SharedWriter,
) {
    let diagnostics = crate::diagnostics::DiagnosticSink::new(quiet, stderr);
    if directional_filter_transform_residual {
        diagnostics.info("warning: native_directional_filter_transform_residual: directional filter offsets may differ under rotation, shear or reflection; best-effort rendering may differ");
    }
    if implicit_viewport_clipping_residual {
        diagnostics.info("warning: pdf_implicit_viewport_clipping_residual: the derived PDF viewport may clip painted content; specify an explicit SVG viewport to control clipping");
    }
    report_fallback_css_residual(quiet, fallback_css_residual, stderr);
}

#[cfg(feature = "svg")]
fn report_fallback_css_residual(quiet: bool, residual: bool, stderr: &SharedWriter) {
    if residual {
        crate::diagnostics::DiagnosticSink::new(quiet, stderr).info("warning: fallback_css_residual: backend CSS may differ for converted HTML labels; best-effort rendering may differ");
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn report_export_font_resolution(
    quiet: bool,
    fonts: merman::svg::export::ExportFontPlan,
    stderr: &SharedWriter,
) {
    if fonts.unresolved_font_request()
        || fonts.unresolved_glyph_fallback()
        || fonts.notdef_glyph_count() != 0
    {
        crate::diagnostics::DiagnosticSink::new(quiet, stderr).info(format!(
            "warning: native export font resolution is incomplete: unresolved font request={}; unresolved glyph fallback={}; .notdef glyphs={}; provide fonts covering the output text",
            fonts.unresolved_font_request(),
            fonts.unresolved_glyph_fallback(),
            fonts.notdef_glyph_count(),
        ));
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
    #[cfg(feature = "rustdoc")]
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

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
