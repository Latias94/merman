#[cfg(feature = "icons")]
use super::icons::load_icon_registry;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::cli::RenderInputKind;
use crate::cli::{ParseCliArgs, RenderCliArgs, RenderFormat, SvgPipelineKind};
#[cfg(feature = "ascii")]
use crate::cli::{TextCharset, TextColorMode, TextDirection, TextOutputCliArgs};
use crate::error::CliError;
#[cfg(feature = "markdown")]
use crate::invocation::ResolvedBatchRender;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::invocation::ResolvedEmbeddedImageOptions;
#[cfg(feature = "pdf")]
use crate::invocation::ResolvedPdfOptions;
#[cfg(any(feature = "png", feature = "jpeg"))]
use crate::invocation::ResolvedRasterOptions;
#[cfg(feature = "markdown")]
use crate::invocation::ResolvedWorkflow;
use crate::invocation::{
    ResolvedDestination, ResolvedMmdcRender, ResolvedOutput, ResolvedSingleRender,
};
use crate::io::{OutputTarget, read_named_text_file, read_optional_text_file};
#[cfg(feature = "markdown")]
use crate::markdown;
#[cfg(feature = "icons")]
use merman::svg::IconRegistry;
#[cfg(feature = "ascii")]
use std::env;
#[cfg(feature = "ascii")]
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
#[cfg(feature = "icons")]
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub(super) enum RenderMode {
    MmdcCompat,
    NativeSingle,
    #[cfg(feature = "markdown")]
    NativeBatch,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderPlan {
    pub(super) input: Option<PathBuf>,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) input_kind: RenderInputKind,
    pub(super) output: Option<OutputTarget>,
    pub(super) format: RenderFormat,
    pub(super) parse: ParseCliArgs,
    pub(super) render: RenderCliArgs,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(super) scale: f32,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(super) raster: ResolvedRasterOptions,
    #[cfg(feature = "pdf")]
    pub(super) pdf: ResolvedPdfOptions,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) embedded_images: ResolvedEmbeddedImageOptions,
    pub(super) background: Option<String>,
    pub(super) css: Option<String>,
    pub(super) svg_pipeline: Option<SvgPipelineKind>,
    #[cfg(feature = "icons")]
    pub(super) icon_registry: Option<Arc<IconRegistry>>,
    #[cfg(feature = "markdown")]
    pub(super) artefacts: Option<PathBuf>,
    #[cfg(feature = "parallel-markdown")]
    pub(super) jobs: usize,
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    pub(super) encoding_parallel_budget_bytes: Option<u64>,
    #[cfg(feature = "pdf")]
    pub(super) pdf_fit: bool,
    pub(super) quiet: bool,
    pub(super) warn_on_implicit_stdin: bool,
    #[cfg(feature = "ascii")]
    pub(super) text: TextOutputCliArgs,
    pub(super) mode: RenderMode,
}

