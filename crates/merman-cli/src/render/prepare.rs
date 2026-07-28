use super::admission::BackendAdmission;
#[cfg(feature = "icons")]
use super::icons::load_icon_registry;
#[cfg(feature = "svg")]
use super::svg_pipeline::svg_output_policy;
#[cfg(feature = "svg")]
use crate::cli::SvgPipelineKind;
use crate::error::CliError;
use crate::input::InputLimit;
#[cfg(feature = "markdown")]
use crate::invocation::ResolvedBatchRender;
#[cfg(feature = "svg")]
use crate::invocation::ResolvedMmdcRender;
use crate::invocation::{
    ResolvedDestination, ResolvedOutput, ResolvedRenderCommon, ResolvedSingleRender,
};
use crate::io::read_input;
#[cfg(feature = "svg")]
use crate::io::{read_named_text_file, read_optional_text_file};
use crate::output::PublicationGuards;
use crate::resources::ResolvedResourcePolicy;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman::svg::RenderEnvironment;
#[cfg(feature = "svg")]
use merman::svg::{HeadlessRenderer, SvgPipeline};
use std::path::Path;
#[cfg(feature = "markdown")]
use std::path::PathBuf;

#[cfg(feature = "pdf")]
const MMDC_DEFAULT_PDF_WIDTH_PT: f32 = 612.0;
#[cfg(feature = "pdf")]
const MMDC_DEFAULT_PDF_HEIGHT_PT: f32 = 792.0;

pub(crate) enum PreparedRender {
    Single(PreparedSingleRender),
    #[cfg(feature = "markdown")]
    Markdown(PreparedMarkdownRender),
}

pub(crate) struct PreparedSingleRender {
    pub(super) source: String,
    pub(super) output: PreparedSingleOutput,
    pub(super) publications: PublicationGuards,
}

pub(super) enum PreparedSingleOutput {
    #[cfg(feature = "ascii")]
    Text {
        destination: ResolvedDestination,
        renderer: merman::ascii::HeadlessAsciiRenderer,
        admission: BackendAdmission,
    },
    #[cfg(feature = "svg")]
    Graphical {
        destination: ResolvedDestination,
        renderer: PreparedGraphicalRender,
    },
}

#[cfg(feature = "svg")]
pub(super) struct PreparedGraphicalRender {
    pub(super) source: PreparedGraphicalSource,
    pub(super) pipeline: SvgPipeline,
    pub(super) output: PreparedGraphicalOutput,
    pub(super) admission: BackendAdmission,
    pub(super) quiet: bool,
}

#[cfg(feature = "svg")]
pub(super) enum PreparedGraphicalSource {
    Mermaid(HeadlessRenderer),
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    RawSvg(RenderEnvironment),
}

#[cfg(feature = "svg")]
pub(super) enum PreparedGraphicalOutput {
    Svg,
    #[cfg(feature = "png")]
    Png {
        options: merman::svg::export::RasterOptions,
    },
    #[cfg(feature = "jpeg")]
    Jpeg {
        options: merman::svg::export::RasterOptions,
    },
    #[cfg(feature = "pdf")]
    Pdf {
        options: merman::svg::export::PdfOptions,
    },
}

#[cfg(feature = "markdown")]
pub(crate) struct PreparedMarkdownRender {
    pub(super) source: String,
    pub(super) renderer: PreparedGraphicalRender,
    pub(super) output_path: PathBuf,
    pub(super) artefacts: Option<PathBuf>,
    pub(super) resources: ResolvedResourcePolicy,
    pub(super) publications: PublicationGuards,
    #[cfg(feature = "parallel-markdown")]
    pub(super) jobs: usize,
    pub(super) quiet: bool,
}

