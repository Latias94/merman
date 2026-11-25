use crate::text::Font;
use crate::text::{GlyphId, KrillaGlyph};
use rustybuzz::ttf_parser::Tag;
use rustybuzz::{Direction, UnicodeBuffer};

/// Naively shape some text with a single font.
pub(crate) fn naive_shape(text: &str, font: Font, direction: TextDirection) -> Vec<KrillaGlyph> {
    let data = font.font_data();
    let mut rb_font = rustybuzz::Face::from_slice(data.as_ref(), font.index()).unwrap();

    for variation in font.variation_coordinates() {
        rb_font.set_variation(Tag::from_bytes(variation.0.get()), variation.1.get());
    }

    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();

    match direction {
        TextDirection::LeftToRight => buffer.set_direction(Direction::LeftToRight),
        TextDirection::RightToLeft => buffer.set_direction(Direction::RightToLeft),
        TextDirection::TopToBottom => buffer.set_direction(Direction::TopToBottom),
        TextDirection::BottomToTop => buffer.set_direction(Direction::BottomToTop),
        TextDirection::Auto => {}
    }

    let dir = buffer.direction();

    let output = rustybuzz::shape(&rb_font, &[], buffer);

    let positions = output.glyph_positions();
    let infos = output.glyph_infos();

    let mut glyphs = vec![];

    for i in 0..output.len() {
        let pos = positions[i];
        let start_info = infos[i];

        let start = start_info.cluster as usize;

        let end = if dir == Direction::LeftToRight || dir == Direction::TopToBottom {
            let mut e = i.checked_add(1);
            loop {
                if let Some(index) = e {
                    if let Some(end_info) = infos.get(index) {
                        if end_info.cluster == start_info.cluster {
                            e = index.checked_add(1);
                            continue;
                        }
                    }
                }

                break;
            }

            e
        } else {
            let mut e = i.checked_sub(1);
            while let Some(index) = e {
                if let Some(end_info) = infos.get(index) {
                    if end_info.cluster == start_info.cluster {
                        e = index.checked_sub(1);
                    } else {
                        break;
                    }
                }
            }

            e
        }
        .and_then(|last| infos.get(last))
        .map_or(text.len(), |info| info.cluster as usize);

        glyphs.push(KrillaGlyph::new(
            GlyphId::new(start_info.glyph_id),
            pos.x_advance as f32 / font.units_per_em(),
            pos.x_offset as f32 / font.units_per_em(),
            pos.y_offset as f32 / font.units_per_em(),
            pos.y_advance as f32 / font.units_per_em(),
            start..end,
            None,
        ));
    }

    glyphs
}

/// The direction of a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextDirection {
    /// Determine the direction automatically.
    Auto,
    /// Left to right.
    LeftToRight,
    /// Right to left.
    RightToLeft,
    /// Top to bottom.
    TopToBottom,
    /// Bottom to top.
    BottomToTop,
}