pub(crate) fn render_plan_for_mmdc(resolved: ResolvedMmdcRender) -> Result<RenderPlan, CliError> {
    #[cfg(feature = "markdown")]
    let markdown_batch = matches!(resolved.workflow, ResolvedWorkflow::MarkdownBatch);
    #[cfg(feature = "markdown")]
    let artefacts = if markdown_batch {
        prepare_artefacts_dir(
            resolved.compatibility.artefacts.as_deref(),
            resolved.input.file(),
        )?
    } else {
        None
    };
    validate_puppeteer_config_file(resolved.compatibility.puppeteer_config_file.as_deref())?;
    let input = resolved
        .input_was_explicit
        .then(|| resolved.input.to_path_buf());
    let output = project_output(resolved.output);
    #[cfg(feature = "parallel-markdown")]
    let jobs = resolved.jobs;
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    let encoding_parallel_budget_bytes =
        markdown_batch.then_some(resolved.encoding_parallel_budget_bytes);
    let common = resolved.common;
    #[cfg(feature = "icons")]
    let icon_registry = load_icon_registry(
        &common.icons.packages,
        &common.icons.named_sources,
        #[cfg(feature = "network-icons")]
        common.icons.allow_network,
    )?;
    let parse = common.parse.into_legacy_cli_args();
    let render = common.render.into_legacy_cli_args();
    #[cfg(feature = "ascii")]
    let text = resolve_text_output_args(output.text, output.destination.as_ref());

    Ok(RenderPlan {
        input,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        input_kind: RenderInputKind::Mermaid,
        output: output.destination,
        format: output.format,
        parse,
        render,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        scale: output.scale,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        raster: output.raster,
        #[cfg(feature = "pdf")]
        pdf: output.pdf,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        embedded_images: output.embedded_images,
        background: common.background,
        css: read_optional_text_file(common.css_file.as_deref(), "CSS file")?,
        svg_pipeline: output.svg_pipeline,
        #[cfg(feature = "icons")]
        icon_registry,
        #[cfg(feature = "markdown")]
        artefacts,
        #[cfg(feature = "parallel-markdown")]
        jobs,
        #[cfg(all(
            feature = "parallel-markdown",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        encoding_parallel_budget_bytes,
        #[cfg(feature = "pdf")]
        pdf_fit: output.pdf_fit,
        quiet: common.quiet,
        warn_on_implicit_stdin: resolved.warn_on_implicit_stdin,
        #[cfg(feature = "ascii")]
        text,
        mode: RenderMode::MmdcCompat,
    })
}

pub(crate) fn render_plan_for_native(
    resolved: ResolvedSingleRender,
) -> Result<RenderPlan, CliError> {
    let input = Some(resolved.input.to_path_buf());
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    let input_kind = resolved.input_kind;
    let output = project_output(resolved.output);
    let common = resolved.common;
    #[cfg(feature = "icons")]
    let icon_registry = load_icon_registry(
        &common.icons.packages,
        &common.icons.named_sources,
        #[cfg(feature = "network-icons")]
        common.icons.allow_network,
    )?;
    let parse = common.parse.into_legacy_cli_args();
    let render = common.render.into_legacy_cli_args();
    #[cfg(feature = "ascii")]
    let text = resolve_text_output_args(output.text, output.destination.as_ref());

    Ok(RenderPlan {
        input,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        input_kind,
        output: output.destination,
        format: output.format,
        parse,
        render,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        scale: output.scale,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        raster: output.raster,
        #[cfg(feature = "pdf")]
        pdf: output.pdf,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        embedded_images: output.embedded_images,
        background: common.background,
        css: read_optional_text_file(common.css_file.as_deref(), "CSS file")?,
        svg_pipeline: output.svg_pipeline,
        #[cfg(feature = "icons")]
        icon_registry,
        #[cfg(feature = "markdown")]
        artefacts: None,
        #[cfg(feature = "parallel-markdown")]
        jobs: 1,
        #[cfg(all(
            feature = "parallel-markdown",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        encoding_parallel_budget_bytes: None,
        #[cfg(feature = "pdf")]
        pdf_fit: output.pdf_fit,
        quiet: common.quiet,
        warn_on_implicit_stdin: false,
        #[cfg(feature = "ascii")]
        text,
        mode: RenderMode::NativeSingle,
    })
}