pub(crate) fn prepare_render_for_native(
    resolved: ResolvedSingleRender,
    publications: PublicationGuards,
) -> Result<PreparedRender, CliError> {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    let raw_svg = matches!(resolved.input_kind, crate::cli::RenderInputKind::Svg);
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    let raw_svg = false;
    let limit = render_input_limit(raw_svg, &resolved.common.resources);
    let source = read_resolved_input(&resolved.input, true, limit)?;
    prepare_single(
        source,
        raw_svg,
        resolved.output,
        resolved.common,
        publications,
        false,
    )
    .map(PreparedRender::Single)
}

#[cfg(feature = "markdown")]
pub(crate) fn prepare_render_for_batch(
    resolved: ResolvedBatchRender,
    publications: PublicationGuards,
) -> Result<PreparedRender, CliError> {
    let source = read_resolved_input(
        &resolved.input,
        true,
        InputLimit::new(
            crate::resources::CliResourceLimitId::MaxMarkdownDocumentBytes.as_str(),
            resolved.common.resources.files().markdown_document_bytes,
        ),
    )?;
    let output_path = resolved
        .output
        .destination()
        .file()
        .expect("native batch normalization always selects a file target")
        .to_path_buf();
    let common = resolved.common;
    let resources = common.resources;
    let (_, renderer) = prepare_graphical_output(resolved.output, common, false, false)?;
    let quiet = renderer_quiet(&renderer);

    publications.prepare_directory(&resolved.output_root)?;
    Ok(PreparedRender::Markdown(PreparedMarkdownRender {
        source,
        renderer,
        output_path,
        artefacts: Some(resolved.output_root),
        resources,
        publications,
        #[cfg(feature = "parallel-markdown")]
        jobs: resolved.jobs,
        quiet,
    }))
}

#[cfg(feature = "svg")]
pub(crate) fn prepare_render_for_mmdc(
    resolved: ResolvedMmdcRender,
    publications: PublicationGuards,
) -> Result<PreparedRender, CliError> {
    validate_puppeteer_config_file(
        resolved.compatibility.puppeteer_config_file.as_deref(),
        resolved.common.resources.files().puppeteer_config_bytes,
    )?;

    #[cfg(feature = "markdown")]
    if matches!(
        resolved.workflow,
        crate::invocation::ResolvedWorkflow::MarkdownBatch
    ) {
        return prepare_mmdc_markdown(resolved, publications);
    }

    let source = read_mmdc_input(
        &resolved,
        InputLimit::new(
            merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
            resolved
                .common
                .resources
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
        ),
    )?;
    prepare_single(
        source,
        false,
        resolved.output,
        resolved.common,
        publications,
        true,
    )
    .map(PreparedRender::Single)
}

#[cfg(feature = "svg")]
fn validate_puppeteer_config_file(
    path: Option<&Path>,
    max_bytes: Option<usize>,
) -> Result<(), CliError> {
    let Some(path) = path else {
        return Ok(());
    };
    let text = read_named_text_file(
        path,
        "Puppeteer configuration file",
        InputLimit::new(
            crate::resources::CliResourceLimitId::MaxPuppeteerConfigBytes.as_str(),
            max_bytes,
        ),
    )?;
    let _: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
        CliError::InvalidInput(format!(
            "JSON error while parsing Puppeteer configuration file {}: {error}",
            crate::error::safe_path(path)
        ))
    })?;
    Ok(())
}

#[cfg(all(feature = "svg", feature = "markdown"))]
fn prepare_mmdc_markdown(
    resolved: ResolvedMmdcRender,
    publications: PublicationGuards,
) -> Result<PreparedRender, CliError> {
    let source = read_mmdc_input(
        &resolved,
        InputLimit::new(
            crate::resources::CliResourceLimitId::MaxMarkdownDocumentBytes.as_str(),
            resolved.common.resources.files().markdown_document_bytes,
        ),
    )?;
    let output_path = resolved
        .output
        .destination()
        .file()
        .expect("mmdc Markdown normalization rejects stdout")
        .to_path_buf();
    let common = resolved.common;
    let resources = common.resources;
    let (_, renderer) = prepare_graphical_output(resolved.output, common, false, true)?;
    let quiet = renderer_quiet(&renderer);

    if let Some(path) = resolved.compatibility.artefacts.as_deref() {
        publications.prepare_directory(path)?;
    }
    Ok(PreparedRender::Markdown(PreparedMarkdownRender {
        source,
        renderer,
        output_path,
        artefacts: resolved.compatibility.artefacts,
        resources,
        publications,
        #[cfg(feature = "parallel-markdown")]
        jobs: resolved.jobs,
        quiet,
    }))
}

