/*!
An extension to krilla that allows rendering SVG files to a PDF file.

It is based on [usvg](https://github.com/linebender/resvg) and passes nearly the whole
resvg test suite. See the [examples]( https://github.com/LaurenzV/krilla/tree/main/crates/krilla-svg/examples)
directory for an example on how to use this crate in combination with krilla to convert SVG files
to PDF.
*/

#![deny(missing_docs)]

use std::io::Read;

use krilla::color::rgb;
use krilla::geom::{Rect, Size, Transform};
use krilla::paint::FillRule;
use krilla::surface::Surface;
use krilla::text::GlyphId;
use usvg::fontdb::Database;
use usvg::{roxmltree, Node, Tree};

use crate::text::Fonts;
use crate::util::RectExt;

mod clip_path;
mod filter;
mod group;
mod image;
mod mask;
mod path;
mod text;
mod util;

/// Settings that should be applied when converting a SVG.
#[derive(Copy, Clone, Debug)]
pub struct SvgSettings {
    /// Whether text should be embedded as properly selectable text. Otherwise,
    /// it will be drawn as outlined paths instead.
    pub embed_text: bool,
    /// How much filters, which will be converted to bitmaps, should be scaled. Higher values
    /// mean better quality, but also bigger file sizes.
    pub filter_scale: f32,
}

impl Default for SvgSettings {
    fn default() -> Self {
        Self {
            embed_text: true,
            filter_scale: 4.0,
        }
    }
}

/// An extension trait for the `Surface` struct that allows you to draw SVGs onto a surface.
pub trait SurfaceExt {
    /// Draw a `usvg` tree onto a surface with the given size and settings.
    fn draw_svg(&mut self, tree: &Tree, size: Size, svg_settings: SvgSettings) -> Option<()>;
}

impl SurfaceExt for Surface<'_> {
    fn draw_svg(&mut self, tree: &Tree, size: Size, svg_settings: SvgSettings) -> Option<()> {
        let old_fill = self.get_fill().cloned();
        let old_stroke = self.get_stroke().cloned();

        let transform = Transform::from_scale(
            size.width() / tree.size().width(),
            size.height() / tree.size().height(),
        );
        self.push_transform(&transform);
        self.push_clip_path(
            &Rect::from_xywh(0.0, 0.0, tree.size().width(), tree.size().height())
                .unwrap()
                .to_clip_path(),
            &FillRule::NonZero,
        );
        render_tree(tree, svg_settings, self);
        self.pop();
        self.pop();

        self.set_fill(old_fill);
        self.set_stroke(old_stroke);

        Some(())
    }
}

struct ProcessContext<'a> {
    pub(crate) fonts: Fonts<'a>,
    svg_settings: SvgSettings,
}

impl<'a> ProcessContext<'a> {
    fn new(tree_fontdb: &'a Database, svg_settings: SvgSettings) -> Self {
        Self {
            fonts: Fonts::new(tree_fontdb),
            svg_settings,
        }
    }
}

pub(crate) fn render_tree(tree: &Tree, svg_settings: SvgSettings, surface: &mut Surface) {
    let mut fc = ProcessContext::new(tree.fontdb(), svg_settings);
    group::render(tree.root(), surface, &mut fc);
}

fn render_node(
    node: &Node,
    tree_fontdb: &Database,
    svg_settings: SvgSettings,
    surface: &mut Surface,
) {
    let mut fc = ProcessContext::new(tree_fontdb, svg_settings);
    group::render_node(node, surface, &mut fc);
}

/// Render an SVG glyph from an OpenType font into a surface. You can plug this method into the
/// `render_svg_glyph_fn` field of `SerializeSettings` in krilla..
pub fn render_svg_glyph(
    data: &[u8],
    context_color: rgb::Color,
    glyph: GlyphId,
    default_size: (f32, f32),
    surface: &mut Surface,
) -> Option<()> {
    let mut data = data;
    let settings = SvgSettings::default();

    let default_size = usvg::Size::from_wh(default_size.0, default_size.1).unwrap();

    let mut decoded = vec![];
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = flate2::read::GzDecoder::new(data);
        decoder.read_to_end(&mut decoded).ok()?;
        data = &decoded;
    }

    let xml = std::str::from_utf8(data).ok()?;
    // Incredibly hacky, but hopefully that's enough for SVG glyphs.
    let has_viewbox = xml.contains("viewBox");
    let document = roxmltree::Document::parse(xml).ok()?;

    // Reparsing every time might be pretty slow in some cases, because Noto Color Emoji
    // for example contains hundreds of glyphs in the same SVG document, meaning that we have
    // to reparse it every time. However, Twitter Color Emoji does have each glyph in a
    // separate SVG document, and since we use COLRv1 for Noto Color Emoji anyway, this is
    // good enough.
    let opts = usvg::Options {
        style_sheet: Some(format!(
            "svg {{ color: rgb({}, {}, {}) }}",
            context_color.red(),
            context_color.green(),
            context_color.blue()
        )),
        default_size,
        ..Default::default()
    };
    let tree = Tree::from_xmltree(&document, &opts).ok()?;

    let apply_scale = default_size != tree.size() && has_viewbox;

    // From the specification:
    //
    // The size of the initial viewport for the SVG document is the em square:
    // height and width both equal to head.unitsPerEm. If a viewBox
    // attribute is specified on the <svg> element with width or
    // height values different from the unitsPerEm value,
    // this will have the effect of a scale transformation on the SVG “user” coordinate
    // system.
    if apply_scale {
        let scale = (default_size.width() / tree.size().width())
            .min(default_size.height() / tree.size().height());
        surface.push_transform(&Transform::from_scale(scale, scale))
    }

    if let Some(node) = tree.node_by_id(&format!("glyph{}", glyph.to_u32())) {
        render_node(node, tree.fontdb(), settings, surface)
    } else {
        // Twitter Color Emoji SVGs contain the glyph ID on the root element, which isn't saved by
        // usvg. So in this case, we simply draw the whole document.
        render_tree(&tree, settings, surface)
    };

    if apply_scale {
        surface.pop();
    }

    Some(())
}
