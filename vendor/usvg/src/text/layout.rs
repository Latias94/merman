// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::num::NonZeroU16;
use std::ops::Range;
use std::sync::Arc;

use fontdb::{Database, ID};
use harfrust::{
    Buffer, Direction, Feature, ShapeOptions, ShapePlan, ShapePlanKey, ShaperFont, Tag,
};
use kurbo::{ParamCurve, ParamCurveArclen, ParamCurveDeriv};
use strict_num::NonZeroPositiveF32;
use tiny_skia_path::{NonZeroRect, Transform};
use unicode_script::{Script, UnicodeScript};

use super::cache::{FontCache, LoadedInstance};
use crate::tree::BBox;
use crate::{
    AlignmentBaseline, ApproxZeroUlps, BaselineShift, DominantBaseline, Fill, FillRule, Font,
    FontInstance, FontOpticalSizing, FontStyle, FontVariation, GlyphId, LengthAdjust, PaintOrder,
    Path, ShapeRendering, Stroke, Text, TextAnchor, TextChunk, TextDecoration, TextDecorationStyle,
    TextFlow, TextPath, TextSpan, WritingMode,
};

/// A glyph that has already been positioned correctly.
///
/// Note that the transform already takes the font size into consideration, so applying the
/// transform to the outline of the glyphs is all that is necessary to display it correctly.
#[derive(Clone, Debug)]
pub struct PositionedGlyph {
    /// Returns the transform of the glyph itself within the cluster. For example,
    /// for zalgo text, it contains the transform to position the glyphs above/below
    /// the main glyph.
    glyph_ts: Transform,
    /// Returns the transform of the whole cluster that the glyph is part of.
    cluster_ts: Transform,
    /// Returns the transform of the span that the glyph is a part of.
    span_ts: Transform,
    /// The units per em of the font the glyph belongs to.
    units_per_em: u16,
    /// The font size the glyph should be scaled to.
    font_size: f32,
    /// The ID of the glyph.
    pub id: GlyphId,
    /// The text from the original string that corresponds to that glyph.
    pub text: String,
    /// The ID of the font the glyph should be taken from. Can be used with the
    /// [font database of the tree](crate::Tree::fontdb) this glyph is part of.
    pub font: ID,
}

impl PositionedGlyph {
    /// Returns the font size for this glyph.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the transform of glyph.
    pub fn transform(&self) -> Transform {
        let sx = self.font_size / self.units_per_em as f32;

        self.span_ts
            .pre_concat(self.cluster_ts)
            .pre_concat(Transform::from_scale(sx, sx))
            .pre_concat(self.glyph_ts)
    }

    /// Returns the transform of glyph, assuming that an outline
    /// glyph is being used (i.e. from the `glyf` or `CFF/CFF2` table).
    pub fn outline_transform(&self) -> Transform {
        // Outlines are mirrored by default.
        self.transform()
            .pre_concat(Transform::from_scale(1.0, -1.0))
    }

    /// Returns the transform for the glyph, assuming that a CBDT-based raster glyph
    /// is being used.
    pub fn cbdt_transform(&self, x: f32, y: f32, pixels_per_em: f32) -> Transform {
        self.transform()
            .pre_concat(Transform::from_scale(
                self.units_per_em as f32 / pixels_per_em,
                self.units_per_em as f32 / pixels_per_em,
            ))
            // Right now, the top-left corner of the image would be placed in
            // on the "text cursor", but we want the bottom-left corner to be there,
            // so we need to shift it up and also apply the x/y offset.
            .pre_translate(x, -y)
    }

    /// Returns the transform for the glyph, assuming that a sbix-based raster glyph
    /// is being used.
    pub fn sbix_transform(
        &self,
        x: f32,
        y: f32,
        bearing_x: f32,
        bearing_y: f32,
        pixels_per_em: f32,
        height: f32,
    ) -> Transform {
        self.transform()
            .pre_translate(bearing_x, -bearing_y)
            .pre_concat(Transform::from_scale(
                self.units_per_em as f32 / pixels_per_em,
                self.units_per_em as f32 / pixels_per_em,
            ))
            // Right now, the top-left corner of the image would be placed in
            // on the "text cursor", but we want the bottom-left corner to be there,
            // so we need to shift it up and also apply the x/y offset.
            .pre_translate(x, -height - y)
    }

    /// Returns the transform for the glyph, assuming that an SVG glyph is
    /// being used.
    pub fn svg_transform(&self) -> Transform {
        self.transform()
    }

    /// Returns the transform for the glyph, assuming that a COLR glyph is
    /// being used.
    pub fn colr_transform(&self) -> Transform {
        self.outline_transform()
    }
}

/// A span contains positioned glyphs with the same paint, size and effective variation settings.
#[derive(Clone, Debug)]
pub struct Span {
    /// The fill of the span.
    pub fill: Option<Fill>,
    /// The stroke of the span.
    pub stroke: Option<Stroke>,
    /// The paint order of the span.
    pub paint_order: PaintOrder,
    /// The font size of the span.
    pub font_size: NonZeroPositiveF32,
    /// Effective OpenType design coordinates. Omitted axes use the face defaults.
    pub variations: Vec<FontVariation>,
    /// Optical sizing mode for the renderer. SVG layout sets this to [`FontOpticalSizing::None`]
    /// because `variations` already reflects optical sizing.
    pub font_optical_sizing: FontOpticalSizing,
    /// The visibility of the span.
    pub visible: bool,
    /// The glyphs that make up the span.
    pub positioned_glyphs: Vec<PositionedGlyph>,
    /// An underline text decoration of the span.
    /// Needs to be rendered before all glyphs.
    pub underline: Option<Box<Path>>,
    /// An overline text decoration of the span.
    /// Needs to be rendered before all glyphs.
    pub overline: Option<Box<Path>>,
    /// A line-through text decoration of the span.
    /// Needs to be rendered after all glyphs.
    pub line_through: Option<Box<Path>>,
}

impl Span {
    pub(crate) fn has_visible_paint(&self) -> bool {
        self.visible
            && (crate::has_nontransparent_paint(self.fill.as_ref(), self.stroke.as_ref())
                || [&self.underline, &self.overline, &self.line_through]
                    .into_iter()
                    .flatten()
                    .any(|path| crate::has_nontransparent_paint(path.fill(), path.stroke())))
    }
}

#[derive(Clone, Debug)]
struct GlyphCluster {
    byte_idx: ByteIndex,
    byte_end: usize,
    codepoint: char,
    width: f32,
    advance: f32,
    text_length_extent: Option<f32>,
    glyph_scale_x: f32,
    ascent: f32,
    descent: f32,
    font: Arc<ResolvedFont>,
    has_relative_shift: bool,
    glyphs: Vec<PositionedGlyph>,
    transform: Transform,
    path_transform: Transform,
    rotation: f32,
    rtl: bool,
    visible: bool,
}

impl GlyphCluster {
    pub(crate) fn height(&self) -> f32 {
        self.ascent - self.descent
    }

    fn inline_transform(&self) -> Transform {
        // Scale along the layout axis around the placed origin, before the character's
        // rotation changes that axis in glyph space. The path tangent remains in effect.
        let mut scale = Transform::from_scale(self.glyph_scale_x, 1.0);
        if self.glyph_scale_x != 1.0 && self.rotation != 0.0 {
            scale = scale.pre_rotate(self.rotation).post_rotate(-self.rotation);
        }
        self.transform.pre_concat(scale)
    }

    fn glyph_transform(&self) -> Transform {
        self.inline_transform().pre_concat(self.path_transform)
    }
}

pub(crate) struct TextLayout {
    pub(crate) spans: Vec<Span>,
    pub(crate) bbox: NonZeroRect,
}

#[derive(Default)]
struct TextGeometry {
    spans: Vec<Span>,
    bbox: BBox,
}

impl TextGeometry {
    fn finish(self) -> Option<TextLayout> {
        Some(TextLayout {
            spans: self.spans,
            bbox: self.bbox.to_non_zero_rect()?,
        })
    }
}

pub(crate) fn can_probe_end_anchor(text: &Text) -> bool {
    text.writing_mode == WritingMode::LeftToRight
        && text.length_adjustments.is_empty()
        && matches!(text.chunks.as_slice(), [chunk]
            if chunk.anchor == TextAnchor::Start
                && matches!(chunk.text_flow, TextFlow::Linear))
}

struct UnpositionedChunk {
    clusters: Vec<GlyphCluster>,
    shifts: RelativeShifts,
    next_chunk_offset: f32,
}

