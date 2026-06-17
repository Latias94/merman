use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use merman::render::FlowchartElkBackend as RenderFlowchartElkBackend;

#[derive(Debug, Parser)]
#[command(
    name = "merman-cli",
    version,
    about = "Headless Mermaid renderer compatible with common mmdc workflows.",
    long_about = "Headless Mermaid renderer compatible with common mmdc workflows.\n\n\
Top-level usage renders like mmdc:\n  merman-cli -i input.mmd -o output.svg\n  merman-cli -i input.mmd -o output.png -t dark -b transparent\n\n\
Developer subcommands expose merman internals:\n  merman-cli parse --pretty input.mmd\n  merman-cli layout --pretty input.mmd\n  merman-cli render --format unicode input.mmd"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,

    #[command(flatten)]
    pub(crate) export: ExportArgs,

    /// Input Mermaid file for top-level render mode. Use `-` for stdin.
    #[arg(value_name = "INPUT")]
    pub(crate) input: Option<String>,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum Command {
    /// Detect the Mermaid diagram type.
    Detect(DetectArgs),
    /// Parse Mermaid source and print the semantic JSON model.
    Parse(ParseArgs),
    /// Parse and layout Mermaid source, then print layout JSON.
    Layout(LayoutArgs),
    /// Render Mermaid source to SVG/PNG/JPG/PDF/ASCII/Unicode.
    Render(RenderArgs),
}

#[derive(Debug, ClapArgs)]
pub(crate) struct DetectArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT")]
    pub(crate) input: Option<String>,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct ParseArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT")]
    pub(crate) input: Option<String>,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub(crate) pretty: bool,

    /// Include parse metadata alongside the model.
    #[arg(long, alias = "with-meta")]
    pub(crate) meta: bool,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct LayoutArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT")]
    pub(crate) input: Option<String>,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub(crate) pretty: bool,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,

    #[command(flatten)]
    pub(crate) render: RenderCliArgs,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct RenderArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT")]
    pub(crate) input: Option<String>,

    #[command(flatten)]
    pub(crate) export: ExportArgs,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct ParseCliArgs {
    /// Emit an error diagram instead of failing on parse errors.
    #[arg(long = "suppress-errors")]
    pub(crate) suppress_errors: bool,

    /// JSON Mermaid configuration file.
    #[arg(short = 'c', long = "configFile", alias = "config-file")]
    pub(crate) config_file: Option<String>,

    /// Mermaid theme override.
    #[arg(short = 't', long)]
    pub(crate) theme: Option<String>,

    /// Override the local "today" date for time-dependent diagrams.
    #[arg(long = "fixed-today", value_parser = parse_naive_date)]
    pub(crate) fixed_today: Option<chrono::NaiveDate>,

    /// Override the local timezone offset in minutes for time-dependent diagrams.
    #[arg(long = "fixed-local-offset-minutes", value_parser = parse_fixed_local_offset_minutes)]
    pub(crate) fixed_local_offset_minutes: Option<i32>,
}

#[derive(Debug, Clone, ClapArgs)]
pub(crate) struct RenderCliArgs {
    /// Text measurement strategy.
    #[arg(long = "text-measurer", value_enum, default_value_t = TextMeasurerKind::Vendored)]
    pub(crate) text_measurer: TextMeasurerKind,

    /// Math renderer for $$...$$ labels.
    #[arg(long = "math-renderer", value_enum, default_value_t = MathRendererKind::None)]
    pub(crate) math_renderer: MathRendererKind,

    /// Flowchart ELK layout backend.
    #[arg(long = "flowchart-elk-backend", value_enum, default_value_t = FlowchartElkBackend::SourcePorted)]
    pub(crate) flowchart_elk_backend: FlowchartElkBackend,

    /// Viewport width for viewport-sensitive layouts.
    #[arg(short = 'w', long = "width", alias = "viewport-width", value_parser = parse_positive_f64)]
    pub(crate) width: Option<f64>,

    /// Viewport height for viewport-sensitive layouts.
    #[arg(short = 'H', long = "height", alias = "viewport-height", value_parser = parse_positive_f64)]
    pub(crate) height: Option<f64>,

    /// Root SVG id and internal marker prefix.
    #[arg(short = 'I', long = "svgId", alias = "svg-id", alias = "id")]
    pub(crate) svg_id: Option<String>,

    /// Stabilize rough/hand-drawn rendering where supported.
    #[arg(long = "hand-drawn-seed")]
    pub(crate) hand_drawn_seed: Option<u64>,
}

