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
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::cli::RenderInputKind;
#[cfg(any(
    feature = "png",
    feature = "jpeg",
    feature = "pdf",
    all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    )
))]
use crate::cli::RuntimePolicyKind;
use crate::cli::{
    CapabilitiesArgs, Cli, DetectArgs, ParseArgs, ParseCliArgs, RawCommand, ResourceCliArgs,
    RuntimeCliArgs,
};
#[cfg(feature = "analysis")]
use crate::cli::{FixArgs, LintArgs, LintRulesArgs};
#[cfg(feature = "svg")]
use crate::cli::{LayoutArgs, MmdcArgs, MmdcOutputFormat, RenderCliArgs};
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::cli::{NativeRenderOptions, RenderArgs, RenderFormat};
#[cfg(feature = "rustdoc")]
use crate::cli::{RustdocArgs, RustdocCommand};
#[cfg(feature = "ascii")]
use crate::cli::{
    TextCharset, TextColorMode, TextDirection, TextLayoutProfile, TextOutputCliArgs,
    TextWidthProfile,
};
use crate::error::CliError;
use crate::resources::ResolvedResourcePolicy;
use merman::runtime::RuntimePolicy;
#[cfg(feature = "analysis")]
use std::collections::BTreeSet;
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
#[cfg(any(feature = "svg", feature = "ascii"))]
use std::time::Duration;

#[derive(Debug, Clone)]
pub(crate) struct InvocationFacts {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) stdin_is_terminal: bool,
    #[cfg(feature = "ascii")]
    pub(crate) stdout_is_terminal: bool,
    #[cfg(feature = "ascii")]
    pub(crate) color: ColorEnvironment,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Default)]
pub(crate) struct ColorEnvironment {
    pub(crate) no_color: bool,
    pub(crate) force_color: bool,
    pub(crate) colorterm: Option<String>,
    pub(crate) term: Option<String>,
}

// Feature-gated command variants are resolved once at the CLI boundary; boxing every large
// render variant would add an allocation to the hot dispatch path without changing ownership.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum ResolvedInvocation {
    Capabilities(CapabilitiesArgs),
    Detect(ResolvedDetect),
    Parse(ResolvedParse),
    #[cfg(feature = "svg")]
    Layout(ResolvedLayout),
    #[cfg(feature = "analysis")]
    Lint(ResolvedLint),
    #[cfg(feature = "analysis")]
    Fix(ResolvedFix),
    #[cfg(feature = "analysis")]
    LintRules(LintRulesArgs),
    #[cfg(any(feature = "svg", feature = "ascii"))]
    Render(ResolvedSingleRender),
    #[cfg(feature = "markdown")]
    Batch(ResolvedBatchRender),
    #[cfg(feature = "svg")]
    Mmdc(ResolvedMmdcRender),
    #[cfg(feature = "rustdoc")]
    Rustdoc(ResolvedRustdoc),
    #[cfg(feature = "shell-completions")]
    Completion(CompletionArgs),
}

#[cfg(feature = "rustdoc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RustdocAction {
    Build,
    Check,
}

#[cfg(feature = "rustdoc")]
#[derive(Debug)]
pub(crate) struct ResolvedRustdoc {
    pub(crate) action: RustdocAction,
    pub(crate) quiet: bool,
    pub(crate) operation_timeout: Option<Duration>,
    pub(crate) resources: ResolvedResourcePolicy,
    config: ResolvedRustdocConfig,
}

#[cfg(feature = "rustdoc")]
#[derive(Debug)]
enum ResolvedRustdocConfig {
    Requested(PathBuf),
    Prepared(crate::rustdoc::config::Config),
}

#[cfg(feature = "rustdoc")]
impl ResolvedRustdoc {
    pub(crate) fn requested_config(&self) -> Result<&Path, CliError> {
        match &self.config {
            ResolvedRustdocConfig::Requested(path) => Ok(path),
            ResolvedRustdocConfig::Prepared(_) => Err(CliError::Internal(
                "Rustdoc configuration was prepared more than once".to_string(),
            )),
        }
    }

    pub(crate) fn anchor_config(&mut self, cwd: &Path) {
        if let ResolvedRustdocConfig::Requested(path) = &mut self.config
            && !path.is_absolute()
        {
            *path = cwd.join(&*path);
        }
    }

    pub(crate) fn prepare_config(
        &mut self,
        config: crate::rustdoc::config::Config,
    ) -> Result<(), CliError> {
        if matches!(self.config, ResolvedRustdocConfig::Prepared(_)) {
            return Err(CliError::Internal(
                "Rustdoc configuration was prepared more than once".to_string(),
            ));
        }
        self.config = ResolvedRustdocConfig::Prepared(config);
        Ok(())
    }