pub(crate) fn layout_text(
    text_node: &Text,
    options: &crate::Options,
    fonts: &mut FontCache,
    probe_end_anchor: bool,
    no_primary_face: &mut bool,
) -> Option<(TextLayout, Option<TextLayout>)> {
    let mut fonts_cache: FontsCache = HashMap::new();
    let mut primary_miss = false;

    for span in text_node.chunks.iter().flat_map(|chunk| &chunk.spans) {
        let size_key = span_font_size_key(span);
        let resolved = match fonts_cache.entry(&span.font) {
            Entry::Occupied(mut entry) => {
                let primary = entry.get_mut();
                match primary.sizes.entry(size_key) {
                    Entry::Occupied(_) => true,
                    Entry::Vacant(entry) => resolve_font(fonts, &primary.selection, span)
                        .map(|font| entry.insert(Arc::new(font)))
                        .is_some(),
                }
            }
            Entry::Vacant(entry) => {
                let selection = match &options.text_fallback.select_instance {
                    Some(select) => select(&span.font, &mut fonts.fontdb),
                    None => (options.font_resolver.select_font)(&span.font, &mut fonts.fontdb)
                        .map(|id| (id, 1.0, Vec::new())),
                };
                selection
                    .and_then(|selection| {
                        let font = resolve_font(fonts, &selection, span)?;
                        Some(entry.insert(PrimaryFont {
                            selection,
                            sizes: HashMap::from([(size_key, Arc::new(font))]),
                        }))
                    })
                    .is_some()
            }
        };
        primary_miss |= !resolved && span.has_visible_paint();
    }

    if primary_miss && !*no_primary_face {
        // A later span may have resolved the same request. Check the font cache that
        // actually controls which spans can emit glyphs.
        *no_primary_face = text_node.chunks.iter().any(|chunk| {
            chunk
                .spans
                .iter()
                .any(|span| span.has_visible_paint() && primary_font(&fonts_cache, span).is_none())
        });
    }

    let mut unpositioned = text_node.chunks.iter().scan(0, |char_offset, chunk| {
        let mut clusters = process_chunk(chunk, &fonts_cache, options, fonts);
        apply_writing_mode(text_node.writing_mode, &mut clusters);
        apply_letter_spacing(chunk, &mut clusters);
        apply_word_spacing(chunk, &mut clusters);
        let shifts = resolve_relative_shifts(text_node, chunk, *char_offset, &clusters);
        *char_offset += chunk.text.chars().count();
        Some(UnpositionedChunk {
            clusters,
            shifts,
            next_chunk_offset: 0.0,
        })
    });
    let mut adjusted = Vec::new();
    // Keep shaping and placement streaming when there is no element-wide length to resolve.
    if !text_node.length_adjustments.is_empty() {
        adjusted.extend(unpositioned.by_ref());
        apply_length_adjust(text_node, &mut adjusted);
    }

    let mut geometry = TextGeometry::default();
    let mut end_layout = None;
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    for (
        chunk,
        UnpositionedChunk {
            mut clusters,
            shifts,
            next_chunk_offset,
        },
    ) in text_node
        .chunks
        .iter()
        .zip(adjusted.into_iter().chain(unpositioned))
    {
        if clusters.is_empty() {
            continue;
        }
        let (x, y) = match chunk.text_flow {
            TextFlow::Linear => (chunk.x.unwrap_or(last_x), chunk.y.unwrap_or(last_y)),
            TextFlow::Path(_) => (0.0, 0.0),
        };

        let mut curr_pos = resolve_clusters_positions(
            text_node,
            chunk,
            &shifts,
            chunk.anchor,
            &fonts_cache,
            &mut clusters,
        );
        append_chunk_geometry(
            text_node,
            chunk,
            (x, y),
            &fonts_cache,
            &clusters,
            &mut geometry,
        );
        if probe_end_anchor {
            // This single horizontal chunk had no transform before positioning. Reuse its
            // shaped clusters and repeat only geometry to retain the old end-anchor rounding.
            for cluster in &mut clusters {
                cluster.transform = Transform::default();
                cluster.has_relative_shift = false;
            }
            let mut end = TextGeometry::default();
            resolve_clusters_positions(
                text_node,
                chunk,
                &shifts,
                TextAnchor::End,
                &fonts_cache,
                &mut clusters,
            );
            append_chunk_geometry(text_node, chunk, (x, y), &fonts_cache, &clusters, &mut end);
            end_layout = end.finish();
        }

        if let TextFlow::Linear = chunk.text_flow {
            curr_pos.0 += next_chunk_offset;
            if text_node.writing_mode == WritingMode::TopToBottom {
                std::mem::swap(&mut curr_pos.0, &mut curr_pos.1);
            }
        }

        last_x = x + curr_pos.0;
        last_y = y + curr_pos.1;
    }

    Some((geometry.finish()?, end_layout))
}

fn append_chunk_geometry(
    text_node: &Text,
    chunk: &TextChunk,
    (x, y): (f32, f32),
    fonts_cache: &FontsCache,
    clusters: &[GlyphCluster],
    geometry: &mut TextGeometry,
) {
    let mut text_ts = Transform::default();
    if text_node.writing_mode == WritingMode::TopToBottom {
        if let TextFlow::Linear = chunk.text_flow {
            text_ts = text_ts.pre_rotate_at(90.0, x, y);
        }
    }

    let baseline_for = |span: &TextSpan, font: &ResolvedFont| match chunk.text_flow {
        TextFlow::Linear => resolve_baseline(span, font, text_node.writing_mode),
        TextFlow::Path(_) => 0.0,
    };
    let mut decoration_ranges = [0..0, 0..0, 0..0];
    for (index, span) in chunk.spans.iter().enumerate() {
        let font = match primary_font(fonts_cache, span) {
            Some((_, font)) => font,
            None => continue,
        };

        let baseline = baseline_for(span, font);
        let span_ts = text_ts.pre_translate(x, y).pre_translate(0.0, baseline);
        let mut decoration = |line: DecorationLine| {
            let paint = line.style(&span.decoration)?;
            let offset = line.offset(font, text_node.writing_mode);
            let range = &mut decoration_ranges[line as usize];
            if index >= range.end {
                let following = chunk.spans[index + 1..]
                    .iter()
                    .take_while(|next| {
                        next.visible == span.visible
                            && line.style(&next.decoration) == Some(paint)
                            && primary_font(fonts_cache, next).is_some_and(|(_, next_font)| {
                                next_font.underline_thickness() == font.underline_thickness()
                                    && line.offset(next_font, text_node.writing_mode) == offset
                                    && baseline_for(next, next_font) == baseline
                            })
                    })
                    .count();
                *range = index..index + 1 + following;
            }
            let owner = match line {
                DecorationLine::LineThrough => range.end - 1,
                _ => range.start,
            };
            if owner != index {
                return None;
            }
            let bytes = chunk.spans[range.start].start..chunk.spans[range.end - 1].end;
            let runs = collect_decoration_spans(bytes, chunk, clusters, line);
            let path = convert_decoration(offset, span, font, paint.clone(), &runs, span_ts)?;
            geometry.bbox = geometry.bbox.expand(path.data.bounds());
            Some(Box::new(path))
        };
        let underline = decoration(DecorationLine::Underline);
        let overline = decoration(DecorationLine::Overline);
        let line_through = decoration(DecorationLine::LineThrough);

        let mut fill = span.fill.clone();
        if let Some(ref mut fill) = fill {
            // The `fill-rule` should be ignored.
            // https://www.w3.org/TR/SVG2/text.html#TextRenderingOrder
            //
            // 'Since the fill-rule property does not apply to SVG text elements,
            // the specific order of the subpaths within the equivalent path does not matter.'
            fill.rule = FillRule::NonZero;
        }

        if let Some((span_fragments, span_bbox)) = convert_span(span, clusters, span_ts) {
            geometry.bbox = geometry.bbox.expand(span_bbox);

            append_font_spans(
                &mut geometry.spans,
                Span {
                    fill,
                    stroke: span.stroke.clone(),
                    paint_order: span.paint_order,
                    font_size: font.size,
                    variations: Vec::new(),
                    font_optical_sizing: FontOpticalSizing::None,
                    visible: span.visible,
                    positioned_glyphs: Vec::new(),
                    underline,
                    overline,
                    line_through,
                },
                span_fragments,
                span_ts,
            );
        }
    }
}

fn append_font_spans(
    spans: &mut Vec<Span>,
    mut span: Span,
    fragments: Vec<GlyphCluster>,
    span_ts: Transform,
) {
    if fragments.is_empty() {
        spans.push(span);
        return;
    }

    // PDF consumers use the span's size and coordinates together with each glyph's face ID.
    // Keep decorations around the glyphs of the original paint span.
    let first = spans.len();
    let mut clusters = fragments.into_iter().peekable();
    let underline = span.underline.take();
    let overline = span.overline.take();
    let line_through = span.line_through.take();
    while let Some(cluster) = clusters.next() {
        let font = Arc::clone(&cluster.font);
        let positioned_glyphs = std::iter::once(cluster)
            .chain(std::iter::from_fn(|| {
                clusters.next_if(|next| {
                    next.font.size.get() == font.size.get()
                        && next.font.variations == font.variations
                })
            }))
            .flat_map(|mut cluster| {
                let cluster_ts = cluster.glyph_transform();
                for glyph in &mut cluster.glyphs {
                    glyph.cluster_ts = cluster_ts;
                    glyph.span_ts = span_ts;
                }
                cluster.glyphs
            })
            .collect();
        spans.push(Span {
            font_size: font.size,
            variations: font.variations.clone(),
            positioned_glyphs,
            ..span.clone()
        });
    }
    spans[first].underline = underline;
    spans[first].overline = overline;
    spans.last_mut().unwrap().line_through = line_through;
}

fn convert_span(
    span: &TextSpan,
    clusters: &[GlyphCluster],
    text_ts: Transform,
) -> Option<(Vec<GlyphCluster>, NonZeroRect)> {
    let mut span_clusters = vec![];
    let mut bboxes_builder = tiny_skia_path::PathBuilder::new();

    for cluster in clusters {
        if !cluster.visible {
            continue;
        }

        if span_contains(span, cluster.byte_idx) {
            span_clusters.push(cluster.clone());
        }

        let extent = cluster.text_length_extent.unwrap_or(cluster.advance);
        let advance = if extent > 0.0 {
            extent
        } else {
            cluster.width.max(1.0)
        };

        // We have to calculate text bbox using font metrics and not glyph shape.
        if let Some(r) = NonZeroRect::from_xywh(
            0.0,
            -cluster.ascent,
            advance / cluster.glyph_scale_x,
            cluster.height(),
        ) {
            if let Some(r) = r.transform(cluster.glyph_transform()) {
                bboxes_builder.push_rect(r.to_rect());
            }
        }
    }

    let mut bboxes = bboxes_builder.finish()?;
    bboxes = bboxes.transform(text_ts)?;
    let bbox = bboxes.compute_tight_bounds()?.to_non_zero_rect()?;

    Some((span_clusters, bbox))
}

#[derive(Clone, Copy)]
enum DecorationLine {
    Underline,
    Overline,
    LineThrough,
}

impl DecorationLine {
    fn style(self, decoration: &TextDecoration) -> Option<&TextDecorationStyle> {
        match self {
            Self::Underline => decoration.underline(),
            Self::Overline => decoration.overline(),
            Self::LineThrough => decoration.line_through(),
        }
    }

    fn offset(self, font: &ResolvedFont, mode: WritingMode) -> f32 {
        match (mode, self) {
            (WritingMode::LeftToRight, Self::Underline) => -font.underline_position(),
            (WritingMode::LeftToRight, Self::Overline) => -font.ascent(),
            (WritingMode::LeftToRight, Self::LineThrough) => -font.line_through_position(),
            (WritingMode::TopToBottom, Self::Underline) => font.height() / 2.0,
            (WritingMode::TopToBottom, Self::Overline) => -font.height() / 2.0,
            (WritingMode::TopToBottom, Self::LineThrough) => 0.0,
        }
    }
}

