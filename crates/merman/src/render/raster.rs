#![forbid(unsafe_code)]

use crate::render::{HeadlessError, LayoutOptions, SvgRenderOptions};
use merman_core::{Engine, ParseOptions};

#[derive(Debug, thiserror::Error)]
pub enum RasterError {
    #[error(transparent)]
    Headless(#[from] HeadlessError),
    #[error("failed to parse SVG")]
    SvgParse,
    #[error("failed to allocate pixmap for raster rendering")]
    PixmapAlloc,
    #[error("failed to encode PNG")]
    PngEncode,
    #[error("invalid background color for JPG rendering")]
    JpegBackground,
    #[error("JPG rendering requires an opaque background color (e.g. white)")]
    JpegOpaqueBackgroundRequired,
    #[error("failed to encode JPG")]
    JpegEncode,
    #[error("failed to convert SVG to PDF")]
    PdfConvert,
}

pub type Result<T> = std::result::Result<T, RasterError>;

#[derive(Debug, Clone)]
pub struct RasterOptions {
    pub scale: f32,
    pub background: Option<String>,
    pub jpeg_quality: u8,
}

impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            background: None,
            jpeg_quality: 90,
        }
    }
}

pub fn render_png_sync(
    engine: &Engine,
    text: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    raster: &RasterOptions,
) -> Result<Option<Vec<u8>>> {
    let Some(svg) =
        super::render_svg_sync(engine, text, parse_options, layout_options, svg_options)?
    else {
        return Ok(None);
    };
    let svg = super::foreign_object_label_fallback_svg_text(&svg);
    Ok(Some(svg_to_png(&svg, raster)?))
}

pub fn render_jpeg_sync(
    engine: &Engine,
    text: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
    raster: &RasterOptions,
) -> Result<Option<Vec<u8>>> {
    let Some(svg) =
        super::render_svg_sync(engine, text, parse_options, layout_options, svg_options)?
    else {
        return Ok(None);
    };
    let svg = super::foreign_object_label_fallback_svg_text(&svg);
    Ok(Some(svg_to_jpeg(&svg, raster)?))
}

pub fn render_pdf_sync(
    engine: &Engine,
    text: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<Vec<u8>>> {
    let Some(svg) =
        super::render_svg_sync(engine, text, parse_options, layout_options, svg_options)?
    else {
        return Ok(None);
    };
    let svg = super::foreign_object_label_fallback_svg_text(&svg);
    Ok(Some(svg_to_pdf(&svg)?))
}

pub fn svg_to_png(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
    let pixmap = svg_to_pixmap(svg, options.scale, options.background.as_deref())?;
    pixmap.encode_png().map_err(|_| RasterError::PngEncode)
}

pub fn svg_to_jpeg(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
    let bg = options.background.as_deref().unwrap_or("white");
    let Some(color) = parse_tiny_skia_color(bg) else {
        return Err(RasterError::JpegBackground);
    };
    if color.alpha() != 1.0 {
        return Err(RasterError::JpegOpaqueBackgroundRequired);
    }

    let pixmap = svg_to_pixmap(svg, options.scale, Some(bg))?;
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
    let mut enc =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, options.jpeg_quality);
    enc.encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|_| RasterError::JpegEncode)?;
    Ok(out)
}

pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let mut opt = svg2pdf::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    // Keep output stable-ish across environments while still using system fonts.
    opt.font_family = "Arial".to_string();

    let tree = svg2pdf::usvg::Tree::from_str(svg, &opt).map_err(|_| RasterError::SvgParse)?;

    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|_| RasterError::PdfConvert)
}

#[derive(Debug, Clone, Copy)]
struct ParsedViewBox {
    width: f32,
    height: f32,
}

fn parse_svg_viewbox(svg: &str) -> Option<ParsedViewBox> {
    // Cheap, non-validating parse for root viewBox: `viewBox="minX minY w h"`.
    // This is sufficient for Mermaid-like SVG output.
    let i = svg.find("viewBox=\"")?;
    let rest = &svg[i + "viewBox=\"".len()..];
    let end = rest.find('"')?;
    let raw = &rest[..end];
    let mut it = raw.split_whitespace();
    let _min_x = it.next()?.parse::<f32>().ok()?;
    let _min_y = it.next()?.parse::<f32>().ok()?;
    let width = it.next()?.parse::<f32>().ok()?;
    let height = it.next()?.parse::<f32>().ok()?;
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        Some(ParsedViewBox { width, height })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct RasterGeometry {
    min_x: f32,
    min_y: f32,
    width: f32,
    height: f32,
}

fn svg_to_pixmap(svg: &str, scale: f32, background: Option<&str>) -> Result<tiny_skia::Pixmap> {
    let mut opt = usvg::Options::default();
    // Keep output stable-ish across environments while still using system fonts.
    opt.fontdb_mut().load_system_fonts();
    // Mermaid baseline assumes a sans-serif stack; system selection may vary, but this is best-effort.
    opt.font_family = "Arial".to_string();

    let tree = usvg::Tree::from_str(svg, &opt).map_err(|_| RasterError::SvgParse)?;

    let (geo, translate_min_to_origin) = if let Some(vb) = parse_svg_viewbox(svg) {
        // `usvg`/`resvg` already apply the root viewBox transform (including translating the
        // viewBox min corner to (0,0)) when building/rendering the tree. If we also translate
        // by `-min_x/-min_y` here, diagrams with negative viewBox mins (e.g. kanban, gitGraph)
        // get shifted fully out of the viewport and render as a blank/transparent pixmap.
        (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: vb.width,
                height: vb.height,
            },
            false,
        )
    } else {
        // Some Mermaid diagrams (e.g. `info`) don't emit a viewBox upstream.
        // For raster formats, fall back to the rendered content bounds as computed by usvg.
        let bbox = tree.root().abs_stroke_bounding_box();
        let w = bbox.width().max(1.0);
        let h = bbox.height().max(1.0);
        if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
            (
                RasterGeometry {
                    min_x: bbox.x(),
                    min_y: bbox.y(),
                    width: w,
                    height: h,
                },
                true,
            )
        } else {
            let size = tree.size();
            (
                RasterGeometry {
                    min_x: 0.0,
                    min_y: 0.0,
                    width: size.width(),
                    height: size.height(),
                },
                false,
            )
        }
    };

    let width_px = (geo.width * scale).ceil().max(1.0) as u32;
    let height_px = (geo.height * scale).ceil().max(1.0) as u32;

    let mut pixmap = tiny_skia::Pixmap::new(width_px, height_px).ok_or(RasterError::PixmapAlloc)?;

    if let Some(bg) = background {
        if let Some(color) = parse_tiny_skia_color(bg) {
            pixmap.fill(color);
        }
    }

    let transform = if translate_min_to_origin {
        // Render at `scale`, translating so min_x/min_y map to (0,0).
        tiny_skia::Transform::from_row(
            scale,
            0.0,
            0.0,
            scale,
            -geo.min_x * scale,
            -geo.min_y * scale,
        )
    } else {
        tiny_skia::Transform::from_scale(scale, scale)
    };

    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
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
        let hi = (*b.first()? as char).to_digit(16)? as u8;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_to_png_produces_png_signature() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
        let bytes = svg_to_png(svg, &RasterOptions::default()).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn svg_to_pdf_produces_pdf_signature() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
        let bytes = svg_to_pdf(svg).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }
}
