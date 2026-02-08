use futures::executor::block_on;
use merman::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, VendoredFontMetricsTextMeasurer,
};
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
                args.diagram_id = Some(id.clone());
            }
            "--out" => {
                let Some(out) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.out = Some(out.clone());
            }
            "--hand-drawn-seed" => {
                let Some(seed) = it.next() else {
                    return Err(CliError::Usage(usage()));
                };
                args.hand_drawn_seed =
                    Some(seed.parse::<u64>().map_err(|_| CliError::Usage(usage()))?);
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

fn default_raster_out_path(input: Option<&str>, ext: &str) -> std::path::PathBuf {
    match input {
        Some(path) if path != "-" => {
            let p = std::path::PathBuf::from(path);
            if p.extension().is_some() {
                p.with_extension(ext)
            } else {
                p.with_extension(ext)
            }
        }
        _ => std::path::PathBuf::from(format!("out.{ext}")),
    }
}

fn parse_svg_viewbox(svg: &str) -> Option<(f32, f32)> {
    // Cheap, non-validating parse for root viewBox: `viewBox="minX minY w h"`.
    // This is sufficient for our own Mermaid-like SVG output.
    let i = svg.find("viewBox=\"")?;
    let rest = &svg[i + "viewBox=\"".len()..];
    let end = rest.find('"')?;
    let raw = &rest[..end];
    let mut it = raw.split_whitespace();
    let _min_x = it.next()?.parse::<f32>().ok()?;
    let _min_y = it.next()?.parse::<f32>().ok()?;
    let w = it.next()?.parse::<f32>().ok()?;
    let h = it.next()?.parse::<f32>().ok()?;
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
        Some((w, h))
    } else {
        None
    }
}

fn render_svg_to_pixmap(
    svg: &str,
    scale: f32,
    background: Option<&str>,
) -> Result<tiny_skia::Pixmap, CliError> {
    let (vb_w, vb_h) =
        parse_svg_viewbox(svg).ok_or(CliError::Usage("render requires an SVG root viewBox"))?;

    let width_px = (vb_w * scale).ceil().max(1.0) as u32;
    let height_px = (vb_h * scale).ceil().max(1.0) as u32;

    let mut opt = usvg::Options::default();
    // Keep output stable-ish across environments while still using system fonts.
    opt.fontdb_mut().load_system_fonts();
    // Mermaid baseline assumes a sans-serif stack; system selection may vary, but this is best-effort.
    opt.font_family = "Arial".to_string();

    let tree = usvg::Tree::from_str(svg, &opt)
        .map_err(|_| CliError::Usage("failed to parse SVG for PNG rendering"))?;

    let mut pixmap = tiny_skia::Pixmap::new(width_px, height_px).ok_or(CliError::Usage(
        "failed to allocate pixmap for raster rendering",
    ))?;

    if let Some(bg) = background {
        if let Some(color) = parse_tiny_skia_color(bg) {
            pixmap.fill(color);
        }
    }

    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(pixmap)
}

fn render_svg_to_png(svg: &str, scale: f32, background: Option<&str>) -> Result<Vec<u8>, CliError> {
    let pixmap = render_svg_to_pixmap(svg, scale, background)?;
    pixmap
        .encode_png()
        .map_err(|_| CliError::Usage("failed to encode PNG"))
}

fn render_svg_to_jpeg(
    svg: &str,
    scale: f32,
    background: Option<&str>,
) -> Result<Vec<u8>, CliError> {
    let bg = background.unwrap_or("white");
    let Some(color) = parse_tiny_skia_color(bg) else {
        return Err(CliError::Usage(
            "invalid --background color for JPG rendering",
        ));
    };
    if color.alpha() != 1.0 {
        return Err(CliError::Usage(
            "JPG rendering requires an opaque --background (e.g. white)",
        ));
    }

    let pixmap = render_svg_to_pixmap(svg, scale, Some(bg))?;
    let (w, h) = (pixmap.width(), pixmap.height());

    // tiny-skia renders into an RGBA8 buffer. When the destination is opaque (we always fill a
    // solid background for JPG), the alpha channel is always 255 and can be dropped safely.
    let rgba = pixmap.data();
    let mut rgb = vec![0u8; (w as usize) * (h as usize) * 3];
    for (src, dst) in rgba.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
    }

    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90);
    enc.encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|_| CliError::Usage("failed to encode JPG"))?;
    Ok(out)
}