fn collect_decoration_spans(
    bytes: Range<usize>,
    chunk: &TextChunk,
    clusters: &[GlyphCluster],
    line: DecorationLine,
) -> Vec<DecorationSpan> {
    let text = &chunk.text;
    let mut spans = Vec::new();
    let mut current: Option<DecorationSpan> = None;
    let mut advance = 0.0;
    let mut clusters = clusters.iter().peekable();
    while let Some(cluster) = clusters.next() {
        let start = cluster.byte_idx.value();
        let end = cluster.byte_end;
        let covered_start = bytes.start.max(start);
        let covered_end = bytes.end.min(end);
        let covered = cluster.visible && covered_start < covered_end;
        let (leading, trailing) = if cluster.rtl {
            (covered_end == end, covered_start == start)
        } else {
            (covered_start == start, covered_end == end)
        };
        if !covered || !leading || cluster.has_relative_shift {
            spans.extend(current.take());
            advance = 0.0;
        }
        if !covered {
            continue;
        }
        // Widths and advances already include lengthAdjust. For rotated characters,
        // put them back in local coordinates before applying the same inline scale as ink.
        let rotated = cluster.rotation != 0.0;
        let (transform, scale) = if rotated {
            (cluster.inline_transform(), cluster.glyph_scale_x)
        } else {
            (cluster.transform, 1.0)
        };
        let width = match cluster.text_length_extent {
            Some(extent) => {
                let continues = !rotated
                    && clusters.peek().is_some_and(|next| {
                        let edge = if next.rtl {
                            next.byte_end.saturating_sub(1)
                        } else {
                            next.byte_idx.value()
                        };
                        next.visible
                            && !next.has_relative_shift
                            && chunk_span_at(chunk, ByteIndex::new(edge))
                                .is_some_and(|s| s.visible && line.style(&s.decoration).is_some())
                    });
                if continues {
                    cluster.advance.max(extent)
                } else {
                    extent
                }
            }
            None => cluster.advance,
        };
        let (from, to) = if leading && trailing {
            (0.0, width)
        } else {
            // A ligature may be shared by several paint spans. Divide its inline
            // advance among its source characters in visual order.
            let Some(characters) = text.get(start..end) else {
                continue;
            };
            let covered = &text[covered_start..covered_end];
            let before = if cluster.rtl {
                &text[covered_end..end]
            } else {
                &text[start..covered_start]
            };
            let unit = cluster.width / characters.chars().count() as f32;
            let from = unit * before.chars().count() as f32;
            let to = if trailing {
                width
            } else {
                from + unit * covered.chars().count() as f32
            };
            (from, to)
        };
        let decoration = current.get_or_insert_with(|| {
            advance = -from;
            DecorationSpan {
                width: 0.0,
                transform: transform.pre_translate(from / scale, 0.0),
            }
        });
        decoration.width = decoration.width.max((advance + to) / scale);
        advance += cluster.advance;
        if !trailing || rotated {
            spans.extend(current.take());
            advance = 0.0;
        }
    }

    spans.extend(current);
    spans
}

pub(crate) fn convert_decoration(
    dy: f32,
    span: &TextSpan,
    font: &ResolvedFont,
    mut decoration: TextDecorationStyle,
    decoration_spans: &[DecorationSpan],
    transform: Transform,
) -> Option<Path> {
    let thickness = font.underline_thickness();

    let mut builder = tiny_skia_path::PathBuilder::new();
    for dec_span in decoration_spans {
        let rect = match NonZeroRect::from_xywh(0.0, -thickness / 2.0, dec_span.width, thickness) {
            Some(v) => v,
            None => {
                log::warn!("a decoration span has a malformed bbox");
                continue;
            }
        };

        let ts = dec_span.transform.pre_translate(0.0, dy);

        let mut path = tiny_skia_path::PathBuilder::from_rect(rect.to_rect());
        path = match path.transform(ts) {
            Some(v) => v,
            None => continue,
        };

        builder.push_path(&path);
    }

    let mut path_data = builder.finish()?;
    path_data = path_data.transform(transform)?;

    Some(Path::new(
        Box::default(),
        span.visible,
        decoration.fill.take(),
        decoration.stroke.take(),
        PaintOrder::default(),
        ShapeRendering::default(),
        Arc::new(path_data),
        Transform::default(),
    ))
}

/// A text decoration span.
///
/// Basically a horizontal line, that will be used for underline, overline and line-through.
/// It doesn't have a height, since it depends on the Font metrics.
#[derive(Clone, Copy)]
pub(crate) struct DecorationSpan {
    pub(crate) width: f32,
    pub(crate) transform: Transform,
}

/// Resolves clusters positions.
///
/// Mainly sets the `transform` property.
///
/// Returns the last text position. The next text chunk should start from that position.
fn resolve_clusters_positions(
    text: &Text,
    chunk: &TextChunk,
    shifts: &RelativeShifts,
    anchor: TextAnchor,
    fonts_cache: &FontsCache,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let (anchor_offset, advance) = inline_anchor_and_advance(anchor, clusters, shifts);
    match chunk.text_flow {
        TextFlow::Linear => {
            resolve_clusters_positions_horizontal(text, shifts, anchor_offset, clusters);
            (advance + shifts.end.x, shifts.end.y)
        }
        TextFlow::Path(ref path) => resolve_clusters_positions_path(
            text,
            chunk,
            &shifts.clusters,
            path,
            anchor_offset,
            fonts_cache,
            clusters,
        ),
    }
}

#[derive(Clone, Copy, Default)]
struct RelativeShift {
    char_index: usize,
    x: f32,
    y: f32,
}

struct RelativeShifts {
    clusters: Vec<RelativeShift>,
    logical_order: Vec<usize>,
    end: RelativeShift,
}

fn resolve_relative_shifts(
    text: &Text,
    chunk: &TextChunk,
    offset: usize,
    clusters: &[GlyphCluster],
) -> RelativeShifts {
    // Relative positions follow the source order, even when bidi reorders the clusters.
    let mut logical_order: Vec<usize> = (0..clusters.len()).collect();
    logical_order.sort_unstable_by_key(|&i| clusters[i].byte_idx.value());

    let mut shifts = vec![RelativeShift::default(); clusters.len()];
    let mut chars = chunk.text.char_indices().zip(offset..).peekable();
    let mut shift = RelativeShift::default();
    let add_shift = |shift: &mut RelativeShift, char_index| {
        shift.char_index = char_index;
        shift.x += text.dx.get(char_index).copied().unwrap_or(0.0);
        shift.y += text.dy.get(char_index).copied().unwrap_or(0.0);
    };
    for &i in &logical_order {
        let cluster_byte = clusters[i].byte_idx.value();
        // Positions attached to characters inside a ligature affect subsequent clusters.
        while let Some((_, char_index)) = chars.next_if(|&((byte, _), _)| byte <= cluster_byte) {
            add_shift(&mut shift, char_index);
        }
        shifts[i] = shift;
    }
    for (_, char_index) in chars {
        add_shift(&mut shift, char_index);
    }

    if text.writing_mode == WritingMode::TopToBottom && matches!(chunk.text_flow, TextFlow::Linear)
    {
        for shift in shifts.iter_mut().chain(std::iter::once(&mut shift)) {
            (shift.x, shift.y) = (shift.y, -shift.x);
        }
    }

    RelativeShifts {
        clusters: shifts,
        logical_order,
        end: shift,
    }
}

fn resolve_clusters_positions_horizontal(
    text: &Text,
    shifts: &RelativeShifts,
    mut x: f32,
    clusters: &mut [GlyphCluster],
) {
    for (cluster, pair) in clusters.iter_mut().skip(1).zip(shifts.clusters.windows(2)) {
        cluster.has_relative_shift = pair[0].x != pair[1].x || pair[0].y != pair[1].y;
    }

    for (cluster, shift) in clusters.iter_mut().zip(&shifts.clusters) {
        cluster.transform = cluster.transform.pre_translate(x + shift.x, shift.y);
        if let Some(angle) = text.rotate.get(shift.char_index).copied() {
            if !angle.approx_zero_ulps(4) {
                cluster.transform = cluster.transform.pre_rotate(angle);
                cluster.rotation = angle;
                cluster.has_relative_shift = true;
            }
        }

        x += cluster.advance;
    }
}

fn inline_anchor_and_advance(
    anchor: TextAnchor,
    clusters: &[GlyphCluster],
    shifts: &RelativeShifts,
) -> (f32, f32) {
    // SVG anchors the positioned chunk before projecting it onto a text path.
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut advance = 0.0;
    for (cluster, shift) in clusters.iter().zip(&shifts.clusters) {
        let start = advance + shift.x;
        let extent = cluster.text_length_extent.unwrap_or(cluster.advance);
        left = left.min(start);
        right = right.max(start + extent.max(0.0));
        advance += cluster.advance;
    }
    let first = shifts.clusters[shifts.logical_order[0]].x;
    (first - left + process_anchor(anchor, right - left), advance)
}

// Baseline resolving in SVG is a mess.
// Not only it's poorly documented, but as soon as you start mixing
// `dominant-baseline` and `alignment-baseline` each application/browser will produce
// different results.
//
// For now, resvg simply tries to match Chrome's output and not the mythical SVG spec output.
//
// See `alignment_baseline_shift` method comment for more details.
pub(crate) fn resolve_baseline(
    span: &TextSpan,
    font: &ResolvedFont,
    writing_mode: WritingMode,
) -> f32 {
    let mut shift = -resolve_baseline_shift(&span.baseline_shift, font);

    // TODO: support vertical layout as well
    if writing_mode == WritingMode::LeftToRight {
        if span.alignment_baseline == AlignmentBaseline::Auto
            || span.alignment_baseline == AlignmentBaseline::Baseline
        {
            shift += font.dominant_baseline_shift(span.dominant_baseline);
        } else {
            shift += font.alignment_baseline_shift(span.alignment_baseline);
        }
    }

    shift
}

fn resolve_baseline_shift(baselines: &[BaselineShift], font: &ResolvedFont) -> f32 {
    let mut shift = 0.0;
    for baseline in baselines.iter().rev() {
        match baseline {
            BaselineShift::Baseline => {}
            BaselineShift::Subscript => shift -= font.subscript_offset(),
            BaselineShift::Superscript => shift += font.superscript_offset(),
            BaselineShift::Number(n) => shift += n,
        }
    }

    shift
}