#[cfg(feature = "markdown")]
pub(crate) fn render_plan_for_batch(resolved: ResolvedBatchRender) -> Result<RenderPlan, CliError> {
    let input = Some(resolved.input.to_path_buf());
    let output_root = resolved.output_root;
    std::fs::create_dir_all(&output_root)?;
    let output = project_output(resolved.output);
    let common = resolved.common;
    #[cfg(feature = "icons")]
    let icon_registry = load_icon_registry(
        &common.icons.packages,
        &common.icons.named_sources,
        #[cfg(feature = "network-icons")]
        common.icons.allow_network,
    )?;
    let parse = common.parse.into_legacy_cli_args();
    let render = common.render.into_legacy_cli_args();
    #[cfg(feature = "ascii")]
    let text = resolve_text_output_args(output.text, output.destination.as_ref());

    Ok(RenderPlan {
        input,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        input_kind: RenderInputKind::Mermaid,
        output: output.destination,
        format: output.format,
        parse,
        render,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        scale: output.scale,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        raster: output.raster,
        #[cfg(feature = "pdf")]
        pdf: output.pdf,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        embedded_images: output.embedded_images,
        background: common.background,
        css: read_optional_text_file(common.css_file.as_deref(), "CSS file")?,
        svg_pipeline: output.svg_pipeline,
        #[cfg(feature = "icons")]
        icon_registry,
        artefacts: Some(output_root),
        #[cfg(feature = "parallel-markdown")]
        jobs: resolved.jobs,
        #[cfg(all(
            feature = "parallel-markdown",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        encoding_parallel_budget_bytes: Some(resolved.encoding_parallel_budget_bytes),
        #[cfg(feature = "pdf")]
        pdf_fit: output.pdf_fit,
        quiet: common.quiet,
        warn_on_implicit_stdin: false,
        #[cfg(feature = "ascii")]
        text,
        mode: RenderMode::NativeBatch,
    })
}

fn output_target(destination: ResolvedDestination) -> Option<OutputTarget> {
    match destination {
        ResolvedDestination::Stdout => Some(OutputTarget::Stdout),
        ResolvedDestination::File(path) => Some(OutputTarget::File(path)),
    }
}

struct ProjectedOutput {
    destination: Option<OutputTarget>,
    format: RenderFormat,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    scale: f32,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    raster: ResolvedRasterOptions,
    #[cfg(feature = "pdf")]
    pdf: ResolvedPdfOptions,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    embedded_images: ResolvedEmbeddedImageOptions,
    svg_pipeline: Option<SvgPipelineKind>,
    #[cfg(feature = "pdf")]
    pdf_fit: bool,
    #[cfg(feature = "ascii")]
    text: TextOutputCliArgs,
}

fn project_output(output: ResolvedOutput) -> ProjectedOutput {
    match output {
        ResolvedOutput::Svg {
            destination,
            pipeline,
        } => ProjectedOutput {
            destination: output_target(destination),
            format: RenderFormat::Svg,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            scale: 1.0,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            raster: ResolvedRasterOptions::default(),
            #[cfg(feature = "pdf")]
            pdf: ResolvedPdfOptions::default(),
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            embedded_images: ResolvedEmbeddedImageOptions::default(),
            svg_pipeline: pipeline,
            #[cfg(feature = "pdf")]
            pdf_fit: true,
            #[cfg(feature = "ascii")]
            text: TextOutputCliArgs::default(),
        },
        #[cfg(feature = "ascii")]
        ResolvedOutput::Text {
            format,
            destination,
            options,
        } => ProjectedOutput {
            destination: output_target(destination),
            format,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            scale: 1.0,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            raster: ResolvedRasterOptions::default(),
            #[cfg(feature = "pdf")]
            pdf: ResolvedPdfOptions::default(),
            #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
            embedded_images: ResolvedEmbeddedImageOptions::default(),
            svg_pipeline: None,
            #[cfg(feature = "pdf")]
            pdf_fit: true,
            text: options.into_legacy_cli_args(),
        },
        #[cfg(feature = "png")]
        ResolvedOutput::Png {
            destination,
            raster,
            embedded_images,
        } => ProjectedOutput {
            destination: output_target(destination),
            format: RenderFormat::Png,
            scale: raster.scale,
            raster,
            #[cfg(feature = "pdf")]
            pdf: ResolvedPdfOptions::default(),
            embedded_images,
            svg_pipeline: None,
            #[cfg(feature = "pdf")]
            pdf_fit: true,
            #[cfg(feature = "ascii")]
            text: TextOutputCliArgs::default(),
        },
        #[cfg(feature = "jpeg")]
        ResolvedOutput::Jpeg {
            destination,
            raster,
            embedded_images,
        } => ProjectedOutput {
            destination: output_target(destination),
            format: RenderFormat::Jpeg,
            scale: raster.scale,
            raster,
            #[cfg(feature = "pdf")]
            pdf: ResolvedPdfOptions::default(),
            embedded_images,
            svg_pipeline: None,
            #[cfg(feature = "pdf")]
            pdf_fit: true,
            #[cfg(feature = "ascii")]
            text: TextOutputCliArgs::default(),
        },
        #[cfg(feature = "pdf")]
        ResolvedOutput::Pdf {
            destination,
            options,
            embedded_images,
            fit,
        } => ProjectedOutput {
            destination: output_target(destination),
            format: RenderFormat::Pdf,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            scale: 1.0,
            #[cfg(any(feature = "png", feature = "jpeg"))]
            raster: ResolvedRasterOptions::default(),
            pdf: options,
            embedded_images,
            svg_pipeline: None,
            pdf_fit: fit,
            #[cfg(feature = "ascii")]
            text: TextOutputCliArgs::default(),
        },
    }
}

