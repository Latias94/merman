#![forbid(unsafe_code)]

use crate::render::{HeadlessError, LayoutOptions, SvgPipeline, SvgRenderOptions};
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
    let Some(svg) = super::render_svg_with_pipeline_sync(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
        &SvgPipeline::resvg_safe(),
    )?
    else {
        return Ok(None);
    };
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
    let Some(svg) = super::render_svg_with_pipeline_sync(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
        &SvgPipeline::resvg_safe(),
    )?
    else {
        return Ok(None);
    };
    Ok(Some(svg_to_jpeg(&svg, raster)?))
}

pub fn render_pdf_sync(
    engine: &Engine,
    text: &str,
    parse_options: ParseOptions,
    layout_options: &LayoutOptions,
    svg_options: &SvgRenderOptions,
) -> Result<Option<Vec<u8>>> {
    let Some(svg) = super::render_svg_with_pipeline_sync(
        engine,
        text,
        parse_options,
        layout_options,
        svg_options,
        &SvgPipeline::resvg_safe(),
    )?
    else {
        return Ok(None);
    };
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
    configure_usvg_options_for_raster(&mut opt, svg);

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

    // Make scaling more intuitive/stable: at scale=1 we already round up to whole pixels, so for
    // scale>1 prefer scaling the *rounded* base size. This avoids surprising off-by-one shrinkage
    // when the viewBox/bounds are fractional (e.g. 342.36 * 2 = 684.72 → ceil = 685, while
    // ceil(342.36) * 2 = 686).
    let base_width_px = geo.width.ceil().max(1.0);
    let base_height_px = geo.height.ceil().max(1.0);
    let width_px = (base_width_px * scale).ceil().max(1.0) as u32;
    let height_px = (base_height_px * scale).ceil().max(1.0) as u32;

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

fn configure_usvg_options_for_raster(opt: &mut usvg::Options<'_>, svg: &str) {
    opt.fontdb_mut().load_system_fonts();

    if parse_svg_viewbox(svg).is_none() {
        if let Some(max_width) = parse_svg_max_width_px(svg) {
            if max_width.is_finite() && max_width > 0.0 {
                if let Some(size) = usvg::Size::from_wh(max_width, opt.default_size.height()) {
                    opt.default_size = size;
                }
            }
        }
    }

    configure_fontdb_generic_families(opt.fontdb_mut());
    opt.font_family =
        raster_default_font_family(opt.fontdb.as_ref()).unwrap_or_else(|| "Arial".to_string());
    opt.font_resolver = browser_like_font_resolver();
}

fn browser_like_font_resolver() -> usvg::FontResolver<'static> {
    let default_select = usvg::FontResolver::default_font_selector();

    usvg::FontResolver {
        select_font: Box::new(move |font, fontdb| {
            default_select(font, fontdb)
                .or_else(|| query_browser_like_fallback_font(font, fontdb.as_ref()))
                .or_else(|| fontdb.faces().next().map(|face| face.id))
        }),
        select_fallback: usvg::FontResolver::default_fallback_selector(),
    }
}

fn configure_fontdb_generic_families(fontdb: &mut usvg::fontdb::Database) {
    let sans = first_font_family(fontdb, |face| !face.monospaced)
        .or_else(|| first_font_family(fontdb, |_| true));
    let mono = first_font_family(fontdb, |face| face.monospaced).or_else(|| sans.clone());

    if query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif).is_none() {
        if let Some(family) = sans.as_ref() {
            fontdb.set_sans_serif_family(family.clone());
        }
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Serif).is_none() {
        if let Some(family) = sans.as_ref() {
            fontdb.set_serif_family(family.clone());
        }
    }
    if query_normal_font_family(fontdb, usvg::fontdb::Family::Monospace).is_none() {
        if let Some(family) = mono.as_ref() {
            fontdb.set_monospace_family(family.clone());
        }
    }
}

