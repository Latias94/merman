use crate::cli::{
    Cli, Command, CompletionArgs, DetectArgs, LayoutArgs, LintArgs, LintOutputFormat, ParseArgs,
    RenderCliArgs,
};
use crate::config::{engine_for, layout_options, math_renderer, parse_options, site_config_for};
use crate::error::CliError;
use crate::io::read_input;
use crate::render::{render_plan_for_mmdc, render_plan_for_subcommand, run_render};
use clap::CommandFactory;
use merman_analysis::document::analyze_document;
use merman_analysis::{AnalysisPayload, AnalysisRuleConfig, Analyzer, SourceDescriptor};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

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

pub(crate) fn run(cli: Cli) -> Result<i32, CliError> {
    let exit_code = match cli.command {
        Some(Command::Detect(args)) => {
            run_detect(args)?;
            0
        }
        Some(Command::Parse(args)) => {
            run_parse(args)?;
            0
        }
        Some(Command::Layout(args)) => {
            run_layout(args)?;
            0
        }
        Some(Command::Lint(args)) => run_lint(args)?,
        Some(Command::Render(args)) => {
            let plan = render_plan_for_subcommand(args)?;
            run_render(plan)?;
            0
        }
        Some(Command::Completion(args)) => {
            run_completion(args)?;
            0
        }
        None => {
            let plan = render_plan_for_mmdc(cli.input, cli.export)?;
            run_render(plan)?;
            0
        }
    };
    Ok(exit_code)
}

fn run_detect(args: DetectArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let engine = engine_for(&args.parse, &RenderCliArgs::default())?;
    let Some(meta) = engine.parse_metadata_sync(&text, parse_options(&args.parse))? else {
        return Err(CliError::NoDiagram);
    };
    println!("{}", meta.diagram_type);
    Ok(())
}

fn run_parse(args: ParseArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let engine = engine_for(&args.parse, &RenderCliArgs::default())?;
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

fn run_layout(args: LayoutArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let engine = engine_for(&args.parse, &args.render)?;
    let layout = layout_options(&args.render, math_renderer(args.render.math_renderer)?);
    let Some(layouted) =
        merman::render::layout_diagram_sync(&engine, &text, parse_options(&args.parse), &layout)?
    else {
        return Err(CliError::NoDiagram);
    };
    print_json(&layouted, args.pretty)
}

fn run_lint(args: LintArgs) -> Result<i32, CliError> {
    let text = read_input(lint_input_path(&args), false)?;
    let source_path = lint_display_path(args.input.as_deref(), args.stdin_file_name.as_deref());
    let markdown_mode = args.markdown || is_markdown_input(source_path.as_deref());
    let source = lint_source_descriptor(markdown_mode, source_path.as_deref());
    let analyzer = Analyzer::with_options(lint_analyzer_options(&args, source.clone())?);

    let payload = analyze_document(&text, &analyzer, source);

    match args.format {
        LintOutputFormat::Json => print_json(&payload, args.pretty)?,
        LintOutputFormat::Text => print_lint_text(&payload),
    }

    Ok(i32::from(!payload.valid))
}

fn run_completion(args: CompletionArgs) -> Result<(), CliError> {
    let mut command = Cli::command();
    clap_complete::generate(
        args.shell,
        &mut command,
        "merman-cli",
        &mut std::io::stdout(),
    );
    Ok(())
}

fn print_json<T: Serialize>(value: &T, pretty: bool) -> Result<(), CliError> {
    if pretty {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", serde_json::to_string(value)?);
    }
    Ok(())
}

fn print_lint_text(payload: &AnalysisPayload) {
    if payload.diagnostics.is_empty() {
        println!("No Mermaid diagnostics.");
        return;
    }

    let path = payload.source.path.as_deref().unwrap_or("-");
    for diagnostic in &payload.diagnostics {
        let location = diagnostic
            .span
            .as_ref()
            .map(|span| format!("{path}:{}:{}", span.line, span.column))
            .unwrap_or_else(|| path.to_string());
        println!(
            "{location}: {} {}: {}",
            diagnostic.severity.as_str(),
            diagnostic.id,
            diagnostic.message
        );
    }

    println!(
        "{} error(s), {} warning(s), {} info(s), {} hint(s)",
        payload.summary.errors,
        payload.summary.warnings,
        payload.summary.infos,
        payload.summary.hints
    );
}

fn lint_analyzer_options(
    args: &LintArgs,
    source: SourceDescriptor,
) -> Result<merman_analysis::AnalysisOptions, CliError> {
    let parse = merman::ParseOptions {
        suppress_errors: false,
    };
    let site_config = site_config_for(
        &crate::cli::ParseCliArgs {
            config_file: args.config_file.clone(),
            theme: None,
            fixed_today: args.fixed_today,
            fixed_local_offset_minutes: args.fixed_local_offset_minutes,
            ..Default::default()
        },
        &RenderCliArgs::default(),
    )?;
    Ok(merman_analysis::AnalysisOptions::default()
        .with_parse_options(parse)
        .with_source(source)
        .with_site_config(site_config)
        .with_fixed_today(args.fixed_today)
        .with_fixed_local_offset_minutes(args.fixed_local_offset_minutes)
        .with_max_source_bytes(args.max_source_bytes)
        .with_rule_config(lint_rule_config(args)))
}

fn lint_rule_config(args: &LintArgs) -> AnalysisRuleConfig {
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
    for override_ in &args.rule_severities {
        config.set_rule_severity(override_.rule_id.clone(), override_.severity);
    }
    config
}

fn lint_display_path(input: Option<&str>, stdin_file_name: Option<&str>) -> Option<String> {
    match input {
        Some("-") | None => stdin_file_name.map(ToString::to_string),
        Some(path) => Some(path.to_string()),
    }
}

fn lint_input_path<'a>(args: &'a LintArgs) -> Option<&'a str> {
    match args.input.as_deref() {
        Some(path) => Some(path),
        None if args.stdin_file_name.is_some() => Some("-"),
        None => None,
    }
}

fn lint_source_descriptor(markdown_mode: bool, path: Option<&str>) -> SourceDescriptor {
    if markdown_mode {
        return merman_analysis::markdown::markdown_source_descriptor(path);
    }

    let mut source = SourceDescriptor::diagram();
    if let Some(path) = path {
        source = source.with_path(path);
    }
    source
}

fn is_markdown_input(input: Option<&str>) -> bool {
    input
        .map(Path::new)
        .map(merman_analysis::markdown::is_markdown_path)
        .unwrap_or(false)
}
