#[cfg(feature = "svg")]
use super::prepare::{PreparedGraphicalOutput, PreparedGraphicalRender, PreparedGraphicalSource};
#[cfg(feature = "markdown")]
use super::prepare::{PreparedMarkdownJob, PreparedMarkdownRender};
use super::prepare::{PreparedRender, PreparedSingleOutput};
#[cfg(feature = "markdown")]
use super::svg_pipeline::svg_metadata;
use crate::error::CliError;
use crate::io::write_output;
#[cfg(feature = "markdown")]
use crate::markdown::{self, MarkdownImage};
#[cfg(feature = "markdown")]
use crate::resources::{ByteLedgerKind, CheckedBytes, CountLedgerKind};
#[cfg(feature = "parallel-markdown")]
use rayon::prelude::*;
#[cfg(feature = "markdown")]
use std::path::Path;
#[cfg(feature = "markdown")]
use std::sync::Mutex;

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
        PreparedRender::Markdown(markdown) => match markdown {
            PreparedMarkdownRender::Native(prepared) => {
                execute_markdown(prepared, markdown::scan_native_limited)
            }
            PreparedMarkdownRender::Mmdc11_16_0(prepared) => {
                execute_markdown(prepared, markdown::scan_mmdc_11_16_0_limited)
            }
        },
    }
}

