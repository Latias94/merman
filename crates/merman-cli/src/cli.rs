#[cfg(any(feature = "ascii", feature = "svg"))]
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum, ValueHint};
#[cfg(feature = "analysis")]
use merman_analysis::{AnalysisRuleProfile, DiagnosticSeverity, configurable_rule_descriptor};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "merman-cli",
    version,
    propagate_version = true,
    subcommand_required = true,
    arg_required_else_help = true,
    override_usage = "merman-cli <COMMAND> [ARGS]",
    about = "Headless Mermaid CLI with parse, analysis, and optional rendering capabilities.",
    long_about = "Headless Mermaid CLI with parse, analysis, and optional rendering capabilities.\n\nThe commands and formats shown below are exactly those compiled into this artifact. Use `capabilities --json` for the machine-readable capability contract."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: RawCommand,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum RawCommand {
    #[cfg(any(feature = "svg", feature = "ascii"))]
    /// Render one Mermaid diagram with Merman's native interface.
    Render(RenderArgs),
    #[cfg(feature = "markdown")]
    /// Render every Mermaid diagram in one Markdown document.
    Batch(BatchArgs),
    #[cfg(feature = "analysis")]
    /// Analyze Mermaid source and print diagnostics JSON or text.
    Lint(LintArgs),
    #[cfg(feature = "analysis")]
    /// Apply non-conflicting diagnostics fixes to Mermaid or Markdown source.
    Fix(FixArgs),
    #[cfg(feature = "analysis")]
    /// List lint rule metadata.
    LintRules(LintRulesArgs),
    #[cfg(feature = "svg")]
    /// Render through the pinned mmdc-compatible interface.
    Mmdc(MmdcArgs),
    /// Detect the Mermaid diagram type.
    Detect(DetectArgs),
    /// Parse Mermaid source and print the semantic JSON model.
    Parse(ParseArgs),
    #[cfg(feature = "svg")]
    /// Parse and layout Mermaid source, then print layout JSON.
    Layout(LayoutArgs),
    /// Print the compiled capabilities from the canonical capability descriptor.
    Capabilities(CapabilitiesArgs),
    #[cfg(feature = "shell-completions")]
    /// Generate shell completion scripts.
    Completion(CompletionArgs),
}

#[derive(Debug, ClapArgs)]
pub(crate) struct CapabilitiesArgs {
    /// Emit the machine-readable capability document.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct DetectArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    #[command(flatten)]
    pub(crate) engine: EngineCliArgs,
}

#[derive(Debug, ClapArgs)]
pub(crate) struct ParseArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub(crate) pretty: bool,

    /// Include parse metadata alongside the model.
    #[arg(long, alias = "with-meta")]
    pub(crate) meta: bool,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,
}

#[cfg(feature = "svg")]
#[derive(Debug, ClapArgs)]
pub(crate) struct LayoutArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub(crate) pretty: bool,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,

    #[command(flatten)]
    pub(crate) render: LayoutRenderCliArgs,
}

#[cfg(feature = "analysis")]
#[derive(Debug, ClapArgs)]
pub(crate) struct LintArgs {
    /// Input Mermaid or Markdown file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    /// Optional file name to use when linting stdin.
    #[arg(
        long = "stdin-file-name",
        value_hint = ValueHint::FilePath,
        help_heading = "Input handling"
    )]
    pub(crate) stdin_file_name: Option<PathBuf>,

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

    #[command(flatten)]
    pub(crate) analysis: AnalysisCliArgs,
}

#[cfg(feature = "analysis")]
#[derive(Debug, ClapArgs)]
pub(crate) struct FixArgs {
    /// Input Mermaid or Markdown file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    /// Optional file name to use when fixing stdin.
    #[arg(
        long = "stdin-file-name",
        value_hint = ValueHint::FilePath,
        help_heading = "Input handling"
    )]
    pub(crate) stdin_file_name: Option<PathBuf>,

    /// Write the result back to the input file instead of stdout.
    #[arg(long, conflicts_with = "output", help_heading = "Output")]
    pub(crate) write: bool,

    /// Write the result to this file instead of stdout.
    #[arg(
        short = 'o',
        long,
        value_hint = ValueHint::FilePath,
        help_heading = "Output"
    )]
    pub(crate) output: Option<PathBuf>,

    /// Apply every non-conflicting fix instead of one preferred fix per diagnostic.
    #[arg(long, help_heading = "Fix selection")]
    pub(crate) all: bool,

    #[command(flatten)]
    pub(crate) analysis: AnalysisCliArgs,
}

#[cfg(feature = "analysis")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct AnalysisCliArgs {
    /// Include Markdown fence diagnostics by scanning `.md`, `.markdown`, or `.mdx` input.
    #[arg(long = "markdown", help_heading = "Analysis options")]
    pub(crate) markdown: bool,

    /// JSON Mermaid configuration file.
    #[arg(
        short = 'c',
        long = "config-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) config_file: Option<PathBuf>,

    #[command(flatten)]
    pub(crate) runtime: RuntimeCliArgs,

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

#[cfg(feature = "analysis")]
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

#[cfg(feature = "analysis")]
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum LintOutputFormat {
    #[default]
    Json,
    Text,
}

#[cfg(feature = "analysis")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LintRuleSeverityOverride {
    pub(crate) rule_id: String,
    pub(crate) severity: DiagnosticSeverity,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, ClapArgs)]
pub(crate) struct RenderArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    /// Output file. Use `-` for stdout.
    #[arg(
        short = 'o',
        long,
        value_name = "OUTPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "Render input and output"
    )]
    pub(crate) output: Option<PathBuf>,

    /// Interpret the input as Mermaid source or an existing SVG document.
    #[cfg(feature = "svg")]
    #[arg(
        long = "input-kind",
        value_enum,
        help_heading = "Render input and output"
    )]
    pub(crate) input_kind: Option<RenderInputKind>,

    #[command(flatten)]
    pub(crate) options: NativeRenderOptions,
}