fn prepare_single(
    source: String,
    raw_svg: bool,
    output: ResolvedOutput,
    common: ResolvedRenderCommon,
    publications: PublicationGuards,
    mmdc_compat: bool,
) -> Result<PreparedSingleRender, CliError> {
    #[cfg(not(feature = "svg"))]
    let _ = (raw_svg, mmdc_compat);
    match output {
        #[cfg(feature = "ascii")]
        ResolvedOutput::Text {
            destination,
            options,
        } => {
            let ascii = options;
            let admission = BackendAdmission::for_text(&common.resources, &ascii)?;
            let renderer = merman::ascii::HeadlessAsciiRenderer::new()
                .with_engine(crate::config::engine_for_resolved(
                    &common.parse,
                    &common.resources,
                )?)
                .with_parse_options(crate::config::parse_options_for_resolved(&common.parse))
                .with_ascii_options(ascii)
                .with_resource_policy(*common.resources.input_policy());
            Ok(PreparedSingleRender {
                source,
                output: PreparedSingleOutput::Text {
                    destination,
                    renderer,
                    admission,
                },
                publications,
            })
        }
        #[cfg(feature = "svg")]
        graphical => {
            let (destination, renderer) =
                prepare_graphical_output(graphical, common, raw_svg, mmdc_compat)?;
            Ok(PreparedSingleRender {
                source,
                output: PreparedSingleOutput::Graphical {
                    destination,
                    renderer,
                },
                publications,
            })
        }
    }
}

