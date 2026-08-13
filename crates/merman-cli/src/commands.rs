use crate::cli::CapabilitiesArgs;
use crate::config::{engine_for, parse_options};
use crate::error::CliError;
use crate::input::InputLimit;
use crate::invocation::{ResolvedDetect, ResolvedInput, ResolvedInvocation, ResolvedParse};
#[cfg(any(feature = "analysis", feature = "shell-completions"))]
use crate::io::write_stdout;
use crate::io::{read_input, write_stdout_line};
use crate::output::LocalPreflight;
use crate::resources::ResolvedResourcePolicy;
use crate::runtime::{ExecutionContext, SharedWriter};
use serde::Serialize;
use serde_json::Value;

#[cfg(feature = "analysis")]
use crate::cli::ParseCliArgs;
#[cfg(feature = "analysis")]
use crate::cli::{AnalysisCliArgs, LintOutputFormat, LintRuleSeverityOverride, LintRulesArgs};
#[cfg(feature = "analysis")]
use crate::config::{runtime_policy_for, site_config_for};
#[cfg(feature = "analysis")]
use crate::diagnostics::DiagnosticSink;
#[cfg(feature = "analysis")]
use crate::fix::{FixCatalog, FixPlanError, FixSelection};
#[cfg(feature = "analysis")]
use crate::input::{InputReadError, ObservedSize};
#[cfg(feature = "analysis")]
use crate::io::{read_fix_source, read_primary_input, write_file, write_file_verified};
#[cfg(feature = "analysis")]
use merman_analysis::document::analyze_document;
#[cfg(feature = "analysis")]
use merman_analysis::{AnalysisPayload, AnalysisRuleConfig, Analyzer, SourceDescriptor};
#[cfg(feature = "analysis")]
use std::fmt::Write as _;
use std::path::Path;

#[cfg(feature = "shell-completions")]
use crate::cli::CompletionArgs;
#[cfg(feature = "svg")]
use crate::config::renderer_for;
#[cfg(feature = "svg")]
use crate::invocation::ResolvedLayout;
#[cfg(feature = "analysis")]
use crate::invocation::{ResolvedFix, ResolvedLint};
#[cfg(feature = "markdown")]
use crate::render::prepare_render_for_batch;
#[cfg(feature = "svg")]
use crate::render::prepare_render_for_mmdc;
#[cfg(any(feature = "svg", feature = "ascii"))]
use crate::render::{execute_render, prepare_render_for_native};

#[derive(Serialize)]
struct MetaOut<'a> {
    diagram_type: &'a str,
    config: &'a Value,
    effective_config: &'a Value,
    title: Option<&'a str>,
}

#[derive(Serialize)]
struct ParseOut<'a> {
    meta: MetaOut<'a>,
    model: &'a Value,
}

