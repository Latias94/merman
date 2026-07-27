#[cfg(feature = "markdown")]
use crate::cli::BatchArgs;
#[cfg(feature = "shell-completions")]
use crate::cli::CompletionArgs;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::cli::EmbeddedImageCliArgs;
#[cfg(feature = "pdf")]
use crate::cli::PdfCliArgs;
#[cfg(any(feature = "png", feature = "jpeg"))]
use crate::cli::RasterCliArgs;
#[cfg(feature = "svg")]
use crate::cli::RenderInputKind;
#[cfg(any(
    feature = "svg",
    feature = "ascii",
    all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    )
))]
use crate::cli::RuntimePolicyKind;
#[cfg(feature = "ascii")]
use crate::cli::TextOutputCliArgs;
use crate::cli::{
    CapabilitiesArgs, Cli, DetectArgs, ParseArgs, ParseCliArgs, RawCommand, RuntimeCliArgs,
};
#[cfg(feature = "analysis")]
use crate::cli::{FixArgs, LintArgs, LintRulesArgs};
#[cfg(feature = "svg")]
use crate::cli::{LayoutArgs, MmdcArgs, MmdcOutputFormat};
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::cli::{NativeRenderOptions, RenderArgs, RenderCliArgs, RenderFormat};
use crate::error::CliError;
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::ffi::OsStr;
#[cfg(any(feature = "analysis", feature = "svg", feature = "ascii"))]
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct InvocationFacts {
    pub(crate) cwd: PathBuf,
    pub(crate) stdin_is_terminal: bool,
    pub(crate) stdout_is_terminal: bool,
    pub(crate) color: ColorEnvironment,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ColorEnvironment {
    pub(crate) no_color: bool,
    pub(crate) force_color: bool,
    pub(crate) colorterm: Option<String>,
    pub(crate) term: Option<String>,
}

impl ColorEnvironment {
    fn is_color_capable(&self) -> bool {
        !self.no_color
            && (self.force_color
                || self
                    .colorterm
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
                || self
                    .term
                    .as_deref()
                    .is_some_and(|value| !value.is_empty() && value != "dumb"))
    }
}

#[derive(Debug)]
pub(crate) enum ResolvedInvocation {
    Capabilities(CapabilitiesArgs),
    Detect(DetectArgs),
    Parse(ParseArgs),
    #[cfg(feature = "svg")]
    Layout(LayoutArgs),
    #[cfg(feature = "analysis")]
    Lint(LintArgs),
    #[cfg(feature = "analysis")]
    Fix(FixArgs),
    #[cfg(feature = "analysis")]
    LintRules(LintRulesArgs),
    #[cfg(any(feature = "svg", feature = "ascii"))]
    Render(ResolvedSingleRender),
    #[cfg(feature = "markdown")]
    Batch(ResolvedBatchRender),
    #[cfg(feature = "svg")]
    Mmdc(ResolvedMmdcRender),
    #[cfg(feature = "shell-completions")]
    Completion(CompletionArgs),
}

#[cfg(all(feature = "svg", feature = "markdown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedWorkflow {
    Single,
    #[cfg(feature = "markdown")]
    MarkdownBatch,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedInput {
    File(PathBuf),
    Stdin,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ResolvedInput {
    pub(crate) fn to_path_buf(&self) -> PathBuf {
        match self {
            Self::File(path) => path.clone(),
            Self::Stdin => PathBuf::from("-"),
        }
    }

    pub(crate) fn file(&self) -> Option<&Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Stdin => None,
        }
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedDestination {
    Stdout,
    File(PathBuf),
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
pub(crate) enum ResolvedOutput {
    #[cfg(feature = "svg")]
    Svg {
        destination: ResolvedDestination,
        pipeline: Option<crate::cli::SvgPipelineKind>,
    },
    #[cfg(feature = "ascii")]
    Text {
        format: RenderFormat,
        destination: ResolvedDestination,
        options: ResolvedTextOutputOptions,
    },
    #[cfg(feature = "png")]
    Png {
        destination: ResolvedDestination,
        raster: ResolvedRasterOptions,
        embedded_images: ResolvedEmbeddedImageOptions,
    },
    #[cfg(feature = "jpeg")]
    Jpeg {
        destination: ResolvedDestination,
        raster: ResolvedRasterOptions,
        embedded_images: ResolvedEmbeddedImageOptions,
    },
    #[cfg(feature = "pdf")]
    Pdf {
        destination: ResolvedDestination,
        options: ResolvedPdfOptions,
        embedded_images: ResolvedEmbeddedImageOptions,
        fit: bool,
    },
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedRasterOptions {
    pub(crate) scale: f32,
    pub(crate) fit_width: Option<u32>,
    pub(crate) fit_height: Option<u32>,
    pub(crate) max_width: Option<u32>,
    pub(crate) max_height: Option<u32>,
    pub(crate) max_pixels: Option<u64>,
    pub(crate) unbounded: bool,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl Default for ResolvedRasterOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            fit_width: None,
            fit_height: None,
            max_width: None,
            max_height: None,
            max_pixels: None,
            unbounded: false,
        }
    }
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ResolvedPdfOptions {
    pub(crate) filter_scale: Option<f32>,
    pub(crate) max_filter_image_pixels: Option<u64>,
    pub(crate) filter_images_unbounded: bool,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ResolvedEmbeddedImageOptions {
    pub(crate) max_bytes_per_image: Option<u64>,
    pub(crate) max_total_bytes: Option<u64>,
    pub(crate) max_pixels_per_image: Option<u64>,
    pub(crate) max_total_pixels: Option<u64>,
    pub(crate) unbounded: bool,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug)]
pub(crate) struct ResolvedRenderCommon {
    pub(crate) parse: ResolvedParseOptions,
    pub(crate) render: ResolvedRenderOptions,
    #[cfg(feature = "svg")]
    pub(crate) background: Option<String>,
    #[cfg(feature = "svg")]
    pub(crate) css_file: Option<PathBuf>,
    pub(crate) quiet: bool,
    #[cfg(feature = "icons")]
    pub(crate) icons: ResolvedIconSources,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
pub(crate) struct ResolvedParseOptions {
    pub(crate) suppress_errors: bool,
    pub(crate) config_file: Option<PathBuf>,
    pub(crate) theme: Option<String>,
    pub(crate) runtime: ResolvedRuntimeOptions,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
pub(crate) struct ResolvedRuntimeOptions {
    pub(crate) policy: RuntimePolicyKind,
    #[cfg(feature = "system-clock")]
    pub(crate) system_clock: bool,
    #[cfg(feature = "system-timezone")]
    pub(crate) system_timezone: bool,
    #[cfg(feature = "system-random")]
    pub(crate) system_random: bool,
    #[cfg(feature = "system-timing")]
    pub(crate) system_timing: bool,
    pub(crate) fixed_today: Option<chrono::NaiveDate>,
    pub(crate) fixed_local_offset_minutes: Option<i32>,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug, Clone)]
pub(crate) struct ResolvedRenderOptions {
    #[cfg(feature = "svg")]
    pub(crate) text_measurer: crate::cli::TextMeasurerKind,
    #[cfg(feature = "svg")]
    pub(crate) math_renderer: Option<crate::cli::MathRendererKind>,
    #[cfg(feature = "svg")]
    pub(crate) container_width: Option<f64>,
    #[cfg(feature = "svg")]
    pub(crate) container_height: Option<f64>,
    #[cfg(feature = "svg")]
    pub(crate) svg_id: Option<String>,
    #[cfg(feature = "svg")]
    pub(crate) hand_drawn_seed: Option<u64>,
    pub(crate) resource_profile: crate::cli::ResourceProfile,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedTextOutputOptions {
    pub(crate) sequence_mirror_actors: bool,
    pub(crate) ascii_charset: Option<crate::cli::TextCharset>,
    pub(crate) ascii_direction: Option<crate::cli::TextDirection>,
    pub(crate) ascii_color: Option<crate::cli::TextColorMode>,
    pub(crate) xychart_vertical_plot_height: Option<usize>,
    pub(crate) xychart_category_band_width: Option<usize>,
    pub(crate) xychart_horizontal_plot_width: Option<usize>,
    pub(crate) ascii_max_grid_cells: Option<usize>,
}

#[cfg(feature = "icons")]
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedIconSources {
    pub(crate) packages: Vec<String>,
    pub(crate) named_sources: Vec<String>,
    #[cfg(feature = "network-icons")]
    pub(crate) allow_network: bool,
}

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug)]
pub(crate) struct ResolvedSingleRender {
    pub(crate) input: ResolvedInput,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(crate) input_kind: RenderInputKind,
    pub(crate) output: ResolvedOutput,
    pub(crate) common: ResolvedRenderCommon,
}

#[cfg(feature = "markdown")]
#[derive(Debug)]
pub(crate) struct ResolvedBatchRender {
    pub(crate) input: ResolvedInput,
    pub(crate) output_root: PathBuf,
    pub(crate) output: ResolvedOutput,
    pub(crate) common: ResolvedRenderCommon,
    #[cfg(feature = "parallel-markdown")]
    pub(crate) jobs: usize,
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    pub(crate) encoding_parallel_budget_bytes: u64,
}

#[cfg(feature = "svg")]
#[derive(Debug)]
pub(crate) struct ResolvedMmdcRender {
    #[cfg(feature = "markdown")]
    pub(crate) workflow: ResolvedWorkflow,
    pub(crate) input: ResolvedInput,
    pub(crate) output: ResolvedOutput,
    pub(crate) common: ResolvedRenderCommon,
    pub(crate) compatibility: MmdcCompatibilityInputs,
    pub(crate) input_was_explicit: bool,
    pub(crate) warn_on_implicit_stdin: bool,
    #[cfg(feature = "parallel-markdown")]
    pub(crate) jobs: usize,
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    pub(crate) encoding_parallel_budget_bytes: u64,
}

#[cfg(feature = "svg")]
#[derive(Debug)]
pub(crate) struct MmdcCompatibilityInputs {
    pub(crate) puppeteer_config_file: Option<PathBuf>,
    #[cfg(feature = "markdown")]
    pub(crate) artefacts: Option<PathBuf>,
}

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
const DEFAULT_MARKDOWN_ENCODING_PARALLEL_BUDGET_MIB: u64 = 512;
#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
const MIB: u64 = 1024 * 1024;

pub(crate) fn resolve(cli: Cli, facts: &InvocationFacts) -> Result<ResolvedInvocation, CliError> {
    let _ = (
        &facts.cwd,
        facts.stdout_is_terminal,
        facts.color.is_color_capable(),
    );
    match cli.command {
        RawCommand::Capabilities(args) => Ok(ResolvedInvocation::Capabilities(args)),
        RawCommand::Detect(mut args) => {
            validate_runtime_args(&args.engine.runtime)?;
            normalize_raw_native_input(&mut args.input, facts.stdin_is_terminal, "detect", false)?;
            Ok(ResolvedInvocation::Detect(args))
        }
        RawCommand::Parse(mut args) => {
            validate_parse_args(&args.parse)?;
            normalize_raw_native_input(&mut args.input, facts.stdin_is_terminal, "parse", false)?;
            Ok(ResolvedInvocation::Parse(args))
        }
        #[cfg(feature = "svg")]
        RawCommand::Layout(mut args) => {
            validate_parse_args(&args.parse)?;
            normalize_raw_native_input(&mut args.input, facts.stdin_is_terminal, "layout", false)?;
            Ok(ResolvedInvocation::Layout(args))
        }
        #[cfg(feature = "analysis")]
        RawCommand::Lint(mut args) => {
            validate_analysis_args(&args.analysis)?;
            validate_analysis_output(args.format, args.pretty)?;
            normalize_raw_native_input(
                &mut args.input,
                facts.stdin_is_terminal,
                "lint",
                args.stdin_file_name.is_some(),
            )?;
            validate_stdin_file_name(args.input.as_deref(), args.stdin_file_name.as_deref())?;
            Ok(ResolvedInvocation::Lint(args))
        }
        #[cfg(feature = "analysis")]
        RawCommand::Fix(mut args) => {
            validate_analysis_args(&args.analysis)?;
            normalize_raw_native_input(
                &mut args.input,
                facts.stdin_is_terminal,
                "fix",
                args.stdin_file_name.is_some(),
            )?;
            validate_stdin_file_name(args.input.as_deref(), args.stdin_file_name.as_deref())?;
            Ok(ResolvedInvocation::Fix(args))
        }
        #[cfg(feature = "analysis")]
        RawCommand::LintRules(args) => {
            validate_analysis_output(args.format, args.pretty)?;
            Ok(ResolvedInvocation::LintRules(args))
        }
        #[cfg(any(feature = "svg", feature = "ascii"))]
        RawCommand::Render(args) => normalize_render(args, facts).map(ResolvedInvocation::Render),
        #[cfg(feature = "markdown")]
        RawCommand::Batch(args) => normalize_batch(args, facts).map(ResolvedInvocation::Batch),
        #[cfg(feature = "svg")]
        RawCommand::Mmdc(args) => normalize_mmdc(args).map(ResolvedInvocation::Mmdc),
        #[cfg(feature = "shell-completions")]
        RawCommand::Completion(args) => Ok(ResolvedInvocation::Completion(args)),
    }
}

fn validate_parse_args(args: &ParseCliArgs) -> Result<(), CliError> {
    validate_runtime_args(&args.runtime)
}

fn normalize_raw_native_input(
    input: &mut Option<PathBuf>,
    stdin_is_terminal: bool,
    command: &'static str,
    stdin_was_selected_by_option: bool,
) -> Result<(), CliError> {
    if input.is_none() {
        if stdin_is_terminal && !stdin_was_selected_by_option {
            return Err(CliError::MissingInput { command });
        }
        *input = Some(PathBuf::from("-"));
    }
    Ok(())
}

#[cfg(feature = "analysis")]
fn validate_analysis_output(
    format: crate::cli::LintOutputFormat,
    pretty: bool,
) -> Result<(), CliError> {
    if pretty && matches!(format, crate::cli::LintOutputFormat::Text) {
        return Err(CliError::InvalidInput(
            "--pretty is only valid with --format json".to_string(),
        ));
    }
    Ok(())
}

#[cfg(feature = "analysis")]
fn validate_stdin_file_name(
    input: Option<&Path>,
    stdin_file_name: Option<&Path>,
) -> Result<(), CliError> {
    if stdin_file_name.is_some() && input.is_some_and(|path| path != Path::new("-")) {
        return Err(CliError::InvalidInput(
            "--stdin-file-name is only valid when reading from stdin".to_string(),
        ));
    }
    Ok(())
}

fn validate_runtime_args(args: &RuntimeCliArgs) -> Result<(), CliError> {
    let _ = args;
    #[cfg(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    ))]
    if args.policy == RuntimePolicyKind::Native
        && (args.system_clock || args.system_timezone || args.system_random)
    {
        return Err(CliError::InvalidInput(
            "--runtime native cannot be combined with individual system adapter flags".to_string(),
        ));
    }

    #[cfg(feature = "system-timezone")]
    if args.system_timezone && args.fixed_local_offset_minutes.is_some() {
        return Err(CliError::InvalidInput(
            "--system-timezone cannot be combined with --fixed-local-offset-minutes".to_string(),
        ));
    }
    Ok(())
}

#[cfg(feature = "analysis")]
fn validate_analysis_args(args: &crate::cli::AnalysisCliArgs) -> Result<(), CliError> {
    validate_runtime_args(&args.runtime)?;
    for rule in &args.enable_rules {
        if args.disable_rules.contains(rule) {
            return Err(CliError::InvalidInput(format!(
                "lint rule `{rule}` cannot be both enabled and disabled"
            )));
        }
    }
    Ok(())
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn normalize_render(
    args: RenderArgs,
    facts: &InvocationFacts,
) -> Result<ResolvedSingleRender, CliError> {
    validate_parse_args(&args.options.parse)?;
    let input = resolve_native_input(args.input, facts.stdin_is_terminal, "render")?;
    if input.file().is_some_and(is_native_markdown_path) {
        return Err(CliError::InvalidInput(
            "render accepts one Mermaid diagram, not Markdown; use `merman-cli batch`".to_string(),
        ));
    }
    let format = infer_native_format(args.output.as_deref(), args.options.format)?;
    #[cfg(feature = "svg")]
    let input_kind = resolve_input_kind(args.input_kind, &input);
    #[cfg(feature = "svg")]
    if input_kind == RenderInputKind::Svg && !format.requires_svg_encoding() {
        return Err(CliError::InvalidInput(
            "SVG input requires a compiled PNG, JPEG, or PDF output format".to_string(),
        ));
    }
    #[cfg(feature = "svg")]
    if input_kind == RenderInputKind::Svg {
        validate_raw_svg_options(&args.options)?;
    }
    validate_native_output_options(format, &args.options)?;
    let destination = resolve_single_destination(args.output, &input, format);
    let output = resolved_native_output(format, destination, &args.options)?;
    let common = resolve_native_common(args.options);

    Ok(ResolvedSingleRender {
        input,
        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        input_kind,
        output,
        common,
    })
}

#[cfg(feature = "markdown")]
fn normalize_batch(
    args: BatchArgs,
    facts: &InvocationFacts,
) -> Result<ResolvedBatchRender, CliError> {
    validate_parse_args(&args.options.parse)?;
    let input = resolve_native_input(args.input, facts.stdin_is_terminal, "batch")?;
    if let Some(path) = input.file()
        && !is_native_markdown_path(path)
    {
        return Err(CliError::InvalidInput(format!(
            "batch input must be a Markdown file: {}",
            path.display()
        )));
    }

    let (source_file_name, output_dir) = match input.file() {
        Some(path) => {
            if args.stdin_file_name.is_some() {
                return Err(CliError::InvalidInput(
                    "--stdin-file-name is only valid when batch input is stdin".to_string(),
                ));
            }
            let source_file_name = path.file_name().map(PathBuf::from).ok_or_else(|| {
                CliError::InvalidInput("batch input must have a file name".to_string())
            })?;
            let output_dir = args
                .output_dir
                .unwrap_or_else(|| path.with_extension("merman"));
            (source_file_name, output_dir)
        }
        None => {
            let source_file_name = args.stdin_file_name.ok_or_else(|| {
                CliError::InvalidInput(
                    "batch stdin requires --stdin-file-name and --output-dir".to_string(),
                )
            })?;
            if source_file_name
                .parent()
                .is_some_and(|parent| !parent.as_os_str().is_empty())
            {
                return Err(CliError::InvalidInput(
                    "--stdin-file-name must be a file name without parent components".to_string(),
                ));
            }
            if !is_native_markdown_path(&source_file_name) {
                return Err(CliError::InvalidInput(
                    "--stdin-file-name must end in .md, .markdown, or .mdx".to_string(),
                ));
            }
            let output_dir = args.output_dir.ok_or_else(|| {
                CliError::InvalidInput(
                    "batch stdin requires --stdin-file-name and --output-dir".to_string(),
                )
            })?;
            (source_file_name, output_dir)
        }
    };

    let format = args.options.format.unwrap_or_default();
    #[cfg(feature = "ascii")]
    if format.is_text() {
        return Err(CliError::InvalidInput(
            "batch does not support ASCII or Unicode output".to_string(),
        ));
    }
    validate_native_output_options(format, &args.options)?;
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    if args.encoding_parallel_budget_mib.is_some() && !format.requires_svg_encoding() {
        return Err(CliError::InvalidInput(
            "--encoding-parallel-budget-mib requires PNG, JPEG, or PDF output".to_string(),
        ));
    }
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    let encoding_parallel_budget_bytes =
        resolve_encoding_parallel_budget(args.encoding_parallel_budget_mib)?;
    let output_path = output_dir.join(&source_file_name);
    let output = resolved_native_output(
        format,
        ResolvedDestination::File(output_path),
        &args.options,
    )?;
    let common = resolve_native_common(args.options);

    Ok(ResolvedBatchRender {
        input,
        output_root: output_dir,
        output,
        common,
        #[cfg(feature = "parallel-markdown")]
        jobs: args.jobs.unwrap_or_else(default_jobs),
        #[cfg(all(
            feature = "parallel-markdown",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        encoding_parallel_budget_bytes,
    })
}

#[cfg(feature = "svg")]
fn normalize_mmdc(args: MmdcArgs) -> Result<ResolvedMmdcRender, CliError> {
    let input_was_explicit = args.input_file.is_some();
    let warn_on_implicit_stdin = !input_was_explicit && !args.quiet;
    let parse = ParseCliArgs {
        suppress_errors: false,
        config_file: args.parse.config_file.clone(),
        theme: Some(args.parse.theme.as_str().to_string()),
        runtime: args.parse.runtime.clone(),
    };
    validate_parse_args(&parse)?;
    let render = RenderCliArgs {
        text_measurer: Some(args.render.text_measurer),
        math_renderer: args.render.math_renderer,
        container_width: Some(args.render.container_width),
        container_height: Some(args.render.container_height),
        svg_id: args.render.svg_id.clone(),
        hand_drawn_seed: args.render.hand_drawn_seed,
        resource_profile: args.render.resource_profile,
    };

    let input = match args.input_file.as_deref() {
        Some(path) if path == Path::new("-") => ResolvedInput::Stdin,
        None => ResolvedInput::Stdin,
        Some(path) => ResolvedInput::File(path.to_path_buf()),
    };
    let format = infer_mmdc_format(args.output.as_deref(), args.output_format)?;
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| default_mmdc_output(&input, format));
    validate_mmdc_output_path(&output_path)?;
    let destination = if output_path == Path::new("-") {
        ResolvedDestination::Stdout
    } else {
        ResolvedDestination::File(output_path)
    };
    let markdown_input = input.file().is_some_and(is_strict_markdown_path);
    #[cfg(not(feature = "markdown"))]
    if markdown_input {
        return Err(CliError::InvalidInput(
            "mmdc Markdown input requires the `markdown` feature".to_string(),
        ));
    }
    #[cfg(feature = "markdown")]
    let workflow = if markdown_input {
        ResolvedWorkflow::MarkdownBatch
    } else {
        ResolvedWorkflow::Single
    };
    let markdown_output = match &destination {
        ResolvedDestination::File(path) => is_strict_markdown_path(path),
        _ => false,
    };
    if markdown_output && !markdown_input {
        return Err(CliError::InvalidInput(
            "mmdc Markdown output requires a .md or .markdown input".to_string(),
        ));
    }
    #[cfg(feature = "markdown")]
    if !markdown_input
        && (args.artefacts.is_some() || {
            #[cfg(all(
                feature = "parallel-markdown",
                any(feature = "png", feature = "jpeg", feature = "pdf")
            ))]
            {
                args.encoding_parallel_budget_mib.is_some()
            }
            #[cfg(not(all(
                feature = "parallel-markdown",
                any(feature = "png", feature = "jpeg", feature = "pdf")
            )))]
            {
                false
            }
        })
    {
        return Err(CliError::InvalidInput(
            "Markdown batch options require Markdown input".to_string(),
        ));
    }
    #[cfg(feature = "markdown")]
    if matches!(workflow, ResolvedWorkflow::MarkdownBatch)
        && matches!(destination, ResolvedDestination::Stdout)
    {
        return Err(CliError::InvalidInput(
            "mmdc cannot write Markdown output to stdout".to_string(),
        ));
    }
    validate_mmdc_output_options(format, &args)?;
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    if args.encoding_parallel_budget_mib.is_some() && !format.requires_svg_encoding() {
        return Err(CliError::InvalidInput(
            "--encoding-parallel-budget-mib requires PNG or PDF output".to_string(),
        ));
    }
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    let encoding_parallel_budget_bytes =
        resolve_encoding_parallel_budget(args.encoding_parallel_budget_mib)?;
    let output_is_stdout = matches!(destination, ResolvedDestination::Stdout);
    let output = resolved_mmdc_output(format, destination, &args)?;
    let common = resolve_mmdc_common(parse, render, &args, output_is_stdout);
    let compatibility = MmdcCompatibilityInputs {
        puppeteer_config_file: args.puppeteer_config_file.clone(),
        #[cfg(feature = "markdown")]
        artefacts: args.artefacts.clone(),
    };

    Ok(ResolvedMmdcRender {
        #[cfg(feature = "markdown")]
        workflow,
        input,
        output,
        common,
        compatibility,
        input_was_explicit,
        warn_on_implicit_stdin,
        #[cfg(feature = "parallel-markdown")]
        jobs: args.jobs.unwrap_or_else(default_jobs),
        #[cfg(all(
            feature = "parallel-markdown",
            any(feature = "png", feature = "jpeg", feature = "pdf")
        ))]
        encoding_parallel_budget_bytes,
    })
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_native_input(
    input: Option<PathBuf>,
    stdin_is_terminal: bool,
    command: &'static str,
) -> Result<ResolvedInput, CliError> {
    match input {
        Some(path) if path == Path::new("-") => Ok(ResolvedInput::Stdin),
        Some(path) => Ok(ResolvedInput::File(path)),
        None if stdin_is_terminal => Err(CliError::MissingInput { command }),
        None => Ok(ResolvedInput::Stdin),
    }
}

