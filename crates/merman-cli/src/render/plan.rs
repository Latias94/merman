use super::icons::{NetworkPolicy, load_icon_registry};
use super::raster::{EmbeddedImageCliOptions, PdfCliOptions, RasterCliOptions};
use crate::cli::{
    ExportArgs, ParseCliArgs, RenderArgs, RenderCliArgs, RenderFormat, SvgPipelineKind,
    TextOutputCliArgs,
};
#[cfg(feature = "ascii")]
use crate::cli::{TextCharset, TextColorMode, TextDirection};
use crate::error::CliError;
use crate::io::{OutputTarget, read_named_text_file, read_optional_text_file};
use crate::markdown;
use merman::render::IconRegistry;
#[cfg(feature = "ascii")]
use std::env;
#[cfg(feature = "ascii")]
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub(super) enum RenderMode {
    MmdcCompat,
    Subcommand,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderPlan {
    pub(super) input: Option<String>,
    pub(super) output: Option<OutputTarget>,
    pub(super) format: RenderFormat,
    pub(super) parse: ParseCliArgs,
    pub(super) render: RenderCliArgs,
    pub(super) scale: f32,
    pub(super) raster: RasterCliOptions,
    pub(super) pdf: PdfCliOptions,
    pub(super) embedded_images: EmbeddedImageCliOptions,
    pub(super) background: Option<String>,
    pub(super) css: Option<String>,
    pub(super) svg_pipeline: Option<SvgPipelineKind>,
    pub(super) icon_registry: Option<Arc<IconRegistry>>,
    pub(super) artefacts: Option<PathBuf>,
    pub(super) jobs: usize,
    pub(super) pdf_fit: bool,
    pub(super) quiet: bool,
    pub(super) text: TextOutputCliArgs,
    pub(super) mode: RenderMode,
}

pub(crate) fn render_plan_for_mmdc(
    positional_input: Option<String>,
    export: ExportArgs,
) -> Result<RenderPlan, CliError> {
    let input = merge_input(export.input_file.clone(), positional_input)?;
    let artefacts = prepare_artefacts_dir(export.artefacts.as_deref(), input.as_deref())?;
    validate_mmdc_output_path(export.output.as_deref())?;
    let icon_registry = load_icon_registry(
        &export.icons.icon_packs,
        &export.icons.icon_packs_names_and_urls,
        NetworkPolicy::from_allow_network(export.icons.allow_network),
    )?;
    let format = infer_output_format(export.output.as_deref(), export.output_format)
        .unwrap_or(RenderFormat::Svg);
    let output = Some(OutputTarget::from_cli(
        export
            .output
            .clone()
            .unwrap_or_else(|| default_mmdc_output_path(input.as_deref(), format)),
    ));
    let text = resolve_text_output_args(export.text.clone(), output.as_ref());

    let mut parse = export.parse.clone();
    let mut render = export.render.clone();

    apply_official_defaults(&mut parse, &mut render);
    validate_puppeteer_config_file(export.puppeteer_config_file.as_deref())?;

    Ok(RenderPlan {
        input,
        output,
        format,
        parse,
        render,
        scale: export.raster.scale.unwrap_or(1.0),
        raster: RasterCliOptions::from_args(&export.raster)?,
        pdf: PdfCliOptions::from_args(&export.pdf),
        embedded_images: EmbeddedImageCliOptions::from_args(&export.embedded_images),
        background: Some(
            export
                .background_color
                .clone()
                .unwrap_or_else(|| "white".to_string()),
        ),
        css: read_optional_text_file(export.css_file.as_deref(), "CSS file")?,
        svg_pipeline: export.svg_pipeline,
        icon_registry,
        artefacts,
        jobs: export.jobs.unwrap_or_else(default_jobs),
        pdf_fit: export.pdf_fit,
        quiet: export.quiet,
        text,
        mode: RenderMode::MmdcCompat,
    })
}

pub(crate) fn render_plan_for_subcommand(args: RenderArgs) -> Result<RenderPlan, CliError> {
    let input = merge_input(args.export.input_file.clone(), args.input)?;
    let format = infer_output_format(args.export.output.as_deref(), args.export.output_format)
        .unwrap_or(RenderFormat::Svg);
    let output = subcommand_output_target(args.export.output.clone(), input.as_deref(), format);
    let text = resolve_text_output_args(args.export.text.clone(), output.as_ref());
    let icon_registry = load_icon_registry(
        &args.export.icons.icon_packs,
        &args.export.icons.icon_packs_names_and_urls,
        NetworkPolicy::from_allow_network(args.export.icons.allow_network),
    )?;

    Ok(RenderPlan {
        input,
        output,
        format,
        parse: args.export.parse.clone(),
        render: args.export.render.clone(),
        scale: args.export.raster.scale.unwrap_or(1.0),
        raster: RasterCliOptions::from_args(&args.export.raster)?,
        pdf: PdfCliOptions::from_args(&args.export.pdf),
        embedded_images: EmbeddedImageCliOptions::from_args(&args.export.embedded_images),
        background: args.export.background_color.clone(),
        css: read_optional_text_file(args.export.css_file.as_deref(), "CSS file")?,
        svg_pipeline: args.export.svg_pipeline,
        icon_registry,
        artefacts: None,
        jobs: 1,
        pdf_fit: true,
        quiet: args.export.quiet,
        text,
        mode: RenderMode::Subcommand,
    })
}

impl RenderPlan {
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
            output_is_stdout: match output {
                None | Some(OutputTarget::Stdout) => true,
                Some(OutputTarget::File(_)) => false,
            },
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

#[cfg(not(feature = "ascii"))]
fn resolve_text_output_args(
    text: TextOutputCliArgs,
    _output: Option<&OutputTarget>,
) -> TextOutputCliArgs {
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
        return TextColorMode::Truecolor;
    }
    if environment.color_256 {
        return TextColorMode::Ansi256;
    }
    if environment.basic_color {
        return TextColorMode::Ansi16;
    }
    TextColorMode::Plain
}

fn apply_official_defaults(parse: &mut ParseCliArgs, render: &mut RenderCliArgs) {
    if parse.theme.is_none() {
        parse.theme = Some("default".to_string());
    }
    if render.container_width.is_none() {
        render.container_width = Some(800.0);
    }
    if render.container_height.is_none() {
        render.container_height = Some(600.0);
    }
}

fn prepare_artefacts_dir(
    artefacts: Option<&str>,
    input: Option<&str>,
) -> Result<Option<PathBuf>, CliError> {
    let Some(raw_path) = artefacts else {
        return Ok(None);
    };

    let is_markdown_input = input
        .filter(|path| *path != "-")
        .map(|path| markdown::is_markdown_path(Path::new(path)))
        .unwrap_or(false);
    if !is_markdown_input {
        return Err(CliError::InvalidInput(
            "Artefacts [-a|--artefacts] path can only be used with Markdown input file".to_string(),
        ));
    }

    let path = PathBuf::from(raw_path);
    std::fs::create_dir_all(&path)?;
    Ok(Some(path))
}

fn validate_puppeteer_config_file(path: Option<&str>) -> Result<(), CliError> {
    let Some(path) = path else {
        return Ok(());
    };

    let text = read_named_text_file(path, "Puppeteer configuration file")?;
    let _: serde_json::Value = serde_json::from_str(&text)?;
    Ok(())
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|count| (count.get() / 2).max(1))
        .unwrap_or(1)
}

