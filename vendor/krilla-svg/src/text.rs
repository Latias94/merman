use std::collections::HashMap;
use std::path::Path;

use krilla::color::rgb;
use krilla::geom::Point;
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph, Tag};
use krilla::Data;
use usvg::fontdb::{Database, Source, ID};
use usvg::{FontVariation, PaintOrder};

use crate::util::{convert_fill, convert_stroke, UsvgTransformExt};
use crate::{path, ProcessContext};

/// Render a text into a surface.
pub(crate) fn render(
    text: &usvg::Text,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    for span in text.layouted() {
        if !span.visible {
            continue;
        }

        if let Some(overline) = &span.overline {
            path::render(overline, surface, process_context);
        }

        if let Some(underline) = &span.underline {
            path::render(underline, surface, process_context);
        }

        for glyph in &span.positioned_glyphs {
            // Ignore glyph if font can't be fetched.
            let Some(font) = process_context.fonts.retrieve(glyph.font, &span.variations) else {
                continue;
            };

            let font_size = glyph.font_size();

            // Krilla applies the font size itself, including for color glyphs.
            let unscale = font.units_per_em() / font_size;
            let transform = glyph.transform().pre_scale(unscale, unscale);

            let Some(inverted) = transform.invert() else {
                continue;
            };

            // We need to apply the inverse transform to fill/stroke because we don't
            // want the paint to be affected by the transform applied to the glyph. See docs
            // of `convert_paint`.
            let fill = span
                .fill
                .as_ref()
                .map(|f| convert_fill(f, surface.stream_builder(), process_context, inverted));

            let stroke = span
                .stroke
                .as_ref()
                .map(|s| convert_stroke(s, surface.stream_builder(), process_context, inverted));

            let draw_op = |s: &mut Surface,
                           fill: Option<Fill>,
                           stroke: Option<Stroke>,
                           font: Font,
                           embed_text: bool| {
                s.set_fill(fill);
                s.set_stroke(stroke);

                s.draw_glyphs(
                    Point::from_xy(0.0, 0.0),
                    &[KrillaGlyph::new(
                        GlyphId::new(glyph.id.0),
                        // Don't care about those, since we render only one glyph.
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0..glyph.text.len(),
                        None,
                    )],
                    font,
                    &glyph.text,
                    font_size,
                    !embed_text,
                );
            };

            surface.push_transform(&transform.to_krilla());

            if fill.is_none() && stroke.is_none() {
                // Emulate invisible glyph by drawing it with an opacity of zero.
                draw_op(
                    surface,
                    Some(Fill {
                        paint: rgb::Color::new(0, 0, 0).into(),
                        opacity: NormalizedF32::ZERO,
                        rule: Default::default(),
                    }),
                    None,
                    font,
                    process_context.svg_settings.embed_text,
                )
            } else if matches!(span.paint_order, PaintOrder::FillAndStroke)
                || fill.is_none()
                || stroke.is_none()
            {
                draw_op(
                    surface,
                    fill,
                    stroke,
                    font.clone(),
                    process_context.svg_settings.embed_text,
                );
            } else {
                // Paint order stroke and fill, and we have BOTH, a fill and
                // stroke.

                // We always draw the text outlined in this case, so that
                // text won't be embedded twice.
                draw_op(surface, None, stroke, font.clone(), false);

                draw_op(
                    surface,
                    fill,
                    None,
                    font.clone(),
                    process_context.svg_settings.embed_text,
                );
            }

            surface.pop();
        }

        if let Some(line_through) = &span.line_through {
            path::render(line_through, surface, process_context);
        }
    }
}

/// Manages the krilla fonts used by an SVG.
pub(crate) struct Fonts<'a> {
    db: &'a Database,
    fonts: HashMap<ID, HashMap<Vec<FontVariation>, Option<Font>>>,
    files: HashMap<&'a Path, Option<Data>>,
}

impl<'a> Fonts<'a> {
    pub(crate) fn new(db: &'a Database) -> Self {
        Self {
            db,
            fonts: HashMap::new(),
            files: HashMap::new(),
        }
    }

    fn retrieve(&mut self, id: ID, variations: &[FontVariation]) -> Option<Font> {
        let instances = self.fonts.entry(id).or_default();
        if let Some(font) = instances.get(variations) {
            return font.clone();
        }

        let font = self.db.face(id).and_then(|face| {
            let data = match &face.source {
                Source::Binary(data) | Source::SharedFile(_, data) => data.clone().into(),
                Source::File(path) => self
                    .files
                    .entry(path.as_path())
                    .or_insert_with(|| std::fs::read(path).ok().map(Data::from))
                    .clone()?,
            };

            // The layout resolves the design coordinates for the chosen face and size.
            let coords: Vec<_> = variations
                .iter()
                .map(|axis| (Tag::new(&axis.tag), axis.value))
                .collect();
            Font::new_variable(data, face.index, &coords)
        });
        instances.insert(variations.to_vec(), font.clone());
        font
    }
}