fn resolve_clusters_positions_path(
    text: &Text,
    chunk: &TextChunk,
    shifts: &[RelativeShift],
    path: &TextPath,
    anchor_offset: f32,
    fonts_cache: &FontsCache,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let mut last_x = 0.0;
    let mut last_y = 0.0;

    // In the text path mode, chunk's x/y coordinates provide an additional offset along the path.
    // The X coordinate is used in a horizontal mode, and Y in vertical.
    let chunk_offset = match text.writing_mode {
        WritingMode::LeftToRight => chunk.x.unwrap_or(0.0),
        WritingMode::TopToBottom => chunk.y.unwrap_or(0.0),
    };

    let start_offset = chunk_offset + path.start_offset + anchor_offset;

    let normals = collect_normals(text, clusters, shifts, &path.path, start_offset);
    for ((cluster, normal), shift) in clusters.iter_mut().zip(normals).zip(shifts) {
        let (x, y, angle) = match normal {
            Some(normal) => (normal.x, normal.y, normal.angle),
            None => {
                // Hide clusters that are outside the text path.
                cluster.visible = false;
                continue;
            }
        };

        // We have to break a decoration line for each cluster during text-on-path.
        cluster.has_relative_shift = true;

        let orig_ts = cluster.transform;

        // Clusters should be rotated by the x-midpoint x baseline position.
        let half_width = cluster.width / 2.0;
        cluster.transform = Transform::default();
        cluster.transform = cluster.transform.pre_translate(x - half_width, y);
        cluster.transform = cluster.transform.pre_rotate_at(angle, half_width, 0.0);

        let baseline_shift = chunk_span_at(chunk, cluster.byte_idx)
            .map(|span| {
                let font = match primary_font(fonts_cache, span) {
                    Some((_, font)) => font,
                    None => return 0.0,
                };
                -resolve_baseline(span, font, text.writing_mode)
            })
            .unwrap_or(0.0);

        // Shift only by `dy` since we already applied `dx`
        // during offset along the path calculation.
        if !shift.y.approx_zero_ulps(4) || !baseline_shift.approx_zero_ulps(4) {
            cluster.transform = cluster
                .transform
                .pre_translate(0.0, shift.y - baseline_shift);
        }

        if let Some(angle) = text.rotate.get(shift.char_index).copied() {
            if !angle.approx_zero_ulps(4) {
                cluster.transform = cluster.transform.pre_rotate(angle);
                cluster.rotation = angle;
            }
        }

        cluster.transform = cluster.transform.pre_concat(orig_ts);

        last_x = x + cluster.advance;
        last_y = y;
    }

    (last_x, last_y)
}

pub(crate) fn process_anchor(a: TextAnchor, text_width: f32) -> f32 {
    match a {
        TextAnchor::Start => 0.0, // Nothing.
        TextAnchor::Middle => -text_width / 2.0,
        TextAnchor::End => -text_width,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PathNormal {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) angle: f32,
}

fn collect_normals(
    text: &Text,
    clusters: &[GlyphCluster],
    shifts: &[RelativeShift],
    path: &tiny_skia_path::Path,
    offset: f32,
) -> Vec<Option<PathNormal>> {
    let mut offsets = Vec::with_capacity(clusters.len());
    let mut normals = vec![None; clusters.len()];
    let mut advance = offset;
    for (index, (cluster, shift)) in clusters.iter().zip(shifts).enumerate() {
        let distance = advance + shift.x + cluster.width / 2.0;
        if distance.is_finite() && distance >= 0.0 {
            offsets.push((f64::from(distance), index));
        }
        advance += cluster.advance;
    }
    if offsets.is_empty() {
        return normals;
    }

    // Bidi and relative shifts can make distances nonmonotone in visual order.
    offsets.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let mut offsets = offsets.into_iter().peekable();

    // Increase the accuracy for paths that are scaled up.
    let (sx, sy) = text.abs_transform.get_scale();
    let accuracy = 0.5 / f64::from((sx * sy).sqrt().max(1.0));
    let point = |p: tiny_skia_path::Point| kurbo::Point::new(f64::from(p.x), f64::from(p.y));
    let mut current = kurbo::Point::ZERO;
    let mut subpath_start = current;
    let mut length = 0.0;
    for seg in path.segments() {
        let curve: kurbo::PathSeg = match seg {
            tiny_skia_path::PathSegment::MoveTo(p) => {
                current = point(p);
                subpath_start = current;
                continue;
            }
            tiny_skia_path::PathSegment::LineTo(p) => kurbo::Line::new(current, point(p)).into(),
            tiny_skia_path::PathSegment::QuadTo(p1, p) => {
                kurbo::QuadBez::new(current, point(p1), point(p)).into()
            }
            tiny_skia_path::PathSegment::CubicTo(p1, p2, p) => {
                kurbo::CubicBez::new(current, point(p1), point(p2), point(p)).into()
            }
            tiny_skia_path::PathSegment::Close => kurbo::Line::new(current, subpath_start).into(),
        };
        current = curve.end();

        let curve_len = curve.arclen(accuracy);
        if !curve_len.is_finite() || curve_len <= 0.0 {
            continue;
        }
        let end = length + curve_len;
        while let Some((distance, index)) = offsets.next_if(|&(distance, _)| distance <= end) {
            let t = curve
                .inv_arclen(distance - length, accuracy)
                .clamp(0.0, 1.0);
            let pos = curve.eval(t);
            let tangent = path_tangent(curve, t);
            normals[index] = Some(PathNormal {
                x: pos.x as f32,
                y: pos.y as f32,
                angle: tangent.atan2().to_degrees() as f32,
            });
        }
        if offsets.peek().is_none() {
            break;
        }
        length = end;
    }

    normals
}

fn path_tangent(curve: kurbo::PathSeg, t: f64) -> kurbo::Vec2 {
    let tangent = match curve {
        kurbo::PathSeg::Line(line) => line.p1 - line.p0,
        kurbo::PathSeg::Quad(quad) => quad.deriv().eval(t).to_vec2(),
        kurbo::PathSeg::Cubic(cubic) => cubic.deriv().eval(t).to_vec2(),
    };
    if tangent != kurbo::Vec2::ZERO || (t != 0.0 && t != 1.0) {
        return tangent;
    }

    // At a repeated endpoint, the first distinct control point gives the limiting direction.
    let curve = if t == 0.0 { curve } else { curve.reverse() }.to_cubic();
    let direction = [curve.p1, curve.p2, curve.p3]
        .into_iter()
        .find(|&p| p != curve.p0)
        .map_or(kurbo::Vec2::ZERO, |p| p - curve.p0);
    if t == 0.0 { direction } else { -direction }
}

/// Converts a text chunk into a list of outlined clusters.
///
/// This function will do the BIDI reordering, text shaping and glyphs outlining,
/// but not the text layouting. So all clusters are in the 0x0 position.
fn process_chunk(
    chunk: &TextChunk,
    fonts_cache: &FontsCache,
    options: &crate::Options,
    fonts: &mut FontCache,
) -> Vec<GlyphCluster> {
    // Reorder the full chunk once, then shape each script in its own context.
    let bidi = unicode_bidi::BidiInfo::new(&chunk.text, Some(unicode_bidi::Level::ltr()));
    let Some(paragraph) = bidi.paragraphs.first() else {
        return Vec::new();
    };
    let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
    let default_language = options.text_fallback.language.as_deref();
    let initial_face_count = fonts.fontdb.len();
    let mut scalar_fallbacks = (options.text_fallback.fallback_instance.is_none()
        && options.text_fallback.select.is_none())
    .then(Fallbacks::new);
    let mut deferred = Vec::new();
    let mut styles: Vec<(&TextSpan, &Arc<ResolvedFont>, Range<usize>)> = Vec::new();
    let mut glyphs = Vec::new();
    for run in runs.into_iter().filter(|run| !run.is_empty()) {
        let ltr = levels[run.start].is_ltr();
        let mut script_runs = script_runs(&chunk.text, run);
        if !ltr {
            script_runs.reverse();
        }
        for (run, script) in script_runs {
            let allow_letter_spacing = script.is_none_or(script_supports_letter_spacing);
            let disable_optional_ligatures =
                |span: &TextSpan| allow_letter_spacing && !span.letter_spacing.approx_zero_ulps(4);
            let start = chunk.spans.partition_point(|span| span.end <= run.start);
            let end = chunk.spans.partition_point(|span| span.start < run.end);
            styles.clear();

            for span in &chunk.spans[start..end] {
                let Some((_, font)) = primary_font(fonts_cache, span) else {
                    continue;
                };
                let bytes = run.start.max(span.start)..run.end.min(span.end);
                match styles.last_mut() {
                    Some((previous, previous_font, previous_bytes))
                        if previous_bytes.end == bytes.start
                            && font.same_instance(previous_font, &fonts.fontdb)
                            && same_shaping_options(previous, span, default_language)
                            && disable_optional_ligatures(previous)
                                == disable_optional_ligatures(span) =>
                    {
                        previous_bytes.end = bytes.end;
                    }
                    _ => styles.push((span, font, bytes)),
                }
            }
            if !ltr {
                styles.reverse();
            }

            let script = script.and_then(|script| {
                harfrust::Script::from_iso15924_tag(Tag::from_be_bytes(
                    script.as_iso15924_tag().to_be_bytes(),
                ))
            });
            for (span, font, bytes) in &styles {
                let run = ShapingRun {
                    chunk,
                    bytes: run.clone(),
                    ltr,
                    span,
                    language: shaping_language(span, default_language),
                    script,
                    disable_optional_ligatures: disable_optional_ligatures(span),
                };
                let mut run_fallbacks = Fallbacks::new();
                let fallbacks = scalar_fallbacks.as_mut().unwrap_or(&mut run_fallbacks);
                let Some(shaped) =
                    run.shape_text(bytes.clone(), font, fonts_cache, options, fonts, fallbacks)
                else {
                    continue;
                };
                // Defer the suffix from the first unresolved scalar fallback to keep glyph order.
                if scalar_fallbacks.is_some()
                    && (!deferred.is_empty() || shaped.repairs.iter().any(|r| r.glyphs.is_empty()))
                {
                    deferred.push(shaped);
                } else {
                    shaped.append_glyphs(&mut glyphs, fonts);
                }
            }
        }
    }

    if let Some(mut fallbacks) = scalar_fallbacks {
        // A later script may have admitted faces that can repair earlier missing clusters.
        if fonts.fontdb.len() != initial_face_count {
            for shaped in &mut deferred {
                for repair in shaped.repairs.iter_mut().filter(|r| r.glyphs.is_empty()) {
                    repair.glyphs = shaped
                        .run
                        .repair_at(
                            repair.bytes.clone(),
                            fonts_cache,
                            options,
                            fonts,
                            &mut fallbacks,
                        )
                        .unwrap_or_default();
                }
            }
        }
        for mut shaped in deferred {
            for repair in shaped.repairs.iter_mut().filter(|r| r.glyphs.is_empty()) {
                let Some(run) = shaped
                    .run
                    .with_cluster_span(repair.bytes.start, default_language)
                else {
                    continue;
                };
                if let Some(glyphs) = fallbacks.get(&run.fallback_key()).and_then(|fallbacks| {
                    fallbacks
                        .fonts
                        .iter()
                        .find_map(|(_, face)| run.shape_covered(repair.bytes.clone(), face, fonts))
                }) {
                    repair.glyphs = glyphs;
                }
            }
            shaped.append_glyphs(&mut glyphs, fonts);
        }
    }

    // Convert glyphs to clusters.
    let mut clusters = Vec::new();
    for (range, byte_idx) in GlyphClusters::new(&glyphs) {
        if chunk_span_at(chunk, byte_idx).is_some() {
            clusters.push(form_glyph_clusters(
                &glyphs[range],
                &chunk.text,
                levels[byte_idx.value()].is_rtl(),
            ));
        }
    }

    clusters
}

fn script_runs(text: &str, bytes: Range<usize>) -> Vec<(Range<usize>, Option<Script>)> {
    let mut runs = Vec::new();
    let mut start = bytes.start;
    let mut script = None;
    // Leading common and inherited characters follow the first strong script.
    // Within a run they stay with the preceding script, including combining marks.
    for (offset, ch) in text[bytes.clone()].char_indices() {
        let next = ch.script();
        if matches!(next, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        if let Some(previous) = script.replace(next) {
            if previous != next {
                let end = bytes.start + offset;
                runs.push((start..end, Some(previous)));
                start = end;
            }
        }
    }
    runs.push((start..bytes.end, script));
    runs
}

fn shaping_language<'a>(span: &'a TextSpan, default: Option<&'a str>) -> Option<&'a str> {
    span.language
        .as_deref()
        .or(default)
        .filter(|tag| super::is_language_tag(tag))
}

fn same_shaping_options(a: &TextSpan, b: &TextSpan, default_language: Option<&str>) -> bool {
    a.small_caps == b.small_caps
        && a.apply_kerning == b.apply_kerning
        && (a.language == b.language
            || match (
                shaping_language(a, default_language),
                shaping_language(b, default_language),
            ) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                (None, None) => true,
                _ => false,
            })
}

struct LengthCluster<'a> {
    cluster: &'a mut GlyphCluster,
    byte: usize,
    chunk_index: usize,
    shift: f32,
    owner: Option<usize>,
    // The interval passed to an enclosing adjustment can exceed the typographic
    // cell when a scaled parent ends with a child whose glyphs are already resolved.
    extent: f32,
}