#[cfg(feature = "markdown")]
#[derive(Debug, ClapArgs)]
pub(crate) struct BatchArgs {
    /// Input Markdown file. Use `-` for stdin.
    #[arg(value_name = "INPUT", value_hint = ValueHint::FilePath)]
    pub(crate) input: Option<PathBuf>,

    /// Logical source file name for Markdown read from stdin.
    #[arg(
        long = "stdin-file-name",
        value_hint = ValueHint::FilePath,
        help_heading = "Batch input and output"
    )]
    pub(crate) stdin_file_name: Option<PathBuf>,

    /// Tool-owned directory for the rewritten document and generated artifacts.
    #[arg(
        short = 'o',
        long = "output-dir",
        value_hint = ValueHint::DirPath,
        help_heading = "Batch input and output"
    )]
    pub(crate) output_dir: Option<PathBuf>,

    #[cfg(feature = "parallel-markdown")]
    /// Maximum number of Markdown charts rendered concurrently.
    #[arg(
        short = 'j',
        long = "jobs",
        value_parser = parse_positive_usize,
        help_heading = "Batch input and output"
    )]
    pub(crate) jobs: Option<usize>,

    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    /// Aggregate scheduling budget for parallel PNG/JPEG/PDF encoding, in MiB.
    #[arg(
        long = "encoding-parallel-budget-mib",
        visible_alias = "encoding-memory-budget-mib",
        value_parser = parse_positive_u64,
        help_heading = "Batch resource controls"
    )]
    pub(crate) encoding_parallel_budget_mib: Option<u64>,

    #[command(flatten)]
    pub(crate) options: NativeRenderOptions,
}

#[cfg(feature = "shell-completions")]
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
        long = "config-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) config_file: Option<PathBuf>,

    /// Mermaid theme override.
    #[arg(short = 't', long, help_heading = "Mermaid configuration")]
    pub(crate) theme: Option<String>,

    #[command(flatten)]
    pub(crate) runtime: RuntimeCliArgs,
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct EngineCliArgs {
    /// JSON Mermaid configuration file.
    #[arg(
        short = 'c',
        long = "config-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) config_file: Option<PathBuf>,

    /// Mermaid theme override.
    #[arg(short = 't', long, help_heading = "Mermaid configuration")]
    pub(crate) theme: Option<String>,

    #[command(flatten)]
    pub(crate) runtime: RuntimeCliArgs,
}

impl EngineCliArgs {
    pub(crate) fn into_parse_args(self) -> ParseCliArgs {
        ParseCliArgs {
            suppress_errors: false,
            config_file: self.config_file,
            theme: self.theme,
            runtime: self.runtime,
        }
    }
}

#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct RuntimeCliArgs {
    /// Runtime source for clock, local timezone, and operation randomness.
    #[arg(
        long = "runtime",
        value_enum,
        default_value_t = RuntimePolicyKind::Deterministic,
        help_heading = "Runtime policy"
    )]
    pub(crate) policy: RuntimePolicyKind,

    #[cfg(feature = "system-clock")]
    /// Use the system clock while keeping other runtime sources deterministic.
    #[arg(long = "system-clock", help_heading = "Runtime policy")]
    pub(crate) system_clock: bool,

    #[cfg(feature = "system-timezone")]
    /// Use the system local timezone while keeping other runtime sources deterministic.
    #[arg(long = "system-timezone", help_heading = "Runtime policy")]
    pub(crate) system_timezone: bool,

    #[cfg(feature = "system-random")]
    /// Use system randomness while keeping other runtime sources deterministic.
    #[arg(long = "system-random", help_heading = "Runtime policy")]
    pub(crate) system_random: bool,

    #[cfg(feature = "system-timing")]
    /// Enable operation timing diagnostics through the compiled system timing adapter.
    #[arg(long = "system-timing", help_heading = "Runtime policy")]
    pub(crate) system_timing: bool,

    /// Override the local "today" date for time-dependent diagrams.
    #[arg(
        long = "fixed-today",
        value_parser = parse_naive_date,
        help_heading = "Runtime policy"
    )]
    pub(crate) fixed_today: Option<chrono::NaiveDate>,

    /// Override the local timezone offset in minutes for time-dependent diagrams.
    #[arg(
        long = "fixed-local-offset-minutes",
        value_parser = parse_fixed_local_offset_minutes,
        help_heading = "Runtime policy"
    )]
    pub(crate) fixed_local_offset_minutes: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum RuntimePolicyKind {
    #[default]
    Deterministic,
    #[cfg(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    ))]
    Native,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, ClapArgs)]