#[cfg(feature = "svg")]
fn prepare_graphical_output(
    output: ResolvedOutput,
    common: ResolvedRenderCommon,
    raw_svg: bool,
    mmdc_compat: bool,
) -> Result<(ResolvedDestination, PreparedGraphicalRender), CliError> {
    #[cfg(not(feature = "pdf"))]
    let _ = mmdc_compat;
    let css = read_optional_text_file(
        common.css_file.as_deref(),
        "CSS file",
        InputLimit::new(
            crate::resources::CliResourceLimitId::MaxCssBytes.as_str(),
            common.resources.files().css_bytes,
        ),
    )?;
    let (destination, output, pipeline_kind) = match output {
        ResolvedOutput::Svg {
            destination,
            pipeline,
        } => (
            destination,
            PreparedGraphicalOutput::Svg,
            pipeline.unwrap_or(SvgPipelineKind::Parity),
        ),
        #[cfg(feature = "ascii")]
        ResolvedOutput::Text { .. } => {
            return Err(CliError::InvalidInput(
                "text output cannot enter the SVG renderer".to_string(),
            ));
        }
        #[cfg(feature = "png")]
        ResolvedOutput::Png {
            destination,
            raster,
            embedded_images,
        } => (
            destination,
            PreparedGraphicalOutput::Png {
                options: raster_options(
                    raster,
                    embedded_images,
                    common.background.clone(),
                    &common.resources,
                ),
            },
            SvgPipelineKind::ResvgSafe,
        ),
        #[cfg(feature = "jpeg")]
        ResolvedOutput::Jpeg {
            destination,
            raster,
            embedded_images,
        } => (
            destination,
            PreparedGraphicalOutput::Jpeg {
                options: raster_options(
                    raster,
                    embedded_images,
                    common.background.clone(),
                    &common.resources,
                ),
            },
            SvgPipelineKind::ResvgSafe,
        ),
        #[cfg(feature = "pdf")]
        ResolvedOutput::Pdf {
            destination,
            options,
            embedded_images,
            mmdc_fit_width_px,
        } => (
            destination,
            PreparedGraphicalOutput::Pdf {
                options: pdf_options(
                    options,
                    embedded_images,
                    common.background.clone(),
                    &common,
                    mmdc_compat,
                    mmdc_fit_width_px,
                ),
            },
            SvgPipelineKind::ResvgSafe,
        ),
    };
    let pipeline =
        svg_output_policy(pipeline_kind, common.background.as_deref(), css.as_deref()).pipeline();
    let admission = match &output {
        PreparedGraphicalOutput::Svg => BackendAdmission::for_svg(&common.resources)?,
        #[cfg(feature = "png")]
        PreparedGraphicalOutput::Png { options } => {
            BackendAdmission::for_raster(&common.resources, options, 8, raw_svg)?
        }
        #[cfg(feature = "jpeg")]
        PreparedGraphicalOutput::Jpeg { options } => {
            BackendAdmission::for_raster(&common.resources, options, 10, raw_svg)?
        }
        #[cfg(feature = "pdf")]
        PreparedGraphicalOutput::Pdf { options } => {
            BackendAdmission::for_pdf(&common.resources, options, raw_svg)?
        }
    };

    let source = if raw_svg {
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        {
            PreparedGraphicalSource::RawSvg(
                RenderEnvironment::deterministic()
                    .with_resource_policy(common.resources.render_policy()),
            )
        }
        #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
        {
            return Err(CliError::InvalidInput(
                "raw SVG input requires an encoded output feature".to_string(),
            ));
        }
    } else {
        let renderer = crate::config::renderer_for_resolved(
            &common.parse,
            &common.render,
            None,
            &common.resources,
        )?;
        #[cfg(feature = "icons")]
        let renderer = if let Some(icon_registry) =
            load_icon_registry(&common.icons, &common.resources, &common.cwd)?
        {
            let environment = renderer
                .environment()
                .clone()
                .with_icon_registry(icon_registry);
            renderer.with_environment(environment)
        } else {
            renderer
        };
        PreparedGraphicalSource::Mermaid(renderer)
    };

    Ok((
        destination,
        PreparedGraphicalRender {
            source,
            pipeline,
            output,
            admission,
            quiet: common.quiet,
        },
    ))
}

fn read_resolved_input(
    input: &crate::invocation::ResolvedInput,
    suppress_implicit_warning: bool,
    limit: InputLimit,
) -> Result<String, CliError> {
    let path = match input {
        crate::invocation::ResolvedInput::File(path) => Some(path.as_path()),
        crate::invocation::ResolvedInput::Stdin if suppress_implicit_warning => {
            Some(Path::new("-"))
        }
        crate::invocation::ResolvedInput::Stdin => None,
    };
    read_input(path, suppress_implicit_warning, limit)
}

#[cfg(feature = "svg")]
fn read_mmdc_input(resolved: &ResolvedMmdcRender, limit: InputLimit) -> Result<String, CliError> {
    let path = if resolved.input_was_explicit {
        match &resolved.input {
            crate::invocation::ResolvedInput::File(path) => Some(path.as_path()),
            crate::invocation::ResolvedInput::Stdin => Some(Path::new("-")),
        }
    } else {
        None
    };
    read_input(path, !resolved.warn_on_implicit_stdin, limit)
}