impl LengthCluster<'_> {
    fn ends_unit_before(&self, next: &Self) -> bool {
        self.owner.is_none() || self.owner != next.owner
    }

    fn scale_cell(&mut self, factor: f32) {
        self.cluster.glyph_scale_x *= factor;
        self.cluster.width *= factor;
        self.extent *= factor;
    }
}

fn apply_length_adjust(text: &Text, unpositioned: &mut [UnpositionedChunk]) {
    // Measure in concatenated inline coordinates before placing chunks at their absolute x/y.
    let mut chunks_start = 0;
    let mut accumulated_shift = 0.0;
    let mut clusters = Vec::new();
    let mut logical = Vec::new();
    let mut chunk_offsets = Vec::with_capacity(unpositioned.len());
    for (
        chunk,
        UnpositionedChunk {
            clusters: glyphs,
            shifts,
            next_chunk_offset,
        },
    ) in text.chunks.iter().zip(unpositioned)
    {
        let chunk_index = chunk_offsets.len();
        chunk_offsets.push(next_chunk_offset);
        let first = clusters.len();
        logical.extend(shifts.logical_order.iter().map(|&index| first + index));
        clusters.extend(
            glyphs
                .iter_mut()
                .zip(&shifts.clusters)
                .map(|(cluster, shift)| LengthCluster {
                    byte: chunks_start + cluster.byte_idx.value(),
                    chunk_index,
                    shift: accumulated_shift + shift.x,
                    extent: cluster.advance,
                    cluster,
                    owner: None,
                }),
        );
        chunks_start += chunk.text.len();
        accumulated_shift += shifts.end.x;
    }
    let mut visual = Vec::new();
    for (index, adjustment) in text.length_adjustments.iter().enumerate() {
        let start = logical.partition_point(|&i| clusters[i].byte < adjustment.bytes.start);
        let end = logical.partition_point(|&i| clusters[i].byte < adjustment.bytes.end);
        visual.clear();
        visual.extend_from_slice(&logical[start..end]);
        visual.sort_unstable();
        let Some(&last) = visual.last() else {
            continue;
        };

        // These are the positions after CSS spacing, relative shifts and child adjustments.
        // Each typographic cell includes its CSS letter and word spacing.
        let mut left = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        let inline_advance = |clusters: &[LengthCluster<'_>], from: usize, to: usize| {
            clusters[from..to]
                .iter()
                .map(|cell| cell.cluster.advance)
                .sum::<f32>()
                + chunk_offsets[clusters[from].chunk_index..clusters[to].chunk_index]
                    .iter()
                    .map(|offset| **offset)
                    .sum::<f32>()
        };
        let mut advance = 0.0;
        let mut previous = visual[0];
        for &i in &visual {
            // An RTL run can place nonmembers between two members of this element.
            advance += inline_advance(&clusters, previous, i);
            previous = i;
            let cluster = &clusters[i];
            let start = advance + cluster.shift;
            let end = start + cluster.extent;
            left = left.min(start).min(end);
            right = right.max(start).max(end);
        }
        let width = right - left;

        let mut advance_change = 0.0;
        if adjustment.mode == LengthAdjust::Spacing {
            // A descendant with its own textLength contributes one spacing unit.
            let count = visual
                .windows(2)
                .filter(|pair| clusters[pair[0]].ends_unit_before(&clusters[pair[1]]))
                .count();
            if count > 0 {
                advance_change = adjustment.length - width;
                let spacing = advance_change / count as f32;
                for pair in visual.windows(2) {
                    if clusters[pair[0]].ends_unit_before(&clusters[pair[1]]) {
                        clusters[pair[0]].cluster.advance += spacing;
                    }
                }
            }
        } else {
            let factor = adjustment.length / width;
            if !factor.is_finite() || factor < 0.001 {
                continue;
            }

            // Scale the interval assigned to each unit. A resolved descendant moves as
            // a single unit and keeps its internal positions and glyph sizes.
            let mut unit_advance = 0.0;
            for pair in visual.windows(2) {
                let (index, next) = (pair[0], pair[1]);
                let end_unit = clusters[index].ends_unit_before(&clusters[next]);
                let gap = clusters[next].shift - clusters[index].shift;
                unit_advance += inline_advance(&clusters, index, next) + gap;
                let cluster = &mut clusters[index];
                if cluster.owner.is_none() {
                    cluster.scale_cell(factor);
                }
                if end_unit {
                    let delta = unit_advance * (factor - 1.0);
                    cluster.cluster.advance += delta;
                    advance_change += delta;
                    unit_advance = 0.0;
                }
            }
            let cluster = &mut clusters[last];
            if cluster.owner.is_none() {
                cluster.scale_cell(factor);
            } else {
                cluster.extent += (unit_advance + cluster.extent) * (factor - 1.0);
            }
        }
        advance_change += clusters[last].extent - clusters[last].cluster.advance;
        clusters[last].cluster.advance = clusters[last].extent;
        *chunk_offsets[clusters[last].chunk_index] -= advance_change;
        for &i in &visual {
            let cluster = &mut clusters[i];
            // Anchors and decoration use the actual cell, even if a parent has
            // reserved additional space for the following text.
            cluster
                .cluster
                .text_length_extent
                .get_or_insert(cluster.extent);
            cluster.owner = Some(index);
        }
    }
}

/// Rotates clusters according to
/// [Unicode Vertical_Orientation Property](https://www.unicode.org/reports/tr50/tr50-19.html).
fn apply_writing_mode(writing_mode: WritingMode, clusters: &mut [GlyphCluster]) {
    if writing_mode != WritingMode::TopToBottom {
        return;
    }

    for cluster in clusters {
        let orientation = unicode_vo::char_orientation(cluster.codepoint);
        if orientation == unicode_vo::Orientation::Upright {
            let mut ts = Transform::default();
            // Position glyph in the center of vertical axis.
            ts = ts.pre_translate(0.0, (cluster.ascent + cluster.descent) / 2.0);
            // Rotate by 90 degrees in the center.
            ts = ts.pre_rotate_at(
                -90.0,
                cluster.width / 2.0,
                -(cluster.ascent + cluster.descent) / 2.0,
            );

            cluster.path_transform = ts;

            // Move "baseline" to the middle and make height equal to width.
            cluster.ascent = cluster.width / 2.0;
            cluster.descent = -cluster.width / 2.0;
        } else {
            // Could not find a spec that explains this,
            // but this is how other applications are shifting the "rotated" characters
            // in the top-to-bottom mode.
            cluster.transform = cluster
                .transform
                .pre_translate(0.0, (cluster.ascent + cluster.descent) / 2.0);
        }
    }
}

/// Applies the `letter-spacing` property to a text chunk clusters.
///
/// [In the CSS spec](https://www.w3.org/TR/css-text-3/#letter-spacing-property).
fn apply_letter_spacing(chunk: &TextChunk, clusters: &mut [GlyphCluster]) {
    // At least one span should have a non-zero spacing.
    if !chunk
        .spans
        .iter()
        .any(|span| !span.letter_spacing.approx_zero_ulps(4))
    {
        return;
    }

    for cluster in clusters {
        // Spacing must be applied only to characters that belongs to the script
        // that supports spacing.
        // We are checking only the first code point, since it should be enough.
        // https://www.w3.org/TR/css-text-3/#cursive-tracking
        let script = cluster.codepoint.script();
        if script_supports_letter_spacing(script) {
            if let Some(span) = chunk_span_at(chunk, cluster.byte_idx) {
                cluster.advance += span.letter_spacing;

                // Finite negative spacing can move the next origin backwards.
                if !cluster.advance.is_finite() {
                    cluster.advance = cluster.width;
                }
            }
        }
    }
}