pub(crate) struct RenderCliArgs {
    #[cfg(feature = "svg")]
    /// Text measurement strategy.
    #[arg(
        long = "text-measurer",
        value_enum,
        help_heading = "Merman renderer controls"
    )]
    pub(crate) text_measurer: Option<TextMeasurerKind>,

    #[cfg(feature = "svg")]
    /// Math renderer override. Unspecified uses the compiled default; `ratex` requires `math`.
    #[arg(
        long = "math-renderer",
        value_enum,
        help_heading = "Merman renderer controls"
    )]
    pub(crate) math_renderer: Option<MathRendererKind>,

    #[cfg(feature = "svg")]
    /// Available container width for size-sensitive layouts. Top-level mmdc-compatible mode defaults to 800.
    #[arg(
        short = 'w',
        long = "width",
        value_parser = parse_positive_f64,
        help_heading = "Layout viewport"
    )]
    pub(crate) container_width: Option<f64>,

    #[cfg(feature = "svg")]
    /// Available container height for size-sensitive layouts. Top-level mmdc-compatible mode defaults to 600.
    #[arg(
        short = 'H',
        long = "height",
        value_parser = parse_positive_f64,
        help_heading = "Layout viewport"
    )]
    pub(crate) container_height: Option<f64>,

    #[cfg(feature = "svg")]
    /// Root SVG id and internal marker prefix.
    #[arg(short = 'I', long = "svg-id", help_heading = "SVG output")]
    pub(crate) svg_id: Option<String>,

    #[cfg(feature = "svg")]
    /// Stabilize rough/hand-drawn rendering where supported.
    #[arg(long = "hand-drawn-seed", help_heading = "Deterministic rendering")]
    pub(crate) hand_drawn_seed: Option<u64>,

    /// Render resource profile for source, semantic model, and compiled output budgets.
    ///
    /// The CLI defaults to trusted-native for mmdc-compatible local workloads. Use interactive for
    /// cooperative local editing and constrained for untrusted, public, or multi-tenant input.
    /// The unbounded profile only removes policy budgets; hard backend capabilities still apply.
    #[arg(
        long = "resource-profile",
        value_parser = resource_profile_value_parser(),
        default_value_t = merman::resources::CLI_DEFAULT_RESOURCE_PROFILE,
        help_heading = "Merman resource controls"
    )]
    pub(crate) resource_profile: ResourceProfile,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl Default for RenderCliArgs {
    fn default() -> Self {
        Self {
            #[cfg(feature = "svg")]
            text_measurer: None,
            #[cfg(feature = "svg")]
            math_renderer: None,
            #[cfg(feature = "svg")]
            container_width: None,
            #[cfg(feature = "svg")]
            container_height: None,
            #[cfg(feature = "svg")]
            svg_id: None,
            #[cfg(feature = "svg")]
            hand_drawn_seed: None,
            resource_profile: merman::resources::CLI_DEFAULT_RESOURCE_PROFILE,
        }
    }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, ClapArgs)]
pub(crate) struct LayoutRenderCliArgs {
    /// Text measurement strategy.
    #[arg(
        long = "text-measurer",
        value_enum,
        default_value_t = TextMeasurerKind::Vendored,
        help_heading = "Layout controls"
    )]
    pub(crate) text_measurer: TextMeasurerKind,

    /// Math renderer override. Unspecified uses the compiled default; `ratex` requires `math`.
    #[arg(long = "math-renderer", value_enum, help_heading = "Layout controls")]
    pub(crate) math_renderer: Option<MathRendererKind>,

    /// Available container width for size-sensitive layouts.
    #[arg(
        short = 'w',
        long = "width",
        value_parser = parse_positive_f64,
        help_heading = "Layout viewport"
    )]
    pub(crate) container_width: Option<f64>,

    /// Available container height for size-sensitive layouts.
    #[arg(
        short = 'H',
        long = "height",
        value_parser = parse_positive_f64,
        help_heading = "Layout viewport"
    )]
    pub(crate) container_height: Option<f64>,

    /// Render resource profile for source, semantic model, and layout budgets.
    #[arg(
        long = "resource-profile",
        value_parser = resource_profile_value_parser(),
        default_value_t = merman::resources::CLI_DEFAULT_RESOURCE_PROFILE,
        help_heading = "Merman resource controls"
    )]
    pub(crate) resource_profile: ResourceProfile,
}

#[cfg(feature = "svg")]
impl LayoutRenderCliArgs {
    pub(crate) fn into_render_args(self) -> RenderCliArgs {
        RenderCliArgs {
            text_measurer: Some(self.text_measurer),
            math_renderer: self.math_renderer,
            container_width: self.container_width,
            container_height: self.container_height,
            svg_id: None,
            hand_drawn_seed: None,
            resource_profile: self.resource_profile,
        }
    }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum MmdcTheme {
    #[default]
    Default,
    Forest,
    Dark,
    Neutral,
}

#[cfg(feature = "svg")]
impl MmdcTheme {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Forest => "forest",
            Self::Dark => "dark",
            Self::Neutral => "neutral",
        }
    }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct MmdcParseCliArgs {
    /// JSON Mermaid configuration file.
    #[arg(
        short = 'c',
        long = "configFile",
        value_hint = ValueHint::FilePath,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) config_file: Option<PathBuf>,

    /// Theme of the chart.
    #[arg(
        short = 't',
        long,
        value_enum,
        default_value_t = MmdcTheme::Default,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) theme: MmdcTheme,

    #[command(flatten)]
    pub(crate) runtime: RuntimeCliArgs,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, ClapArgs)]
pub(crate) struct MmdcRenderCliArgs {
    /// Text measurement strategy.
    #[arg(
        long = "text-measurer",
        value_enum,
        default_value_t = TextMeasurerKind::Vendored,
        help_heading = "Merman renderer controls"
    )]
    pub(crate) text_measurer: TextMeasurerKind,

    /// Math renderer override. Unspecified uses the compiled default.
    #[arg(
        long = "math-renderer",
        value_enum,
        help_heading = "Merman renderer controls"
    )]
    pub(crate) math_renderer: Option<MathRendererKind>,

    /// Width of the page.
    #[arg(
        short = 'w',
        long = "width",
        value_parser = parse_positive_f64,
        default_value_t = 800.0,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) container_width: f64,

    /// Height of the page.
    #[arg(
        short = 'H',
        long = "height",
        value_parser = parse_positive_f64,
        default_value_t = 600.0,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) container_height: f64,

    /// Root SVG id and internal marker prefix.
    #[arg(short = 'I', long = "svgId", help_heading = "mmdc-compatible export")]
    pub(crate) svg_id: Option<String>,

    /// Stabilize rough/hand-drawn rendering where supported.
    #[arg(long = "hand-drawn-seed", help_heading = "Deterministic rendering")]
    pub(crate) hand_drawn_seed: Option<u64>,

    /// Render resource profile.
    #[arg(
        long = "resource-profile",
        value_parser = resource_profile_value_parser(),
        default_value_t = merman::resources::CLI_DEFAULT_RESOURCE_PROFILE,
        help_heading = "Merman resource controls"
    )]
    pub(crate) resource_profile: ResourceProfile,
}