#[cfg(feature = "svg")]
fn resolve_input_kind(
    requested: Option<RenderInputKind>,
    input: &ResolvedInput,
) -> RenderInputKind {
    requested.unwrap_or_else(|| {
        if input
            .file()
            .and_then(Path::extension)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        {
            #[cfg(feature = "svg")]
            {
                return RenderInputKind::Svg;
            }
        }
        RenderInputKind::Mermaid
    })
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn infer_native_format(
    output: Option<&Path>,
    requested: Option<RenderFormat>,
) -> Result<RenderFormat, CliError> {
    let inferred = output
        .filter(|path| *path != Path::new("-"))
        .map(format_from_path)
        .transpose()?
        .flatten();
    match (requested, inferred) {
        (Some(requested), Some(inferred)) if requested != inferred => {
            Err(CliError::InvalidInput(format!(
                "--format {} conflicts with output extension .{}",
                requested.extension(),
                inferred.extension()
            )))
        }
        (Some(requested), _) => Ok(requested),
        (None, Some(inferred)) => Ok(inferred),
        (None, None) => Ok(RenderFormat::default()),
    }
}

#[cfg(feature = "svg")]
fn infer_mmdc_format(
    output: Option<&Path>,
    requested: Option<MmdcOutputFormat>,
) -> Result<RenderFormat, CliError> {
    if let Some(requested) = requested {
        return Ok(match requested {
            MmdcOutputFormat::Svg => RenderFormat::Svg,
            #[cfg(feature = "png")]
            MmdcOutputFormat::Png => RenderFormat::Png,
            #[cfg(feature = "pdf")]
            MmdcOutputFormat::Pdf => RenderFormat::Pdf,
        });
    }
    let Some(output) = output.filter(|output| *output != Path::new("-")) else {
        return Ok(RenderFormat::Svg);
    };
    let Some(extension) = output.extension().and_then(OsStr::to_str) else {
        return Err(invalid_mmdc_output_extension());
    };
    match extension {
        "md" | "markdown" | "svg" => Ok(RenderFormat::Svg),
        #[cfg(feature = "png")]
        "png" => Ok(RenderFormat::Png),
        #[cfg(feature = "pdf")]
        "pdf" => Ok(RenderFormat::Pdf),
        _ => Err(invalid_mmdc_output_extension()),
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn format_from_path(path: &Path) -> Result<Option<RenderFormat>, CliError> {
    let Some(extension) = path.extension().and_then(OsStr::to_str) else {
        return Ok(None);
    };
    let format = match extension.to_ascii_lowercase().as_str() {
        #[cfg(feature = "svg")]
        "svg" => Some(RenderFormat::Svg),
        #[cfg(feature = "ascii")]
        "txt" | "ascii" => Some(RenderFormat::Ascii),
        #[cfg(feature = "ascii")]
        "unicode" => Some(RenderFormat::Unicode),
        #[cfg(feature = "png")]
        "png" => Some(RenderFormat::Png),
        #[cfg(feature = "jpeg")]
        "jpg" | "jpeg" => Some(RenderFormat::Jpeg),
        #[cfg(feature = "pdf")]
        "pdf" => Some(RenderFormat::Pdf),
        _ => {
            return Err(CliError::InvalidInput(format!(
                "cannot infer a compiled output format from {}",
                path.display()
            )));
        }
    };
    Ok(format)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_single_destination(
    output: Option<PathBuf>,
    input: &ResolvedInput,
    format: RenderFormat,
) -> ResolvedDestination {
    match output {
        Some(path) if path == Path::new("-") => ResolvedDestination::Stdout,
        Some(path) => ResolvedDestination::File(path),
        None => match input.file() {
            Some(path) => ResolvedDestination::File(path.with_extension(format.extension())),
            None => ResolvedDestination::Stdout,
        },
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolved_native_output(
    format: RenderFormat,
    destination: ResolvedDestination,
    options: &NativeRenderOptions,
) -> Result<ResolvedOutput, CliError> {
    let output = match format {
        #[cfg(feature = "svg")]
        RenderFormat::Svg => ResolvedOutput::Svg {
            destination,
            pipeline: options.svg_pipeline,
        },
        #[cfg(feature = "ascii")]
        RenderFormat::Ascii | RenderFormat::Unicode => ResolvedOutput::Text {
            format,
            destination,
            options: resolve_text_output_options(&options.text),
        },
        #[cfg(feature = "png")]
        RenderFormat::Png => ResolvedOutput::Png {
            destination,
            raster: resolve_raster_options(&options.raster)?,
            embedded_images: resolve_embedded_image_options(&options.embedded_images),
        },
        #[cfg(feature = "jpeg")]
        RenderFormat::Jpeg => ResolvedOutput::Jpeg {
            destination,
            raster: resolve_raster_options(&options.raster)?,
            embedded_images: resolve_embedded_image_options(&options.embedded_images),
        },
        #[cfg(feature = "pdf")]
        RenderFormat::Pdf => ResolvedOutput::Pdf {
            destination,
            options: resolve_pdf_options(&options.pdf),
            embedded_images: resolve_embedded_image_options(&options.embedded_images),
            fit: true,
        },
    };
    Ok(output)
}

#[cfg(feature = "ascii")]
fn resolve_text_output_options(args: &TextOutputCliArgs) -> ResolvedTextOutputOptions {
    ResolvedTextOutputOptions {
        sequence_mirror_actors: args.sequence_mirror_actors,
        ascii_charset: args.ascii_charset,
        ascii_direction: args.ascii_direction,
        ascii_color: args.ascii_color,
        xychart_vertical_plot_height: args.xychart_vertical_plot_height,
        xychart_category_band_width: args.xychart_category_band_width,
        xychart_horizontal_plot_width: args.xychart_horizontal_plot_width,
        ascii_max_grid_cells: args.ascii_max_grid_cells,
    }
}

#[cfg(feature = "svg")]
fn resolved_mmdc_output(
    format: RenderFormat,
    destination: ResolvedDestination,
    args: &MmdcArgs,
) -> Result<ResolvedOutput, CliError> {
    let output = match format {
        RenderFormat::Svg => ResolvedOutput::Svg {
            destination,
            pipeline: args.svg_pipeline,
        },
        #[cfg(feature = "ascii")]
        RenderFormat::Ascii | RenderFormat::Unicode => unreachable!("mmdc has no text output"),
        #[cfg(feature = "png")]
        RenderFormat::Png => ResolvedOutput::Png {
            destination,
            raster: resolve_raster_options(&args.raster)?,
            embedded_images: resolve_embedded_image_options(&args.embedded_images),
        },
        #[cfg(feature = "jpeg")]
        RenderFormat::Jpeg => unreachable!("mmdc has no JPEG output"),
        #[cfg(feature = "pdf")]
        RenderFormat::Pdf => ResolvedOutput::Pdf {
            destination,
            options: resolve_pdf_options(&args.pdf),
            embedded_images: resolve_embedded_image_options(&args.embedded_images),
            fit: args.pdf_fit,
        },
    };
    Ok(output)
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_native_common(options: NativeRenderOptions) -> ResolvedRenderCommon {
    ResolvedRenderCommon {
        parse: resolve_parse_options(options.parse),
        render: resolve_render_options(options.render),
        #[cfg(feature = "svg")]
        background: options.background_color,
        #[cfg(feature = "svg")]
        css_file: options.css_file,
        quiet: options.quiet,
        #[cfg(feature = "icons")]
        icons: ResolvedIconSources {
            packages: options.icons.icon_packs,
            named_sources: options.icons.icon_packs_names_and_urls,
            #[cfg(feature = "network-icons")]
            allow_network: options.icons.allow_network,
        },
    }
}

#[cfg(feature = "svg")]
fn resolve_mmdc_common(
    parse: ParseCliArgs,
    render: RenderCliArgs,
    args: &MmdcArgs,
    output_is_stdout: bool,
) -> ResolvedRenderCommon {
    ResolvedRenderCommon {
        parse: resolve_parse_options(parse),
        render: resolve_render_options(render),
        background: Some(
            args.background_color
                .clone()
                .unwrap_or_else(|| "white".to_string()),
        ),
        css_file: args.css_file.clone(),
        quiet: args.quiet || output_is_stdout,
        #[cfg(feature = "icons")]
        icons: ResolvedIconSources {
            packages: args.icons.icon_packs.clone(),
            named_sources: args.icons.icon_packs_names_and_urls.clone(),
            #[cfg(feature = "network-icons")]
            allow_network: args.icons.allow_network,
        },
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_parse_options(args: ParseCliArgs) -> ResolvedParseOptions {
    ResolvedParseOptions {
        suppress_errors: args.suppress_errors,
        config_file: args.config_file,
        theme: args.theme,
        runtime: resolve_runtime_options(args.runtime),
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_runtime_options(args: RuntimeCliArgs) -> ResolvedRuntimeOptions {
    ResolvedRuntimeOptions {
        policy: args.policy,
        #[cfg(feature = "system-clock")]
        system_clock: args.system_clock,
        #[cfg(feature = "system-timezone")]
        system_timezone: args.system_timezone,
        #[cfg(feature = "system-random")]
        system_random: args.system_random,
        #[cfg(feature = "system-timing")]
        system_timing: args.system_timing,
        fixed_today: args.fixed_today,
        fixed_local_offset_minutes: args.fixed_local_offset_minutes,
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_render_options(args: RenderCliArgs) -> ResolvedRenderOptions {
    ResolvedRenderOptions {
        #[cfg(feature = "svg")]
        text_measurer: args
            .text_measurer
            .unwrap_or(crate::cli::TextMeasurerKind::Vendored),
        #[cfg(feature = "svg")]
        math_renderer: args.math_renderer,
        #[cfg(feature = "svg")]
        container_width: args.container_width,
        #[cfg(feature = "svg")]
        container_height: args.container_height,
        #[cfg(feature = "svg")]
        svg_id: args.svg_id,
        #[cfg(feature = "svg")]
        hand_drawn_seed: args.hand_drawn_seed,
        resource_profile: args.resource_profile,
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ResolvedParseOptions {
    pub(crate) fn into_legacy_cli_args(self) -> ParseCliArgs {
        ParseCliArgs {
            suppress_errors: self.suppress_errors,
            config_file: self.config_file,
            theme: self.theme,
            runtime: self.runtime.into_legacy_cli_args(),
        }
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ResolvedRuntimeOptions {
    fn into_legacy_cli_args(self) -> RuntimeCliArgs {
        RuntimeCliArgs {
            policy: self.policy,
            #[cfg(feature = "system-clock")]
            system_clock: self.system_clock,
            #[cfg(feature = "system-timezone")]
            system_timezone: self.system_timezone,
            #[cfg(feature = "system-random")]
            system_random: self.system_random,
            #[cfg(feature = "system-timing")]
            system_timing: self.system_timing,
            fixed_today: self.fixed_today,
            fixed_local_offset_minutes: self.fixed_local_offset_minutes,
        }
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ResolvedRenderOptions {
    pub(crate) fn into_legacy_cli_args(self) -> RenderCliArgs {
        RenderCliArgs {
            #[cfg(feature = "svg")]
            text_measurer: Some(self.text_measurer),
            #[cfg(feature = "svg")]
            math_renderer: self.math_renderer,
            #[cfg(feature = "svg")]
            container_width: self.container_width,
            #[cfg(feature = "svg")]
            container_height: self.container_height,
            #[cfg(feature = "svg")]
            svg_id: self.svg_id,
            #[cfg(feature = "svg")]
            hand_drawn_seed: self.hand_drawn_seed,
            resource_profile: self.resource_profile,
        }
    }
}

#[cfg(feature = "ascii")]
impl ResolvedTextOutputOptions {
    pub(crate) fn into_legacy_cli_args(self) -> TextOutputCliArgs {
        TextOutputCliArgs {
            sequence_mirror_actors: self.sequence_mirror_actors,
            ascii_charset: self.ascii_charset,
            ascii_direction: self.ascii_direction,
            ascii_color: self.ascii_color,
            xychart_vertical_plot_height: self.xychart_vertical_plot_height,
            xychart_category_band_width: self.xychart_category_band_width,
            xychart_horizontal_plot_width: self.xychart_horizontal_plot_width,
            ascii_max_grid_cells: self.ascii_max_grid_cells,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn resolve_raster_options(args: &RasterCliArgs) -> Result<ResolvedRasterOptions, CliError> {
    if args.raster_unbounded
        && (args.raster_max_width.is_some()
            || args.raster_max_height.is_some()
            || args.raster_max_pixels.is_some())
    {
        return Err(CliError::InvalidInput(
            "--raster-unbounded cannot be combined with --raster-max-* limits".to_string(),
        ));
    }
    Ok(ResolvedRasterOptions {
        scale: args.scale.unwrap_or(1.0),
        fit_width: args.raster_fit_width,
        fit_height: args.raster_fit_height,
        max_width: args.raster_max_width,
        max_height: args.raster_max_height,
        max_pixels: args.raster_max_pixels,
        unbounded: args.raster_unbounded,
    })
}

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
fn resolve_encoding_parallel_budget(value_mib: Option<u64>) -> Result<u64, CliError> {
    value_mib
        .unwrap_or(DEFAULT_MARKDOWN_ENCODING_PARALLEL_BUDGET_MIB)
        .checked_mul(MIB)
        .ok_or_else(|| {
            CliError::InvalidInput("--encoding-parallel-budget-mib is too large".to_string())
        })
}

#[cfg(feature = "pdf")]
const fn resolve_pdf_options(args: &PdfCliArgs) -> ResolvedPdfOptions {
    ResolvedPdfOptions {
        filter_scale: args.filter_scale,
        max_filter_image_pixels: args.max_filter_image_pixels,
        filter_images_unbounded: args.filter_images_unbounded,
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
const fn resolve_embedded_image_options(
    args: &EmbeddedImageCliArgs,
) -> ResolvedEmbeddedImageOptions {
    ResolvedEmbeddedImageOptions {
        max_bytes_per_image: args.max_image_bytes,
        max_total_bytes: args.max_total_bytes,
        max_pixels_per_image: args.max_image_pixels,
        max_total_pixels: args.max_total_pixels,
        unbounded: args.embedded_images_unbounded,
    }
}

#[cfg(feature = "svg")]
fn validate_mmdc_output_options(format: RenderFormat, args: &MmdcArgs) -> Result<(), CliError> {
    if args.svg_pipeline.is_some() && format != RenderFormat::Svg {
        return Err(CliError::InvalidInput(
            "--svg-pipeline is only valid with SVG output".to_string(),
        ));
    }

    #[cfg(feature = "png")]
    {
        let custom_raster_limits = args.raster.raster_fit_width.is_some()
            || args.raster.raster_fit_height.is_some()
            || args.raster.raster_max_width.is_some()
            || args.raster.raster_max_height.is_some()
            || args.raster.raster_max_pixels.is_some()
            || args.raster.raster_unbounded;
        if custom_raster_limits && format != RenderFormat::Png {
            return Err(CliError::InvalidInput(
                "Merman raster limit options require PNG output".to_string(),
            ));
        }
    }

    #[cfg(feature = "pdf")]
    {
        let pdf_is_configured = args.pdf.filter_scale.is_some()
            || args.pdf.max_filter_image_pixels.is_some()
            || args.pdf.filter_images_unbounded;
        if pdf_is_configured && format != RenderFormat::Pdf {
            return Err(CliError::InvalidInput(
                "Merman PDF options require PDF output".to_string(),
            ));
        }
    }

    #[cfg(any(feature = "png", feature = "pdf"))]
    {
        let embedded_is_configured = embedded_image_options_are_configured(&args.embedded_images);
        if embedded_is_configured && !format.requires_svg_encoding() {
            return Err(CliError::InvalidInput(
                "embedded-image options require PNG or PDF output".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn embedded_image_options_are_configured(args: &EmbeddedImageCliArgs) -> bool {
    args.max_image_bytes.is_some()
        || args.max_total_bytes.is_some()
        || args.max_image_pixels.is_some()
        || args.max_total_pixels.is_some()
        || args.embedded_images_unbounded
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn validate_native_output_options(
    format: RenderFormat,
    options: &NativeRenderOptions,
) -> Result<(), CliError> {
    #[cfg(any(feature = "png", feature = "jpeg"))]
    {
        let raster_is_configured = options.raster.scale.is_some()
            || options.raster.raster_fit_width.is_some()
            || options.raster.raster_fit_height.is_some()
            || options.raster.raster_max_width.is_some()
            || options.raster.raster_max_height.is_some()
            || options.raster.raster_max_pixels.is_some()
            || options.raster.raster_unbounded;
        let raster_format = match format {
            #[cfg(feature = "png")]
            RenderFormat::Png => true,
            #[cfg(feature = "jpeg")]
            RenderFormat::Jpeg => true,
            _ => false,
        };
        if raster_is_configured && !raster_format {
            return Err(CliError::InvalidInput(
                "raster options require --format png or --format jpg".to_string(),
            ));
        }
    }

    #[cfg(feature = "pdf")]
    {
        let pdf_is_configured = options.pdf.filter_scale.is_some()
            || options.pdf.max_filter_image_pixels.is_some()
            || options.pdf.filter_images_unbounded;
        if pdf_is_configured && format != RenderFormat::Pdf {
            return Err(CliError::InvalidInput(format!(
                "--pdf-filter-scale and other PDF options are not valid with --format {}",
                format.extension()
            )));
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    {
        let embedded_is_configured =
            embedded_image_options_are_configured(&options.embedded_images);
        if embedded_is_configured && !format.requires_svg_encoding() {
            return Err(CliError::InvalidInput(
                "embedded-image options require PNG, JPEG, or PDF output".to_string(),
            ));
        }
    }

    #[cfg(feature = "svg")]
    if options.svg_pipeline.is_some() && format != RenderFormat::Svg {
        return Err(CliError::InvalidInput(
            "--svg-pipeline is only valid with --format svg".to_string(),
        ));
    }

    #[cfg(feature = "ascii")]
    if format.is_text() {
        #[cfg(feature = "svg")]
        if options.background_color.is_some() || options.css_file.is_some() {
            return Err(CliError::InvalidInput(
                "--background and --css-file require SVG-based output".to_string(),
            ));
        }
        #[cfg(feature = "svg")]
        if options.render.text_measurer.is_some()
            || options.render.math_renderer.is_some()
            || options.render.container_width.is_some()
            || options.render.container_height.is_some()
            || options.render.svg_id.is_some()
            || options.render.hand_drawn_seed.is_some()
        {
            return Err(CliError::InvalidInput(
                "SVG renderer options are not valid with text output".to_string(),
            ));
        }
        #[cfg(feature = "icons")]
        if !options.icons.icon_packs.is_empty()
            || !options.icons.icon_packs_names_and_urls.is_empty()
            || {
                #[cfg(feature = "network-icons")]
                {
                    options.icons.allow_network
                }
                #[cfg(not(feature = "network-icons"))]
                {
                    false
                }
            }
        {
            return Err(CliError::InvalidInput(
                "icon packs are not valid with text output".to_string(),
            ));
        }
    }

    #[cfg(feature = "ascii")]
    if text_options_are_configured(&options.text) && !format.is_text() {
        return Err(CliError::InvalidInput(
            "text output options require --format ascii or --format unicode".to_string(),
        ));
    }

    Ok(())
}

#[cfg(feature = "svg")]
fn validate_raw_svg_options(options: &NativeRenderOptions) -> Result<(), CliError> {
    if options.parse.suppress_errors
        || options.parse.config_file.is_some()
        || options.parse.theme.is_some()
        || runtime_options_are_configured(&options.parse.runtime)
    {
        return Err(CliError::InvalidInput(
            "Mermaid parsing, configuration, and runtime options are not valid with SVG input"
                .to_string(),
        ));
    }
    if options.render.text_measurer.is_some()
        || options.render.math_renderer.is_some()
        || options.render.container_width.is_some()
        || options.render.container_height.is_some()
        || options.render.svg_id.is_some()
        || options.render.hand_drawn_seed.is_some()
    {
        return Err(CliError::InvalidInput(
            "Mermaid renderer options are not valid with SVG input".to_string(),
        ));
    }
    #[cfg(feature = "icons")]
    if !options.icons.icon_packs.is_empty()
        || !options.icons.icon_packs_names_and_urls.is_empty()
        || {
            #[cfg(feature = "network-icons")]
            {
                options.icons.allow_network
            }
            #[cfg(not(feature = "network-icons"))]
            {
                false
            }
        }
    {
        return Err(CliError::InvalidInput(
            "icon packs are not valid with SVG input".to_string(),
        ));
    }
    Ok(())
}

#[cfg(feature = "svg")]
fn runtime_options_are_configured(options: &RuntimeCliArgs) -> bool {
    options.policy != RuntimePolicyKind::Deterministic
        || {
            #[cfg(feature = "system-clock")]
            {
                options.system_clock
            }
            #[cfg(not(feature = "system-clock"))]
            {
                false
            }
        }
        || {
            #[cfg(feature = "system-timezone")]
            {
                options.system_timezone
            }
            #[cfg(not(feature = "system-timezone"))]
            {
                false
            }
        }
        || {
            #[cfg(feature = "system-random")]
            {
                options.system_random
            }
            #[cfg(not(feature = "system-random"))]
            {
                false
            }
        }
        || {
            #[cfg(feature = "system-timing")]
            {
                options.system_timing
            }
            #[cfg(not(feature = "system-timing"))]
            {
                false
            }
        }
        || options.fixed_today.is_some()
        || options.fixed_local_offset_minutes.is_some()
}

#[cfg(feature = "ascii")]
fn text_options_are_configured(options: &TextOutputCliArgs) -> bool {
    options.sequence_mirror_actors
        || options.ascii_charset.is_some()
        || options.ascii_direction.is_some()
        || options.ascii_color.is_some()
        || options.xychart_vertical_plot_height.is_some()
        || options.xychart_category_band_width.is_some()
        || options.xychart_horizontal_plot_width.is_some()
        || options.ascii_max_grid_cells.is_some()
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn is_native_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md")
                || extension.eq_ignore_ascii_case("markdown")
                || extension.eq_ignore_ascii_case("mdx")
        })
}

#[cfg(feature = "svg")]
fn is_strict_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| matches!(extension, "md" | "markdown"))
}

#[cfg(feature = "svg")]
fn validate_mmdc_output_path(path: &Path) -> Result<(), CliError> {
    if path == Path::new("-") {
        return Ok(());
    }
    let Some(extension) = path.extension().and_then(OsStr::to_str) else {
        return Err(invalid_mmdc_output_extension());
    };
    if matches!(extension, "md" | "markdown" | "svg") {
        return Ok(());
    }
    #[cfg(feature = "png")]
    if extension == "png" {
        return Ok(());
    }
    #[cfg(feature = "pdf")]
    if extension == "pdf" {
        return Ok(());
    }
    Err(invalid_mmdc_output_extension())
}

#[cfg(feature = "svg")]
fn invalid_mmdc_output_extension() -> CliError {
    CliError::InvalidInput(
        "Output file must end with .md, .markdown, or a compiled SVG/PNG/PDF extension".to_string(),
    )
}

#[cfg(feature = "svg")]
fn default_mmdc_output(input: &ResolvedInput, format: RenderFormat) -> PathBuf {
    match input.file() {
        Some(path) => PathBuf::from(format!("{}.{}", path.to_string_lossy(), format.extension())),
        None => PathBuf::from(format!("out.{}", format.extension())),
    }
}

#[cfg(feature = "parallel-markdown")]
fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .map(|jobs| (jobs / 2).max(1))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn facts(stdin_is_terminal: bool) -> InvocationFacts {
        InvocationFacts {
            cwd: PathBuf::from("."),
            stdin_is_terminal,
            stdout_is_terminal: false,
            color: ColorEnvironment::default(),
        }
    }

    #[test]
    fn native_content_commands_require_explicit_terminal_input() {
        let assert_missing = |command: &'static str| {
            let cli = Cli::try_parse_from(["merman-cli", command]).expect("parse command");
            let error = resolve(cli, &facts(true)).expect_err("terminal input should be explicit");
            assert!(
                matches!(
                    error,
                    CliError::MissingInput {
                        command: missing_command
                    } if missing_command == command
                ),
                "unexpected error for {command}: {error}"
            );
        };

        for command in ["detect", "parse"] {
            assert_missing(command);
        }
        #[cfg(feature = "svg")]
        assert_missing("layout");
        #[cfg(feature = "analysis")]
        {
            assert_missing("lint");
            assert_missing("fix");
        }
    }

    #[test]
    fn native_content_commands_use_non_terminal_stdin_when_omitted() {
        for command in ["detect", "parse"] {
            let cli = Cli::try_parse_from(["merman-cli", command]).expect("parse command");
            resolve(cli, &facts(false)).expect("piped stdin should be selected");
        }
    }

    #[cfg(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    ))]
    #[test]
    fn native_runtime_accepts_fixed_overrides() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "parse",
            "-",
            "--runtime",
            "native",
            "--fixed-local-offset-minutes",
            "480",
        ])
        .expect("parse command");
        resolve(cli, &facts(false)).expect("fixed values may override native adapters");
    }
}