/// Applies the `word-spacing` property to a text chunk clusters.
///
/// [In the CSS spec](https://www.w3.org/TR/css-text-3/#propdef-word-spacing).
fn apply_word_spacing(chunk: &TextChunk, clusters: &mut [GlyphCluster]) {
    // At least one span should have a non-zero spacing.
    if !chunk
        .spans
        .iter()
        .any(|span| !span.word_spacing.approx_zero_ulps(4))
    {
        return;
    }

    for cluster in clusters {
        if is_word_separator_characters(cluster.codepoint) {
            if let Some(span) = chunk_span_at(chunk, cluster.byte_idx) {
                // Technically, word spacing 'should be applied half on each
                // side of the character', but it doesn't affect us in any way,
                // so we are ignoring this.
                cluster.advance += span.word_spacing;

                // After word spacing, `advance` can be negative.
            }
        }
    }
}

fn form_glyph_clusters(glyphs: &[Glyph], text: &str, rtl: bool) -> GlyphCluster {
    debug_assert!(!glyphs.is_empty());

    let mut width = 0.0;
    let mut x: f32 = 0.0;

    let mut positioned_glyphs = vec![];

    for glyph in glyphs {
        // Apply offset.
        //
        // The first glyph in the cluster will have an offset from 0x0,
        // but the later one will have an offset from the "current position".
        // So we have to keep an advance.
        // TODO: should be done only inside a single text span
        let ts = Transform::from_translate(x + glyph.dx as f32, -glyph.dy as f32);

        positioned_glyphs.push(PositionedGlyph {
            glyph_ts: ts,
            // Will be set later.
            cluster_ts: Transform::default(),
            // Will be set later.
            span_ts: Transform::default(),
            units_per_em: glyph.font.units_per_em.get(),
            font_size: glyph.font.size.get(),
            font: glyph.font.id,
            text: glyph.text.clone(),
            id: glyph.id,
        });

        x += glyph.width as f32;

        width += glyph.width as f32 * glyph.font.scale();
    }

    let byte_idx = glyphs[0].byte_idx;
    let byte_end = byte_idx.value() + glyphs.iter().map(|glyph| glyph.text.len()).sum::<usize>();
    let font = &glyphs[0].font;
    GlyphCluster {
        byte_idx,
        byte_end,
        codepoint: byte_idx.char_from(text),
        width,
        advance: width,
        text_length_extent: None,
        glyph_scale_x: 1.0,
        ascent: font.ascent(),
        descent: font.descent(),
        font: Arc::clone(font),
        has_relative_shift: false,
        transform: Transform::default(),
        path_transform: Transform::default(),
        rotation: 0.0,
        rtl,
        glyphs: positioned_glyphs,
        visible: true,
    }
}

fn resolve_font(
    fonts: &mut FontCache,
    selection: &FontInstance,
    span: &TextSpan,
) -> Option<ResolvedFont> {
    let (id, scale, native) = selection;
    let size = NonZeroPositiveF32::new(span.font_size.get() * scale)?;
    let loaded = fonts.load(*id)?;
    let variations = resolve_variations(&loaded.font, span, size.get(), native);
    let font = &loaded.instance(&variations).font;
    let metrics = font.metrics();
    if !(16..=16384).contains(&metrics.raw_units_per_em) {
        return None;
    }
    let units_per_em = NonZeroU16::new(metrics.raw_units_per_em)?;
    let line = metrics.h_line().unwrap_or_default();
    let ascent = line.ascender.to_f32();
    let descent = line.descender.to_f32();

    let x_height = metrics
        .x_height
        .map(|x| x.to_f32())
        .filter(|x| *x > 0.0)
        // Firefox uses 45% of the height when the font has no x-height.
        .unwrap_or_else(|| (ascent - descent) * 0.45);
    let x_height = NonZeroU16::new(u16::try_from(x_height.round() as i32).ok()?)?;

    let style = font.style_metrics();
    let line_through_position = style
        .strike_through
        .map_or(x_height.get() as i16 / 2, |metrics| {
            metrics.position.to_f32().round() as i16
        });
    let default_thickness = NonZeroU16::new(units_per_em.get() / 12)?;
    let (underline_position, underline_thickness) = style.underline.map_or(
        (-(units_per_em.get() as i16) / 9, default_thickness),
        |metrics| {
            let thickness = u16::try_from(metrics.thickness.to_f32().round() as i32)
                .ok()
                .and_then(NonZeroU16::new)
                .unwrap_or(default_thickness);
            (metrics.position.to_f32().round() as i16, thickness)
        },
    );

    // Match the upstream fallback for fonts with no OS/2 script offsets.
    let em = units_per_em.get() as f32;
    let subscript_offset = style
        .subscript
        .map_or((em / 0.2).round() as i16, |metrics| {
            metrics.y_offset.to_f32().round() as i16
        });
    let superscript_offset = style
        .superscript
        .map_or((em / 0.4).round() as i16, |metrics| {
            metrics.y_offset.to_f32().round() as i16
        });

    Some(ResolvedFont {
        id: *id,
        size,
        variations,
        units_per_em,
        ascent: ascent.round() as i16,
        descent: descent.round() as i16,
        x_height,
        underline_position,
        underline_thickness,
        line_through_position,
        subscript_offset,
        superscript_offset,
    })
}

fn resolve_variations(
    face: &skrifa::Font,
    span: &TextSpan,
    size: f32,
    native: &[FontVariation],
) -> Vec<FontVariation> {
    fn axis_value(variations: &[FontVariation], tag: [u8; 4]) -> Option<f32> {
        variations
            .iter()
            .rev()
            .find(|v| v.tag == tag && v.value.is_finite())
            .map(|v| v.value)
    }

    let mut effective = Vec::new();
    for axis in face.axes().iter() {
        let tag = axis.tag().to_be_bytes();
        let explicit = axis_value(span.font.explicit_variations(), tag);
        let authored = explicit.or_else(|| {
            // An italic request may be fulfilled by a slant-only face (and vice versa).
            // Keep its selected native angle when the preferred style axis is absent.
            let use_native = match (&tag, span.font.style) {
                (b"slnt", Some(FontStyle::Italic)) => {
                    face.axes().get_by_tag(Tag::new(b"ital")).is_none()
                }
                (b"ital", Some(FontStyle::Oblique)) => {
                    face.axes().get_by_tag(Tag::new(b"slnt")).is_none()
                }
                (b"wght", _) => span.font.weight.is_none(),
                _ => false,
            };
            if use_native {
                None
            } else {
                axis_value(&span.font.variations[span.font.explicit_axis_count..], tag)
            }
        });
        let value = authored.or_else(|| {
            if tag == *b"opsz" && span.font_optical_sizing == FontOpticalSizing::Auto {
                Some(size)
            } else {
                // CSS normal is the weight fallback even on fonts with a different default.
                axis_value(native, tag).or_else(|| (tag == *b"wght").then_some(400.0))
            }
        });
        if let Some(value) = value {
            let value = value.max(axis.min_value()).min(axis.max_value());
            if value != axis.default_value() {
                // Make signed zeros equivalent in instance keys.
                effective.push(FontVariation::new(tag, value + 0.0));
            }
        }
    }
    effective.sort_unstable_by_key(|v| v.tag);
    effective
}

fn same_physical_face(db: &Database, first: ID, second: ID) -> bool {
    use fontdb::Source;

    if first == second {
        return true;
    }
    let (Some(first), Some(second)) = (db.face(first), db.face(second)) else {
        return false;
    };
    if first.index != second.index {
        return false;
    }
    #[allow(unreachable_patterns)]
    match (&first.source, &second.source) {
        (Source::Binary(a), Source::Binary(b)) => Arc::ptr_eq(a, b),
        #[cfg(any(feature = "system-fonts", feature = "memmap-fonts"))]
        (Source::File(a), Source::File(b)) => a == b,
        #[cfg(feature = "memmap-fonts")]
        (Source::SharedFile(_, a), Source::SharedFile(_, b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

#[derive(Clone)]
struct ShapingRun<'a> {
    chunk: &'a TextChunk,
    bytes: Range<usize>,
    ltr: bool,
    span: &'a TextSpan,
    language: Option<&'a str>,
    script: Option<harfrust::Script>,
    disable_optional_ligatures: bool,
}

struct ShapedRun<'a> {
    run: ShapingRun<'a>,
    primary: Vec<Glyph>,
    repairs: Vec<ClusterRepair>,
}

impl ShapedRun<'_> {
    fn append_glyphs(self, glyphs: &mut Vec<Glyph>, fonts: &mut FontCache) {
        let Self {
            run,
            primary,
            mut repairs,
        } = self;
        repairs.retain(|r| {
            if r.glyphs.is_empty() {
                let c = ByteIndex::new(r.bytes.start).char_from(&run.chunk.text);
                log::warn!(
                    "No fonts with a {}/U+{:X} character were found.",
                    c,
                    c as u32
                );
            }
            !r.glyphs.is_empty()
        });
        let mut primary = primary.into_iter();
        let mut cursor = 0;
        let mut remaining = repairs.as_mut_slice();
        while !remaining.is_empty() {
            let (group, rest) =
                remaining.split_at_mut(ClusterRepair::run_length(remaining, &fonts.fontdb));
            remaining = rest;
            let start = group[0].original.start;
            let end = group.last().unwrap().original.end;
            glyphs.extend(primary.by_ref().take(start - cursor));
            primary.by_ref().take(end - start).for_each(drop);
            cursor = end;

            if let Some(joined) = run.shape_joined(group, fonts) {
                glyphs.extend(joined);
            } else {
                for repair in group {
                    for glyph in &mut repair.glyphs {
                        glyph.byte_idx = ByteIndex::new(repair.bytes.start);
                    }
                    glyphs.append(&mut repair.glyphs);
                }
            }
        }
        glyphs.extend(primary);
    }
}

struct ClusterRepair {
    // UTF-8 bytes in logical order, glyph indices in visual order.
    bytes: Range<usize>,
    original: Range<usize>,
    glyphs: Vec<Glyph>,
}

impl ClusterRepair {
    fn run_length(repairs: &[Self], fontdb: &Database) -> usize {
        let first = &repairs[0].glyphs[0];
        let mut next_glyph = repairs[0].original.start;
        for (index, repair) in repairs.iter().enumerate() {
            let glyph = &repair.glyphs[0];
            if next_glyph != repair.original.start || !glyph.font.same_instance(&first.font, fontdb)
            {
                return index;
            }
            next_glyph = repair.original.end;
        }
        repairs.len()
    }
}

#[derive(Default)]
struct FallbackCandidates {
    tried: Vec<FontInstance>,
    fonts: Vec<Arc<ResolvedFont>>,
    exhausted_at: Option<usize>,
}

fn same_selection(a: &FontInstance, b: &FontInstance) -> bool {
    a.0 == b.0 && a.1.to_bits() == b.1.to_bits() && a.2 == b.2
}

#[derive(Default)]
struct RunFallbacks<'a> {
    candidates: HashMap<&'a str, FallbackCandidates>,
    // Retain discovery order for the legacy scalar resolver.
    fonts: Vec<(FontInstance, Arc<ResolvedFont>)>,
}

