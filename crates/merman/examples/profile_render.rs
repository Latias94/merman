use merman::render::{LayoutOptions, SvgRenderOptions, headless_layout_options, sanitize_svg_id};
use merman_core::{Engine, ParseOptions};
use std::env;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Parse,
    Layout,
    Render,
    EndToEnd,
}

impl Stage {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "parse" => Ok(Self::Parse),
            "layout" => Ok(Self::Layout),
            "render" => Ok(Self::Render),
            "end-to-end" | "end_to_end" | "e2e" => Ok(Self::EndToEnd),
            _ => Err(format!(
                "unknown stage `{value}`; expected parse, layout, render, or end-to-end"
            )),
        }
    }

    fn default_batch_size(self) -> usize {
        match self {
            Self::Parse | Self::Layout | Self::EndToEnd => 1,
            Self::Render => 100,
        }
    }
}

#[derive(Debug)]
struct Args {
    input: PathBuf,
    stage: Stage,
    seconds: u64,
    batch_size: Option<usize>,
    diagram_id: Option<String>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut input = None;
        let mut stage = Stage::Render;
        let mut seconds = 20;
        let mut batch_size = None;
        let mut diagram_id = None;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--input" | "-i" => {
                    input = Some(PathBuf::from(next_arg(&mut args, &arg)?));
                }
                "--stage" => {
                    stage = Stage::parse(&next_arg(&mut args, &arg)?)?;
                }
                "--seconds" => {
                    seconds = parse_u64(&next_arg(&mut args, &arg)?, "--seconds")?;
                    if seconds == 0 {
                        return Err("--seconds must be greater than 0".to_string());
                    }
                }
                "--batch-size" => {
                    let parsed = parse_usize(&next_arg(&mut args, &arg)?, "--batch-size")?;
                    if parsed == 0 {
                        return Err("--batch-size must be greater than 0".to_string());
                    }
                    batch_size = Some(parsed);
                }
                "--diagram-id" => {
                    diagram_id = Some(next_arg(&mut args, &arg)?);
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => return Err(format!("unexpected argument `{arg}`")),
            }
        }

        let Some(input) = input else {
            return Err("missing required --input <path>".to_string());
        };

        Ok(Self {
            input,
            stage,
            seconds,
            batch_size,
            diagram_id,
        })
    }

    fn batch_size(&self) -> usize {
        self.batch_size
            .unwrap_or_else(|| self.stage.default_batch_size())
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_u64(value: &str, flag: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} expects a positive integer, got `{value}`"))
}

fn parse_usize(value: &str, flag: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} expects a positive integer, got `{value}`"))
}

fn print_usage() {
    eprintln!(
        "\
Usage:
  profile_render --input <path> [--stage render] [--seconds 20] [--batch-size N]

Stages:
  parse       repeatedly parse Mermaid source into the render model
  layout      parse once, then repeatedly layout the parsed render model
  render      parse and layout once, then repeatedly render SVG
  end-to-end  repeatedly parse, layout, and render SVG
"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse().map_err(|message| {
        eprintln!("{message}");
        print_usage();
        message
    })?;

    let source = fs::read_to_string(&args.input)?;
    let engine = Engine::new();
    let parse_options = ParseOptions::strict();
    let layout_options: LayoutOptions = headless_layout_options();
    let svg_options = SvgRenderOptions {
        diagram_id: Some(diagram_id_for(&args.input, args.diagram_id.as_deref())),
        ..SvgRenderOptions::default()
    };

    let duration = Duration::from_secs(args.seconds);
    let batch_size = args.batch_size();
    let (iterations, checksum, elapsed) = match args.stage {
        Stage::Parse => run_parse(&engine, &source, parse_options, duration, batch_size)?,
        Stage::Layout => run_layout(
            &engine,
            &source,
            parse_options,
            &layout_options,
            duration,
            batch_size,
        )?,
        Stage::Render => run_render(
            &engine,
            &source,
            parse_options,
            &layout_options,
            &svg_options,
            duration,
            batch_size,
        )?,
        Stage::EndToEnd => run_end_to_end(
            &engine,
            &source,
            parse_options,
            &layout_options,
            &svg_options,
            duration,
            batch_size,
        )?,
    };

    eprintln!(
        "profile_render stage={:?} input={} iterations={} elapsed={:.3}s checksum={}",
        args.stage,
        args.input.display(),
        iterations,
        elapsed.as_secs_f64(),
        checksum
    );

    Ok(())
}

fn diagram_id_for(path: &Path, explicit: Option<&str>) -> String {
    if let Some(id) = explicit {
        return sanitize_svg_id(id);
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(sanitize_svg_id)
        .unwrap_or_else(|| sanitize_svg_id("profile-render"))
}

fn run_parse(
    engine: &Engine,
    source: &str,
    parse_options: ParseOptions,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    engine
        .parse_diagram_for_render_model_sync(source, parse_options)?
        .ok_or("no Mermaid diagram detected")?;

    run_for_duration(duration, batch_size, || {
        let parsed = engine
            .parse_diagram_for_render_model_sync(black_box(source), parse_options)?
            .ok_or("no Mermaid diagram detected")?;
        Ok(parsed.model.kind().len())
    })
}

fn run_layout(
    engine: &Engine,
    source: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, parse_options)?
        .ok_or("no Mermaid diagram detected")?;
    merman_render::layout_parsed_render_layout_only(&parsed, layout_options)?;

    run_for_duration(duration, batch_size, || {
        let layouted =
            merman_render::layout_parsed_render_layout_only(black_box(&parsed), layout_options)?;
        black_box(layouted);
        Ok(1)
    })
}

fn run_render(
    engine: &Engine,
    source: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, parse_options)?
        .ok_or("no Mermaid diagram detected")?;
    let layouted = merman_render::layout_parsed_render_layout_only(&parsed, layout_options)?;
    merman_render::svg::render_layout_svg_parts_for_render_model_with_config(
        &layouted,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        layout_options.text_measurer.as_ref(),
        svg_options,
    )?;

    run_for_duration(duration, batch_size, || {
        let svg = merman_render::svg::render_layout_svg_parts_for_render_model_with_config(
            black_box(&layouted),
            &parsed.model,
            &parsed.meta.effective_config,
            parsed.meta.title.as_deref(),
            layout_options.text_measurer.as_ref(),
            svg_options,
        )?;
        Ok(svg.len())
    })
}

fn run_end_to_end(
    engine: &Engine,
    source: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    merman::render::render_svg_sync(engine, source, parse_options, layout_options, svg_options)?
        .ok_or("no Mermaid diagram detected")?;

    run_for_duration(duration, batch_size, || {
        let svg = merman::render::render_svg_sync(
            engine,
            black_box(source),
            parse_options,
            layout_options,
            svg_options,
        )?
        .ok_or("no Mermaid diagram detected")?;
        Ok(svg.len())
    })
}

fn run_for_duration(
    duration: Duration,
    batch_size: usize,
    mut run_once: impl FnMut() -> Result<usize, Box<dyn std::error::Error>>,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let deadline = start + duration;
    let mut iterations = 0u64;
    let mut checksum = 0usize;

    while Instant::now() < deadline {
        for _ in 0..batch_size {
            checksum ^= black_box(run_once()?);
            iterations += 1;
        }
    }

    Ok((iterations, checksum, start.elapsed()))
}