#[cfg(feature = "svg")]
impl Default for MmdcRenderCliArgs {
    fn default() -> Self {
        Self {
            text_measurer: TextMeasurerKind::Vendored,
            math_renderer: None,
            container_width: 800.0,
            container_height: 600.0,
            svg_id: None,
            hand_drawn_seed: None,
            resource_profile: merman::resources::CLI_DEFAULT_RESOURCE_PROFILE,
        }
    }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct MmdcArgs {
    /// Input Mermaid file. Use `-` for stdin.
    #[arg(
        short = 'i',
        long = "input",
        value_name = "INPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) input_file: Option<PathBuf>,

    /// Output file. Use `-` for stdout.
    #[arg(
        short = 'o',
        long = "output",
        alias = "out",
        value_name = "OUTPUT",
        value_hint = ValueHint::FilePath,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) output: Option<PathBuf>,

    #[cfg(feature = "markdown")]
    /// Output artefacts directory for Markdown input.
    #[arg(
        short = 'a',
        long = "artefacts",
        alias = "artifacts",
        value_hint = ValueHint::DirPath,
        help_heading = "Markdown batch export"
    )]
    pub(crate) artefacts: Option<PathBuf>,

    #[cfg(feature = "parallel-markdown")]
    /// Parallel jobs for Markdown input. Defaults to half available CPUs, minimum 1.
    #[arg(
        short = 'j',
        long = "jobs",
        value_parser = parse_positive_usize,
        help_heading = "Markdown batch export"
    )]
    pub(crate) jobs: Option<usize>,

    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    /// Aggregate scheduling budget for parallel PNG/JPEG/PDF encoding, in MiB.
    #[arg(
        long = "encoding-parallel-budget-mib",
        visible_alias = "encoding-memory-budget-mib",
        value_parser = parse_positive_u64,
        help_heading = "Markdown batch export"
    )]
    pub(crate) encoding_parallel_budget_mib: Option<u64>,

    /// Output format. Defaults to the output extension, then SVG.
    #[arg(
        short = 'e',
        long = "outputFormat",
        alias = "output-format",
        visible_alias = "format",
        value_enum,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) output_format: Option<MmdcOutputFormat>,

    /// SVG output pipeline. Compiled binary exports always start from resvg-safe.
    #[arg(
        long = "svg-pipeline",
        value_enum,
        help_heading = "Merman renderer controls"
    )]
    pub(crate) svg_pipeline: Option<SvgPipelineKind>,

    /// Background color for the selected rendered output. Top-level mmdc-compatible mode defaults to white.
    #[arg(
        short = 'b',
        long = "backgroundColor",
        alias = "background-color",
        alias = "background",
        help_heading = "mmdc-compatible export"
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
    pub(crate) css_file: Option<PathBuf>,

    #[cfg(feature = "pdf")]
    /// Scale PDF to fit chart. Accepted for mmdc compatibility.
    #[arg(
        short = 'f',
        long = "pdfFit",
        alias = "pdf-fit",
        help_heading = "mmdc-compatible export"
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
    pub(crate) puppeteer_config_file: Option<PathBuf>,

    #[cfg(feature = "png")]
    #[command(flatten)]
    pub(crate) raster: RasterCliArgs,

    #[cfg(feature = "pdf")]
    #[command(flatten)]
    pub(crate) pdf: PdfCliArgs,

    #[cfg(any(feature = "png", feature = "pdf"))]
    #[command(flatten)]
    pub(crate) embedded_images: EmbeddedImageCliArgs,

    #[cfg(feature = "icons")]
    #[command(flatten)]
    pub(crate) icons: MmdcIconCliArgs,

    #[command(flatten)]
    pub(crate) parse: MmdcParseCliArgs,

    #[command(flatten)]
    pub(crate) render: MmdcRenderCliArgs,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct NativeRenderOptions {
    /// Output format. Defaults to the first compiled output format.
    #[arg(
        short = 'e',
        long = "format",
        value_enum,
        help_heading = "Render input and output"
    )]
    pub(crate) format: Option<RenderFormat>,

    #[cfg(feature = "svg")]
    /// SVG output pipeline. Compiled binary exports always start from resvg-safe.
    #[arg(
        long = "svg-pipeline",
        value_enum,
        help_heading = "Merman renderer controls"
    )]
    pub(crate) svg_pipeline: Option<SvgPipelineKind>,

    #[cfg(feature = "svg")]
    /// Background color for the selected rendered output.
    #[arg(short = 'b', long = "background", help_heading = "SVG-based output")]
    pub(crate) background_color: Option<String>,

    #[cfg(feature = "svg")]
    /// CSS file injected into SVG output before export.
    #[arg(
        short = 'C',
        long = "css-file",
        value_hint = ValueHint::FilePath,
        help_heading = "Mermaid configuration"
    )]
    pub(crate) css_file: Option<PathBuf>,

    /// Suppress non-error log output.
    #[arg(short = 'q', long = "quiet", help_heading = "Render input and output")]
    pub(crate) quiet: bool,

    #[cfg(any(feature = "png", feature = "jpeg"))]
    #[command(flatten)]
    pub(crate) raster: RasterCliArgs,

    #[cfg(feature = "pdf")]
    #[command(flatten)]
    pub(crate) pdf: PdfCliArgs,

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[command(flatten)]
    pub(crate) embedded_images: EmbeddedImageCliArgs,

    #[cfg(feature = "icons")]
    #[command(flatten)]
    pub(crate) icons: NativeIconCliArgs,

    #[command(flatten)]
    pub(crate) parse: ParseCliArgs,

    #[command(flatten)]
    pub(crate) render: RenderCliArgs,

    #[cfg(feature = "ascii")]
    #[command(flatten)]
    pub(crate) text: TextOutputCliArgs,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct RasterCliArgs {
    /// PNG/JPG scale factor. Defaults to 1.
    #[arg(
        short = 's',
        long = "scale",
        value_parser = parse_positive_f32,
        help_heading = "mmdc-compatible export"
    )]
    pub(crate) scale: Option<f32>,

    /// Fit PNG/JPG raster output to this CSS-pixel width before applying --scale.
    #[arg(
        long = "raster-fit-width",
        value_parser = parse_positive_u32,
        help_heading = "Merman raster controls"
    )]
    pub(crate) raster_fit_width: Option<u32>,

    /// Fit PNG/JPG raster output to this CSS-pixel height before applying --scale.
    #[arg(
        long = "raster-fit-height",
        value_parser = parse_positive_u32,
        help_heading = "Merman raster controls"
    )]
    pub(crate) raster_fit_height: Option<u32>,

    /// Maximum PNG/JPG output width after scale and fit. Defaults to 4096.
    #[arg(
        long = "raster-max-width",
        value_parser = parse_positive_u32,
        help_heading = "Merman raster controls"
    )]
    pub(crate) raster_max_width: Option<u32>,

    /// Maximum PNG/JPG output height after scale and fit. Defaults to 4096.
    #[arg(
        long = "raster-max-height",
        value_parser = parse_positive_u32,
        help_heading = "Merman raster controls"
    )]
    pub(crate) raster_max_height: Option<u32>,

    /// Maximum PNG/JPG output pixels after scale and fit. Defaults to 4096*4096.
    #[arg(
        long = "raster-max-pixels",
        value_parser = parse_positive_u64,
        help_heading = "Merman raster controls"
    )]
    pub(crate) raster_max_pixels: Option<u64>,

    /// Disable PNG/JPG raster size limits. Use only for trusted oversized exports.
    #[arg(
        long = "raster-unbounded",
        conflicts_with_all = ["raster_max_width", "raster_max_height", "raster_max_pixels"],
        help_heading = "Merman raster controls"
    )]
    pub(crate) raster_unbounded: bool,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct PdfCliArgs {
    /// Sampling scale for SVG filters that require localized PDF bitmaps. Defaults to 4.
    #[arg(
        long = "pdf-filter-scale",
        value_parser = parse_positive_f32,
        help_heading = "Merman PDF controls"
    )]
    pub(crate) filter_scale: Option<f32>,

    /// Maximum aggregate pixels retained as localized PDF filter images. Defaults to 33554432.
    #[arg(
        long = "pdf-max-filter-image-pixels",
        visible_alias = "pdf-max-filter-pixels",
        value_parser = parse_positive_u64,
        conflicts_with = "filter_images_unbounded",
        help_heading = "Merman PDF controls"
    )]
    pub(crate) max_filter_image_pixels: Option<u64>,

    /// Disable the retained PDF filter-image pixel budget for trusted inputs.
    #[arg(
        long = "pdf-filter-images-unbounded",
        visible_alias = "pdf-filter-unbounded",
        conflicts_with = "max_filter_image_pixels",
        help_heading = "Merman PDF controls"
    )]
    pub(crate) filter_images_unbounded: bool,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct EmbeddedImageCliArgs {
    /// Maximum decoded data-URL bytes for one embedded image. Defaults to 16777216.
    #[arg(
        long = "embedded-image-max-bytes",
        value_parser = parse_positive_u64,
        conflicts_with = "embedded_images_unbounded",
        help_heading = "Merman embedded-image controls"
    )]
    pub(crate) max_image_bytes: Option<u64>,

    /// Maximum aggregate decoded data-URL bytes for embedded images. Defaults to 33554432.
    #[arg(
        long = "embedded-image-max-total-bytes",
        value_parser = parse_positive_u64,
        conflicts_with = "embedded_images_unbounded",
        help_heading = "Merman embedded-image controls"
    )]
    pub(crate) max_total_bytes: Option<u64>,

    /// Maximum intrinsic pixels for one embedded raster image. Defaults to 16777216.
    #[arg(
        long = "embedded-image-max-pixels",
        value_parser = parse_positive_u64,
        conflicts_with = "embedded_images_unbounded",
        help_heading = "Merman embedded-image controls"
    )]
    pub(crate) max_image_pixels: Option<u64>,

    /// Maximum aggregate intrinsic pixels for embedded raster images. Defaults to 33554432.
    #[arg(
        long = "embedded-image-max-total-pixels",
        value_parser = parse_positive_u64,
        conflicts_with = "embedded_images_unbounded",
        help_heading = "Merman embedded-image controls"
    )]
    pub(crate) max_total_pixels: Option<u64>,

    /// Disable embedded raster image decode budgets for trusted inputs.
    #[arg(
        long = "embedded-images-unbounded",
        conflicts_with_all = [
            "max_image_bytes",
            "max_total_bytes",
            "max_image_pixels",
            "max_total_pixels"
        ],
        help_heading = "Merman embedded-image controls"
    )]
    pub(crate) embedded_images_unbounded: bool,
}