type FallbackKey<'a> = (&'a Font, (u32, FontOpticalSizing), Option<&'a str>);
type Fallbacks<'a> = HashMap<FallbackKey<'a>, RunFallbacks<'a>>;

impl<'a> ShapingRun<'a> {
    fn shape_text(
        self,
        bytes: Range<usize>,
        font: &Arc<ResolvedFont>,
        fonts_cache: &FontsCache,
        options: &'a crate::Options<'_>,
        fonts: &mut FontCache,
        fallbacks: &mut Fallbacks<'a>,
    ) -> Option<ShapedRun<'a>> {
        let run_end = bytes.end;
        let primary = self.shape(bytes, font, fonts)?;
        let mut repairs = Vec::new();
        if primary.iter().all(|glyph| !glyph.is_missing()) {
            return Some(ShapedRun {
                run: self,
                primary,
                repairs,
            });
        }

        for (range, start) in GlyphClusters::new(&primary) {
            if !primary[range.clone()].iter().any(Glyph::is_missing) {
                continue;
            }
            let next_logical = if self.ltr {
                primary.get(range.end)
            } else {
                range
                    .start
                    .checked_sub(1)
                    .and_then(|index| primary.get(index))
            };
            let bytes = start.value()..next_logical.map_or(run_end, |glyph| glyph.byte_idx.value());
            let glyphs = self
                .repair_at(bytes.clone(), fonts_cache, options, fonts, fallbacks)
                .unwrap_or_default();
            repairs.push(ClusterRepair {
                bytes,
                original: range,
                glyphs,
            });
        }

        Some(ShapedRun {
            run: self,
            primary,
            repairs,
        })
    }

    fn with_cluster_span(&self, start: usize, default_language: Option<&'a str>) -> Option<Self> {
        // Equivalent primary instances can still have different fallback requests.
        let span = chunk_span_at(self.chunk, ByteIndex::new(start))?;
        Some(Self {
            span,
            language: shaping_language(span, default_language),
            ..self.clone()
        })
    }

    fn repair_at(
        &self,
        bytes: Range<usize>,
        fonts_cache: &FontsCache,
        options: &'a crate::Options<'_>,
        fonts: &mut FontCache,
        fallbacks: &mut Fallbacks<'a>,
    ) -> Option<Vec<Glyph>> {
        let run = self.with_cluster_span(bytes.start, options.text_fallback.language.as_deref())?;
        let (selection, primary) = primary_font(fonts_cache, run.span)?;
        run.repair(
            bytes,
            (selection, primary.as_ref()),
            options,
            fonts,
            fallbacks.entry(run.fallback_key()).or_default(),
        )
    }

    fn fallback_key(&self) -> FallbackKey<'a> {
        (
            &self.span.font,
            span_font_size_key(self.span),
            self.language,
        )
    }

    fn shape_joined(&self, group: &[ClusterRepair], fonts: &mut FontCache) -> Option<Vec<Glyph>> {
        if group.len() < 2 {
            return None;
        }
        let first = &group[0];
        let last = group.last()?;
        let bytes = first.bytes.start.min(last.bytes.start)..first.bytes.end.max(last.bytes.end);
        let face = &first.glyphs[0];
        let mut glyphs = self.shape_covered(bytes, &face.font, fonts)?;

        // HarfRust emits monotone clusters in visual order, including for RTL.
        // A ligature belongs to its logical start; a split primary cluster keeps
        // its original positioning and paint even if the fallback exposes finer indices.
        let mut owners = group.iter();
        let mut owner = owners.next()?;
        for glyph in &mut glyphs {
            while !owner.bytes.contains(&glyph.byte_idx.value()) {
                owner = owners.next()?;
            }
            glyph.byte_idx = ByteIndex::new(owner.bytes.start);
        }
        Some(glyphs)
    }

    fn repair(
        &self,
        bytes: Range<usize>,
        (selection, primary): (&FontInstance, &ResolvedFont),
        options: &crate::Options,
        fonts: &mut FontCache,
        fallbacks: &mut RunFallbacks<'a>,
    ) -> Option<Vec<Glyph>> {
        if bytes.is_empty() {
            return None;
        }
        let cluster = self.chunk.text.get(bytes.clone())?;
        let candidates =
            fallbacks
                .candidates
                .entry(cluster)
                .or_insert_with(|| FallbackCandidates {
                    tried: vec![selection.clone()],
                    ..FallbackCandidates::default()
                });
        // Cache choices for identical cluster text, while always shaping in context.
        for font in &candidates.fonts {
            if let Some(glyphs) = self.shape_covered(bytes.clone(), font, fonts) {
                return Some(glyphs);
            }
        }

        while candidates.exhausted_at != Some(fonts.fontdb.len()) {
            let choice =
                self.select_fallback(cluster, &candidates.tried, options, &mut fonts.fontdb);
            let Some(choice) = choice else {
                candidates.exhausted_at = Some(fonts.fontdb.len());
                break;
            };
            candidates.tried.push(choice);
            let choice = candidates.tried.last()?;
            let font = match fallbacks
                .fonts
                .iter()
                .find(|(old, _)| same_selection(old, choice))
            {
                Some((_, font)) => Arc::clone(font),
                None => {
                    let Some(font) = resolve_font(fonts, choice, self.span) else {
                        continue;
                    };
                    let font = Arc::new(font);
                    fallbacks.fonts.push((choice.clone(), Arc::clone(&font)));
                    font
                }
            };
            // Different native selections can resolve to the same effective font after CSS.
            if font.same_instance(primary, &fonts.fontdb)
                || candidates
                    .fonts
                    .iter()
                    .any(|old| font.same_instance(old, &fonts.fontdb))
            {
                continue;
            }
            let glyphs = self.shape_covered(bytes.clone(), &font, fonts);
            candidates.fonts.push(font);
            if glyphs.is_some() {
                return glyphs;
            }
        }
        None
    }

    fn select_fallback(
        &self,
        cluster: &str,
        tried: &[FontInstance],
        options: &crate::Options,
        fontdb: &mut Arc<Database>,
    ) -> Option<FontInstance> {
        if let Some(select) = &options.text_fallback.fallback_instance {
            return select(
                &self.span.font,
                cluster,
                self.language,
                !self.ltr,
                tried,
                fontdb,
            )
            .filter(|next| !tried.iter().any(|old| same_selection(old, next)));
        }

        let used: Vec<_> = tried.iter().map(|face| face.0).collect();
        let (id, scale) = if let Some(select) = &options.text_fallback.select {
            select(
                &self.span.font,
                cluster,
                self.language,
                !self.ltr,
                &used,
                fontdb,
            )?
        } else {
            let id =
                (options.font_resolver.select_fallback)(cluster.chars().next()?, &used, fontdb)?;
            (id, 1.0)
        };
        (!used.contains(&id)).then(|| (id, scale, Vec::new()))
    }

    fn shape_covered(
        &self,
        bytes: Range<usize>,
        font: &Arc<ResolvedFont>,
        fonts: &mut FontCache,
    ) -> Option<Vec<Glyph>> {
        self.shape(bytes, font, fonts)
            .filter(|glyphs| !glyphs.is_empty() && !glyphs.iter().any(Glyph::is_missing))
    }

    fn shape(
        &self,
        bytes: Range<usize>,
        font: &Arc<ResolvedFont>,
        fonts: &mut FontCache,
    ) -> Option<Vec<Glyph>> {
        let chunk_text = &self.chunk.text;
        let text = chunk_text.get(bytes.clone())?;
        let LoadedInstance {
            font: instance,
            plans,
            ..
        } = fonts.load(font.id)?.instance(&font.variations);
        let direction = if self.ltr {
            Direction::LeftToRight
        } else {
            Direction::RightToLeft
        };

        let mut buffer = Buffer::new();
        buffer.push_str(text);
        buffer.set_pre_context(&chunk_text[self.bytes.start..bytes.start]);
        buffer.set_post_context(&chunk_text[bytes.end..self.bytes.end]);
        buffer.set_direction(direction);
        buffer.set_language(self.language.and_then(harfrust::Language::new));
        buffer.set_script(self.script);
        buffer.guess_segment_properties();

        let mut features = Vec::new();
        if self.span.small_caps {
            features.push(Feature::new(Tag::new(b"smcp"), 1, ..));
        }
        if !self.span.apply_kerning {
            features.push(Feature::new(Tag::new(b"kern"), 0, ..));
        }
        if self.disable_optional_ligatures {
            features.extend(
                [b"liga", b"clig", b"dlig", b"hlig", b"cala"]
                    .map(|tag| Feature::new(Tag::new(tag), 0, ..)),
            );
        }

        let key = ShapePlanKey::new(instance, buffer.script(), direction)
            .language(buffer.language())
            .features(&features);
        let plan = match plans.iter().position(|plan| key.matches(plan)) {
            Some(index) => &plans[index],
            None => {
                plans.push(ShapePlan::new(
                    instance,
                    direction,
                    buffer.script(),
                    buffer.language(),
                    &features,
                ));
                plans.last()?
            }
        };
        harfrust::shape(
            &ShaperFont::new(instance),
            &mut buffer,
            ShapeOptions::new().features(&features).plan(Some(plan)),
        )
        .ok()?;

        let infos = buffer.glyph_infos();
        let glyphs = infos
            .iter()
            .zip(buffer.glyph_positions())
            .enumerate()
            .map(|(i, (info, pos))| {
                let start = info.cluster as usize;
                let next = if self.ltr {
                    i.checked_add(1)
                } else {
                    i.checked_sub(1)
                };
                let end = next
                    .and_then(|j| infos.get(j))
                    .map_or(text.len(), |next| next.cluster as usize);
                Glyph {
                    byte_idx: ByteIndex::new(bytes.start + start),
                    text: text.get(start..end).unwrap_or_default().to_owned(),
                    id: GlyphId(info.glyph_id),
                    dx: pos.x_offset,
                    dy: pos.y_offset,
                    width: pos.x_advance,
                    font: Arc::clone(font),
                }
            })
            .collect();
        Some(glyphs)
    }
}