    pub(crate) fn into_config(self) -> Result<crate::rustdoc::config::Config, CliError> {
        match self.config {
            ResolvedRustdocConfig::Prepared(config) => Ok(config),
            ResolvedRustdocConfig::Requested(_) => Err(CliError::Internal(
                "Rustdoc command reached dispatch without configuration preflight".to_string(),
            )),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ResolvedDetect {
    pub(crate) input: ResolvedInput,
    pub(crate) resources: ResolvedResourcePolicy,
}

#[derive(Debug)]
pub(crate) struct ResolvedParse {
    pub(crate) input: ResolvedInput,
    pub(crate) pretty: bool,
    pub(crate) meta: bool,
    pub(crate) parse: ParseCliArgs,
    pub(crate) resources: ResolvedResourcePolicy,
}

#[cfg(feature = "svg")]
#[derive(Debug)]
pub(crate) struct ResolvedLayout {
    pub(crate) input: ResolvedInput,
    pub(crate) pretty: bool,
    pub(crate) operation_timeout: Option<Duration>,
    pub(crate) parse: ResolvedParseOptions,
    pub(crate) render: ResolvedRenderOptions,
    pub(crate) resources: ResolvedResourcePolicy,
}

#[cfg(feature = "analysis")]
#[derive(Debug)]
pub(crate) struct ResolvedLint {
    pub(crate) input: ResolvedInput,
    pub(crate) stdin_file_name: Option<PathBuf>,
    pub(crate) format: crate::cli::LintOutputFormat,
    pub(crate) pretty: bool,
    pub(crate) analysis: crate::cli::AnalysisCliArgs,
    pub(crate) resources: ResolvedResourcePolicy,
}

#[cfg(feature = "analysis")]
#[derive(Debug)]
pub(crate) struct ResolvedFix {
    pub(crate) input: ResolvedInput,
    pub(crate) stdin_file_name: Option<PathBuf>,
    pub(crate) mode: ResolvedFixMode,
    pub(crate) selectors: FixSelectors,
    pub(crate) quiet: bool,
    pub(crate) analysis: crate::cli::AnalysisCliArgs,
    pub(crate) resources: ResolvedResourcePolicy,
}

#[cfg(feature = "analysis")]
#[derive(Debug)]
pub(crate) enum ResolvedFixMode {
    Stdout,
    Check,
    Diff,
    Output(PathBuf),
    WriteInput(PathBuf),
}

#[cfg(feature = "analysis")]
#[derive(Debug)]
pub(crate) struct FixSelectors {
    pub(crate) rules: BTreeSet<String>,
    pub(crate) fixes: BTreeSet<crate::fix::FixId>,
}

#[cfg(all(feature = "svg", feature = "markdown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolvedWorkflow {
    Single,
    #[cfg(feature = "markdown")]
    MarkdownBatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedInput {
    File(PathBuf),
    Stdin,
}

impl ResolvedInput {
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
impl ResolvedDestination {
    pub(crate) fn file(&self) -> Option<&Path> {
        match self {
            Self::Stdout => None,
            Self::File(path) => Some(path),
        }
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
// Text rendering carries the canonical ASCII policy and resource records by value; boxing this
// dispatch enum would add an allocation without changing the ownership boundary.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum ResolvedOutput {
    #[cfg(feature = "svg")]
    Svg {
        destination: ResolvedDestination,
        pipeline: Option<crate::cli::SvgPipelineKind>,
    },
    #[cfg(feature = "ascii")]
    Text {
        destination: ResolvedDestination,
        options: Box<merman::ascii::AsciiRenderOptions>,
        resources: merman::ascii::AsciiResourcePolicy,
        viewport: merman::ascii::AsciiViewportPolicy,
        report: bool,
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
        mmdc_fit_width_px: Option<f32>,
    },
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl ResolvedOutput {
    pub(crate) fn destination(&self) -> &ResolvedDestination {
        match self {
            #[cfg(feature = "svg")]
            Self::Svg { destination, .. } => destination,
            #[cfg(feature = "ascii")]
            Self::Text { destination, .. } => destination,
            #[cfg(feature = "png")]
            Self::Png { destination, .. } => destination,
            #[cfg(feature = "jpeg")]
            Self::Jpeg { destination, .. } => destination,
            #[cfg(feature = "pdf")]
            Self::Pdf { destination, .. } => destination,
        }
    }

    #[cfg(feature = "markdown")]
    pub(crate) fn format(&self) -> RenderFormat {
        match self {
            #[cfg(feature = "svg")]
            Self::Svg { .. } => RenderFormat::Svg,
            #[cfg(feature = "ascii")]
            Self::Text { .. } => {
                unreachable!("batch normalization rejects ASCII and Unicode output")
            }
            #[cfg(feature = "png")]
            Self::Png { .. } => RenderFormat::Png,
            #[cfg(feature = "jpeg")]
            Self::Jpeg { .. } => RenderFormat::Jpeg,
            #[cfg(feature = "pdf")]
            Self::Pdf { .. } => RenderFormat::Pdf,
        }
    }
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
    pub(crate) cwd: PathBuf,
    pub(crate) operation_timeout: Option<Duration>,
    pub(crate) parse: ResolvedParseOptions,
    #[cfg(feature = "svg")]
    pub(crate) render: ResolvedRenderOptions,
    pub(crate) resources: ResolvedResourcePolicy,
    #[cfg(feature = "svg")]
    pub(crate) background: Option<String>,
    #[cfg(feature = "svg")]
    pub(crate) css_file: Option<PathBuf>,
    #[cfg(any(
        feature = "markdown",
        feature = "png",
        feature = "jpeg",
        feature = "pdf"
    ))]
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
    pub(crate) runtime_policy: RuntimePolicy,
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub(crate) struct ResolvedRenderOptions {
    pub(crate) presentation_profile: Option<merman::svg::PresentationProfile>,
    pub(crate) math_renderer: Option<crate::cli::MathRendererKind>,
    pub(crate) container_width: Option<f64>,
    pub(crate) container_height: Option<f64>,
    pub(crate) svg_id: Option<String>,
    pub(crate) hand_drawn_seed: Option<u64>,
}

#[cfg(feature = "icons")]
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedIconSources {
    pub(crate) packages: Vec<String>,
    pub(crate) named_sources: Vec<String>,
    #[cfg(feature = "network-icons")]
    pub(crate) allow_network: bool,
    #[cfg(feature = "network-icons")]
    pub(crate) allow_private_network: bool,
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
    pub(crate) warn_on_implicit_stdin: bool,
    pub(crate) warn_on_implicit_output_format: bool,
    #[cfg(feature = "parallel-markdown")]
    pub(crate) jobs: usize,
}

#[cfg(feature = "svg")]
#[derive(Debug)]
pub(crate) struct MmdcCompatibilityInputs {
    pub(crate) puppeteer_config_file: Option<PathBuf>,
    #[cfg(feature = "markdown")]
    pub(crate) artefacts: Option<PathBuf>,
}

pub(crate) fn resolve(cli: Cli, facts: &InvocationFacts) -> Result<ResolvedInvocation, CliError> {
    match cli.command {
        RawCommand::Capabilities(args) => Ok(ResolvedInvocation::Capabilities(args)),
        RawCommand::Detect(args) => normalize_detect(args, facts).map(ResolvedInvocation::Detect),
        RawCommand::Parse(args) => normalize_parse(args, facts).map(ResolvedInvocation::Parse),
        #[cfg(feature = "svg")]
        RawCommand::Layout(args) => normalize_layout(args, facts).map(ResolvedInvocation::Layout),
        #[cfg(feature = "analysis")]
        RawCommand::Lint(args) => normalize_lint(args, facts).map(ResolvedInvocation::Lint),
        #[cfg(feature = "analysis")]
        RawCommand::Fix(args) => normalize_fix(args, facts).map(ResolvedInvocation::Fix),
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
        RawCommand::Mmdc(args) => normalize_mmdc(args, facts).map(ResolvedInvocation::Mmdc),
        #[cfg(feature = "rustdoc")]
        RawCommand::Rustdoc(args) => normalize_rustdoc(args).map(ResolvedInvocation::Rustdoc),
        #[cfg(feature = "shell-completions")]
        RawCommand::Completion(args) => Ok(ResolvedInvocation::Completion(args)),
    }
}

#[cfg(feature = "rustdoc")]
fn normalize_rustdoc(args: RustdocArgs) -> Result<ResolvedRustdoc, CliError> {
    let (action, args) = match args.command {
        RustdocCommand::Build(args) => (RustdocAction::Build, args),
        RustdocCommand::Check(args) => (RustdocAction::Check, args),
    };
    Ok(ResolvedRustdoc {
        action,
        quiet: args.quiet,
        operation_timeout: args.operation.timeout_ms.map(Duration::from_millis),
        resources: resolve_resource_policy(ResourceCliArgs::default())?,
        config: ResolvedRustdocConfig::Requested(args.config),
    })
}

fn validate_parse_args(args: &ParseCliArgs) -> Result<(), CliError> {
    validate_runtime_args(&args.runtime)
}

fn normalize_detect(args: DetectArgs, facts: &InvocationFacts) -> Result<ResolvedDetect, CliError> {
    Ok(ResolvedDetect {
        input: resolve_native_input(args.input, facts.stdin_is_terminal, "detect")?,
        resources: resolve_resource_policy(args.resources.into())?,
    })
}

fn normalize_parse(args: ParseArgs, facts: &InvocationFacts) -> Result<ResolvedParse, CliError> {
    validate_parse_args(&args.parse)?;
    Ok(ResolvedParse {
        input: resolve_native_input(args.input, facts.stdin_is_terminal, "parse")?,
        pretty: args.pretty,
        meta: args.meta,
        parse: args.parse,
        resources: resolve_resource_policy(args.resources)?,
    })
}

#[cfg(feature = "svg")]
fn normalize_layout(args: LayoutArgs, facts: &InvocationFacts) -> Result<ResolvedLayout, CliError> {
    let runtime_policy = resolve_render_runtime_policy(&args.parse, false)?;
    Ok(ResolvedLayout {
        input: resolve_native_input(args.input, facts.stdin_is_terminal, "layout")?,
        pretty: args.pretty,
        operation_timeout: args.operation.timeout_ms.map(Duration::from_millis),
        parse: resolve_parse_options(args.parse, runtime_policy),
        render: resolve_render_options(args.render.into_render_args()),
        resources: resolve_resource_policy(args.resources)?,
    })
}

#[cfg(feature = "analysis")]
fn normalize_lint(args: LintArgs, facts: &InvocationFacts) -> Result<ResolvedLint, CliError> {
    validate_analysis_args(&args.analysis)?;
    validate_analysis_output(args.format, args.pretty)?;
    let input = resolve_native_input_with_named_stdin(
        args.input,
        facts.stdin_is_terminal,
        "lint",
        args.stdin_file_name.is_some(),
    )?;
    validate_stdin_file_name(&input, args.stdin_file_name.as_deref())?;
    Ok(ResolvedLint {
        input,
        stdin_file_name: args.stdin_file_name,
        format: args.format,
        pretty: args.pretty,
        analysis: args.analysis,
        resources: resolve_resource_policy(args.resources)?,
    })
}

#[cfg(feature = "analysis")]
fn normalize_fix(args: FixArgs, facts: &InvocationFacts) -> Result<ResolvedFix, CliError> {
    let mut analysis = args.analysis;
    if analysis.lint_profile.is_none() {
        analysis.lint_profile = Some(merman_analysis::AnalysisRuleProfile::Recommended);
    }
    for rule_id in &args.rules {
        analysis
            .disable_rules
            .retain(|disabled| disabled != rule_id);
        if !analysis.enable_rules.contains(rule_id) {
            analysis.enable_rules.push(rule_id.clone());
        }
    }
    validate_analysis_args(&analysis)?;
    let input = resolve_native_input_with_named_stdin(
        args.input,
        facts.stdin_is_terminal,
        "fix",
        args.stdin_file_name.is_some(),
    )?;
    validate_stdin_file_name(&input, args.stdin_file_name.as_deref())?;
    let mode = if args.check {
        ResolvedFixMode::Check
    } else if args.diff {
        ResolvedFixMode::Diff
    } else if args.write {
        let Some(path) = input.file() else {
            return Err(CliError::InvalidInput(
                "--write requires a file input, not stdin".to_string(),
            ));
        };
        ResolvedFixMode::WriteInput(path.to_path_buf())
    } else if let Some(path) = args.output {
        if path == Path::new("-") {
            return Err(CliError::InvalidInput(
                "`--output -` is ambiguous; omit `--output` to write fixed source to stdout"
                    .to_string(),
            ));
        }
        ResolvedFixMode::Output(path)
    } else {
        ResolvedFixMode::Stdout
    };
    Ok(ResolvedFix {
        input,
        stdin_file_name: args.stdin_file_name,
        mode,
        selectors: FixSelectors {
            rules: args.rules.into_iter().collect(),
            fixes: args.fixes.into_iter().collect(),
        },
        quiet: args.quiet,
        analysis,
        resources: resolve_resource_policy(args.resources)?,
    })
}

#[cfg(feature = "analysis")]
fn validate_analysis_output(
    format: crate::cli::LintOutputFormat,
    pretty: bool,
) -> Result<(), CliError> {
    if pretty && matches!(format, crate::cli::LintOutputFormat::Text) {
        return Err(CliError::InvalidInput(
            "--pretty requires JSON output; use `--format json --pretty`".to_string(),
        ));
    }
    Ok(())
}

#[cfg(feature = "analysis")]
fn validate_stdin_file_name(
    input: &ResolvedInput,
    stdin_file_name: Option<&Path>,
) -> Result<(), CliError> {
    if stdin_file_name.is_some() && !matches!(input, ResolvedInput::Stdin) {
        return Err(CliError::InvalidInput(
            "--stdin-file-name is only valid when reading from stdin".to_string(),
        ));
    }
    Ok(())
}

fn resolve_runtime_policy(args: &RuntimeCliArgs) -> Result<RuntimePolicy, CliError> {
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
    crate::config::resolve_runtime_policy(args)
}

fn validate_runtime_args(args: &RuntimeCliArgs) -> Result<(), CliError> {
    resolve_runtime_policy(args).map(|_| ())
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_render_runtime_policy(
    parse: &ParseCliArgs,
    quiet: bool,
) -> Result<RuntimePolicy, CliError> {
    #[cfg(feature = "system-timing")]
    if quiet && parse.runtime.system_timing {
        let mut runtime = parse.runtime.clone();
        runtime.system_timing = false;
        return resolve_runtime_policy(&runtime);
    }
    let _ = quiet;
    resolve_runtime_policy(&parse.runtime)
}

fn resolve_resource_policy(args: ResourceCliArgs) -> Result<ResolvedResourcePolicy, CliError> {
    let mut policy = ResolvedResourcePolicy::for_profile(args.profile);
    let mut applied = std::collections::HashSet::new();

    for resource_override in args.limits {
        apply_resource_override(
            &mut policy,
            &mut applied,
            &resource_override.stable_id,
            resource_override.value,
        )?;
    }

    Ok(policy)
}

fn apply_resource_override(
    policy: &mut ResolvedResourcePolicy,
    applied: &mut std::collections::HashSet<String>,
    stable_id: &str,
    value: u64,
) -> Result<(), CliError> {
    if !applied.insert(stable_id.to_string()) {
        return Err(CliError::InvalidInput(format!(
            "resource limit `{stable_id}` was specified more than once"
        )));
    }
    policy
        .apply_override(stable_id, value)
        .map_err(|error| CliError::InvalidInput(error.to_string()))
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
    let runtime_policy =
        resolve_render_runtime_policy(&args.options.graphical.parse, args.options.graphical.quiet)?;
    let resources = resolve_resource_policy(args.resources.clone())?;
    let input = resolve_native_input(args.input, facts.stdin_is_terminal, "render")?;
    if input.file().is_some_and(is_native_markdown_path) {
        return Err(CliError::InvalidInput(
            "render accepts one Mermaid diagram, not Markdown; use `merman-cli batch`".to_string(),
        ));
    }
    let format = infer_native_format(args.output.as_deref(), args.options.format)?;
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    let input_kind = resolve_input_kind(args.input_kind, &input);
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    if input_kind == RenderInputKind::Svg && !format.requires_svg_encoding() {
        return Err(CliError::InvalidInput(
            "SVG input requires a compiled PNG, JPEG, or PDF output format".to_string(),
        ));
    }
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    if input_kind == RenderInputKind::Svg {
        validate_raw_svg_options(&args.options.graphical)?;
    }
    validate_native_output_options(format, &args.options)?;
    let destination = resolve_single_destination(args.output, &input, format);
    let output = resolved_native_output(format, destination, &args.options, facts)?;
    let common = resolve_native_common(
        args.options.graphical,
        args.operation,
        runtime_policy,
        resources,
        working_directory(facts)?,
    );

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
    let runtime_policy =
        resolve_render_runtime_policy(&args.options.graphical.parse, args.options.graphical.quiet)?;
    let resources = resolve_resource_policy(args.resources.clone())?;
    let input = resolve_native_input(args.input, facts.stdin_is_terminal, "batch")?;
    if let Some(path) = input.file()
        && !is_native_markdown_path(path)
    {
        return Err(CliError::InvalidInput(format!(
            "batch input must be a Markdown file: {}",
            crate::error::safe_path(path)
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
    let render_format = RenderFormat::from(format);
    validate_graphical_output_options(render_format, &args.options.graphical)?;
    let output_path = output_dir.join(&source_file_name);
    let output = resolved_graphical_output(
        render_format,
        ResolvedDestination::File(output_path),
        &args.options.graphical,
    )?;
    #[cfg(feature = "parallel-markdown")]
    let jobs = resolve_parallel_jobs(args.jobs, &resources)?;
    let common = resolve_native_common(
        args.options.graphical,
        args.operation,
        runtime_policy,
        resources,
        working_directory(facts)?,
    );

    Ok(ResolvedBatchRender {
        input,
        output_root: output_dir,
        output,
        common,
        #[cfg(feature = "parallel-markdown")]
        jobs,
    })
}

#[cfg(feature = "svg")]
fn normalize_mmdc(args: MmdcArgs, facts: &InvocationFacts) -> Result<ResolvedMmdcRender, CliError> {
    let resources = resolve_resource_policy(args.resources.clone())?;
    let warn_on_implicit_stdin = args.input_file.is_none();
    let warn_on_implicit_output_format =
        args.output.as_deref() == Some(Path::new("-")) && args.output_format.is_none();
    let parse = ParseCliArgs {
        suppress_errors: false,
        config_file: args.parse.config_file.clone(),
        theme: args
            .parse
            .theme
            .or_else(|| {
                args.render
                    .presentation_profile
                    .is_none()
                    .then_some(crate::cli::MmdcTheme::Default)
            })
            .map(|theme| theme.as_str().to_string()),
        runtime: args.parse.runtime.clone(),
    };
    let runtime_policy = resolve_render_runtime_policy(&parse, args.quiet)?;
    let render = RenderCliArgs {
        presentation_profile: args.render.presentation_profile,
        math_renderer: args.render.math_renderer,
        container_width: Some(args.render.container_width),
        container_height: Some(args.render.container_height),
        svg_id: args.render.svg_id.clone(),
        hand_drawn_seed: args.render.hand_drawn_seed,
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
    if !markdown_input && args.artefacts.is_some() {
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
    let output_is_stdout = matches!(destination, ResolvedDestination::Stdout);
    let output = resolved_mmdc_output(format, destination, &args)?;
    #[cfg(feature = "parallel-markdown")]
    let jobs = if markdown_input {
        resolve_parallel_jobs(args.jobs, &resources)?
    } else {
        1
    };
    let common = ResolvedRenderCommon {
        cwd: working_directory(facts)?.to_path_buf(),
        operation_timeout: args.operation.timeout_ms.map(Duration::from_millis),
        parse: resolve_parse_options(parse, runtime_policy),
        render: resolve_render_options(render),
        resources,
        background: Some(
            args.background_color
                .clone()
                .unwrap_or_else(|| "white".to_string()),
        ),
        css_file: args.css_file.clone(),
        #[cfg(any(
            feature = "markdown",
            feature = "png",
            feature = "jpeg",
            feature = "pdf"
        ))]
        quiet: args.quiet || output_is_stdout,
        #[cfg(feature = "icons")]
        icons: ResolvedIconSources {
            packages: args.icons.icon_packs.clone(),
            named_sources: args.icons.icon_packs_names_and_urls.clone(),
            #[cfg(feature = "network-icons")]
            allow_network: args.icons.allow_network,
            #[cfg(feature = "network-icons")]
            allow_private_network: args.icons.allow_private_network,
        },
    };
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
        warn_on_implicit_stdin,
        warn_on_implicit_output_format,
        #[cfg(feature = "parallel-markdown")]
        jobs,
    })
}

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

#[cfg(any(feature = "svg", feature = "ascii"))]
fn working_directory(facts: &InvocationFacts) -> Result<&Path, CliError> {
    facts.cwd.as_deref().ok_or_else(|| {
        CliError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "current working directory is unavailable",
        ))
    })
}

#[cfg(feature = "analysis")]
fn resolve_native_input_with_named_stdin(
    input: Option<PathBuf>,
    stdin_is_terminal: bool,
    command: &'static str,
    stdin_was_selected_by_option: bool,
) -> Result<ResolvedInput, CliError> {
    if input.is_none() && stdin_was_selected_by_option {
        return Ok(ResolvedInput::Stdin);
    }
    resolve_native_input(input, stdin_is_terminal, command)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
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
        return Err(invalid_mmdc_output_extension(output));
    };
    match extension {
        "md" | "markdown" | "svg" => Ok(RenderFormat::Svg),
        #[cfg(feature = "png")]
        "png" => Ok(RenderFormat::Png),
        #[cfg(feature = "pdf")]
        "pdf" => Ok(RenderFormat::Pdf),
        _ => Err(invalid_mmdc_output_extension(output)),
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
                crate::error::safe_path(path)
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
    facts: &InvocationFacts,
) -> Result<ResolvedOutput, CliError> {
    #[cfg(not(feature = "ascii"))]
    let _ = facts;
    match format {
        #[cfg(feature = "ascii")]
        RenderFormat::Ascii | RenderFormat::Unicode => {
            let mut resources = merman::ascii::AsciiResourcePolicy::default();
            if let Some(max_grid_cells) = options.text.ascii_max_grid_cells {
                resources
                    .apply_limit(
                        merman::ascii::AsciiResourceLimitId::MaxGridCells,
                        max_grid_cells,
                    )
                    .map_err(|error| {
                        CliError::InvalidInput(format!("invalid ASCII resource limit: {error}"))
                    })?;
            }
            let viewport = resolve_text_viewport_options(&options.text)?;
            let render_options =
                resolve_text_output_options(format, &options.text, &destination, facts)?;
            Ok(ResolvedOutput::Text {
                destination,
                options: Box::new(render_options),
                resources,
                viewport,
                report: options.text.ascii_report,
            })
        }
        #[cfg(feature = "svg")]
        graphical_format => {
            resolved_graphical_output(graphical_format, destination, &options.graphical)
        }
    }
}

#[cfg(feature = "svg")]
fn resolved_graphical_output(
    format: RenderFormat,
    destination: ResolvedDestination,
    options: &crate::cli::GraphicalRenderCliArgs,
) -> Result<ResolvedOutput, CliError> {
    let output = match format {
        RenderFormat::Svg => ResolvedOutput::Svg {
            destination,
            pipeline: options.svg_pipeline,
        },
        #[cfg(feature = "ascii")]
        RenderFormat::Ascii | RenderFormat::Unicode => {
            unreachable!("graphical output cannot use a text format")
        }
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
            mmdc_fit_width_px: None,
        },
    };
    Ok(output)
}

#[cfg(feature = "ascii")]
fn resolve_text_output_options(
    format: RenderFormat,
    args: &TextOutputCliArgs,
    destination: &ResolvedDestination,
    facts: &InvocationFacts,
) -> Result<merman::ascii::AsciiRenderOptions, CliError> {
    let mut options = match format {
        RenderFormat::Ascii => merman::ascii::AsciiRenderOptions::ascii(),
        RenderFormat::Unicode => merman::ascii::AsciiRenderOptions::unicode(),
        #[cfg(feature = "svg")]
        _ => unreachable!("text options are resolved only for text output"),
    };
    options.layout_profile = resolve_text_layout_profile(args);
    if let Some(charset) = args.ascii_charset {
        options.charset = match charset {
            TextCharset::Ascii => merman::ascii::AsciiCharset::Ascii,
            TextCharset::Unicode => merman::ascii::AsciiCharset::Unicode,
        };
    }
    if let Some(profile) = args.ascii_width_profile {
        options.terminal_width_profile = match profile {
            TextWidthProfile::Unicode => merman::ascii::TerminalWidthProfile::Unicode,
            TextWidthProfile::Cjk => merman::ascii::TerminalWidthProfile::Cjk,
        };
    }
    if let Some(width) = args.ascii_flowchart_wrap_width {
        options = options.with_flowchart_node_label_wrap_width(width);
    }
    if let Some(direction) = args.ascii_direction {
        options.default_direction = match direction {
            TextDirection::LeftRight => merman::ascii::AsciiDirection::LeftRight,
            TextDirection::TopDown => merman::ascii::AsciiDirection::TopDown,
        };
    }
    options.color_mode = resolve_text_color_mode(args, destination, facts)?;
    options.sequence_mirror_actors = args.sequence_mirror_actors;
    if let Some(height) = args.xychart_vertical_plot_height {
        options.xychart_vertical_plot_height = height;
    }
    if let Some(width) = args.xychart_category_band_width {
        options.xychart_category_band_width = width;
    }
    if let Some(width) = args.xychart_horizontal_plot_width {
        options.xychart_horizontal_plot_width = width;
    }
    options
        .validate()
        .map_err(|error| CliError::InvalidInput(format!("invalid ASCII options: {error}")))?;
    Ok(options)
}

#[cfg(feature = "ascii")]
fn resolve_text_color_mode(
    args: &TextOutputCliArgs,
    destination: &ResolvedDestination,
    facts: &InvocationFacts,
) -> Result<merman::ascii::AsciiColorMode, CliError> {
    if args.ascii_report {
        return match args.ascii_color {
            None | Some(TextColorMode::Plain | TextColorMode::Auto) => {
                Ok(merman::ascii::AsciiColorMode::Plain)
            }
            Some(
                TextColorMode::Ansi16
                | TextColorMode::Ansi256
                | TextColorMode::Truecolor
                | TextColorMode::Html,
            ) => Err(CliError::AsciiReportRequiresPlain),
        };
    }

    let color_mode = match args.ascii_color.unwrap_or(TextColorMode::Plain) {
        TextColorMode::Auto => resolve_auto_text_color(facts, destination),
        explicit => explicit,
    };
    Ok(match color_mode {
        TextColorMode::Plain => merman::ascii::AsciiColorMode::Plain,
        TextColorMode::Ansi16 => merman::ascii::AsciiColorMode::Ansi16,
        TextColorMode::Ansi256 => merman::ascii::AsciiColorMode::Ansi256,
        TextColorMode::Truecolor => merman::ascii::AsciiColorMode::TrueColor,
        TextColorMode::Html => merman::ascii::AsciiColorMode::Html,
        TextColorMode::Auto => unreachable!("automatic text color is resolved above"),
    })
}

#[cfg(feature = "ascii")]
fn resolve_text_viewport_options(
    args: &TextOutputCliArgs,
) -> Result<merman::ascii::AsciiViewportPolicy, CliError> {
    let overflow = match args.ascii_overflow {
        crate::cli::TextOverflowPolicy::Allow => merman::ascii::OverflowPolicy::Allow,
        crate::cli::TextOverflowPolicy::Fallback => merman::ascii::OverflowPolicy::Fallback,
        crate::cli::TextOverflowPolicy::Error => merman::ascii::OverflowPolicy::Error,
    };
    let mut viewport = merman::ascii::AsciiViewportPolicy::default().overflow(overflow);
    if let Some(max_width) = args.ascii_max_width {
        viewport = viewport.max_width(max_width);
    }
    if args.ascii_trim_trailing_spaces {
        viewport = viewport.trim(merman::ascii::AsciiTrimPolicy::TrimTrailingSpaces);
    }
    viewport
        .validate()
        .map_err(|error| CliError::InvalidInput(format!("invalid ASCII viewport: {error}")))?;
    Ok(viewport)
}

#[cfg(feature = "ascii")]
fn resolve_text_layout_profile(args: &TextOutputCliArgs) -> merman::ascii::AsciiLayoutProfile {
    match args.ascii_layout_profile {
        TextLayoutProfile::Canonical => merman::ascii::AsciiLayoutProfile::Canonical,
        TextLayoutProfile::Compact => merman::ascii::AsciiLayoutProfile::Compact,
    }
}

#[cfg(feature = "ascii")]
fn resolve_auto_text_color(
    facts: &InvocationFacts,
    destination: &ResolvedDestination,
) -> crate::cli::TextColorMode {
    use crate::cli::TextColorMode;

    if facts.color.no_color {
        return TextColorMode::Plain;
    }

    let colorterm = facts
        .color
        .colorterm
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = facts
        .color
        .term
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let truecolor = colorterm.contains("truecolor") || colorterm.contains("24bit");

    if facts.color.force_color {
        return if truecolor {
            TextColorMode::Truecolor
        } else {
            TextColorMode::Ansi256
        };
    }

    if !matches!(destination, ResolvedDestination::Stdout) || !facts.stdout_is_terminal {
        return TextColorMode::Plain;
    }

    if truecolor {
        TextColorMode::Truecolor
    } else if term.contains("256color") {
        TextColorMode::Ansi256
    } else if !term.is_empty() && term != "dumb" {
        TextColorMode::Ansi16
    } else {
        TextColorMode::Plain
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
            mmdc_fit_width_px: resolve_mmdc_pdf_fit_width(args)?,
        },
    };
    Ok(output)
}

#[cfg(feature = "pdf")]
fn resolve_mmdc_pdf_fit_width(args: &MmdcArgs) -> Result<Option<f32>, CliError> {
    if !args.pdf_fit {
        return Ok(None);
    }
    let width = args.render.container_width as f32;
    if !(width.is_finite() && width > 0.0) {
        return Err(CliError::InvalidInput(
            "PDF viewport width is outside the supported numeric range".to_string(),
        ));
    }
    Ok(Some(width))
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_native_common(
    options: crate::cli::GraphicalRenderCliArgs,
    operation: crate::cli::OperationCliArgs,
    runtime_policy: RuntimePolicy,
    resources: ResolvedResourcePolicy,
    cwd: &Path,
) -> ResolvedRenderCommon {
    let crate::cli::GraphicalRenderCliArgs {
        #[cfg(feature = "svg")]
        background_color,
        #[cfg(feature = "svg")]
        css_file,
        #[cfg(any(
            feature = "markdown",
            feature = "png",
            feature = "jpeg",
            feature = "pdf"
        ))]
        quiet,
        #[cfg(feature = "icons")]
        icons,
        parse,
        #[cfg(feature = "svg")]
        render,
        ..
    } = options;
    ResolvedRenderCommon {
        cwd: cwd.to_path_buf(),
        operation_timeout: operation.timeout_ms.map(Duration::from_millis),
        parse: resolve_parse_options(parse, runtime_policy),
        #[cfg(feature = "svg")]
        render: resolve_render_options(render),
        resources,
        #[cfg(feature = "svg")]
        background: background_color,
        #[cfg(feature = "svg")]
        css_file,
        #[cfg(any(
            feature = "markdown",
            feature = "png",
            feature = "jpeg",
            feature = "pdf"
        ))]
        quiet,
        #[cfg(feature = "icons")]
        icons: ResolvedIconSources {
            packages: icons.icon_packs,
            named_sources: icons.icon_packs_names_and_urls,
            #[cfg(feature = "network-icons")]
            allow_network: icons.allow_network,
            #[cfg(feature = "network-icons")]
            allow_private_network: icons.allow_private_network,
        },
    }
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn resolve_parse_options(
    args: ParseCliArgs,
    runtime_policy: RuntimePolicy,
) -> ResolvedParseOptions {
    ResolvedParseOptions {
        suppress_errors: args.suppress_errors,
        config_file: args.config_file,
        theme: args.theme,
        runtime: ResolvedRuntimeOptions { runtime_policy },
    }
}