impl Default for RenderCliArgs {
    fn default() -> Self {
        Self {
            text_measurer: TextMeasurerKind::Vendored,
            math_renderer: MathRendererKind::None,
            flowchart_elk_backend: FlowchartElkBackend::SourcePorted,
            width: None,
            height: None,
            svg_id: None,
            hand_drawn_seed: None,
        }
    }
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct ExportArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(short = 'i', long = "input", value_name = "INPUT")]
    pub(crate) input_file: Option<String>,

    /// Output file. Use `-` for stdout.
    #[arg(short = 'o', long = "output", alias = "out", value_name = "OUTPUT")]
    pub(crate) output: Option<String>,

    /// Output artefacts directory for Markdown input.
    #[arg(short = 'a', long = "artefacts", alias = "artifacts")]
    pub(crate) artefacts: Option<String>,

    /// Parallel jobs for Markdown input. Accepted for mmdc compatibility.
    #[arg(short = 'j', long = "jobs", value_parser = parse_positive_usize)]
    pub(crate) jobs: Option<usize>,

    /// Output format. Defaults to the output extension, then SVG.
    #[arg(
        short = 'e',
        long = "outputFormat",
        alias = "output-format",
        alias = "format",
        value_enum
    )]
    pub(crate) output_format: Option<RenderFormat>,

    /// Background color for SVG/PNG/JPG output.
    #[arg(
        short = 'b',
        long = "backgroundColor",
        alias = "background-color",
        alias = "background"
    )]
    pub(crate) background_color: Option<String>,

    /// JSON Mermaid configuration file.
    #[arg(short = 'c', long = "configFile", alias = "config-file")]
    pub(crate) config_file: Option<String>,

    /// CSS file injected into SVG output before export.
    #[arg(short = 'C', long = "cssFile", alias = "css-file")]
    pub(crate) css_file: Option<String>,

    /// Root SVG id and internal marker prefix.
    #[arg(short = 'I', long = "svgId", alias = "svg-id", alias = "id")]
    pub(crate) svg_id: Option<String>,

    /// Raster/PDF scale factor.
    #[arg(short = 's', long = "scale", value_parser = parse_positive_f32)]
    pub(crate) scale: Option<f32>,

    /// Fit PNG/JPG raster output to this CSS-pixel width before applying --scale.
    #[arg(long = "raster-fit-width", value_parser = parse_positive_u32)]
    pub(crate) raster_fit_width: Option<u32>,

    /// Fit PNG/JPG raster output to this CSS-pixel height before applying --scale.
    #[arg(long = "raster-fit-height", value_parser = parse_positive_u32)]
    pub(crate) raster_fit_height: Option<u32>,

    /// Maximum PNG/JPG output width after scale and fit. Defaults to 8192.
    #[arg(long = "raster-max-width", value_parser = parse_positive_u32)]
    pub(crate) raster_max_width: Option<u32>,

    /// Maximum PNG/JPG output height after scale and fit. Defaults to 8192.
    #[arg(long = "raster-max-height", value_parser = parse_positive_u32)]
    pub(crate) raster_max_height: Option<u32>,

    /// Maximum PNG/JPG output pixels after scale and fit. Defaults to 8192*8192.
    #[arg(long = "raster-max-pixels", value_parser = parse_positive_u64)]
    pub(crate) raster_max_pixels: Option<u64>,

    /// Disable PNG/JPG raster size limits. Use only for trusted oversized exports.
    #[arg(long = "raster-unbounded")]
    pub(crate) raster_unbounded: bool,

    /// Scale PDF to fit chart. Accepted for mmdc compatibility.
    #[arg(short = 'f', long = "pdfFit", alias = "pdf-fit")]
    pub(crate) pdf_fit: bool,

    /// Suppress non-error log output.
    #[arg(short = 'q', long = "quiet")]
    pub(crate) quiet: bool,

    /// JSON Puppeteer configuration file. Accepted for mmdc compatibility.
    #[arg(
        short = 'p',
        long = "puppeteerConfigFile",
        alias = "puppeteer-config-file"
    )]
    pub(crate) puppeteer_config_file: Option<String>,

    /// Iconify package names. Accepted for mmdc compatibility.
    #[arg(long = "iconPacks", num_args = 1..)]
    pub(crate) icon_packs: Vec<String>,

    /// Iconify prefix#url definitions. Accepted for mmdc compatibility.
    #[arg(long = "iconPacksNamesAndUrls", num_args = 1..)]
    pub(crate) icon_packs_names_and_urls: Vec<String>,

    /// Mermaid theme override.
    #[arg(short = 't', long)]
    pub(crate) theme: Option<String>,

    /// Viewport width for viewport-sensitive layouts.
    #[arg(short = 'w', long = "width", alias = "viewport-width", value_parser = parse_positive_f64)]
    pub(crate) width: Option<f64>,

    /// Viewport height for viewport-sensitive layouts.
    #[arg(short = 'H', long = "height", alias = "viewport-height", value_parser = parse_positive_f64)]
    pub(crate) height: Option<f64>,

    /// Text measurement strategy.
    #[arg(long = "text-measurer", value_enum, default_value_t = TextMeasurerKind::Vendored)]
    pub(crate) text_measurer: TextMeasurerKind,

    /// Math renderer for $$...$$ labels.
    #[arg(long = "math-renderer", value_enum, default_value_t = MathRendererKind::None)]
    pub(crate) math_renderer: MathRendererKind,

    /// Flowchart ELK layout backend.
    #[arg(long = "flowchart-elk-backend", value_enum, default_value_t = FlowchartElkBackend::SourcePorted)]
    pub(crate) flowchart_elk_backend: FlowchartElkBackend,

    /// Emit an error diagram instead of failing on parse errors.
    #[arg(long = "suppress-errors")]
    pub(crate) suppress_errors: bool,

    /// Override the local "today" date for time-dependent diagrams.
    #[arg(long = "fixed-today", value_parser = parse_naive_date)]
    pub(crate) fixed_today: Option<chrono::NaiveDate>,

    /// Override the local timezone offset in minutes for time-dependent diagrams.
    #[arg(long = "fixed-local-offset-minutes", value_parser = parse_fixed_local_offset_minutes)]
    pub(crate) fixed_local_offset_minutes: Option<i32>,

    /// Mirror sequence participants below lifelines for ASCII/Unicode output.
    #[arg(long = "sequence-mirror-actors")]
    pub(crate) sequence_mirror_actors: bool,

    /// Stabilize rough/hand-drawn rendering where supported.
    #[arg(long = "hand-drawn-seed")]
    pub(crate) hand_drawn_seed: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum TextMeasurerKind {
    Deterministic,
    #[default]
    Vendored,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum MathRendererKind {
    #[default]
    None,
    Ratex,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum FlowchartElkBackend {
    Compat,
    #[default]
    SourcePorted,
}

impl From<FlowchartElkBackend> for RenderFlowchartElkBackend {
    fn from(value: FlowchartElkBackend) -> Self {
        match value {
            FlowchartElkBackend::Compat => Self::Compat,
            FlowchartElkBackend::SourcePorted => Self::SourcePorted,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum RenderFormat {
    #[default]
    Svg,
    Ascii,
    Unicode,
    Png,
    #[value(name = "jpg", alias = "jpeg")]
    Jpeg,
    Pdf,
}

impl RenderFormat {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            RenderFormat::Svg => "svg",
            RenderFormat::Ascii | RenderFormat::Unicode => "txt",
            RenderFormat::Png => "png",
            RenderFormat::Jpeg => "jpg",
            RenderFormat::Pdf => "pdf",
        }
    }

    pub(crate) fn is_raster(self) -> bool {
        matches!(
            self,
            RenderFormat::Png | RenderFormat::Jpeg | RenderFormat::Pdf
        )
    }

    pub(crate) fn is_text(self) -> bool {
        matches!(self, RenderFormat::Ascii | RenderFormat::Unicode)
    }
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "expected a positive integer".to_string())?;
    if parsed == 0 {
        return Err("expected a positive integer".to_string());
    }
    Ok(parsed)
}

fn parse_positive_u32(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "expected a positive integer".to_string())?;
    if parsed == 0 {
        return Err("expected a positive integer".to_string());
    }
    Ok(parsed)
}

fn parse_positive_u64(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "expected a positive integer".to_string())?;
    if parsed == 0 {
        return Err("expected a positive integer".to_string());
    }
    Ok(parsed)
}

fn parse_positive_f32(value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| "expected a positive number".to_string())?;
    if !(parsed.is_finite() && parsed > 0.0) {
        return Err("expected a positive number".to_string());
    }
    Ok(parsed)
}

fn parse_positive_f64(value: &str) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| "expected a positive number".to_string())?;
    if !(parsed.is_finite() && parsed > 0.0) {
        return Err("expected a positive number".to_string());
    }
    Ok(parsed)
}

fn parse_naive_date(value: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "expected a date in YYYY-MM-DD format".to_string())
}

fn parse_fixed_local_offset_minutes(value: &str) -> Result<i32, String> {
    let parsed = value
        .parse::<i32>()
        .map_err(|_| "expected a timezone offset in minutes".to_string())?;
    let Some(seconds) = parsed.checked_mul(60) else {
        return Err("expected a timezone offset in minutes between -1439 and 1439".to_string());
    };
    if chrono::FixedOffset::east_opt(seconds).is_none() {
        return Err("expected a timezone offset in minutes between -1439 and 1439".to_string());
    }
    Ok(parsed)
}