fn raster_default_font_family(fontdb: &usvg::fontdb::Database) -> Option<String> {
    query_normal_font_family(fontdb, usvg::fontdb::Family::SansSerif)
        .or_else(|| query_normal_font_family(fontdb, usvg::fontdb::Family::Serif))
        .or_else(|| first_font_family(fontdb, |_| true))
}

fn query_browser_like_fallback_font(
    font: &usvg::Font,
    fontdb: &usvg::fontdb::Database,
) -> Option<usvg::fontdb::ID> {
    let mut families = Vec::with_capacity(3);
    if font_requests_monospace(font) {
        families.push(usvg::fontdb::Family::Monospace);
        families.push(usvg::fontdb::Family::SansSerif);
        families.push(usvg::fontdb::Family::Serif);
    } else {
        families.push(usvg::fontdb::Family::SansSerif);
        families.push(usvg::fontdb::Family::Serif);
        families.push(usvg::fontdb::Family::Monospace);
    }

    let query = usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font.weight()),
        stretch: font.stretch().into(),
        style: font.style().into(),
    };
    fontdb.query(&query)
}

fn query_normal_font_family(
    fontdb: &usvg::fontdb::Database,
    family: usvg::fontdb::Family<'_>,
) -> Option<String> {
    let families = [family];
    let query = usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight::NORMAL,
        stretch: usvg::fontdb::Stretch::Normal,
        style: usvg::fontdb::Style::Normal,
    };
    fontdb
        .query(&query)
        .and_then(|id| fontdb.face(id))
        .and_then(face_family_name)
}

fn first_font_family<F>(fontdb: &usvg::fontdb::Database, mut predicate: F) -> Option<String>
where
    F: FnMut(&usvg::fontdb::FaceInfo) -> bool,
{
    fontdb
        .faces()
        .find(|face| predicate(face))
        .and_then(face_family_name)
}

fn face_family_name(face: &usvg::fontdb::FaceInfo) -> Option<String> {
    face.families
        .iter()
        .find(|(_, lang)| *lang == usvg::fontdb::Language::English_UnitedStates)
        .or_else(|| face.families.first())
        .map(|(family, _)| family.clone())
}

fn font_requests_monospace(font: &usvg::Font) -> bool {
    font.families().iter().any(|family| match family {
        usvg::FontFamily::Monospace => true,
        usvg::FontFamily::Named(name) => {
            let name = name.to_ascii_lowercase();
            name.contains("mono")
                || name.contains("courier")
                || name.contains("consolas")
                || name.contains("menlo")
        }
        _ => false,
    })
}

fn parse_svg_max_width_px(svg: &str) -> Option<f32> {
    let start = svg.find("max-width:")? + "max-width:".len();
    let rest = svg[start..].trim_start();
    let px_end = rest.find("px")?;
    rest[..px_end].trim().parse::<f32>().ok()
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

    #[test]
    fn svg_to_png_keeps_text_visible_when_requested_font_is_missing() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width: 400px; background-color: white;"><text x="100" y="40" fill="#333333" font-size="32" style="font-family: '__merman_missing_font__'; text-anchor: middle;">v11.12.2</text></svg>"##;
        let bytes = svg_to_png(svg, &RasterOptions::default()).unwrap();
        assert_png_has_visible_non_background_ink(&bytes);
    }

    fn assert_png_has_visible_non_background_ink(bytes: &[u8]) {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        let pixels = img.as_raw();
        let background = &pixels[..4];
        let differing_pixels = pixels
            .chunks_exact(4)
            .filter(|px| {
                let alpha_delta = px[3].abs_diff(background[3]) as u16;
                let rgb_delta = px[0].abs_diff(background[0]) as u16
                    + px[1].abs_diff(background[1]) as u16
                    + px[2].abs_diff(background[2]) as u16;
                alpha_delta > 3 || (px[3] > 0 && rgb_delta > 8)
            })
            .take(16)
            .count();
        assert!(
            differing_pixels >= 8,
            "expected visible text ink in rasterized PNG"
        );
    }
}