#[cfg(feature = "svg")]
pub(super) fn execute_graphical(
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

pub(super) struct ExecutedArtifact {
    pub(super) bytes: Vec<u8>,
    _permit: Option<super::admission::BackendPermit>,
    #[cfg(feature = "markdown")]
    pub(super) title: Option<String>,
    #[cfg(feature = "markdown")]
    pub(super) desc: Option<String>,
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
    fn charge_and_into_staged(
        mut self,
        staged_bytes: &Mutex<CheckedBytes>,
    ) -> Result<StagedArtifact, CliError> {
        // Until this charge succeeds, the bytes remain backend working memory
        // covered by the live permit rather than staged publication memory.
        let bytes = u64::try_from(self.bytes.len())
            .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
        staged_bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_add(bytes)?;
        let permit = self._permit.take();
        let staged = StagedArtifact {
            bytes: self.bytes,
            title: self.title,
            desc: self.desc,
        };
        drop(permit);
        Ok(staged)
    }
}

#[cfg(feature = "markdown")]
struct StagedArtifact {
    bytes: Vec<u8>,
    title: Option<String>,
    desc: Option<String>,
}

#[cfg(feature = "markdown")]
struct RenderedMarkdownChart {
    output_file: std::path::PathBuf,
    artifact: StagedArtifact,
    image: MarkdownImage,
}

#[cfg(feature = "markdown")]
fn execute_markdown(
    prepared: PreparedMarkdownJob,
    scan: for<'source> fn(
        &'source str,
        Option<u64>,
    ) -> Result<
        Vec<markdown::MarkdownChart<'source>>,
        markdown::MarkdownChartLimitExceeded,
    >,
) -> Result<(), CliError> {
    let mut chart_counter = prepared
        .resources
        .checked_count(CountLedgerKind::MarkdownCharts);
    let charts = match scan(&prepared.source, chart_counter.max()) {
        Ok(charts) => charts,
        Err(error) => {
            debug_assert_eq!(chart_counter.max(), Some(error.max));
            let limit_error = chart_counter
                .try_add(error.observed)
                .expect_err("scanner reported a count above the same policy limit");
            return Err(CliError::markdown_chart(
                error.observed,
                error.location,
                limit_error.into(),
            ));
        }
    };
    let chart_count = u64::try_from(charts.len())
        .map_err(|_| CliError::InvalidInput("Markdown chart count overflow".to_string()))?;
    chart_counter.try_add(chart_count)?;

    if charts.is_empty() {
        crate::diagnostics::DiagnosticSink::new(prepared.quiet)
            .info("No mermaid charts found in Markdown input");
    } else {
        crate::diagnostics::DiagnosticSink::new(prepared.quiet).info(format!(
            "Found {} mermaid charts in Markdown input",
            charts.len()
        ));
    }

    let format = graphical_format(&prepared.renderer.output);
    let output_files = (1..=charts.len())
        .map(|index| {
            markdown::numbered_output_path(
                &prepared.output_path,
                index,
                format,
                prepared.artefacts.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let staged_bytes = Mutex::new(
        prepared
            .resources
            .checked_bytes(ByteLedgerKind::StagedOutput),
    );
    let rendered = render_markdown_charts(&prepared, &charts, &output_files, &staged_bytes)?;
    let writes_document = markdown::is_markdown_path(&prepared.output_path);
    let rewritten = if writes_document {
        let images = rendered
            .iter()
            .map(|rendered| rendered.image.clone())
            .collect::<Vec<_>>();
        let rewritten_len = markdown::rewritten_markdown_len(&prepared.source, &charts, &images)?;
        let bytes = u64::try_from(rewritten_len)
            .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
        staged_bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_add(bytes)?;
        let rewritten = markdown::replace_known_charts_with_images(
            &prepared.source,
            &charts,
            &images,
            rewritten_len,
        );
        Some(rewritten)
    } else {
        None
    };

    for rendered in &rendered {
        crate::io::write_file(
            &rendered.output_file,
            &rendered.artifact.bytes,
            &prepared.publications,
        )?;
    }
    if let Some(rewritten) = rewritten.as_deref() {
        crate::io::write_file(
            &prepared.output_path,
            rewritten.as_bytes(),
            &prepared.publications,
        )?;
    }
    for rendered in &rendered {
        crate::diagnostics::DiagnosticSink::new(prepared.quiet)
            .info(format!(" ✅ {}", rendered.image.url));
    }
    if writes_document {
        crate::diagnostics::DiagnosticSink::new(prepared.quiet).info(format!(
            " ✅ {}",
            crate::error::safe_path(&prepared.output_path)
        ));
    }
    Ok(())
}

#[cfg(feature = "markdown")]
fn render_markdown_charts(
    prepared: &PreparedMarkdownJob,
    charts: &[markdown::MarkdownChart<'_>],
    output_files: &[std::path::PathBuf],
    staged_bytes: &Mutex<CheckedBytes>,
) -> Result<Vec<RenderedMarkdownChart>, CliError> {
    #[cfg(feature = "parallel-markdown")]
    if charts.len() > 1 && prepared.jobs > 1 {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(prepared.jobs)
            .build()
            .map_err(|error| {
                CliError::InvalidInput(format!("failed to configure Markdown render jobs: {error}"))
            })?;
        return pool.install(|| {
            charts
                .par_iter()
                .zip(output_files.par_iter())
                .enumerate()
                .map(|(index, (chart, output_file))| {
                    render_markdown_chart(
                        &prepared.renderer,
                        &prepared.output_path,
                        chart,
                        output_file,
                        staged_bytes,
                        chart_number(index),
                    )
                })
                .collect()
        });
    }

    charts
        .iter()
        .zip(output_files)
        .enumerate()
        .map(|(index, (chart, output_file))| {
            render_markdown_chart(
                &prepared.renderer,
                &prepared.output_path,
                chart,
                output_file,
                staged_bytes,
                chart_number(index),
            )
        })
        .collect()
}

#[cfg(feature = "markdown")]
fn render_markdown_chart(
    renderer: &PreparedGraphicalRender,
    output_path: &Path,
    chart: &markdown::MarkdownChart<'_>,
    output_file: &Path,
    staged_bytes: &Mutex<CheckedBytes>,
    chart_index: u64,
) -> Result<RenderedMarkdownChart, CliError> {
    let result = (|| {
        let artifact = execute_graphical(renderer, chart.definition())?;
        let artifact = artifact.charge_and_into_staged(staged_bytes)?;
        let url = markdown::relative_markdown_url(output_path, output_file)?;
        Ok(RenderedMarkdownChart {
            output_file: output_file.to_path_buf(),
            image: MarkdownImage {
                url,
                title: artifact.title.clone(),
                alt: artifact
                    .desc
                    .clone()
                    .unwrap_or_else(|| "diagram".to_string()),
            },
            artifact,
        })
    })();
    result.map_err(|error| CliError::markdown_chart(chart_index, chart.location(), error))
}

#[cfg(feature = "markdown")]
fn chart_number(index: usize) -> u64 {
    u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1)
}

#[cfg(feature = "markdown")]
fn graphical_format(output: &PreparedGraphicalOutput) -> crate::cli::RenderFormat {
    match output {
        PreparedGraphicalOutput::Svg => crate::cli::RenderFormat::Svg,
        #[cfg(feature = "png")]
        PreparedGraphicalOutput::Png { .. } => crate::cli::RenderFormat::Png,
        #[cfg(feature = "jpeg")]
        PreparedGraphicalOutput::Jpeg { .. } => crate::cli::RenderFormat::Jpeg,
        #[cfg(feature = "pdf")]
        PreparedGraphicalOutput::Pdf { .. } => crate::cli::RenderFormat::Pdf,
    }
}