fn render_input_limit(raw_svg: bool, resources: &ResolvedResourcePolicy) -> InputLimit {
    #[cfg(feature = "svg")]
    if raw_svg {
        return InputLimit::new(
            merman::svg::ResourceLimitId::MaxSvgBytes.as_str(),
            resources
                .render_policy()
                .value(merman::svg::ResourceLimitId::MaxSvgBytes),
        );
    }
    let _ = raw_svg;
    InputLimit::new(
        merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
        resources
            .input_policy()
            .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn embedded_image_limit(
    options: crate::invocation::ResolvedEmbeddedImageOptions,
) -> merman::svg::export::EmbeddedImageLimit {
    if options.unbounded {
        return merman::svg::export::EmbeddedImageLimit::unbounded();
    }
    let default = merman::svg::export::EmbeddedImageLimit::default();
    merman::svg::export::EmbeddedImageLimit::new(
        options.max_bytes_per_image.or(default.max_bytes_per_image),
        options.max_total_bytes.or(default.max_total_bytes),
        options
            .max_pixels_per_image
            .or(default.max_pixels_per_image),
        options.max_total_pixels.or(default.max_total_pixels),
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn conversion_limits(
    resources: &ResolvedResourcePolicy,
) -> merman::svg::export::SvgConversionLimits {
    if matches!(
        resources.profile(),
        merman::resources::ResourceProfile::UnboundedForTrustedInput
    ) {
        merman::svg::export::SvgConversionLimits::unbounded()
    } else {
        merman::svg::export::SvgConversionLimits::default()
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_options(
    raster: crate::invocation::ResolvedRasterOptions,
    embedded_images: crate::invocation::ResolvedEmbeddedImageOptions,
    background: Option<String>,
    resources: &ResolvedResourcePolicy,
) -> merman::svg::export::RasterOptions {
    let mut options = merman::svg::export::RasterOptions {
        scale: raster.scale,
        background,
        ..Default::default()
    };
    if raster.fit_width.is_some() || raster.fit_height.is_some() {
        options.fit_to = Some(merman::svg::export::RasterFitBox::new(
            raster.fit_width,
            raster.fit_height,
        ));
    }
    if raster.unbounded {
        options.size_limit = merman::svg::export::RasterSizeLimit::unbounded();
    } else if raster.max_width.is_some()
        || raster.max_height.is_some()
        || raster.max_pixels.is_some()
    {
        let default = merman::svg::export::RasterSizeLimit::default();
        options.size_limit = merman::svg::export::RasterSizeLimit::new(
            raster.max_width.or(default.max_width),
            raster.max_height.or(default.max_height),
            raster.max_pixels.or(default.max_pixels),
        );
    }
    options.embedded_image_limit = embedded_image_limit(embedded_images);
    options.conversion_limits = conversion_limits(resources);
    options
}

#[cfg(feature = "pdf")]
fn pdf_options(
    pdf: crate::invocation::ResolvedPdfOptions,
    embedded_images: crate::invocation::ResolvedEmbeddedImageOptions,
    background: Option<String>,
    common: &ResolvedRenderCommon,
    mmdc_compat: bool,
    mmdc_fit_width_px: Option<f32>,
) -> merman::svg::export::PdfOptions {
    let page_policy = if !mmdc_compat {
        merman::svg::export::PdfPagePolicy::FitSvg
    } else if let Some(max_width_px) = mmdc_fit_width_px {
        merman::svg::export::PdfPagePolicy::FitCssWidth { max_width_px }
    } else {
        merman::svg::export::PdfPagePolicy::Fixed {
            width_pt: MMDC_DEFAULT_PDF_WIDTH_PT,
            height_pt: MMDC_DEFAULT_PDF_HEIGHT_PT,
        }
    };
    let mut options = merman::svg::export::PdfOptions::default().with_page_policy(page_policy);
    if let Some(filter_scale) = pdf.filter_scale {
        options.filter_scale = filter_scale;
    }
    if pdf.filter_images_unbounded {
        options.filter_image_limit = merman::svg::export::PdfFilterImageLimit::unbounded();
    } else if let Some(max_pixels) = pdf.max_filter_image_pixels {
        options.filter_image_limit =
            merman::svg::export::PdfFilterImageLimit::new(Some(max_pixels));
    }
    options.background = background;
    options.embedded_image_limit = embedded_image_limit(embedded_images);
    options.conversion_limits = conversion_limits(&common.resources);
    options
}

#[cfg(feature = "markdown")]
fn renderer_quiet(renderer: &PreparedGraphicalRender) -> bool {
    renderer.quiet
}
