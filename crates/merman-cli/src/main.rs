use futures::executor::block_on;
use merman::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, VendoredFontMetricsTextMeasurer,
};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::sync::Arc;

#[derive(Debug)]
enum CliError {
    Usage(&'static str),
    Io(std::io::Error),
    Mermaid(merman::Error),
    Render(merman_render::Error),
    Json(serde_json::Error),
    NoDiagram,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Usage(msg) => write!(f, "{msg}"),
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Mermaid(err) => write!(f, "{err}"),
            CliError::Render(err) => write!(f, "{err}"),
            CliError::Json(err) => write!(f, "JSON error: {err}"),
            CliError::NoDiagram => write!(f, "No Mermaid diagram detected"),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<merman::Error> for CliError {
    fn from(value: merman::Error) -> Self {
        Self::Mermaid(value)
    }
}

impl From<merman_render::Error> for CliError {
    fn from(value: merman_render::Error) -> Self {
        Self::Render(value)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum Command {
    #[default]
    Parse,
    Detect,
    Layout,
    Render,
}

#[derive(Debug, Clone, Copy, Default)]
enum TextMeasurerKind {
    Deterministic,
    #[default]
    Vendored,
}

#[derive(Debug, Default)]
struct Args {
    command: Command,
    input: Option<String>,
    pretty: bool,
    with_meta: bool,
    suppress_errors: bool,
    text_measurer: TextMeasurerKind,
    viewport_width: f64,
    viewport_height: f64,
    diagram_id: Option<String>,
    out: Option<String>,
}

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

fn usage() -> &'static str {
    "merman-cli\n\
\n\
USAGE:\n\
  merman-cli [parse] [--pretty] [--meta] [--suppress-errors] [<path>|-]\n\
  merman-cli detect [<path>|-]\n\
  merman-cli layout [--pretty] [--text-measurer deterministic|vendored] [--viewport-width <w>] [--viewport-height <h>] [--suppress-errors] [<path>|-]\n\
  merman-cli render [--text-measurer deterministic|vendored] [--viewport-width <w>] [--viewport-height <h>] [--id <diagram-id>] [--out <path>] [--suppress-errors] [<path>|-]\n\
\n\
NOTES:\n\
  - If <path> is omitted or '-', input is read from stdin.\n\
  - parse prints the semantic JSON model by default; --meta wraps it with parse metadata.\n\
  - render prints SVG to stdout by default; use --out to write a file.\n\
"
}

fn parse_args(argv: &[String]) -> Result<Args, CliError> {
    let mut args = Args {
        command: Command::Parse,
        viewport_width: 800.0,
        viewport_height: 600.0,
        ..Default::default()
    };

    let mut it = argv.iter().skip(1).peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => return Err(CliError::Usage(usage())),
            "parse" => args.command = Command::Parse,
            "detect" => args.command = Command::Detect,
            "layout" => args.command = Command::Layout,
            "render" => args.command = Command::Render,
            "--pretty" => args.pretty = true,
            "--meta" => args.with_meta = true,
            "--suppress-errors" => args.suppress_errors = true,
            "--text-measurer" => {
                let Some(kind) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.text_measurer = match kind.as_str() {
                    "deterministic" => TextMeasurerKind::Deterministic,
                    "vendored" => TextMeasurerKind::Vendored,
                    _ => return Err(CliError::Usage(usage())),
                };
            }
            "--viewport-width" => {
                let Some(w) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.viewport_width = w.parse::<f64>().map_err(|_| CliError::Usage(usage()))?;
            }
            "--viewport-height" => {
                let Some(h) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.viewport_height = h.parse::<f64>().map_err(|_| CliError::Usage(usage()))?;
            }
            "--id" => {
                let Some(id) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.diagram_id = Some(id.clone());
            }
            "--out" => {
                let Some(out) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.out = Some(out.clone());
            }
            "--" => {
                if let Some(rest) = it.next() {
                    if args.input.is_some() {
                        return Err(CliError::Usage(usage()));
                    }
                    args.input = Some(rest.clone());
                }
                while it.next().is_some() {
                    return Err(CliError::Usage(usage()));
                }
            }
            other if other.starts_with('-') => return Err(CliError::Usage(usage())),
            path => {
                if args.input.is_some() {
                    return Err(CliError::Usage(usage()));
                }
                args.input = Some(path.to_string());
            }
        }
    }

    Ok(args)
}

fn read_input(input: Option<&str>) -> Result<String, CliError> {
    match input {
        None | Some("-") => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        Some(path) => Ok(std::fs::read_to_string(path)?),
    }
}

fn write_json(value: &impl Serialize, pretty: bool) -> Result<(), CliError> {
    if pretty {
        serde_json::to_writer_pretty(std::io::stdout().lock(), value)?;
    } else {
        serde_json::to_writer(std::io::stdout().lock(), value)?;
    }
    Ok(())
}

fn build_text_measurer(kind: TextMeasurerKind) -> Arc<dyn TextMeasurer + Send + Sync> {
    match kind {
        TextMeasurerKind::Deterministic => Arc::new(DeterministicTextMeasurer::default()),
        TextMeasurerKind::Vendored => Arc::new(VendoredFontMetricsTextMeasurer::default()),
    }
}

fn write_text(text: &str, out: Option<&str>) -> Result<(), CliError> {
    match out {
        None => {
            print!("{text}");
            Ok(())
        }
        Some(path) => {
            std::fs::write(path, text)?;
            Ok(())
        }
    }
}

fn run(args: Args) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref())?;
    let engine = Engine::new();
    let options = ParseOptions {
        suppress_errors: args.suppress_errors,
    };

    match args.command {
        Command::Detect => {
            let Some(meta) = block_on(engine.parse_metadata(&text, options))? else {
                return Err(CliError::NoDiagram);
            };
            println!("{}", meta.diagram_type);
            Ok(())
        }
        Command::Parse => {
            let Some(parsed) = block_on(engine.parse_diagram(&text, options))? else {
                return Err(CliError::NoDiagram);
            };

            if args.with_meta {
                let out = ParseOut {
                    meta: MetaOut {
                        diagram_type: &parsed.meta.diagram_type,
                        config: parsed.meta.config.as_value(),
                        effective_config: parsed.meta.effective_config.as_value(),
                        title: parsed.meta.title.as_deref(),
                    },
                    model: &parsed.model,
                };
                write_json(&out, args.pretty)?;
            } else {
                write_json(&parsed.model, args.pretty)?;
            }
            Ok(())
        }
        Command::Layout => {
            let Some(parsed) = block_on(engine.parse_diagram(&text, options))? else {
                return Err(CliError::NoDiagram);
            };

            let measurer = build_text_measurer(args.text_measurer);
            let layout_opts = LayoutOptions {
                text_measurer: Arc::clone(&measurer),
                viewport_width: args.viewport_width,
                viewport_height: args.viewport_height,
            };
            let layouted = merman_render::layout_parsed(&parsed, &layout_opts)?;
            write_json(&layouted, args.pretty)?;
            Ok(())
        }
        Command::Render => {
            let Some(parsed) = block_on(engine.parse_diagram(&text, options))? else {
                return Err(CliError::NoDiagram);
            };

            let measurer = build_text_measurer(args.text_measurer);
            let layout_opts = LayoutOptions {
                text_measurer: Arc::clone(&measurer),
                viewport_width: args.viewport_width,
                viewport_height: args.viewport_height,
            };
            let layouted = merman_render::layout_parsed(&parsed, &layout_opts)?;

            let svg_options = merman_render::svg::SvgRenderOptions {
                diagram_id: args.diagram_id.clone(),
                ..Default::default()
            };
            let svg = merman_render::svg::render_layouted_svg(
                &layouted,
                measurer.as_ref(),
                &svg_options,
            )?;
            write_text(&svg, args.out.as_deref())?;
            Ok(())
        }
    }
}

fn main() {
    let args = match parse_args(&std::env::args().collect::<Vec<_>>()) {
        Ok(v) => v,
        Err(CliError::Usage(msg)) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    match run(args) {
        Ok(()) => {}
        Err(CliError::NoDiagram) => {
            eprintln!("{}", CliError::NoDiagram);
            std::process::exit(3);
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