impl RenderPlan {
    #[cfg(feature = "icons")]
    pub(super) fn icon_registry(&self) -> Option<Arc<IconRegistry>> {
        self.icon_registry.clone()
    }

    #[cfg(not(feature = "icons"))]
    pub(super) fn icon_registry(&self) -> Option<std::sync::Arc<merman::svg::IconRegistry>> {
        None
    }

    #[cfg(feature = "markdown")]
    #[cfg(feature = "parallel-markdown")]
    pub(super) const fn markdown_jobs(&self) -> usize {
        self.jobs
    }

    #[cfg(feature = "markdown")]
    #[cfg(not(feature = "parallel-markdown"))]
    pub(super) const fn markdown_jobs(&self) -> usize {
        1
    }

    #[cfg(feature = "ascii")]
    pub(super) fn apply_text_options(
        &self,
        mut options: merman::ascii::AsciiRenderOptions,
    ) -> Result<merman::ascii::AsciiRenderOptions, CliError> {
        if let Some(charset) = self.text.ascii_charset {
            options.charset = match charset {
                TextCharset::Ascii => merman::ascii::AsciiCharset::Ascii,
                TextCharset::Unicode => merman::ascii::AsciiCharset::Unicode,
            };
        }
        if let Some(direction) = self.text.ascii_direction {
            options.default_direction = match direction {
                TextDirection::LeftRight => merman::ascii::AsciiDirection::LeftRight,
                TextDirection::TopDown => merman::ascii::AsciiDirection::TopDown,
            };
        }
        if let Some(color_mode) = self.text.ascii_color {
            options.color_mode = match color_mode {
                TextColorMode::Plain => merman::ascii::AsciiColorMode::Plain,
                TextColorMode::Auto => {
                    return Err(CliError::InvalidInput(
                        "ASCII color mode `auto` was not resolved when the render plan was created"
                            .to_string(),
                    ));
                }
                TextColorMode::Ansi16 => merman::ascii::AsciiColorMode::Ansi16,
                TextColorMode::Ansi256 => merman::ascii::AsciiColorMode::Ansi256,
                TextColorMode::Truecolor => merman::ascii::AsciiColorMode::TrueColor,
                TextColorMode::Html => merman::ascii::AsciiColorMode::Html,
            };
        }
        if self.text.sequence_mirror_actors {
            options.sequence_mirror_actors = true;
        }
        if let Some(height) = self.text.xychart_vertical_plot_height {
            options.xychart_vertical_plot_height = height;
        }
        if let Some(width) = self.text.xychart_category_band_width {
            options.xychart_category_band_width = width;
        }
        if let Some(width) = self.text.xychart_horizontal_plot_width {
            options.xychart_horizontal_plot_width = width;
        }
        if let Some(max_grid_cells) = self.text.ascii_max_grid_cells {
            options.max_grid_cells = max_grid_cells;
        }

        options
            .validate()
            .map_err(|err| CliError::InvalidInput(format!("invalid ASCII options: {err}")))?;
        Ok(options)
    }

    pub(super) fn warn_for_accepted_compat_options(&self) {
        if self.quiet {
            return;
        }
        if matches!(self.mode, RenderMode::MmdcCompat) {
            // Kept intentionally quiet for no-op options that are only meaningful in a browser.
        }
    }
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Copy)]
struct TerminalColorEnvironment {
    no_color: bool,
    force_color: bool,
    truecolor: bool,
    color_256: bool,
    basic_color: bool,
    stdout_is_terminal: bool,
    output_is_stdout: bool,
}

