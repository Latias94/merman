use crate::cli::{CapabilitiesArgs, DetectArgs, ParseArgs};
use crate::config::{engine_for, parse_options};
use crate::error::CliError;
use crate::invocation::ResolvedInvocation;
#[cfg(any(feature = "analysis", feature = "shell-completions"))]
use crate::io::write_stdout;
use crate::io::{read_input, write_stdout_line};
use serde::Serialize;
use serde_json::Value;

#[cfg(feature = "analysis")]
use crate::cli::ParseCliArgs;
#[cfg(feature = "analysis")]
use crate::cli::{
    AnalysisCliArgs, FixArgs, LintArgs, LintOutputFormat, LintRuleSeverityOverride, LintRulesArgs,
};
#[cfg(feature = "analysis")]
use crate::config::{runtime_policy_for, site_config_for};
#[cfg(feature = "analysis")]
use crate::io::write_file;
#[cfg(feature = "analysis")]
use merman_analysis::document::analyze_document;
#[cfg(feature = "analysis")]
use merman_analysis::{
    AnalysisPayload, AnalysisRuleConfig, Analyzer, DiagnosticFixEdit, SourceDescriptor,
};
#[cfg(feature = "analysis")]
use std::fmt::Write as _;
#[cfg(feature = "analysis")]
use std::path::Path;

#[cfg(all(feature = "ascii", not(feature = "svg")))]
use crate::ascii_render::run_ascii_render;
#[cfg(feature = "shell-completions")]
use crate::cli::CompletionArgs;
#[cfg(feature = "svg")]
use crate::cli::LayoutArgs;
#[cfg(feature = "svg")]
use crate::config::renderer_for;
#[cfg(feature = "markdown")]
use crate::render::render_plan_for_batch;
#[cfg(feature = "svg")]
use crate::render::render_plan_for_mmdc;
#[cfg(feature = "svg")]
use crate::render::{render_plan_for_native, run_render};

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