/// An iterator over glyph clusters.
///
/// Input:  0 2 2 2 3 4 4 5 5
/// Result: 0 1     4 5   7
pub(crate) struct GlyphClusters<'a> {
    data: &'a [Glyph],
    idx: usize,
}

impl<'a> GlyphClusters<'a> {
    pub(crate) fn new(data: &'a [Glyph]) -> Self {
        GlyphClusters { data, idx: 0 }
    }
}

impl Iterator for GlyphClusters<'_> {
    type Item = (std::ops::Range<usize>, ByteIndex);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == self.data.len() {
            return None;
        }

        let start = self.idx;
        let cluster = self.data[self.idx].byte_idx;
        for g in &self.data[self.idx..] {
            if g.byte_idx != cluster {
                break;
            }

            self.idx += 1;
        }

        Some((start..self.idx, cluster))
    }
}

/// Checks that selected script supports letter spacing.
///
/// [In the CSS spec](https://www.w3.org/TR/css-text-3/#cursive-tracking).
///
/// The list itself is from: https://github.com/harfbuzz/harfbuzz/issues/64
pub(crate) fn script_supports_letter_spacing(script: unicode_script::Script) -> bool {
    use unicode_script::Script;

    !matches!(
        script,
        Script::Arabic
            | Script::Syriac
            | Script::Nko
            | Script::Manichaean
            | Script::Psalter_Pahlavi
            | Script::Mandaic
            | Script::Mongolian
            | Script::Phags_Pa
            | Script::Devanagari
            | Script::Bengali
            | Script::Gurmukhi
            | Script::Modi
            | Script::Sharada
            | Script::Syloti_Nagri
            | Script::Tirhuta
            | Script::Ogham
    )
}

/// A glyph.
///
/// Basically, a glyph ID and it's metrics.
#[derive(Clone)]
pub(crate) struct Glyph {
    /// The glyph ID in the font.
    pub(crate) id: GlyphId,

    /// Position in bytes in the original string.
    ///
    /// We use it to match a glyph with a character in the text chunk and therefore with the style.
    pub(crate) byte_idx: ByteIndex,

    /// The text from the original string that corresponds to that glyph.
    pub(crate) text: String,

    /// The glyph offset in font units.
    pub(crate) dx: i32,

    /// The glyph offset in font units.
    pub(crate) dy: i32,

    /// The glyph width / X-advance in font units.
    pub(crate) width: i32,

    /// Reference to the source font.
    ///
    /// Each glyph can have it's own source font.
    pub(crate) font: Arc<ResolvedFont>,
}

impl Glyph {
    fn is_missing(&self) -> bool {
        self.id.0 == 0
    }
}

#[derive(Debug)]
pub(crate) struct ResolvedFont {
    pub(crate) id: ID,
    size: NonZeroPositiveF32,
    variations: Vec<FontVariation>,

    units_per_em: NonZeroU16,

    // All values below are in font units.
    ascent: i16,
    descent: i16,
    x_height: NonZeroU16,

    underline_position: i16,
    underline_thickness: NonZeroU16,

    // line-through thickness should be the the same as underline thickness
    // according to the TrueType spec:
    // https://docs.microsoft.com/en-us/typography/opentype/spec/os2#ystrikeoutsize
    line_through_position: i16,

    subscript_offset: i16,
    superscript_offset: i16,
}

pub(crate) fn chunk_span_at(chunk: &TextChunk, byte_offset: ByteIndex) -> Option<&TextSpan> {
    chunk_span_index_at(chunk, byte_offset).map(|index| &chunk.spans[index])
}

fn chunk_span_index_at(chunk: &TextChunk, byte_offset: ByteIndex) -> Option<usize> {
    let index = chunk
        .spans
        .partition_point(|span| span.end <= byte_offset.value());
    span_contains(chunk.spans.get(index)?, byte_offset).then_some(index)
}

pub(crate) fn span_contains(span: &TextSpan, byte_offset: ByteIndex) -> bool {
    byte_offset.value() >= span.start && byte_offset.value() < span.end
}

/// Checks that the selected character is a word separator.
///
/// According to: https://www.w3.org/TR/css-text-3/#word-separator
pub(crate) fn is_word_separator_characters(c: char) -> bool {
    matches!(
        c as u32,
        0x0020 | 0x00A0 | 0x1361 | 0x010100 | 0x010101 | 0x01039F | 0x01091F
    )
}

impl ResolvedFont {
    fn same_instance(&self, other: &Self, db: &Database) -> bool {
        std::ptr::eq(self, other)
            || (self.size.get() == other.size.get()
                && self.variations == other.variations
                && same_physical_face(db, self.id, other.id))
    }

    #[inline]
    fn scale(&self) -> f32 {
        self.size.get() / self.units_per_em.get() as f32
    }

    #[inline]
    fn ascent(&self) -> f32 {
        self.ascent as f32 * self.scale()
    }

    #[inline]
    fn descent(&self) -> f32 {
        self.descent as f32 * self.scale()
    }

    #[inline]
    fn height(&self) -> f32 {
        self.ascent() - self.descent()
    }

    #[inline]
    fn x_height(&self) -> f32 {
        self.x_height.get() as f32 * self.scale()
    }

    #[inline]
    fn underline_position(&self) -> f32 {
        self.underline_position as f32 * self.scale()
    }

    #[inline]
    fn underline_thickness(&self) -> f32 {
        self.underline_thickness.get() as f32 * self.scale()
    }

    #[inline]
    fn line_through_position(&self) -> f32 {
        self.line_through_position as f32 * self.scale()
    }

    #[inline]
    fn subscript_offset(&self) -> f32 {
        self.subscript_offset as f32 * self.scale()
    }

    #[inline]
    fn superscript_offset(&self) -> f32 {
        self.superscript_offset as f32 * self.scale()
    }

    fn dominant_baseline_shift(&self, baseline: DominantBaseline) -> f32 {
        let alignment = match baseline {
            DominantBaseline::Auto => AlignmentBaseline::Auto,
            DominantBaseline::UseScript => AlignmentBaseline::Auto, // unsupported
            DominantBaseline::NoChange => AlignmentBaseline::Auto,  // already resolved
            DominantBaseline::ResetSize => AlignmentBaseline::Auto, // unsupported
            DominantBaseline::Ideographic => AlignmentBaseline::Ideographic,
            DominantBaseline::Alphabetic => AlignmentBaseline::Alphabetic,
            DominantBaseline::Hanging => AlignmentBaseline::Hanging,
            DominantBaseline::Mathematical => AlignmentBaseline::Mathematical,
            DominantBaseline::Central => AlignmentBaseline::Central,
            DominantBaseline::Middle => AlignmentBaseline::Middle,
            DominantBaseline::TextAfterEdge => AlignmentBaseline::TextAfterEdge,
            DominantBaseline::TextBeforeEdge => AlignmentBaseline::TextBeforeEdge,
        };

        self.alignment_baseline_shift(alignment)
    }

    // The `alignment-baseline` property is a mess.
    //
    // The SVG 1.1 spec (https://www.w3.org/TR/SVG11/text.html#BaselineAlignmentProperties)
    // goes on and on about what this property suppose to do, but doesn't actually explain
    // how it should be implemented. It's just a very verbose overview.
    //
    // As of Nov 2022, only Chrome and Safari support `alignment-baseline`. Firefox isn't.
    // Same goes for basically every SVG library in existence.
    // Meaning we have no idea how exactly it should be implemented.
    //
    // And even Chrome and Safari cannot agree on how to handle `baseline`, `after-edge`,
    // `text-after-edge` and `ideographic` variants. Producing vastly different output.
    //
    // As per spec, a proper implementation should get baseline values from the font itself,
    // using `BASE` and `bsln` TrueType tables. If those tables are not present,
    // we have to synthesize them (https://drafts.csswg.org/css-inline/#baseline-synthesis-fonts).
    // And in the worst case scenario simply fallback to hardcoded values.
    //
    // Also, most fonts do not provide `BASE` and `bsln` tables to begin with.
    //
    // Again, as of Nov 2022, Chrome does only the latter:
    // https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/platform/fonts/font_metrics.cc#L153
    //
    // Since baseline TrueType tables parsing and baseline synthesis are pretty hard,
    // we do what Chrome does - use hardcoded values. And it seems like Safari does the same.
    //
    //
    // But that's not all! SVG 2 and CSS Inline Layout 3 did a baseline handling overhaul,
    // and it's far more complex now. Not sure if anyone actually supports it.
    fn alignment_baseline_shift(&self, alignment: AlignmentBaseline) -> f32 {
        match alignment {
            AlignmentBaseline::Auto => 0.0,
            AlignmentBaseline::Baseline => 0.0,
            AlignmentBaseline::BeforeEdge | AlignmentBaseline::TextBeforeEdge => self.ascent(),
            AlignmentBaseline::Middle => self.x_height() * 0.5,
            AlignmentBaseline::Central => self.ascent() - self.height() * 0.5,
            AlignmentBaseline::AfterEdge | AlignmentBaseline::TextAfterEdge => self.descent(),
            AlignmentBaseline::Ideographic => self.descent(),
            AlignmentBaseline::Alphabetic => 0.0,
            AlignmentBaseline::Hanging => self.ascent() * 0.8,
            AlignmentBaseline::Mathematical => self.ascent() * 0.5,
        }
    }
}

type FontsCache<'a> = HashMap<&'a Font, PrimaryFont>;

struct PrimaryFont {
    selection: FontInstance,
    sizes: HashMap<(u32, FontOpticalSizing), Arc<ResolvedFont>>,
}

fn span_font_size_key(span: &TextSpan) -> (u32, FontOpticalSizing) {
    (span.font_size.get().to_bits(), span.font_optical_sizing)
}

fn primary_font<'a>(
    cache: &'a FontsCache,
    span: &TextSpan,
) -> Option<(&'a FontInstance, &'a Arc<ResolvedFont>)> {
    let primary = cache.get(&span.font)?;
    let font = primary.sizes.get(&span_font_size_key(span))?;
    Some((&primary.selection, font))
}

/// A read-only text index in bytes.
///
/// Guarantee to be on a char boundary and in text bounds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct ByteIndex(usize);

impl ByteIndex {
    fn new(i: usize) -> Self {
        ByteIndex(i)
    }

    pub(crate) fn value(&self) -> usize {
        self.0
    }

    /// Converts byte position into a character.
    pub(crate) fn char_from(&self, text: &str) -> char {
        text[self.0..].chars().next().unwrap()
    }
}