#[cfg(feature = "ascii")]
impl TerminalColorEnvironment {
    fn capture(output: Option<&OutputTarget>) -> Self {
        let color_term = env::var("COLORTERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();

        Self {
            no_color: env::var_os("NO_COLOR").is_some(),
            force_color: env::var("CLICOLOR_FORCE")
                .is_ok_and(|value| !value.is_empty() && value != "0"),
            truecolor: color_term.contains("truecolor") || color_term.contains("24bit"),
            color_256: term.contains("256color"),
            basic_color: !term.is_empty() && term != "dumb",
            stdout_is_terminal: io::stdout().is_terminal(),
            output_is_stdout: matches!(output, None | Some(OutputTarget::Stdout)),
        }
    }
}

#[cfg(feature = "ascii")]
fn resolve_text_output_args(
    mut text: TextOutputCliArgs,
    output: Option<&OutputTarget>,
) -> TextOutputCliArgs {
    if text.ascii_color == Some(TextColorMode::Auto) {
        text.ascii_color = Some(resolve_auto_text_color(TerminalColorEnvironment::capture(
            output,
        )));
    }
    text
}

#[cfg(feature = "ascii")]
fn resolve_auto_text_color(environment: TerminalColorEnvironment) -> TextColorMode {
    if environment.no_color {
        return TextColorMode::Plain;
    }
    if environment.force_color {
        return if environment.truecolor {
            TextColorMode::Truecolor
        } else {
            TextColorMode::Ansi256
        };
    }
    if !environment.output_is_stdout || !environment.stdout_is_terminal {
        return TextColorMode::Plain;
    }
    if environment.truecolor {
        TextColorMode::Truecolor
    } else if environment.color_256 {
        TextColorMode::Ansi256
    } else if environment.basic_color {
        TextColorMode::Ansi16
    } else {
        TextColorMode::Plain
    }
}

#[cfg(feature = "markdown")]
fn prepare_artefacts_dir(
    artefacts: Option<&Path>,
    input: Option<&Path>,
) -> Result<Option<PathBuf>, CliError> {
    let Some(path) = artefacts else {
        return Ok(None);
    };

    let is_markdown_input = input.map(markdown::is_markdown_path).unwrap_or(false);
    if !is_markdown_input {
        return Err(CliError::InvalidInput(
            "Artefacts [-a|--artefacts] path can only be used with Markdown input file".to_string(),
        ));
    }

    std::fs::create_dir_all(&path)?;
    Ok(Some(path.to_path_buf()))
}

fn validate_puppeteer_config_file(path: Option<&Path>) -> Result<(), CliError> {
    let Some(path) = path else {
        return Ok(());
    };

    let text = read_named_text_file(path, "Puppeteer configuration file")?;
    let _: serde_json::Value = serde_json::from_str(&text)?;
    Ok(())
}

#[cfg(all(test, feature = "ascii"))]
mod tests {
    use super::*;

    fn terminal_environment() -> TerminalColorEnvironment {
        TerminalColorEnvironment {
            no_color: false,
            force_color: false,
            truecolor: false,
            color_256: false,
            basic_color: false,
            stdout_is_terminal: true,
            output_is_stdout: true,
        }
    }

    #[test]
    fn no_color_takes_precedence_over_forced_truecolor() {
        let environment = TerminalColorEnvironment {
            no_color: true,
            force_color: true,
            truecolor: true,
            ..terminal_environment()
        };
        assert_eq!(resolve_auto_text_color(environment), TextColorMode::Plain);
    }

    #[test]
    fn forced_color_is_preserved_for_file_output() {
        let environment = TerminalColorEnvironment {
            force_color: true,
            output_is_stdout: false,
            stdout_is_terminal: false,
            ..terminal_environment()
        };
        assert_eq!(resolve_auto_text_color(environment), TextColorMode::Ansi256);
    }
}
