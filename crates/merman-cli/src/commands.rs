use crate::cli::{Cli, Command, CompletionArgs, DetectArgs, LayoutArgs, ParseArgs, RenderCliArgs};
use crate::config::{engine_for, layout_options, math_renderer, parse_options};
use crate::error::CliError;
use crate::io::{read_input, write_stdout, write_stdout_line};
use crate::render::{render_plan_for_mmdc, render_plan_for_subcommand, run_render};
use clap::CommandFactory;
use serde::Serialize;
use serde_json::Value;

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

pub(crate) fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Some(Command::Detect(args)) => run_detect(args),
        Some(Command::Parse(args)) => run_parse(args),
        Some(Command::Layout(args)) => run_layout(args),
        Some(Command::Render(args)) => {
            let plan = render_plan_for_subcommand(args)?;
            run_render(plan)
        }
        Some(Command::Completion(args)) => run_completion(args),
        None => {
            let plan = render_plan_for_mmdc(cli.input, cli.export)?;
            run_render(plan)
        }
    }
}

fn run_detect(args: DetectArgs) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref(), false)?;
    let engine = engine_for(&args.parse, &RenderCliArgs::default())?;
    let Some(meta) = engine.parse_metadata_sync(&text, parse_options(&args.parse))? else {
        return Err(CliError::NoDiagram);
    };
    write_stdout_line(&meta.diagram_type)?;
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

fn run_completion(args: CompletionArgs) -> Result<(), CliError> {
    let mut command = Cli::command();
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
