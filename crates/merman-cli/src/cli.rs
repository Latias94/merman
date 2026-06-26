use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum, ValueHint};
use merman::render::FlowchartElkBackend as RenderFlowchartElkBackend;
use merman_analysis::{AnalysisRuleProfile, DiagnosticSeverity, configurable_rule_descriptor};

#[derive(Debug, Parser)]
#[command(
    name = "merman-cli",
    version,
    subcommand_precedence_over_arg = true,
    override_usage = "merman-cli [OPTIONS] [INPUT]\n       merman-cli <COMMAND> [ARGS]",
    about = "Headless Mermaid renderer compatible with common mmdc workflows.",
    long_about = "Headless Mermaid renderer compatible with common mmdc workflows.\n\n\
Top-level usage functionally mirrors common mmdc workflows:\n  merman-cli -i input.mmd -o output.svg\n  merman-cli -i input.mmd -o output.png -t dark -b transparent\n\n\
Developer subcommands expose merman internals:\n  merman-cli parse --pretty input.mmd\n  merman-cli layout --pretty input.mmd\n  merman-cli lint --format json input.mmd\n  merman-cli render --format unicode input.mmd"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,

    #[command(flatten)]
    pub(crate) export: ExportArgs,

    /// Input Mermaid file for top-level render mode. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
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
    /// Analyze Mermaid source and print diagnostics JSON or text.
    Lint(LintArgs),
    /// List lint rule metadata.
    LintRules(LintRulesArgs),
    /// Render Mermaid source to SVG/PNG/JPG/PDF/ASCII/Unicode.
    Render(RenderArgs),
    /// Generate shell completion scripts.
    Completion(CompletionArgs),
}

#[derive(Debug, ClapArgs)]
pub(crate) struct DetectArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<String>,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct ParseArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
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
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
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
pub(crate) struct LintArgs {
    /// Input Mermaid or Markdown file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<String>,

    /// Optional file name to use when linting stdin.
    #[arg(
        long = "stdin-file-name",
        value_hint = ValueHint::FilePath,
        help_heading = "Input handling"
    )]
    pub(crate) stdin_file_name: Option<String>,

    /// Output format for diagnostics.
    #[arg(
        long,
        value_enum,
        default_value_t = LintOutputFormat::Json,
        help_heading = "Output"
    )]
    pub(crate) format: LintOutputFormat,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub(crate) pretty: bool,

    /// Include Markdown fence diagnostics by scanning `.md`, `.markdown`, or `.mdx` input.
    #[arg(long = "markdown", help_heading = "Analysis options")]
    pub(crate) markdown: bool,

    /// JSON Mermaid configuration file.
    #[arg(
        short = 'c',
        long = "configFile",
        alias = "config-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) config_file: Option<String>,

    /// Override the local "today" date for time-dependent diagrams.
    #[arg(
        long = "fixed-today",
        value_parser = parse_naive_date,
        help_heading = "Deterministic rendering"
    )]
    pub(crate) fixed_today: Option<chrono::NaiveDate>,

    /// Override the local timezone offset in minutes for time-dependent diagrams.
    #[arg(
        long = "fixed-local-offset-minutes",
        value_parser = parse_fixed_local_offset_minutes,
        help_heading = "Deterministic rendering"
    )]
    pub(crate) fixed_local_offset_minutes: Option<i32>,

    /// Maximum source bytes accepted by the analyzer.
    #[arg(
        long = "max-source-bytes",
        value_parser = parse_positive_usize,
        help_heading = "Analysis options"
    )]
    pub(crate) max_source_bytes: Option<usize>,

    /// Built-in lint rule profile: core, recommended, or strict.
    #[arg(
        long = "lint-profile",
        value_name = "PROFILE",
        value_parser = parse_lint_profile,
        help_heading = "Lint rules"
    )]
    pub(crate) lint_profile: Option<AnalysisRuleProfile>,

    /// Enable a configurable lint rule by stable rule id. Can be repeated.
    #[arg(
        long = "enable-rule",
        value_name = "RULE_ID",
        value_parser = parse_lint_rule_id,
        help_heading = "Lint rules"
    )]
    pub(crate) enable_rules: Vec<String>,

    /// Disable a configurable lint rule by stable rule id. Can be repeated.
    #[arg(
        long = "disable-rule",
        value_name = "RULE_ID",
        value_parser = parse_lint_rule_id,
        help_heading = "Lint rules"
    )]
    pub(crate) disable_rules: Vec<String>,

    /// Override a configurable lint rule severity as RULE_ID=error|warning|info|hint. Can be repeated.
    #[arg(
        long = "rule-severity",
        value_name = "RULE_ID=SEVERITY",
        value_parser = parse_lint_rule_severity_override,
        help_heading = "Lint rules"
    )]
    pub(crate) rule_severities: Vec<LintRuleSeverityOverride>,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct LintRulesArgs {
    /// Output format for rule metadata.
    #[arg(
        long,
        value_enum,
        default_value_t = LintOutputFormat::Json,
        help_heading = "Output"
    )]
    pub(crate) format: LintOutputFormat,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub(crate) pretty: bool,

    /// Only list rules that public lint configuration can reference.
    #[arg(long = "configurable", help_heading = "Rule filters")]
    pub(crate) configurable: bool,
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum LintOutputFormat {
    #[default]
    Json,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LintRuleSeverityOverride {
    pub(crate) rule_id: String,
    pub(crate) severity: DiagnosticSeverity,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct RenderArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<String>,

    #[command(flatten)]
    pub(crate) export: RenderExportArgs,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct CompletionArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub(crate) shell: clap_complete::Shell,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct ParseCliArgs {
    /// Emit an error diagram instead of failing on parse errors.
    #[arg(long = "suppress-errors", help_heading = "Mermaid configuration")]
    pub(crate) suppress_errors: bool,

    /// JSON Mermaid configuration file.
    #[arg(
        short = 'c',
        long = "configFile",
        alias = "config-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) config_file: Option<String>,

    /// Mermaid theme override.
    #[arg(short = 't', long, help_heading = "Mermaid configuration")]
    pub(crate) theme: Option<String>,

    /// Override the local "today" date for time-dependent diagrams.
    #[arg(
        long = "fixed-today",
        value_parser = parse_naive_date,
        help_heading = "Deterministic rendering"
    )]
    pub(crate) fixed_today: Option<chrono::NaiveDate>,

    /// Override the local timezone offset in minutes for time-dependent diagrams.
    #[arg(
        long = "fixed-local-offset-minutes",
        value_parser = parse_fixed_local_offset_minutes,
        help_heading = "Deterministic rendering"
    )]
    pub(crate) fixed_local_offset_minutes: Option<i32>,
}