pub(crate) fn run(
    preflight: LocalPreflight,
    context: &mut ExecutionContext,
) -> Result<i32, CliError> {
    let (invocation, publications) = preflight.into_parts();
    #[cfg(not(any(feature = "analysis", feature = "svg", feature = "ascii")))]
    let _ = &publications;
    let exit_code = match invocation {
        ResolvedInvocation::Capabilities(args) => {
            run_capabilities(args, &context.stdout)?;
            0
        }
        ResolvedInvocation::Detect(args) => {
            run_detect(args, context)?;
            0
        }
        ResolvedInvocation::Parse(args) => {
            run_parse(args, context)?;
            0
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Layout(args) => {
            run_layout(args, context)?;
            0
        }
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Lint(args) => run_lint(args, context)?,
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Fix(args) => run_fix(args, &publications, context)?,
        #[cfg(feature = "analysis")]
        ResolvedInvocation::LintRules(args) => {
            run_lint_rules(args, &context.stdout)?;
            0
        }
        #[cfg(any(feature = "svg", feature = "ascii"))]
        ResolvedInvocation::Render(args) => {
            let prepared = prepare_render_for_native(
                args,
                publications,
                context.stdin.as_mut(),
                &context.stderr,
                #[cfg(feature = "network-icons")]
                context.network.as_mut(),
            )?;
            execute_render(prepared, context)?;
            0
        }
        #[cfg(feature = "markdown")]
        ResolvedInvocation::Batch(args) => {
            let prepared = prepare_render_for_batch(
                args,
                publications,
                context.stdin.as_mut(),
                &context.stderr,
            )?;
            execute_render(prepared, context)?;
            0
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Mmdc(args) => {
            let prepared = prepare_render_for_mmdc(
                args,
                publications,
                context.stdin.as_mut(),
                &context.stderr,
                #[cfg(feature = "network-icons")]
                context.network.as_mut(),
            )?;
            execute_render(prepared, context)?;
            0
        }
        #[cfg(feature = "shell-completions")]
        ResolvedInvocation::Completion(args) => {
            run_completion(args, &context.stdout)?;
            0
        }
    };
    Ok(exit_code)
}

fn run_capabilities(args: CapabilitiesArgs, stdout: &SharedWriter) -> Result<(), CliError> {
    crate::capabilities::write_compiled_capabilities(args.json, stdout)
}

fn run_detect(args: ResolvedDetect, context: &mut ExecutionContext) -> Result<(), CliError> {
    let text = read_resolved_input(
        &args.input,
        source_limit(&args.resources),
        context.stdin.as_mut(),
        &context.stderr,
    )?;
    let engine = engine_for(&crate::cli::ParseCliArgs::default(), &args.resources)?;
    let meta = engine.parse_metadata_sync(&text)?;
    write_stdout_line(&meta.diagram_type, &context.stdout)?;
    Ok(())
}

fn run_parse(args: ResolvedParse, context: &mut ExecutionContext) -> Result<(), CliError> {
    let text = read_resolved_input(
        &args.input,
        source_limit(&args.resources),
        context.stdin.as_mut(),
        &context.stderr,
    )?;
    let engine = engine_for(&args.parse, &args.resources)?;
    let Some(parsed) = engine.parse_diagram_sync(&text, parse_options(&args.parse))? else {
        return Err(CliError::NoDiagram);
    };

    if args.meta {
        let out = ParseOut {
            meta: MetaOut {
                diagram_type: &parsed.meta.diagram_type,
                config: parsed.meta.config.as_value(),
                effective_config: parsed.meta.effective_config.as_value(),
                title: parsed.meta.title.as_deref(),
            },
            model: &parsed.model,
        };
        print_json(&out, args.pretty, &context.stdout)?;
    } else {
        print_json(&parsed.model, args.pretty, &context.stdout)?;
    }
    Ok(())
}

#[cfg(feature = "svg")]
fn run_layout(args: ResolvedLayout, context: &mut ExecutionContext) -> Result<(), CliError> {
    let text = read_resolved_input(
        &args.input,
        source_limit(&args.resources),
        context.stdin.as_mut(),
        &context.stderr,
    )?;
    let configured = renderer_for(
        &args.parse,
        &args.render.into_render_args(),
        None,
        &args.resources,
    )?;
    let output = configured.renderer.render(configured.request(
        &text,
        merman::RenderTarget::LayoutJson(configured.svg.clone()),
        merman::OperationControl::new(),
    ))?;
    let merman::RenderOutput::LayoutJson(Some(layout_json)) = output else {
        return Err(CliError::NoDiagram);
    };
    print_json(layout_json.layout(), args.pretty, &context.stdout)
}

#[cfg(feature = "analysis")]
fn run_lint(args: ResolvedLint, context: &mut ExecutionContext) -> Result<i32, CliError> {
    let source = analysis_source_for(&args.input, args.stdin_file_name.as_deref(), &args.analysis);
    let max_source_bytes = source_limit(&args.resources);
    let text = match read_analysis_input(
        &args.input,
        max_source_bytes,
        context.stdin.as_mut(),
        &context.stderr,
    ) {
        Ok(text) => text,
        Err(InputReadError::LimitExceeded { actual, limit, .. }) => {
            let source_len = observed_size_as_usize(actual);
            let payload = AnalysisPayload::new(
                source,
                vec![merman_analysis::source_limit_diagnostic_for_len(
                    source_len, limit,
                )],
            );
            match args.format {
                LintOutputFormat::Json => print_json(&payload, args.pretty, &context.stdout)?,
                LintOutputFormat::Text => print_lint_text(&payload, &context.stdout)?,
            }
            return Ok(1);
        }
        Err(error) => return Err(CliError::primary_input(error)),
    };
    let analyzer = analyzer_for(
        &args.analysis,
        source.clone(),
        max_source_bytes,
        &args.resources,
    )?;
    let payload = analyze_document(&text, &analyzer, source);

    match args.format {
        LintOutputFormat::Json => print_json(&payload, args.pretty, &context.stdout)?,
        LintOutputFormat::Text => print_lint_text(&payload, &context.stdout)?,
    }

    Ok(i32::from(!payload.valid))
}

#[cfg(feature = "analysis")]
fn run_fix(
    args: ResolvedFix,
    publications: &crate::output::PublicationGuards,
    context: &mut ExecutionContext,
) -> Result<i32, CliError> {
    use crate::invocation::ResolvedFixMode;

    let source = analysis_source_for(&args.input, args.stdin_file_name.as_deref(), &args.analysis);
    let max_source_bytes = source_limit(&args.resources);
    let input_limit = source_input_limit(max_source_bytes);
    let acquired = read_fix_source(&args.input, input_limit, context.stdin.as_mut())
        .map_err(CliError::primary_input)?;
    let text = acquired.text();
    let analyzer = analyzer_for(
        &args.analysis,
        source.clone(),
        max_source_bytes,
        &args.resources,
    )?;
    let payload = analyze_document(text, &analyzer, source);
    let catalog = FixCatalog::build(text, &payload.diagnostics).map_err(map_fix_plan_error)?;
    let source_label = fix_source_label(&args);
    let selection = FixSelection {
        rule_ids: args.selectors.rules,
        fix_ids: args.selectors.fixes,
    };
    let plan = catalog.plan(&selection).map_err(map_fix_plan_error)?;
    let sink = DiagnosticSink::new(args.quiet, &context.stderr);
    for conflict in plan.skipped_conflicts() {
        sink.info(format!(
            "skipped rule `{}` fix `{}` because it conflicts with selected fix `{}`",
            conflict.rule_id, conflict.fix_id, conflict.conflicting_fix_id
        ));
    }
    let fixed = plan.apply(text).map_err(map_fix_plan_error)?;
    let changed = fixed.as_bytes() != text.as_bytes();

    match &args.mode {
        ResolvedFixMode::Stdout => write_stdout(fixed.as_bytes(), &context.stdout)?,
        ResolvedFixMode::Check => return Ok(i32::from(changed)),
        ResolvedFixMode::Diff => {
            if changed {
                write_stdout(
                    unified_fix_diff(text, &fixed, &source_label).as_bytes(),
                    &context.stdout,
                )?;
            }
            return Ok(i32::from(changed));
        }
        ResolvedFixMode::Output(path) => {
            write_file(
                path,
                fixed.as_bytes(),
                publications,
                context.publication.as_mut(),
            )?;
        }
        ResolvedFixMode::WriteInput(path) => {
            if changed {
                let mut verify = |approved_path: &Path| acquired.verify_unchanged(approved_path);
                write_file_verified(
                    path,
                    fixed.as_bytes(),
                    publications,
                    context.publication.as_mut(),
                    &mut verify,
                )?;
            }
        }
    }

    Ok(0)
}

#[cfg(feature = "analysis")]
fn map_fix_plan_error(error: FixPlanError) -> CliError {
    if error.is_selection_error() {
        CliError::InvalidInput(error.to_string())
    } else {
        CliError::InvalidFixPlan(error.to_string())
    }
}

#[cfg(feature = "analysis")]
fn fix_source_label(args: &ResolvedFix) -> String {
    args.input
        .file()
        .or(args.stdin_file_name.as_deref())
        .map(|path| {
            path.to_string_lossy()
                .chars()
                .flat_map(char::escape_debug)
                .collect()
        })
        .unwrap_or_else(|| "stdin".to_string())
}

#[cfg(feature = "analysis")]
fn unified_fix_diff(source: &str, fixed: &str, label: &str) -> String {
    let old = format!("a/{label}");
    let new = format!("b/{label}");
    similar::TextDiff::from_lines(source, fixed)
        .unified_diff()
        .header(&old, &new)
        .to_string()
}

#[cfg(feature = "analysis")]
fn run_lint_rules(args: LintRulesArgs, stdout: &SharedWriter) -> Result<(), CliError> {
    match args.format {
        LintOutputFormat::Json => {
            let response = if args.configurable {
                merman_analysis::configurable_rule_catalog_response()
            } else {
                merman_analysis::rule_catalog_response()
            };
            print_json(&response, args.pretty, stdout)
        }
        LintOutputFormat::Text => {
            let catalog = if args.configurable {
                merman_analysis::configurable_rule_catalog()
            } else {
                merman_analysis::rule_catalog()
            };
            print_lint_rules_text(&catalog, stdout)
        }
    }
}

#[cfg(feature = "shell-completions")]
fn run_completion(args: CompletionArgs, stdout: &SharedWriter) -> Result<(), CliError> {
    let output = crate::app::completion_script(args.shell);
    write_stdout(&output, stdout)
}

fn print_json<T: Serialize>(
    value: &T,
    pretty: bool,
    stdout: &SharedWriter,
) -> Result<(), CliError> {
    crate::diagnostics::write_json_stdout(value, pretty, stdout)
}

#[cfg(feature = "analysis")]
fn print_lint_rules_text(
    catalog: &[merman_analysis::RuleCatalogEntry],
    stdout: &SharedWriter,
) -> Result<(), CliError> {
    let mut output = String::new();
    output
        .push_str("ID\tSeverity\tProfile\tOrigin\tConfigurable\tFixable\tEvidence\tDescription\n");
    for rule in catalog {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            rule.id,
            rule.default_severity.as_str(),
            rule.default_profile.as_str(),
            rule.origin.as_str(),
            rule.configurable,
            rule.fixable,
            rule.evidence.join(","),
            rule.description
        )
        .expect("writing to a String should not fail");
    }
    write_stdout(output.as_bytes(), stdout)
}

#[cfg(feature = "analysis")]
fn print_lint_text(payload: &AnalysisPayload, stdout: &SharedWriter) -> Result<(), CliError> {
    let mut output = String::new();
    if payload.diagnostics.is_empty() {
        output.push_str("No Mermaid diagnostics.\n");
        return write_stdout(output.as_bytes(), stdout);
    }

    let path = payload.source.path.as_deref().unwrap_or("-");
    for diagnostic in &payload.diagnostics {
        let location = diagnostic
            .span
            .as_ref()
            .map(|span| format!("{path}:{}:{}", span.line, span.column))
            .unwrap_or_else(|| path.to_string());
        writeln!(
            output,
            "{location}: {} {}: {}",
            diagnostic.severity.as_str(),
            diagnostic.id,
            diagnostic.message
        )
        .expect("writing to a String should not fail");
    }

    writeln!(
        output,
        "{} error(s), {} warning(s), {} info(s), {} hint(s)",
        payload.summary.errors,
        payload.summary.warnings,
        payload.summary.infos,
        payload.summary.hints
    )
    .expect("writing to a String should not fail");
    write_stdout(output.as_bytes(), stdout)
}

#[cfg(feature = "analysis")]
fn read_analysis_input(
    input: &ResolvedInput,
    max_source_bytes: Option<usize>,
    stdin: &mut dyn std::io::Read,
    stderr: &SharedWriter,
) -> Result<String, InputReadError> {
    read_primary_input(
        Some(resolved_input_path(input)),
        true,
        source_input_limit(max_source_bytes),
        stdin,
        stderr,
    )
}

#[cfg(feature = "analysis")]
fn analysis_source_for(
    input: &ResolvedInput,
    stdin_file_name: Option<&Path>,
    args: &AnalysisCliArgs,
) -> SourceDescriptor {
    let source_path = input
        .file()
        .map(Path::to_path_buf)
        .or_else(|| stdin_file_name.map(Path::to_path_buf));
    let markdown_mode = args.markdown || is_markdown_input(source_path.as_deref());
    analysis_source_descriptor(markdown_mode, source_path.as_deref())
}

fn source_limit(resources: &ResolvedResourcePolicy) -> Option<usize> {
    resources
        .input_policy()
        .value(merman::resources::InputResourceLimitId::MaxSourceBytes)
}

fn resolved_input_path(input: &ResolvedInput) -> &Path {
    input.file().unwrap_or_else(|| Path::new("-"))
}

fn read_resolved_input(
    input: &ResolvedInput,
    max_source_bytes: Option<usize>,
    stdin: &mut dyn std::io::Read,
    stderr: &SharedWriter,
) -> Result<String, CliError> {
    read_input(
        Some(resolved_input_path(input)),
        true,
        source_input_limit(max_source_bytes),
        stdin,
        stderr,
    )
}

fn source_input_limit(max_source_bytes: Option<usize>) -> InputLimit {
    InputLimit::new(
        merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
        max_source_bytes,
    )
}

#[cfg(feature = "analysis")]
fn observed_size_as_usize(actual: ObservedSize) -> usize {
    let bytes = match actual {
        ObservedSize::Exact(bytes) | ObservedSize::AtLeast(bytes) => bytes,
    };
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

#[cfg(feature = "analysis")]
fn analyzer_for(
    args: &AnalysisCliArgs,
    source: SourceDescriptor,
    max_source_bytes: Option<usize>,
    resources: &ResolvedResourcePolicy,
) -> Result<Analyzer, CliError> {
    let parse = ParseCliArgs {
        config_file: args.config_file.clone(),
        theme: None,
        runtime: args.runtime.clone(),
        ..Default::default()
    };
    let runtime_policy = runtime_policy_for(&parse)?;
    let site_config = site_config_for(&parse, resources)?;
    Ok(Analyzer::with_options(
        merman_analysis::AnalysisOptions::default()
            .with_source(source)
            .with_site_config(site_config)
            .with_runtime_policy(runtime_policy)
            .with_max_source_bytes(max_source_bytes)
            .with_rule_config(lint_rule_config(args)),
    ))
}

#[cfg(feature = "analysis")]
fn lint_rule_config(args: &AnalysisCliArgs) -> AnalysisRuleConfig {
    let mut config = AnalysisRuleConfig::default();
    if let Some(profile) = args.lint_profile {
        config.set_profile(profile);
    }
    for rule_id in &args.enable_rules {
        config
            .enable_rule(rule_id.clone())
            .expect("CLI enable-rule ids are validated by clap");
    }
    for rule_id in &args.disable_rules {
        config
            .disable_rule(rule_id.clone())
            .expect("CLI disable-rule ids are validated by clap");
    }
    for LintRuleSeverityOverride { rule_id, severity } in &args.rule_severities {
        config
            .set_rule_severity(rule_id.clone(), *severity)
            .expect("CLI rule-severity ids are validated by clap");
    }
    config
}

#[cfg(feature = "analysis")]
fn analysis_source_descriptor(markdown_mode: bool, path: Option<&Path>) -> SourceDescriptor {
    let path = path.map(|path| path.to_string_lossy());
    if markdown_mode {
        return merman_analysis::source_descriptor_for_markdown_path(path.as_deref());
    }

    let mut source = SourceDescriptor::diagram();
    if let Some(path) = path.as_deref() {
        source = source.with_path(path);
    }
    source
}

#[cfg(feature = "analysis")]
fn is_markdown_input(input: Option<&Path>) -> bool {
    input
        .map(merman_analysis::markdown::is_markdown_path)
        .unwrap_or(false)
}