fn merge_input(
    option_input: Option<String>,
    positional_input: Option<String>,
) -> Result<Option<String>, CliError> {
    match (option_input, positional_input) {
        (Some(a), Some(b)) if a != b => Err(CliError::InvalidInput(
            "input was provided both positionally and with --input; choose one".to_string(),
        )),
        (Some(a), _) => Ok(Some(a)),
        (_, Some(b)) => Ok(Some(b)),
        (None, None) => Ok(None),
    }
}

fn infer_output_format(
    output: Option<&str>,
    explicit: Option<RenderFormat>,
) -> Option<RenderFormat> {
    explicit.or_else(|| output.and_then(format_from_output_path))
}

fn validate_mmdc_output_path(output: Option<&str>) -> Result<(), CliError> {
    let Some(output) = output else {
        return Ok(());
    };
    if output == "-" {
        return Ok(());
    }

    let Some(ext) = Path::new(output)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return Err(invalid_mmdc_output_extension());
    };

    if matches!(
        ext.as_str(),
        "md" | "markdown" | "svg" | "png" | "pdf" | "jpg" | "jpeg" | "txt" | "ascii"
    ) {
        Ok(())
    } else {
        Err(invalid_mmdc_output_extension())
    }
}

fn invalid_mmdc_output_extension() -> CliError {
    CliError::InvalidOutput(
        "Output file must end with \".md\"/\".markdown\", \".svg\", \".png\", \".pdf\", \
         \".jpg\"/\".jpeg\", \".txt\" or \".ascii\""
            .to_string(),
    )
}