#[cfg(feature = "icons")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct MmdcIconCliArgs {
    #[cfg(feature = "network-icons")]
    /// Allow icon pack loading from HTTP(S) URLs.
    #[arg(long = "allow-network", help_heading = "Icon packs")]
    pub(crate) allow_network: bool,

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

#[cfg(feature = "icons")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct NativeIconCliArgs {
    #[cfg(feature = "network-icons")]
    /// Allow icon pack loading from public HTTP(S) destinations.
    #[arg(long = "allow-network", help_heading = "Icon packs")]
    pub(crate) allow_network: bool,

    /// Iconify package name or local package path. Can be repeated.
    #[arg(
        long = "icon-pack",
        value_name = "PACKAGE_OR_PATH",
        action = clap::ArgAction::Append,
        help_heading = "Icon packs"
    )]
    pub(crate) icon_packs: Vec<String>,

    /// Iconify prefix and source as PREFIX#SOURCE. Can be repeated.
    #[arg(
        long = "icon-pack-source",
        value_name = "PREFIX#SOURCE",
        action = clap::ArgAction::Append,
        help_heading = "Icon packs"
    )]
    pub(crate) icon_packs_names_and_urls: Vec<String>,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, ClapArgs, Default)]
pub(crate) struct TextOutputCliArgs {
    /// Mirror sequence participants below lifelines for ASCII/Unicode output.
    #[arg(long = "sequence-mirror-actors", help_heading = "Text output")]
    pub(crate) sequence_mirror_actors: bool,