fn render_svg_to_pdf(svg: &str) -> Result<Vec<u8>, CliError> {
    let mut opt = svg2pdf::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    // Keep output stable-ish across environments while still using system fonts.
    opt.font_family = "Arial".to_string();

    let tree = svg2pdf::usvg::Tree::from_str(svg, &opt)
        .map_err(|_| CliError::Usage("failed to parse SVG for PDF rendering"))?;

    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|_| CliError::Usage("failed to convert SVG to PDF"))
}

fn parse_tiny_skia_color(text: &str) -> Option<tiny_skia::Color> {
    let s = text.trim().to_ascii_lowercase();
    match s.as_str() {
        "transparent" => return Some(tiny_skia::Color::from_rgba8(0, 0, 0, 0)),
        "white" => return Some(tiny_skia::Color::from_rgba8(255, 255, 255, 255)),
        "black" => return Some(tiny_skia::Color::from_rgba8(0, 0, 0, 255)),
        _ => {}
    }

    let hex = s.strip_prefix('#')?;
    fn hex2(b: &[u8]) -> Option<u8> {
        let hi = (*b.get(0)? as char).to_digit(16)? as u8;
        let lo = (*b.get(1)? as char).to_digit(16)? as u8;
        Some((hi << 4) | lo)
    }
    fn hex1(c: u8) -> Option<u8> {
        let v = (c as char).to_digit(16)? as u8;
        Some((v << 4) | v)
    }

    let bytes = hex.as_bytes();
    match bytes.len() {
        3 => Some(tiny_skia::Color::from_rgba8(
            hex1(bytes[0])?,
            hex1(bytes[1])?,
            hex1(bytes[2])?,
            255,
        )),
        4 => Some(tiny_skia::Color::from_rgba8(
            hex1(bytes[0])?,
            hex1(bytes[1])?,
            hex1(bytes[2])?,
            hex1(bytes[3])?,
        )),
        6 => Some(tiny_skia::Color::from_rgba8(
            hex2(&bytes[0..2])?,
            hex2(&bytes[2..4])?,
            hex2(&bytes[4..6])?,
            255,
        )),
        8 => Some(tiny_skia::Color::from_rgba8(
            hex2(&bytes[0..2])?,
            hex2(&bytes[2..4])?,
            hex2(&bytes[4..6])?,
            hex2(&bytes[6..8])?,
        )),
        _ => None,
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
                use_manatee_layout: false,
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
                use_manatee_layout: false,
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

            match args.render_format {
                RenderFormat::Svg => {
                    write_text(&svg, args.out.as_deref())?;
                }
                RenderFormat::Png => {
                    let bytes =
                        render_svg_to_png(&svg, args.render_scale, args.background.as_deref())?;
                    let out = args.out.clone().unwrap_or_else(|| {
                        default_raster_out_path(args.input.as_deref(), "png")
                            .to_string_lossy()
                            .to_string()
                    });
                    if out == "-" {
                        use std::io::Write;
                        std::io::stdout().lock().write_all(&bytes)?;
                    } else {
                        std::fs::write(out, bytes)?;
                    }
                }
                RenderFormat::Jpeg => {
                    let bytes =
                        render_svg_to_jpeg(&svg, args.render_scale, args.background.as_deref())?;
                    let out = args.out.clone().unwrap_or_else(|| {
                        default_raster_out_path(args.input.as_deref(), "jpg")
                            .to_string_lossy()
                            .to_string()
                    });
                    if out == "-" {
                        use std::io::Write;
                        std::io::stdout().lock().write_all(&bytes)?;
                    } else {
                        std::fs::write(out, bytes)?;
                    }
                }
                RenderFormat::Pdf => {
                    let bytes = render_svg_to_pdf(&svg)?;
                    let out = args.out.clone().unwrap_or_else(|| {
                        default_raster_out_path(args.input.as_deref(), "pdf")
                            .to_string_lossy()
                            .to_string()
                    });
                    if out == "-" {
                        use std::io::Write;
                        std::io::stdout().lock().write_all(&bytes)?;
                    } else {
                        std::fs::write(out, bytes)?;
                    }
                }
            }
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