#[derive(Debug, Clone, ClapArgs)]
pub(crate) struct RenderCliArgs {
    /// Text measurement strategy.
    #[arg(
        long = "text-measurer",
        value_enum,
        default_value_t = TextMeasurerKind::Vendored,
        help_heading = "Rust renderer controls"
    )]
    pub(crate) text_measurer: TextMeasurerKind,

    /// Math renderer for $$...$$ labels.
    #[arg(
        long = "math-renderer",
        value_enum,
        default_value_t = MathRendererKind::None,
        help_heading = "Rust renderer controls"
    )]
    pub(crate) math_renderer: MathRendererKind,

    /// Flowchart ELK layout backend.
    #[arg(
        long = "flowchart-elk-backend",
        value_enum,
        default_value_t = FlowchartElkBackend::SourcePorted,
        help_heading = "Rust renderer controls"
    )]
    pub(crate) flowchart_elk_backend: FlowchartElkBackend,

    /// Viewport width for viewport-sensitive layouts. Top-level mmdc-compatible mode defaults to 800.
    #[arg(
        short = 'w',
        long = "width",
        alias = "viewport-width",
        value_parser = parse_positive_f64,
        help_heading = "Rust renderer controls"
    )]
    pub(crate) width: Option<f64>,

    /// Viewport height for viewport-sensitive layouts. Top-level mmdc-compatible mode defaults to 600.
    #[arg(
        short = 'H',
        long = "height",
        alias = "viewport-height",
        value_parser = parse_positive_f64,
        help_heading = "Rust renderer controls"
    )]
    pub(crate) height: Option<f64>,

    /// Root SVG id and internal marker prefix.
    #[arg(
        short = 'I',
        long = "svgId",
        alias = "svg-id",
        alias = "id",
        help_heading = "Rust renderer controls"
    )]
    pub(crate) svg_id: Option<String>,

    /// Stabilize rough/hand-drawn rendering where supported.
    #[arg(long = "hand-drawn-seed", help_heading = "Deterministic rendering")]
    pub(crate) hand_drawn_seed: Option<u64>,

    /// Render resource profile for source, layout model, and SVG output budgets.
    #[arg(
        long = "resource-profile",
        value_enum,
        default_value_t = ResourceProfile::TrustedNative,
        help_heading = "Rust renderer controls"
    )]
    pub(crate) resource_profile: ResourceProfile,
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
            resource_profile: ResourceProfile::TrustedNative,
        }
    }
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct ExportArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(
        short = 'i',
        long = "input",
        value_name = "INPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) input_file: Option<String>,

    /// Output file. Use `-` for stdout.
    #[arg(
        short = 'o',
        long = "output",
        alias = "out",
        value_name = "OUTPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) output: Option<String>,

    /// Output artefacts directory for Markdown input.
    #[arg(
        short = 'a',
        long = "artefacts",
        alias = "artifacts",
        value_hint = ValueHint::DirPath,
        help_heading = "Markdown batch export"
    )]
    pub(crate) artefacts: Option<String>,

    /// Parallel jobs for Markdown input. Defaults to half available CPUs, minimum 1.
    #[arg(
        short = 'j',
        long = "jobs",
        value_parser = parse_positive_usize,
        help_heading = "Markdown batch export"
    )]
    pub(crate) jobs: Option<usize>,

    /// Output format. Defaults to the output extension, then SVG.
    #[arg(
        short = 'e',
        long = "outputFormat",
        alias = "output-format",
        visible_alias = "format",
        value_enum,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) output_format: Option<RenderFormat>,

    /// Background color for SVG/PNG/JPG output. Top-level mmdc-compatible mode defaults to white.
    #[arg(
        short = 'b',
        long = "backgroundColor",
        alias = "background-color",
        alias = "background",
        help_heading = "Raster and PDF export"
    )]
    pub(crate) background_color: Option<String>,

    /// CSS file injected into SVG output before export.
    #[arg(
        short = 'C',
        long = "cssFile",
        alias = "css-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) css_file: Option<String>,

    /// Scale PDF to fit chart. Accepted for mmdc compatibility.
    #[arg(
        short = 'f',
        long = "pdfFit",
        alias = "pdf-fit",
        help_heading = "Raster and PDF export"
    )]
    pub(crate) pdf_fit: bool,

    /// Suppress non-error log output.
    #[arg(short = 'q', long = "quiet", help_heading = "mmdc-compatible export")]
    pub(crate) quiet: bool,

    /// JSON Puppeteer configuration file. Accepted for mmdc compatibility.
    #[arg(
        short = 'p',
        long = "puppeteerConfigFile",
        alias = "puppeteer-config-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Accepted browser compatibility flags"
    )]
    pub(crate) puppeteer_config_file: Option<String>,

    #[command(flatten)]
    pub(crate) raster: RasterCliArgs,

    #[command(flatten)]
    pub(crate) icons: IconCliArgs,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,

    #[command(flatten)]
    pub(crate) render: RenderCliArgs,

    #[command(flatten)]
    pub(crate) text: TextOutputCliArgs,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct RenderExportArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(
        short = 'i',
        long = "input",
        value_name = "INPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "Render input and output"
    )]
    pub(crate) input_file: Option<String>,

    /// Output file. Use `-` for stdout.
    #[arg(
        short = 'o',
        long = "output",
        alias = "out",
        value_name = "OUTPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "Render input and output"
    )]
    pub(crate) output: Option<String>,

    /// Output format. Defaults to the output extension, then SVG.
    #[arg(
        short = 'e',
        long = "outputFormat",
        alias = "output-format",
        visible_alias = "format",
        value_enum,
        help_heading = "Render input and output"
    )]
    pub(crate) output_format: Option<RenderFormat>,

    /// Background color for SVG/PNG/JPG output.
    #[arg(
        short = 'b',
        long = "backgroundColor",
        alias = "background-color",
        alias = "background",
        help_heading = "Raster and PDF export"
    )]
    pub(crate) background_color: Option<String>,

    /// CSS file injected into SVG output before export.
    #[arg(
        short = 'C',
        long = "cssFile",
        alias = "css-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) css_file: Option<String>,

    /// Suppress non-error log output.
    #[arg(short = 'q', long = "quiet", help_heading = "Render input and output")]
    pub(crate) quiet: bool,

    #[command(flatten)]
    pub(crate) raster: RasterCliArgs,

    #[command(flatten)]
    pub(crate) icons: IconCliArgs,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,

    #[command(flatten)]
    pub(crate) render: RenderCliArgs,

    #[command(flatten)]
    pub(crate) text: TextOutputCliArgs,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct RasterCliArgs {
    /// Raster/PDF scale factor. Defaults to 1.
    #[arg(
        short = 's',
        long = "scale",
        value_parser = parse_positive_f32,
        help_heading = "Raster and PDF export"
    )]
    pub(crate) scale: Option<f32>,

    /// Fit PNG/JPG raster output to this CSS-pixel width before applying --scale.
    #[arg(
        long = "raster-fit-width",
        value_parser = parse_positive_u32,
        help_heading = "Raster and PDF export"
    )]
    pub(crate) raster_fit_width: Option<u32>,

    /// Fit PNG/JPG raster output to this CSS-pixel height before applying --scale.
    #[arg(
        long = "raster-fit-height",
        value_parser = parse_positive_u32,
        help_heading = "Raster and PDF export"
    )]
    pub(crate) raster_fit_height: Option<u32>,

    /// Maximum PNG/JPG output width after scale and fit. Defaults to 8192.
    #[arg(
        long = "raster-max-width",
        value_parser = parse_positive_u32,
        help_heading = "Raster and PDF export"
    )]
    pub(crate) raster_max_width: Option<u32>,

    /// Maximum PNG/JPG output height after scale and fit. Defaults to 8192.
    #[arg(
        long = "raster-max-height",
        value_parser = parse_positive_u32,
        help_heading = "Raster and PDF export"
    )]
    pub(crate) raster_max_height: Option<u32>,

    /// Maximum PNG/JPG output pixels after scale and fit. Defaults to 8192*8192.
    #[arg(
        long = "raster-max-pixels",
        value_parser = parse_positive_u64,
        help_heading = "Raster and PDF export"
    )]
    pub(crate) raster_max_pixels: Option<u64>,

    /// Disable PNG/JPG raster size limits. Use only for trusted oversized exports.
    #[arg(
        long = "raster-unbounded",
        conflicts_with_all = ["raster_max_width", "raster_max_height", "raster_max_pixels"],
        help_heading = "Raster and PDF export"
    )]
    pub(crate) raster_unbounded: bool,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct IconCliArgs {
    /// Iconify package names.
    #[arg(long = "iconPacks", num_args = 1.., help_heading = "Icon packs")]
    pub(crate) icon_packs: Vec<String>,

    /// Iconify prefix#url definitions.
    #[arg(
        long = "iconPacksNamesAndUrls",
        num_args = 1..,
        help_heading = "Icon packs"
    )]
    pub(crate) icon_packs_names_and_urls: Vec<String>,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct TextOutputCliArgs {
    /// Mirror sequence participants below lifelines for ASCII/Unicode output.
    #[arg(long = "sequence-mirror-actors", help_heading = "Text output")]
    pub(crate) sequence_mirror_actors: bool,
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

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum ResourceProfile {
    Interactive,
    TypstPackage,
    #[default]
    TrustedNative,
    UnboundedForTrustedInput,
}

impl From<ResourceProfile> for merman::render::RenderResourceProfile {
    fn from(value: ResourceProfile) -> Self {
        match value {
            ResourceProfile::Interactive => Self::Interactive,
            ResourceProfile::TypstPackage => Self::TypstPackage,
            ResourceProfile::TrustedNative => Self::TrustedNative,
            ResourceProfile::UnboundedForTrustedInput => Self::UnboundedForTrustedInput,
        }
    }
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

fn parse_lint_rule_severity_override(value: &str) -> Result<LintRuleSeverityOverride, String> {
    let Some((rule_id, severity)) = value.split_once('=') else {
        return Err("expected RULE_ID=SEVERITY".to_string());
    };
    if rule_id.trim().is_empty() {
        return Err("rule id must not be empty".to_string());
    }
    if configurable_rule_descriptor(rule_id).is_none() {
        return Err(format!("unknown or internal lint rule id `{rule_id}`"));
    }

    Ok(LintRuleSeverityOverride {
        rule_id: rule_id.to_string(),
        severity: parse_lint_severity(severity.trim())?,
    })
}

fn parse_lint_rule_id(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("rule id must not be empty".to_string());
    }
    if configurable_rule_descriptor(value).is_none() {
        return Err(format!("unknown or internal lint rule id `{value}`"));
    }
    Ok(value.to_string())
}

fn parse_lint_profile(value: &str) -> Result<AnalysisRuleProfile, String> {
    match value.to_ascii_lowercase().as_str() {
        "core" => Ok(AnalysisRuleProfile::Core),
        "recommended" => Ok(AnalysisRuleProfile::Recommended),
        "strict" => Ok(AnalysisRuleProfile::Strict),
        _ => Err("expected profile core, recommended, or strict".to_string()),
    }
}

fn parse_lint_severity(value: &str) -> Result<DiagnosticSeverity, String> {
    match value.to_ascii_lowercase().as_str() {
        "error" => Ok(DiagnosticSeverity::Error),
        "warning" | "warn" => Ok(DiagnosticSeverity::Warning),
        "info" => Ok(DiagnosticSeverity::Info),
        "hint" => Ok(DiagnosticSeverity::Hint),
        _ => Err("expected severity error, warning, info, or hint".to_string()),
    }
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