#[cfg(feature = "svg")]
fn resolve_render_options(args: RenderCliArgs) -> ResolvedRenderOptions {
    ResolvedRenderOptions {
        presentation_profile: args.presentation_profile,
        math_renderer: args.math_renderer,
        container_width: args.container_width,
        container_height: args.container_height,
        svg_id: args.svg_id,
        hand_drawn_seed: args.hand_drawn_seed,
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
    validate_graphical_output_options(format, &options.graphical)?;

    #[cfg(feature = "ascii")]
    if text_options_are_configured(&options.text) && !format.is_text() {
        return Err(CliError::InvalidInput(
            "text output options require --format ascii or --format unicode".to_string(),
        ));
    }

    Ok(())
}

#[cfg(any(feature = "svg", feature = "ascii"))]
fn validate_graphical_output_options(
    format: RenderFormat,
    options: &crate::cli::GraphicalRenderCliArgs,
) -> Result<(), CliError> {
    #[cfg(not(feature = "svg"))]
    let _ = options;

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
        if options.render.presentation_profile.is_some()
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
                    options.icons.allow_network || options.icons.allow_private_network
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

    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn validate_raw_svg_options(options: &crate::cli::GraphicalRenderCliArgs) -> Result<(), CliError> {
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
    if options.render.presentation_profile.is_some()
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
                options.icons.allow_network || options.icons.allow_private_network
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

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
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
        || options.ascii_width_profile.is_some()
        || options.ascii_flowchart_wrap_width.is_some()
        || options.ascii_direction.is_some()
        || options.ascii_color.is_some()
        || options.xychart_vertical_plot_height.is_some()
        || options.xychart_category_band_width.is_some()
        || options.xychart_horizontal_plot_width.is_some()
        || options.ascii_max_grid_cells.is_some()
        || options.ascii_max_width.is_some()
        || !matches!(
            options.ascii_overflow,
            crate::cli::TextOverflowPolicy::Allow
        )
        || options.ascii_trim_trailing_spaces
        || !matches!(
            options.ascii_layout_profile,
            crate::cli::TextLayoutProfile::Canonical
        )
        || options.ascii_report
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
        return Err(invalid_mmdc_output_extension(path));
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
    Err(invalid_mmdc_output_extension(path))
}

#[cfg(feature = "svg")]
fn invalid_mmdc_output_extension(path: &Path) -> CliError {
    if let Some(format) = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
        .and_then(crate::cli::native_only_render_format)
    {
        if format.available {
            return CliError::InvalidInput(format!(
                "`mmdc` does not support .{} output; use `merman-cli render -f {} ...`",
                format.canonical, format.canonical
            ));
        }
        return CliError::InvalidInput(format!(
            "`mmdc` does not support .{} output, and native `{}` output is unavailable because this binary was built without the `{}` feature",
            format.canonical, format.canonical, format.feature
        ));
    }
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
fn resolve_parallel_jobs(
    requested: Option<usize>,
    resources: &ResolvedResourcePolicy,
) -> Result<usize, CliError> {
    let batch = resources.batch();
    let jobs = requested.unwrap_or(batch.default_jobs);
    if jobs > batch.max_jobs {
        return Err(CliError::InvalidInput(format!(
            "--jobs {jobs} exceeds the resolved maximum of {} for the {} profile",
            batch.max_jobs,
            resources.profile()
        )));
    }
    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn facts(stdin_is_terminal: bool) -> InvocationFacts {
        InvocationFacts {
            cwd: Some(PathBuf::from(".")),
            stdin_is_terminal,
            #[cfg(feature = "ascii")]
            stdout_is_terminal: false,
            #[cfg(feature = "ascii")]
            color: ColorEnvironment::default(),
        }
    }

    #[cfg(feature = "ascii")]
    fn color_facts(
        stdout_is_terminal: bool,
        no_color: bool,
        force_color: bool,
        colorterm: Option<&str>,
        term: Option<&str>,
    ) -> InvocationFacts {
        InvocationFacts {
            cwd: Some(PathBuf::from(".")),
            stdin_is_terminal: false,
            stdout_is_terminal,
            color: ColorEnvironment {
                no_color,
                force_color,
                colorterm: colorterm.map(str::to_owned),
                term: term.map(str::to_owned),
            },
        }
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn auto_text_color_honors_no_color_before_force_color() {
        let facts = color_facts(true, true, true, Some("truecolor"), Some("xterm-256color"));

        assert_eq!(
            resolve_auto_text_color(&facts, &ResolvedDestination::Stdout),
            crate::cli::TextColorMode::Plain
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn auto_text_color_force_bypasses_destination_and_tty_checks() {
        let truecolor = color_facts(false, false, true, Some("24BIT"), Some("dumb"));
        assert_eq!(
            resolve_auto_text_color(
                &truecolor,
                &ResolvedDestination::File(PathBuf::from("diagram.txt"))
            ),
            crate::cli::TextColorMode::Truecolor
        );

        let fallback = color_facts(false, false, true, None, Some("dumb"));
        assert_eq!(
            resolve_auto_text_color(&fallback, &ResolvedDestination::Stdout),
            crate::cli::TextColorMode::Ansi256
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn auto_text_color_requires_terminal_stdout_without_force() {
        let capable = color_facts(
            true,
            false,
            false,
            Some("truecolor"),
            Some("xterm-256color"),
        );

        assert_eq!(
            resolve_auto_text_color(
                &capable,
                &ResolvedDestination::File(PathBuf::from("diagram.txt"))
            ),
            crate::cli::TextColorMode::Plain
        );

        let piped = color_facts(
            false,
            false,
            false,
            Some("truecolor"),
            Some("xterm-256color"),
        );
        assert_eq!(
            resolve_auto_text_color(&piped, &ResolvedDestination::Stdout),
            crate::cli::TextColorMode::Plain
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn auto_text_color_distinguishes_terminal_capabilities() {
        for (colorterm, term, expected) in [
            (
                Some("TRUECOLOR"),
                Some("screen"),
                crate::cli::TextColorMode::Truecolor,
            ),
            (
                None,
                Some("xterm-256color"),
                crate::cli::TextColorMode::Ansi256,
            ),
            (None, Some("xterm"), crate::cli::TextColorMode::Ansi16),
            (None, Some("dumb"), crate::cli::TextColorMode::Plain),
            (None, None, crate::cli::TextColorMode::Plain),
        ] {
            let facts = color_facts(true, false, false, colorterm, term);
            assert_eq!(
                resolve_auto_text_color(&facts, &ResolvedDestination::Stdout),
                expected,
                "unexpected mode for COLORTERM={colorterm:?}, TERM={term:?}"
            );
        }
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn text_output_options_resolve_auto_during_invocation_normalization() {
        let facts = color_facts(true, false, false, None, Some("xterm"));
        let args = TextOutputCliArgs {
            ascii_color: Some(crate::cli::TextColorMode::Auto),
            ..TextOutputCliArgs::default()
        };

        let resolved = resolve_text_output_options(
            RenderFormat::Ascii,
            &args,
            &ResolvedDestination::Stdout,
            &facts,
        )
        .expect("resolve text options");

        assert_eq!(resolved.color_mode, merman::ascii::AsciiColorMode::Ansi16);
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn report_output_normalizes_auto_color_to_plain_for_every_host_context() {
        let args = TextOutputCliArgs {
            ascii_color: Some(crate::cli::TextColorMode::Auto),
            ascii_report: true,
            ..TextOutputCliArgs::default()
        };
        let cases = [
            (
                ResolvedDestination::Stdout,
                color_facts(
                    true,
                    false,
                    false,
                    Some("truecolor"),
                    Some("xterm-256color"),
                ),
            ),
            (
                ResolvedDestination::Stdout,
                color_facts(false, false, false, None, Some("xterm")),
            ),
            (
                ResolvedDestination::File(PathBuf::from("report.json")),
                color_facts(true, false, true, Some("truecolor"), Some("xterm-256color")),
            ),
            (
                ResolvedDestination::Stdout,
                color_facts(true, true, false, Some("truecolor"), Some("dumb")),
            ),
        ];

        for (destination, facts) in cases {
            let resolved =
                resolve_text_output_options(RenderFormat::Unicode, &args, &destination, &facts)
                    .expect("report auto color should normalize to plain");
            assert_eq!(resolved.color_mode, merman::ascii::AsciiColorMode::Plain);
        }
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn report_output_rejects_every_explicit_styled_encoding() {
        for color_mode in [
            crate::cli::TextColorMode::Ansi16,
            crate::cli::TextColorMode::Ansi256,
            crate::cli::TextColorMode::Truecolor,
            crate::cli::TextColorMode::Html,
        ] {
            let args = TextOutputCliArgs {
                ascii_color: Some(color_mode),
                ascii_report: true,
                ..TextOutputCliArgs::default()
            };
            let error = resolve_text_output_options(
                RenderFormat::Ascii,
                &args,
                &ResolvedDestination::Stdout,
                &color_facts(true, false, false, Some("truecolor"), Some("xterm")),
            )
            .expect_err("styled report output must fail during invocation normalization");

            assert!(matches!(error, CliError::AsciiReportRequiresPlain));
        }
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn text_output_options_resolve_cjk_width_profile() {
        let args = TextOutputCliArgs {
            ascii_width_profile: Some(crate::cli::TextWidthProfile::Cjk),
            ..TextOutputCliArgs::default()
        };

        let resolved = resolve_text_output_options(
            RenderFormat::Unicode,
            &args,
            &ResolvedDestination::Stdout,
            &facts(false),
        )
        .expect("resolve text options");

        assert_eq!(
            resolved.terminal_width_profile,
            merman::ascii::TerminalWidthProfile::Cjk
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn text_output_options_resolve_viewport_and_compact_profile() {
        let args = TextOutputCliArgs {
            ascii_max_width: Some(80),
            ascii_overflow: crate::cli::TextOverflowPolicy::Fallback,
            ascii_trim_trailing_spaces: true,
            ascii_layout_profile: crate::cli::TextLayoutProfile::Compact,
            ..TextOutputCliArgs::default()
        };
        let viewport = resolve_text_viewport_options(&args).expect("resolve viewport");
        assert_eq!(viewport.max_width, Some(80));
        assert_eq!(viewport.overflow, merman::ascii::OverflowPolicy::Fallback);
        assert_eq!(
            viewport.trim,
            merman::ascii::AsciiTrimPolicy::TrimTrailingSpaces
        );
        let options = resolve_text_output_options(
            RenderFormat::Ascii,
            &args,
            &ResolvedDestination::Stdout,
            &facts(false),
        )
        .expect("resolve compact options");
        assert_eq!(
            options.layout_profile,
            merman::ascii::AsciiLayoutProfile::Compact
        );
    }

    #[cfg(all(feature = "ascii", feature = "svg"))]
    #[test]
    fn compact_layout_profile_requires_text_output() {
        let mut options = NativeRenderOptions::default();
        options.text.ascii_layout_profile = crate::cli::TextLayoutProfile::Compact;

        let error = validate_native_output_options(RenderFormat::Svg, &options)
            .expect_err("compact text profile must not be ignored for SVG output");

        assert!(
            error
                .to_string()
                .contains("text output options require --format ascii or --format unicode")
        );
    }

    #[test]
    fn content_commands_resolve_one_canonical_resource_policy() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "parse",
            "-",
            "--resource-profile",
            "constrained",
            "--resource-limit",
            "max_source_bytes=17",
            "--resource-limit",
            "max_config_bytes=29",
        ])
        .expect("parse resource options");

        let ResolvedInvocation::Parse(resolved) =
            resolve(cli, &facts(false)).expect("resolve resource policy")
        else {
            panic!("expected parse invocation");
        };

        assert_eq!(
            resolved.resources.profile(),
            merman::resources::ResourceProfile::Constrained
        );
        assert_eq!(
            resolved
                .resources
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
            Some(17)
        );
        assert_eq!(resolved.resources.files().config_bytes, Some(29));
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[test]
    fn render_common_owns_the_resolved_resource_policy() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "render",
            "-",
            "--resource-profile",
            "interactive",
            "--resource-limit",
            "max_source_bytes=31",
        ])
        .expect("parse render resource options");

        let ResolvedInvocation::Render(resolved) =
            resolve(cli, &facts(false)).expect("resolve render resources")
        else {
            panic!("expected render invocation");
        };

        assert_eq!(
            resolved.common.resources.profile(),
            merman::resources::ResourceProfile::Interactive
        );
        assert_eq!(
            resolved
                .common
                .resources
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
            Some(31)
        );
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[test]
    fn render_resolves_the_host_operation_timeout() {
        let cli =
            Cli::try_parse_from(["merman-cli", "render", "-", "--operation-timeout-ms", "37"])
                .expect("parse operation timeout");

        let ResolvedInvocation::Render(resolved) =
            resolve(cli, &facts(false)).expect("resolve operation timeout")
        else {
            panic!("expected render invocation");
        };

        assert_eq!(
            resolved.common.operation_timeout,
            Some(Duration::from_millis(37))
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn layout_resolves_the_same_host_operation_timeout() {
        let cli =
            Cli::try_parse_from(["merman-cli", "layout", "-", "--operation-timeout-ms", "39"])
                .expect("parse layout operation timeout");

        let ResolvedInvocation::Layout(resolved) =
            resolve(cli, &facts(false)).expect("resolve layout operation timeout")
        else {
            panic!("expected layout invocation");
        };

        assert_eq!(resolved.operation_timeout, Some(Duration::from_millis(39)));
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn batch_resolves_one_deadline_for_the_complete_generation() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "batch",
            "input.md",
            "--operation-timeout-ms",
            "41",
        ])
        .expect("parse batch operation timeout");

        let ResolvedInvocation::Batch(resolved) =
            resolve(cli, &facts(false)).expect("resolve batch operation timeout")
        else {
            panic!("expected batch invocation");
        };

        assert_eq!(
            resolved.common.operation_timeout,
            Some(Duration::from_millis(41))
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn mmdc_resolves_the_same_host_operation_timeout() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "mmdc",
            "-i",
            "-",
            "-o",
            "-",
            "--operation-timeout-ms",
            "43",
        ])
        .expect("parse mmdc operation timeout");

        let ResolvedInvocation::Mmdc(resolved) =
            resolve(cli, &facts(false)).expect("resolve mmdc operation timeout")
        else {
            panic!("expected mmdc invocation");
        };

        assert_eq!(
            resolved.common.operation_timeout,
            Some(Duration::from_millis(43))
        );
    }

    #[cfg(feature = "rustdoc")]
    #[test]
    fn rustdoc_resolves_the_same_host_operation_timeout() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "rustdoc",
            "check",
            "--operation-timeout-ms",
            "47",
        ])
        .expect("parse Rustdoc operation timeout");

        let ResolvedInvocation::Rustdoc(resolved) =
            resolve(cli, &facts(false)).expect("resolve Rustdoc operation timeout")
        else {
            panic!("expected Rustdoc invocation");
        };

        assert_eq!(resolved.operation_timeout, Some(Duration::from_millis(47)));
    }

    #[test]
    fn resource_overrides_reject_unknown_and_duplicate_stable_ids() {
        let unknown = Cli::try_parse_from([
            "merman-cli",
            "parse",
            "-",
            "--resource-limit",
            "future_limit=1",
        ])
        .expect("parse unknown resource id");
        let error = resolve(unknown, &facts(false)).expect_err("unknown id must fail");
        assert!(
            error.to_string().contains("future_limit"),
            "unexpected error: {error}"
        );

        let duplicate = Cli::try_parse_from([
            "merman-cli",
            "parse",
            "definitely-missing.mmd",
            "--resource-limit",
            "max_source_bytes=17",
            "--resource-limit",
            "max_source_bytes=23",
        ])
        .expect("parse duplicate resource ids");
        let error = resolve(duplicate, &facts(false)).expect_err("duplicate id must fail");
        assert!(
            error.to_string().contains("max_source_bytes")
                && error.to_string().contains("specified more than once"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn removed_source_limit_alias_is_not_parsed() {
        assert!(
            Cli::try_parse_from(["merman-cli", "parse", "-", "--max-source-bytes", "23"]).is_err()
        );
    }

    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    #[test]
    fn removed_encoding_budget_aliases_are_not_parsed() {
        for option in [
            "--encoding-parallel-budget-mib",
            "--encoding-memory-budget-mib",
        ] {
            assert!(
                Cli::try_parse_from(["merman-cli", "batch", "input.md", option, "7"]).is_err(),
                "{option} must be replaced by --resource-limit"
            );
        }
    }

    #[cfg(feature = "parallel-markdown")]
    #[test]
    fn batch_jobs_use_the_profile_default_and_maximum() {
        let defaulted = Cli::try_parse_from([
            "merman-cli",
            "batch",
            "input.md",
            "--resource-profile",
            "constrained",
        ])
        .expect("parse constrained batch");
        let ResolvedInvocation::Batch(defaulted) =
            resolve(defaulted, &facts(false)).expect("resolve constrained batch")
        else {
            panic!("expected batch invocation");
        };
        assert_eq!(defaulted.jobs, 1);

        let exact = Cli::try_parse_from([
            "merman-cli",
            "batch",
            "input.md",
            "--resource-profile",
            "constrained",
            "--jobs",
            "2",
        ])
        .expect("parse jobs at profile maximum");
        let ResolvedInvocation::Batch(exact) =
            resolve(exact, &facts(false)).expect("profile maximum should be accepted")
        else {
            panic!("expected batch invocation");
        };
        assert_eq!(exact.jobs, 2);

        let excessive = Cli::try_parse_from([
            "merman-cli",
            "batch",
            "input.md",
            "--resource-profile",
            "constrained",
            "--jobs",
            "3",
        ])
        .expect("parse excessive jobs");
        let error = resolve(excessive, &facts(false)).expect_err("profile maximum must apply");
        assert!(
            error.to_string().contains("resolved maximum of 2"),
            "unexpected error: {error}"
        );
    }

    #[cfg(feature = "parallel-markdown")]
    #[test]
    fn scheduling_weight_uses_the_unified_resource_override() {
        let cli = Cli::try_parse_from([
            "merman-cli",
            "batch",
            "input.md",
            "--resource-limit",
            "max_scheduling_weight_bytes=7340032",
        ])
        .expect("parse scheduling weight");

        let ResolvedInvocation::Batch(resolved) =
            resolve(cli, &facts(false)).expect("resolve scheduling weight")
        else {
            panic!("expected batch invocation");
        };

        assert_eq!(
            resolved.common.resources.batch().scheduling_weight_bytes,
            Some(7 * 1024 * 1024)
        );
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