    /// Override the text renderer character set.
    #[arg(long = "ascii-charset", value_enum, help_heading = "Text output")]
    pub(crate) ascii_charset: Option<TextCharset>,

    /// Override the default graph direction when Mermaid input omits one.
    #[arg(long = "ascii-direction", value_enum, help_heading = "Text output")]
    pub(crate) ascii_direction: Option<TextDirection>,

    /// Color mode for terminal text output.
    #[arg(long = "ascii-color", value_enum, help_heading = "Text output")]
    pub(crate) ascii_color: Option<TextColorMode>,

    /// XYChart vertical plot height for text output.
    #[arg(
        long = "xychart-vertical-plot-height",
        value_parser = parse_positive_usize,
        help_heading = "Text output"
    )]
    pub(crate) xychart_vertical_plot_height: Option<usize>,

    /// XYChart category band width for text output.
    #[arg(
        long = "xychart-category-band-width",
        value_parser = parse_positive_usize,
        help_heading = "Text output"
    )]
    pub(crate) xychart_category_band_width: Option<usize>,

    /// XYChart horizontal plot width for text output.
    #[arg(
        long = "xychart-horizontal-plot-width",
        value_parser = parse_positive_usize,
        help_heading = "Text output"
    )]
    pub(crate) xychart_horizontal_plot_width: Option<usize>,

    /// Maximum graph grid cells for text route planning.
    #[arg(
        long = "ascii-max-grid-cells",
        value_parser = parse_positive_usize,
        help_heading = "Text output"
    )]
    pub(crate) ascii_max_grid_cells: Option<usize>,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum TextCharset {
    Ascii,
    Unicode,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum TextDirection {
    LeftRight,
    TopDown,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum TextColorMode {
    Plain,
    Auto,
    Ansi16,
    Ansi256,
    Truecolor,
    Html,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum TextMeasurerKind {
    Deterministic,
    #[default]
    Vendored,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub(crate) enum MathRendererKind {
    #[default]
    None,
    #[cfg(feature = "math")]
    Ratex,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum SvgPipelineKind {
    Parity,
    Readable,
    #[value(name = "resvg-safe", alias = "resvg_safe")]
    ResvgSafe,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
pub(crate) type ResourceProfile = merman::resources::ResourceProfile;

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resource_profile_value_parser() -> impl TypedValueParser<Value = ResourceProfile> {
    PossibleValuesParser::new(
        merman::resources::RESOURCE_PROFILE_DESCRIPTORS
            .iter()
            .map(|descriptor| PossibleValue::new(descriptor.id).help(descriptor.purpose)),
    )
    .map(|id| {
        ResourceProfile::from_id(&id).expect("possible values come from the resource descriptor")
    })
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum RenderFormat {
    #[cfg(feature = "svg")]
    #[default]
    Svg,
    #[cfg(feature = "ascii")]
    #[cfg_attr(not(feature = "svg"), default)]
    Ascii,
    #[cfg(feature = "ascii")]
    Unicode,
    #[cfg(feature = "png")]
    Png,
    #[cfg(feature = "jpeg")]
    #[value(name = "jpg", alias = "jpeg")]
    Jpeg,
    #[cfg(feature = "pdf")]
    Pdf,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum MmdcOutputFormat {
    #[default]
    Svg,
    #[cfg(feature = "png")]
    Png,
    #[cfg(feature = "pdf")]
    Pdf,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RenderInputKind {
    Mermaid,
    Svg,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl RenderFormat {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            #[cfg(feature = "svg")]
            RenderFormat::Svg => "svg",
            #[cfg(feature = "ascii")]
            RenderFormat::Ascii | RenderFormat::Unicode => "txt",
            #[cfg(feature = "png")]
            RenderFormat::Png => "png",
            #[cfg(feature = "jpeg")]
            RenderFormat::Jpeg => "jpg",
            #[cfg(feature = "pdf")]
            RenderFormat::Pdf => "pdf",
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn requires_svg_encoding(self) -> bool {
        match self {
            #[cfg(feature = "png")]
            Self::Png => true,
            #[cfg(feature = "jpeg")]
            Self::Jpeg => true,
            #[cfg(feature = "pdf")]
            Self::Pdf => true,
            #[cfg(feature = "svg")]
            Self::Svg => false,
            #[cfg(feature = "ascii")]
            Self::Ascii | Self::Unicode => false,
        }
    }

    #[cfg(feature = "ascii")]
    pub(crate) fn is_text(self) -> bool {
        match self {
            Self::Ascii | Self::Unicode => true,
            #[cfg(feature = "svg")]
            Self::Svg => false,
            #[cfg(feature = "png")]
            Self::Png => false,
            #[cfg(feature = "jpeg")]
            Self::Jpeg => false,
            #[cfg(feature = "pdf")]
            Self::Pdf => false,
        }
    }
}

#[cfg(any(feature = "analysis", feature = "ascii", feature = "parallel-markdown"))]
fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "expected a positive integer".to_string())?;
    if parsed == 0 {
        return Err("expected a positive integer".to_string());
    }
    Ok(parsed)
}

#[cfg(feature = "analysis")]
fn parse_lint_rule_severity_override(value: &str) -> Result<LintRuleSeverityOverride, String> {
    let Some((rule_id, severity)) = value.split_once('=') else {
        return Err("expected RULE_ID=SEVERITY".to_string());
    };
    if rule_id.trim().is_empty() {
        return Err("rule id must not be empty".to_string());
    }
    if configurable_rule_descriptor(rule_id).is_none() {
        return Err(format!(
            "unknown or non-configurable lint rule id `{rule_id}`"
        ));
    }

    Ok(LintRuleSeverityOverride {
        rule_id: rule_id.to_string(),
        severity: parse_lint_severity(severity.trim())?,
    })
}

#[cfg(feature = "analysis")]
fn parse_lint_rule_id(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("rule id must not be empty".to_string());
    }
    if configurable_rule_descriptor(value).is_none() {
        return Err(format!(
            "unknown or non-configurable lint rule id `{value}`"
        ));
    }
    Ok(value.to_string())
}

#[cfg(feature = "analysis")]
fn parse_lint_profile(value: &str) -> Result<AnalysisRuleProfile, String> {
    match value.to_ascii_lowercase().as_str() {
        "core" => Ok(AnalysisRuleProfile::Core),
        "recommended" => Ok(AnalysisRuleProfile::Recommended),
        "strict" => Ok(AnalysisRuleProfile::Strict),
        _ => Err("expected profile core, recommended, or strict".to_string()),
    }
}

#[cfg(feature = "analysis")]
fn parse_lint_severity(value: &str) -> Result<DiagnosticSeverity, String> {
    match value.to_ascii_lowercase().as_str() {
        "error" => Ok(DiagnosticSeverity::Error),
        "warning" | "warn" => Ok(DiagnosticSeverity::Warning),
        "info" => Ok(DiagnosticSeverity::Info),
        "hint" => Ok(DiagnosticSeverity::Hint),
        _ => Err("expected severity error, warning, info, or hint".to_string()),
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_positive_u32(value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| "expected a positive integer".to_string())?;
    if parsed == 0 {
        return Err("expected a positive integer".to_string());
    }
    Ok(parsed)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn parse_positive_u64(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "expected a positive integer".to_string())?;
    if parsed == 0 {
        return Err("expected a positive integer".to_string());
    }
    Ok(parsed)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn parse_positive_f32(value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|_| "expected a positive number".to_string())?;
    if !(parsed.is_finite() && parsed > 0.0) {
        return Err("expected a positive number".to_string());
    }
    Ok(parsed)
}

#[cfg(feature = "svg")]
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

#[cfg(all(
    test,
    any(
        feature = "pdf",
        all(feature = "parallel-markdown", any(feature = "png", feature = "jpeg"))
    )
))]
mod tests {
    use super::*;

    #[cfg(feature = "pdf")]
    #[test]
    fn documented_pdf_budget_flags_are_accepted() {
        for args in [
            [
                "merman-cli",
                "render",
                "-",
                "--format",
                "pdf",
                "--pdf-max-filter-pixels",
                "1",
            ]
            .as_slice(),
            [
                "merman-cli",
                "render",
                "-",
                "--format",
                "pdf",
                "--pdf-filter-unbounded",
            ]
            .as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(args).is_ok(),
                "documented PDF flag must parse: {args:?}"
            );
        }
    }

    #[cfg(all(feature = "parallel-markdown", any(feature = "png", feature = "jpeg")))]
    #[test]
    fn documented_encoding_budget_flag_is_accepted() {
        #[cfg(feature = "png")]
        let format = "png";
        #[cfg(all(not(feature = "png"), feature = "jpeg"))]
        let format = "jpg";
        let args = [
            "merman-cli",
            "batch",
            "input.md",
            "--format",
            format,
            "--encoding-memory-budget-mib",
            "1",
        ];
        assert!(
            Cli::try_parse_from(args).is_ok(),
            "documented encoding budget flag must parse"
        );
    }
}

#[cfg(all(
    test,
    any(
        all(
            feature = "analysis",
            not(feature = "svg"),
            not(feature = "ascii"),
            not(feature = "shell-completions")
        ),
        all(
            feature = "svg",
            not(feature = "analysis"),
            not(feature = "ascii"),
            not(feature = "png"),
            not(feature = "jpeg"),
            not(feature = "pdf"),
            not(feature = "layout-cytoscape"),
            not(feature = "layout-elk"),
            not(feature = "math"),
            not(feature = "icons"),
            not(feature = "network-icons"),
            not(feature = "markdown"),
            not(feature = "parallel-markdown"),
            not(feature = "shell-completions")
        ),
        all(
            feature = "svg",
            feature = "icons",
            not(feature = "analysis"),
            not(feature = "ascii"),
            not(feature = "png"),
            not(feature = "jpeg"),
            not(feature = "pdf"),
            not(feature = "layout-cytoscape"),
            not(feature = "layout-elk"),
            not(feature = "math"),
            not(feature = "network-icons"),
            not(feature = "markdown"),
            not(feature = "parallel-markdown"),
            not(feature = "shell-completions")
        ),
        all(
            feature = "svg",
            feature = "markdown",
            not(feature = "analysis"),
            not(feature = "ascii"),
            not(feature = "png"),
            not(feature = "jpeg"),
            not(feature = "pdf"),
            not(feature = "layout-cytoscape"),
            not(feature = "layout-elk"),
            not(feature = "math"),
            not(feature = "icons"),
            not(feature = "network-icons"),
            not(feature = "parallel-markdown"),
            not(feature = "shell-completions")
        ),
        all(
            feature = "ascii",
            not(feature = "svg"),
            not(feature = "analysis"),
            not(feature = "shell-completions")
        )
    )
))]
mod profile_tests {
    use super::*;
    use clap::Parser;

    #[cfg(all(
        feature = "analysis",
        not(feature = "svg"),
        not(feature = "ascii"),
        not(feature = "shell-completions")
    ))]
    #[test]
    fn lint_only_profile_has_no_render_or_tool_commands() {
        for accepted in [
            ["merman-cli", "capabilities"].as_slice(),
            ["merman-cli", "detect", "-"].as_slice(),
            ["merman-cli", "parse", "-"].as_slice(),
            ["merman-cli", "lint", "-"].as_slice(),
            ["merman-cli", "fix", "-"].as_slice(),
            ["merman-cli", "lint-rules"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(accepted).is_ok(),
                "expected CLI to accept {accepted:?}"
            );
        }

        for rejected in [
            ["merman-cli", "render"].as_slice(),
            ["merman-cli", "layout"].as_slice(),
            ["merman-cli", "completion"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(rejected).is_err(),
                "lint-only CLI must not expose {rejected:?}"
            );
        }

        let help = <Cli as clap::CommandFactory>::command()
            .render_long_help()
            .to_string();
        for omitted in [
            "\n  render ",
            "\n  layout ",
            "\n  completion ",
            "--output",
            "--format",
        ] {
            assert!(
                !help.contains(omitted),
                "lint-only help must not expose {omitted}:\n{help}"
            );
        }
    }

    #[cfg(all(
        feature = "svg",
        not(feature = "analysis"),
        not(feature = "ascii"),
        not(feature = "png"),
        not(feature = "jpeg"),
        not(feature = "pdf"),
        not(feature = "layout-cytoscape"),
        not(feature = "layout-elk"),
        not(feature = "math"),
        not(feature = "icons"),
        not(feature = "network-icons"),
        not(feature = "markdown"),
        not(feature = "parallel-markdown"),
        not(feature = "shell-completions")
    ))]
    #[test]
    fn svg_basic_profile_hides_uncompiled_output_and_tool_options() {
        assert!(Cli::try_parse_from(["merman-cli", "render", "--format", "svg", "-"]).is_ok());

        for rejected in [
            ["merman-cli", "render", "--format", "png", "-"].as_slice(),
            ["merman-cli", "--artefacts", "images"].as_slice(),
            ["merman-cli", "--pdfFit"].as_slice(),
            ["merman-cli", "--jobs", "2"].as_slice(),
            ["merman-cli", "--allow-network"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(rejected).is_err(),
                "SVG-basic CLI must not expose {rejected:?}"
            );
        }

        let help = <Cli as clap::CommandFactory>::command()
            .render_long_help()
            .to_string();
        for omitted in [
            "Markdown batch export:",
            "--artefacts",
            "--pdfFit",
            "--jobs",
            "--allow-network",
            "--raster-max-width",
        ] {
            assert!(
                !help.contains(omitted),
                "SVG-basic help must not expose {omitted}:\n{help}"
            );
        }
    }

    #[cfg(all(
        feature = "svg",
        feature = "icons",
        not(feature = "analysis"),
        not(feature = "ascii"),
        not(feature = "png"),
        not(feature = "jpeg"),
        not(feature = "pdf"),
        not(feature = "layout-cytoscape"),
        not(feature = "layout-elk"),
        not(feature = "math"),
        not(feature = "network-icons"),
        not(feature = "markdown"),
        not(feature = "parallel-markdown"),
        not(feature = "shell-completions")
    ))]
    #[test]
    fn local_icons_profile_exposes_local_sources_without_network_controls() {
        for accepted in [
            [
                "merman-cli",
                "render",
                "-",
                "--icon-pack",
                "@iconify-json/logos",
            ]
            .as_slice(),
            [
                "merman-cli",
                "render",
                "-",
                "--icon-pack-source",
                "logos#icons.json",
            ]
            .as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(accepted).is_ok(),
                "local-icons CLI must accept {accepted:?}"
            );
        }

        assert!(
            Cli::try_parse_from(["merman-cli", "--allow-network"]).is_err(),
            "local-icons CLI must not expose network authorization"
        );

        let help = <Cli as clap::CommandFactory>::command()
            .find_subcommand("render")
            .expect("render command")
            .clone()
            .render_long_help()
            .to_string();
        for expected in ["--icon-pack", "--icon-pack-source"] {
            assert!(
                help.contains(expected),
                "local-icons help must expose {expected}:\n{help}"
            );
        }
        assert!(
            !help.contains("--allow-network"),
            "local-icons help must not expose network authorization:\n{help}"
        );
    }

    #[cfg(all(
        feature = "svg",
        feature = "markdown",
        not(feature = "analysis"),
        not(feature = "ascii"),
        not(feature = "png"),
        not(feature = "jpeg"),
        not(feature = "pdf"),
        not(feature = "layout-cytoscape"),
        not(feature = "layout-elk"),
        not(feature = "math"),
        not(feature = "icons"),
        not(feature = "network-icons"),
        not(feature = "parallel-markdown"),
        not(feature = "shell-completions")
    ))]
    #[test]
    fn markdown_profile_converts_documents_without_analysis_or_parallel_controls() {
        assert!(
            Cli::try_parse_from([
                "merman-cli",
                "mmdc",
                "-i",
                "input.md",
                "-o",
                "output.md",
                "--artefacts",
                "images",
            ])
            .is_ok(),
            "Markdown conversion must be available without analysis"
        );

        for rejected in [
            ["merman-cli", "lint", "-"].as_slice(),
            ["merman-cli", "--jobs", "2"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(rejected).is_err(),
                "markdown-only CLI must not expose {rejected:?}"
            );
        }

        let help = <Cli as clap::CommandFactory>::command()
            .find_subcommand("mmdc")
            .expect("mmdc command")
            .clone()
            .render_long_help()
            .to_string();
        assert!(
            help.contains("Markdown batch export:") && help.contains("--artefacts"),
            "markdown-only help must expose conversion options:\n{help}"
        );
        for omitted in ["\n  lint ", "--jobs"] {
            assert!(
                !help.contains(omitted),
                "markdown-only help must not expose {omitted}:\n{help}"
            );
        }
    }

    #[cfg(all(
        feature = "ascii",
        not(feature = "svg"),
        not(feature = "analysis"),
        not(feature = "shell-completions")
    ))]
    #[test]
    fn ascii_only_profile_has_only_text_rendering() {
        assert!(Cli::try_parse_from(["merman-cli", "render", "--format", "ascii", "-"]).is_ok());

        for rejected in [
            ["merman-cli", "layout"].as_slice(),
            ["merman-cli", "render", "--format", "svg", "-"].as_slice(),
            ["merman-cli", "render", "--svg-pipeline", "parity", "-"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(rejected).is_err(),
                "ASCII-only CLI must not expose {rejected:?}"
            );
        }
    }
}