pub(crate) fn run(invocation: ResolvedInvocation) -> Result<i32, CliError> {
    let exit_code = match invocation {
        ResolvedInvocation::Capabilities(args) => {
            run_capabilities(args)?;
            0
        }
        ResolvedInvocation::Detect(args) => {
            run_detect(args)?;
            0
        }
        ResolvedInvocation::Parse(args) => {
            run_parse(args)?;
            0
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Layout(args) => {
            run_layout(args)?;
            0
        }
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Lint(args) => run_lint(args)?,
        #[cfg(feature = "analysis")]
        ResolvedInvocation::Fix(args) => run_fix(args)?,
        #[cfg(feature = "analysis")]
        ResolvedInvocation::LintRules(args) => {
            run_lint_rules(args)?;
            0
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Render(args) => {
            let plan = render_plan_for_native(args)?;
            run_render(plan)?;
            0
        }
        #[cfg(all(feature = "ascii", not(feature = "svg")))]
        ResolvedInvocation::Render(args) => {
            run_ascii_render(args)?;
            0
        }
        #[cfg(feature = "markdown")]
        ResolvedInvocation::Batch(args) => {
            let plan = render_plan_for_batch(args)?;
            run_render(plan)?;
            0
        }
        #[cfg(feature = "svg")]
        ResolvedInvocation::Mmdc(args) => {
            let plan = render_plan_for_mmdc(args)?;
            run_render(plan)?;
            0
        }
        #[cfg(feature = "shell-completions")]
        ResolvedInvocation::Completion(args) => {
            run_completion(args)?;
            0
        }
    };
    Ok(exit_code)
}

fn run_capabilities(args: CapabilitiesArgs) -> Result<(), CliError> {
    crate::capabilities::write_compiled_capabilities(args.json)
}

fn run_detect(args: DetectArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let engine = engine_for(&args.engine.into_parse_args())?;
    let meta = engine.parse_metadata_sync(&text)?;
    write_stdout_line(&meta.diagram_type)?;
    Ok(())
}

fn run_parse(args: ParseArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let engine = engine_for(&args.parse)?;
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
        print_json(&out, args.pretty)?;
    } else {
        print_json(&parsed.model, args.pretty)?;
    }
    Ok(())
}

#[cfg(feature = "svg")]
fn run_layout(args: LayoutArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let renderer = renderer_for(&args.parse, &args.render.into_render_args(), None)?;
    let Some(layout_json) = renderer.layout_json_sync(&text)? else {
        return Err(CliError::NoDiagram);
    };
    print_json(&layout_json, args.pretty)
}

#[cfg(feature = "analysis")]
fn run_lint(args: LintArgs) -> Result<i32, CliError> {
    let (text, source) = read_analysis_input(
        args.input.as_deref(),
        args.stdin_file_name.as_deref(),
        &args.analysis,
    )?;
    let analyzer = analyzer_for(&args.analysis, source.clone())?;
    let payload = analyze_document(&text, &analyzer, source);

    match args.format {
        LintOutputFormat::Json => print_json(&payload, args.pretty)?,
        LintOutputFormat::Text => print_lint_text(&payload)?,
    }

    Ok(i32::from(!payload.valid))
}

#[cfg(feature = "analysis")]
fn run_fix(args: FixArgs) -> Result<i32, CliError> {
    let (text, source) = read_analysis_input(
        args.input.as_deref(),
        args.stdin_file_name.as_deref(),
        &args.analysis,
    )?;
    let analyzer = analyzer_for(&args.analysis, source.clone())?;
    let payload = analyze_document(&text, &analyzer, source.clone());
    let fixed = apply_diagnostic_fixes(&text, &payload, args.all)?;

    if args.write {
        let Some(path) = args.input.as_deref().filter(|path| *path != Path::new("-")) else {
            return Err(CliError::InvalidInput(
                "--write requires a file input, not stdin".to_string(),
            ));
        };
        write_file(path, fixed.as_bytes())?;
    } else if let Some(path) = args.output.as_deref() {
        write_file(path, fixed.as_bytes())?;
    } else {
        write_stdout(fixed.as_bytes())?;
    }

    let after = analyze_document(&fixed, &analyzer, source);
    Ok(i32::from(!after.valid))
}

#[cfg(feature = "analysis")]
fn run_lint_rules(args: LintRulesArgs) -> Result<(), CliError> {
    match args.format {
        LintOutputFormat::Json => {
            let response = if args.configurable {
                merman_analysis::configurable_rule_catalog_response()
            } else {
                merman_analysis::rule_catalog_response()
            };
            print_json(&response, args.pretty)
        }
        LintOutputFormat::Text => {
            let catalog = if args.configurable {
                merman_analysis::configurable_rule_catalog()
            } else {
                merman_analysis::rule_catalog()
            };
            print_lint_rules_text(&catalog)
        }
    }
}

#[cfg(feature = "shell-completions")]
fn run_completion(args: CompletionArgs) -> Result<(), CliError> {
    let mut command = crate::app::cli_command();
    let mut output = Vec::new();
    clap_complete::generate(args.shell, &mut command, "merman-cli", &mut output);
    write_stdout(&output)
}

fn print_json<T: Serialize>(value: &T, pretty: bool) -> Result<(), CliError> {
    if pretty {
        write_stdout_line(&serde_json::to_string_pretty(value)?)?;
    } else {
        write_stdout_line(&serde_json::to_string(value)?)?;
    }
    Ok(())
}

#[cfg(feature = "analysis")]
fn print_lint_rules_text(catalog: &[merman_analysis::RuleCatalogEntry]) -> Result<(), CliError> {
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
    write_stdout(output.as_bytes())
}

#[cfg(feature = "analysis")]
fn print_lint_text(payload: &AnalysisPayload) -> Result<(), CliError> {
    let mut output = String::new();
    if payload.diagnostics.is_empty() {
        output.push_str("No Mermaid diagnostics.\n");
        return write_stdout(output.as_bytes());
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
    write_stdout(output.as_bytes())
}

#[cfg(feature = "analysis")]
fn read_analysis_input(
    input: Option<&Path>,
    stdin_file_name: Option<&Path>,
    args: &AnalysisCliArgs,
) -> Result<(String, SourceDescriptor), CliError> {
    let input_path = match input {
        Some(path) => Some(path),
        None if stdin_file_name.is_some() => Some(Path::new("-")),
        None => None,
    };
    let text = read_input(input_path, false)?;
    let source_path = match input {
        Some(path) if path == Path::new("-") => stdin_file_name.map(Path::to_path_buf),
        None => stdin_file_name.map(Path::to_path_buf),
        Some(path) => Some(path.to_path_buf()),
    };
    let markdown_mode = args.markdown || is_markdown_input(source_path.as_deref());
    Ok((
        text,
        analysis_source_descriptor(markdown_mode, source_path.as_deref()),
    ))
}

#[cfg(feature = "analysis")]
fn analyzer_for(args: &AnalysisCliArgs, source: SourceDescriptor) -> Result<Analyzer, CliError> {
    let parse = ParseCliArgs {
        config_file: args.config_file.clone(),
        theme: None,
        runtime: args.runtime.clone(),
        ..Default::default()
    };
    let runtime_policy = runtime_policy_for(&parse)?;
    let site_config = site_config_for(&parse)?;
    Ok(Analyzer::with_options(
        merman_analysis::AnalysisOptions::default()
            .with_source(source)
            .with_site_config(site_config)
            .with_runtime_policy(runtime_policy)
            .with_max_source_bytes(args.max_source_bytes)
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
        config.enable_rule(rule_id.clone());
    }
    for rule_id in &args.disable_rules {
        config.disable_rule(rule_id.clone());
    }
    for LintRuleSeverityOverride { rule_id, severity } in &args.rule_severities {
        config.set_rule_severity(rule_id.clone(), *severity);
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

#[cfg(feature = "analysis")]
fn apply_diagnostic_fixes(
    text: &str,
    payload: &AnalysisPayload,
    apply_all: bool,
) -> Result<String, CliError> {
    let fixes = payload.diagnostics.iter().flat_map(|diagnostic| {
        if apply_all {
            diagnostic.fixes.iter().collect::<Vec<_>>()
        } else {
            diagnostic
                .fixes
                .iter()
                .find(|fix| fix.is_preferred)
                .or_else(|| diagnostic.fixes.first())
                .into_iter()
                .collect()
        }
    });
    let edits = fixes.flat_map(|fix| fix.edits.iter()).collect::<Vec<_>>();
    apply_fix_edits(text, edits)
}

#[cfg(feature = "analysis")]
fn apply_fix_edits(text: &str, edits: Vec<&DiagnosticFixEdit>) -> Result<String, CliError> {
    let mut edits = edits;
    edits.sort_by_key(|edit| (edit.span.byte_start, edit.span.byte_end));

    let mut previous_end = 0;
    for edit in &edits {
        let start = edit.span.byte_start;
        let end = edit.span.byte_end;
        if start > end
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return Err(CliError::InvalidInput(
                "diagnostic fix contains an invalid UTF-8 byte range".to_string(),
            ));
        }
        if start < previous_end {
            return Err(CliError::InvalidInput(
                "selected diagnostic fixes overlap; choose a narrower fix set".to_string(),
            ));
        }
        previous_end = end;
    }

    let mut result = text.to_string();
    for edit in edits.into_iter().rev() {
        result.replace_range(edit.span.byte_start..edit.span.byte_end, &edit.replacement);
    }
    Ok(result)
}

#[cfg(all(test, feature = "analysis"))]
mod tests {
    use super::*;
    use merman_analysis::{
        AnalysisDiagnostic, DiagnosticCategory, DiagnosticFix, DiagnosticFixEdit, DiagnosticSpan,
        LspRange, SourceDescriptor, Utf16Position,
    };

    fn span(start: usize, end: usize) -> DiagnosticSpan {
        DiagnosticSpan {
            byte_start: start,
            byte_end: end,
            line: 1,
            column: start + 1,
            end_line: 1,
            end_column: end + 1,
            lsp_range: LspRange::new(
                Utf16Position {
                    line: 0,
                    character: start,
                },
                Utf16Position {
                    line: 0,
                    character: end,
                },
            ),
        }
    }

    #[test]
    fn diagnostic_fix_application_rejects_overlapping_edits() {
        let edits = [
            DiagnosticFixEdit::new(span(0, 2), "A"),
            DiagnosticFixEdit::new(span(1, 3), "B"),
        ];
        let error = apply_fix_edits("abcd", edits.iter().collect()).expect_err("overlap");
        assert!(error.to_string().contains("overlap"));
    }

    #[test]
    fn diagnostic_fix_application_uses_byte_ranges_without_shifting_later_edits() {
        let edits = [
            DiagnosticFixEdit::new(span(0, 1), "first"),
            DiagnosticFixEdit::new(span(4, 5), "second"),
        ];
        assert_eq!(
            apply_fix_edits("a中b", edits.iter().collect()).expect("apply edits"),
            "first中second"
        );
    }

    #[test]
    fn default_fix_selection_prefers_preferred_fix_per_diagnostic() {
        let diagnostic = AnalysisDiagnostic::error("test", DiagnosticCategory::Parse, "test")
            .with_fix(DiagnosticFix::new(
                "fallback",
                vec![DiagnosticFixEdit::new(span(0, 1), "x")],
            ))
            .with_fix(
                DiagnosticFix::new("preferred", vec![DiagnosticFixEdit::new(span(0, 1), "y")])
                    .preferred(),
            );
        let payload = AnalysisPayload::new(SourceDescriptor::diagram(), vec![diagnostic]);
        assert_eq!(
            apply_diagnostic_fixes("a", &payload, false).expect("apply preferred fix"),
            "y"
        );
    }
}
