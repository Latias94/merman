use merman::render::{
    DeterministicTextMeasurer, LayoutOptions, SvgRenderOptions, TextMeasurer,
    VendoredFontMetricsTextMeasurer,
};
use merman::{Engine, MermaidConfig, ParseOptions};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug)]
enum CliError {
    Usage(&'static str),
    Io(std::io::Error),
    Mermaid(merman::Error),
    Headless(merman::render::HeadlessError),
    Raster(merman::render::raster::RasterError),
    Json(serde_json::Error),
    NoDiagram,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Usage(msg) => write!(f, "{msg}"),
            CliError::Io(err) => write!(f, "I/O error: {err}"),
            CliError::Mermaid(err) => write!(f, "{err}"),
            CliError::Headless(err) => write!(f, "{err}"),
            CliError::Raster(err) => write!(f, "{err}"),
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

impl From<merman::render::HeadlessError> for CliError {
    fn from(value: merman::render::HeadlessError) -> Self {
        Self::Headless(value)
    }
}

impl From<merman::render::raster::RasterError> for CliError {
    fn from(value: merman::render::raster::RasterError) -> Self {
        Self::Raster(value)
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

#[derive(Debug, Clone, Copy, Default)]
enum RenderFormat {
    #[default]
    Svg,
    Png,
    Jpeg,
    Pdf,
}

impl FromStr for RenderFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "svg" => Ok(Self::Svg),
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "pdf" => Ok(Self::Pdf),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Default)]
struct Args {
    command: Command,
    input: Option<String>,
    pretty: bool,
    with_meta: bool,
    suppress_errors: bool,
    hand_drawn_seed: Option<u64>,
    text_measurer: TextMeasurerKind,
    render_format: RenderFormat,
    render_scale: f32,
    background: Option<String>,
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
  merman-cli render [--format svg|png|jpg|pdf] [--scale <n>] [--background <css-color>] [--text-measurer deterministic|vendored] [--viewport-width <w>] [--viewport-height <h>] [--id <diagram-id>] [--out <path>] [--hand-drawn-seed <n>] [--suppress-errors] [<path>|-]\n\
\n\
NOTES:\n\
  - If <path> is omitted or '-', input is read from stdin.\n\
  - parse prints the semantic JSON model by default; --meta wraps it with parse metadata.\n\
  - render prints SVG to stdout by default; use --out to write a file.\n\
  - PNG output defaults to writing next to the input file (or ./out.png for stdin).\n\
  - JPG output defaults to writing next to the input file (or ./out.jpg for stdin).\n\
  - PDF output defaults to writing next to the input file (or ./out.pdf for stdin).\n\
"
}

fn parse_args(argv: &[String]) -> Result<Args, CliError> {
    let mut args = Args {
        command: Command::Parse,
        render_format: RenderFormat::Svg,
        render_scale: 1.0,
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
            "--format" => {
                let Some(fmt) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.render_format = fmt
                    .parse::<RenderFormat>()
                    .map_err(|_| CliError::Usage(usage()))?;
            }
            "--scale" => {
                let Some(scale) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.render_scale = scale.parse::<f32>().map_err(|_| CliError::Usage(usage()))?;
                if !(args.render_scale.is_finite() && args.render_scale > 0.0) {
                    return Err(CliError::Usage(usage()));
                }
            }
            "--background" => {
                let Some(bg) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                if !bg.trim().is_empty() {
                    args.background = Some(bg.trim().to_string());
                }
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
                args.diagram_id = Some(id.trim().to_string());
            }
            "--out" => {
                let Some(out) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.out = Some(out.trim().to_string());
            }
            "--hand-drawn-seed" => {
                let Some(seed) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.hand_drawn_seed =
                    Some(seed.parse::<u64>().map_err(|_| CliError::Usage(usage()))?);
            }
            s if s.starts_with('-') => return Err(CliError::Usage(usage())),
            _ => {
                // Positional: input path
                args.input = Some(a.to_string());
            }
        }
    }

    Ok(args)
}

fn read_input(path: Option<&str>) -> Result<String, CliError> {
    let mut buf = String::new();
    match path {
        None | Some("-") => {
            std::io::stdin().read_to_string(&mut buf)?;
        }
        Some(p) => {
            std::fs::File::open(p)?.read_to_string(&mut buf)?;
        }
    }
    Ok(buf)
}

fn write_output(out: Option<&str>, bytes: &[u8]) -> Result<(), CliError> {
    match out {
        None => {
            use std::io::Write as _;
            std::io::stdout().write_all(bytes)?;
        }
        Some(p) => {
            std::fs::write(p, bytes)?;
        }
    }
    Ok(())
}

fn default_raster_out_path(input: Option<&str>, ext: &str) -> std::path::PathBuf {
    match input {
        Some(path) if path != "-" => {
            let p = std::path::PathBuf::from(path);
            p.with_extension(ext)
        }
        _ => std::path::PathBuf::from(format!("out.{ext}")),
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let result = (|| -> Result<(), CliError> {
        let args = parse_args(&argv)?;
        run(args)
    })();

    match result {
        Ok(()) => {}
        Err(CliError::Usage(msg)) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run(args: Args) -> Result<(), CliError> {
    let text = read_input(args.input.as_deref())?;

    let mut engine = Engine::new();
    if let Some(seed) = args.hand_drawn_seed {
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("handDrawnSeed", serde_json::json!(seed));
        engine = engine.with_site_config(cfg);
    }

    let options = ParseOptions {
        suppress_errors: args.suppress_errors,
    };

    match args.command {
        Command::Detect => {
            let Some(meta) = engine.parse_metadata_sync(&text, options)? else {
                return Err(CliError::NoDiagram);
            };
            println!("{}", meta.diagram_type);
            Ok(())
        }
        Command::Parse => {
            let Some(parsed) = engine.parse_diagram_sync(&text, options)? else {
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
                if args.pretty {
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    println!("{}", serde_json::to_string(&out)?);
                }
            } else if args.pretty {
                println!("{}", serde_json::to_string_pretty(&parsed.model)?);
            } else {
                println!("{}", serde_json::to_string(&parsed.model)?);
            }
            Ok(())
        }
        Command::Layout => {
            let measurer: Arc<dyn TextMeasurer + Send + Sync> = match args.text_measurer {
                TextMeasurerKind::Deterministic => Arc::new(DeterministicTextMeasurer::default()),
                TextMeasurerKind::Vendored => Arc::new(VendoredFontMetricsTextMeasurer::default()),
            };

            let layout = LayoutOptions {
                viewport_width: args.viewport_width,
                viewport_height: args.viewport_height,
                text_measurer: measurer,
                // Mermaid parity for some diagrams (e.g. mindmap/architecture) relies on
                // manatee-backed layout engines. Prefer correctness for CLI output.
                use_manatee_layout: true,
            };

            let Some(layouted) =
                merman::render::layout_diagram_sync(&engine, &text, options, &layout)?
            else {
                return Err(CliError::NoDiagram);
            };
            if args.pretty {
                println!("{}", serde_json::to_string_pretty(&layouted)?);
            } else {
                println!("{}", serde_json::to_string(&layouted)?);
            }
            Ok(())
        }
        Command::Render => {
            let measurer: Arc<dyn TextMeasurer + Send + Sync> = match args.text_measurer {
                TextMeasurerKind::Deterministic => Arc::new(DeterministicTextMeasurer::default()),
                TextMeasurerKind::Vendored => Arc::new(VendoredFontMetricsTextMeasurer::default()),
            };

            let layout = LayoutOptions {
                viewport_width: args.viewport_width,
                viewport_height: args.viewport_height,
                text_measurer: measurer,
                // Mermaid parity for some diagrams (e.g. mindmap/architecture) relies on
                // manatee-backed layout engines. Prefer correctness for CLI output.
                use_manatee_layout: true,
            };

            let svg_opts = SvgRenderOptions {
                diagram_id: args.diagram_id.clone(),
                ..Default::default()
            };

            let Some(svg) =
                merman::render::render_svg_sync(&engine, &text, options, &layout, &svg_opts)?
            else {
                return Err(CliError::NoDiagram);
            };

            match args.render_format {
                RenderFormat::Svg => {
                    let out = args.out.as_deref();
                    write_output(out, svg.as_bytes())
                }
                RenderFormat::Png => {
                    let raster = merman::render::raster::RasterOptions {
                        scale: args.render_scale,
                        background: args.background.clone(),
                        ..Default::default()
                    };
                    let svg = merman::render::foreign_object_label_fallback_svg_text(&svg);
                    let bytes = merman::render::raster::svg_to_png(&svg, &raster)?;
                    let default_out_path = default_raster_out_path(args.input.as_deref(), "png")
                        .to_string_lossy()
                        .into_owned();
                    let out = args.out.as_deref().unwrap_or(default_out_path.as_str());
                    write_output(Some(out), &bytes)
                }
                RenderFormat::Jpeg => {
                    let raster = merman::render::raster::RasterOptions {
                        scale: args.render_scale,
                        background: args.background.clone(),
                        ..Default::default()
                    };
                    let svg = merman::render::foreign_object_label_fallback_svg_text(&svg);
                    let bytes = merman::render::raster::svg_to_jpeg(&svg, &raster)?;
                    let default_out_path = default_raster_out_path(args.input.as_deref(), "jpg")
                        .to_string_lossy()
                        .into_owned();
                    let out = args.out.as_deref().unwrap_or(default_out_path.as_str());
                    write_output(Some(out), &bytes)
                }
                RenderFormat::Pdf => {
                    let svg = merman::render::foreign_object_label_fallback_svg_text(&svg);
                    let bytes = merman::render::raster::svg_to_pdf(&svg)?;
                    let default_out_path = default_raster_out_path(args.input.as_deref(), "pdf")
                        .to_string_lossy()
                        .into_owned();
                    let out = args.out.as_deref().unwrap_or(default_out_path.as_str());
                    write_output(Some(out), &bytes)
                }
            }
        }
    }
}
