use merman::render::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgRenderOptions, headless_layout_options,
    sanitize_svg_id,
};
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

struct ProfileCase<'a> {
    engine: &'a Engine,
    source: &'a str,
    parse_options: ParseOptions,
    layout_options: &'a LayoutOptions,
    environment: &'a RenderEnvironment,
    svg_options: &'a SvgRenderOptions,
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
  layout      parse once, then repeatedly prepare the typed layout artifact
  render      parse once, then repeatedly prepare the typed artifact and render SVG
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
    let environment = RenderEnvironment::deterministic();
    let svg_options = SvgRenderOptions {
        diagram_id: Some(diagram_id_for(&args.input, args.diagram_id.as_deref())),
        ..SvgRenderOptions::default()
    };

    let duration = Duration::from_secs(args.seconds);
    let batch_size = args.batch_size();
    let case = ProfileCase {
        engine: &engine,
        source: &source,
        parse_options,
        layout_options: &layout_options,
        environment: &environment,
        svg_options: &svg_options,
    };
    let (iterations, checksum, elapsed) = match args.stage {
        Stage::Parse => run_parse(&case, duration, batch_size)?,
        Stage::Layout => run_layout(&case, duration, batch_size)?,
        Stage::Render => run_render(&case, duration, batch_size)?,
        Stage::EndToEnd => run_end_to_end(&case, duration, batch_size)?,
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
    case: &ProfileCase<'_>,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    case.engine
        .parse_diagram_for_render_model_sync(case.source, case.parse_options)?
        .ok_or("no Mermaid diagram detected")?;

    run_for_duration(duration, batch_size, || {
        let parsed = case
            .engine
            .parse_diagram_for_render_model_sync(black_box(case.source), case.parse_options)?
            .ok_or("no Mermaid diagram detected")?;
        Ok(parsed.model().kind().len())
    })
}

fn run_layout(
    case: &ProfileCase<'_>,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    let parsed = case
        .engine
        .parse_diagram_for_render_model_sync(case.source, case.parse_options)?
        .ok_or("no Mermaid diagram detected")?;
    merman_render::family::prepare(
        parsed.clone(),
        case.layout_options,
        case.environment.begin_session()?,
    )?;

    run_for_duration(duration, batch_size, || {
        let artifact = merman_render::family::prepare(
            black_box(parsed.clone()),
            case.layout_options,
            case.environment.begin_session()?,
        )?;
        black_box(artifact);
        Ok(1)
    })
}

fn run_render(
    case: &ProfileCase<'_>,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    let parsed = case
        .engine
        .parse_diagram_for_render_model_sync(case.source, case.parse_options)?
        .ok_or("no Mermaid diagram detected")?;
    merman_render::family::prepare(
        parsed.clone(),
        case.layout_options,
        case.environment.begin_session()?,
    )?
    .render_svg(case.svg_options, &SvgDebugOptions::default())?;

    run_for_duration(duration, batch_size, || {
        let artifact = merman_render::family::prepare(
            black_box(parsed.clone()),
            case.layout_options,
            case.environment.begin_session()?,
        )?;
        let svg = artifact.render_svg(case.svg_options, &SvgDebugOptions::default())?;
        Ok(svg.svg().len())
    })
}

fn run_end_to_end(
    case: &ProfileCase<'_>,
    duration: Duration,
    batch_size: usize,
) -> Result<(u64, usize, Duration), Box<dyn std::error::Error>> {
    merman::render::render_svg_sync(
        case.engine,
        case.source,
        case.parse_options,
        case.layout_options,
        case.svg_options,
    )?
    .ok_or("no Mermaid diagram detected")?;

    run_for_duration(duration, batch_size, || {
        let svg = merman::render::render_svg_sync(
            case.engine,
            black_box(case.source),
            case.parse_options,
            case.layout_options,
            case.svg_options,
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