fn format_from_output_path(path: &str) -> Option<RenderFormat> {
    if path == "-" {
        return Some(RenderFormat::Svg);
    }
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "svg" => Some(RenderFormat::Svg),
        "png" => Some(RenderFormat::Png),
        "jpg" | "jpeg" => Some(RenderFormat::Jpeg),
        "pdf" => Some(RenderFormat::Pdf),
        "txt" | "ascii" => Some(RenderFormat::Ascii),
        _ => None,
    }
}

fn default_mmdc_output_path(input: Option<&str>, format: RenderFormat) -> String {
    let ext = format.extension();
    match input.filter(|p| *p != "-") {
        Some(path) => format!("{path}.{ext}"),
        None => format!("out.{ext}"),
    }
}

fn subcommand_output_target(
    output: Option<String>,
    input: Option<&str>,
    format: RenderFormat,
) -> Option<OutputTarget> {
    if let Some(output) = output {
        return Some(OutputTarget::from_cli(output));
    }

    if format == RenderFormat::Svg || format.is_text() {
        return None;
    }

    Some(OutputTarget::File(default_raster_out_path(
        input,
        format.extension(),
    )))
}

fn default_raster_out_path(input: Option<&str>, ext: &str) -> PathBuf {
    match input.filter(|p| *p != "-") {
        Some(path) => PathBuf::from(path).with_extension(ext),
        None => PathBuf::from(format!("out.{ext}")),
    }
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

    #[test]
    fn automatic_color_requires_terminal_stdout() {
        let redirected = TerminalColorEnvironment {
            truecolor: true,
            stdout_is_terminal: false,
            ..terminal_environment()
        };
        let file = TerminalColorEnvironment {
            truecolor: true,
            output_is_stdout: false,
            ..terminal_environment()
        };

        assert_eq!(resolve_auto_text_color(redirected), TextColorMode::Plain);
        assert_eq!(resolve_auto_text_color(file), TextColorMode::Plain);
    }

    #[test]
    fn automatic_color_chooses_the_most_capable_terminal_mode() {
        let truecolor = TerminalColorEnvironment {
            truecolor: true,
            color_256: true,
            basic_color: true,
            ..terminal_environment()
        };
        let color_256 = TerminalColorEnvironment {
            color_256: true,
            basic_color: true,
            ..terminal_environment()
        };
        let basic = TerminalColorEnvironment {
            basic_color: true,
            ..terminal_environment()
        };

        assert_eq!(resolve_auto_text_color(truecolor), TextColorMode::Truecolor);
        assert_eq!(resolve_auto_text_color(color_256), TextColorMode::Ansi256);
        assert_eq!(resolve_auto_text_color(basic), TextColorMode::Ansi16);
    }
}
