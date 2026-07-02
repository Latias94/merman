use super::icons::{NetworkPolicy, load_icon_registry};
use super::raster::RasterCliOptions;
use crate::cli::{
    ExportArgs, ParseCliArgs, RenderArgs, RenderCliArgs, RenderFormat, SvgPipelineKind,
    TextCharset, TextColorMode, TextDirection, TextOutputCliArgs,
};
use crate::error::CliError;
use crate::io::{OutputTarget, read_named_text_file, read_optional_text_file};
use crate::markdown;
use merman::render::IconRegistry;
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
        text: export.text.clone(),
        mode: RenderMode::MmdcCompat,
    })
}

pub(crate) fn render_plan_for_subcommand(args: RenderArgs) -> Result<RenderPlan, CliError> {
    let input = merge_input(args.export.input_file.clone(), args.input)?;
    let format = infer_output_format(args.export.output.as_deref(), args.export.output_format)
        .unwrap_or(RenderFormat::Svg);
    let output = subcommand_output_target(args.export.output.clone(), input.as_deref(), format);
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
        background: args.export.background_color.clone(),
        css: read_optional_text_file(args.export.css_file.as_deref(), "CSS file")?,
        svg_pipeline: args.export.svg_pipeline,
        icon_registry,
        artefacts: None,
        jobs: 1,
        pdf_fit: true,
        quiet: args.export.quiet,
        text: args.export.text.clone(),
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
                TextColorMode::Auto => merman::ascii::AsciiColorMode::Auto,
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

fn apply_official_defaults(parse: &mut ParseCliArgs, render: &mut RenderCliArgs) {
    if parse.theme.is_none() {
        parse.theme = Some("default".to_string());
    }
    if render.width.is_none() {
        render.width = Some(800.0);
    }
    if render.height.is_none() {
        render.height = Some(600.0);
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
