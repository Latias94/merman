// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::num::NonZeroU16;
use std::ops::Range;
use std::sync::Arc;

use fontdb::{Database, ID};
use kurbo::{ParamCurve, ParamCurveArclen, ParamCurveDeriv};
use rustybuzz::ttf_parser;
use rustybuzz::ttf_parser::{GlyphId, Tag};
use strict_num::NonZeroPositiveF32;
use tiny_skia_path::{NonZeroRect, Transform};
use unicode_script::UnicodeScript;

use crate::tree::{BBox, IsValidLength};
use crate::{
    AlignmentBaseline, ApproxZeroUlps, BaselineShift, DominantBaseline, Fill, FillRule, Font,
    LengthAdjust, PaintOrder, Path, ShapeRendering, Stroke, Text, TextAnchor, TextChunk,
    TextDecorationStyle, TextFlow, TextPath, TextSpan, WritingMode,
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

    /// Returns the transform for the glyph, assuming that a CBTD-based raster glyph
    /// is being used.
    pub fn cbdt_transform(&self, x: f32, y: f32, pixels_per_em: f32, height: f32) -> Transform {
        self.transform()
            .pre_concat(Transform::from_scale(
                self.units_per_em as f32 / pixels_per_em,
                self.units_per_em as f32 / pixels_per_em,
            ))
            // Right now, the top-left corner of the image would be placed in
            // on the "text cursor", but we want the bottom-left corner to be there,
            // so we need to shift it up and also apply the x/y offset.
            .pre_translate(x, -height - y)
    }

    /// Returns the transform for the glyph, assuming that a sbix-based raster glyph
    /// is being used.
    pub fn sbix_transform(
        &self,
        x: f32,
        y: f32,
        x_min: f32,
        y_min: f32,
        pixels_per_em: f32,
        height: f32,
    ) -> Transform {
        // In contrast to CBDT, we also need to look at the outline bbox of the glyph and add a shift if necessary.
        let bbox_x_shift = -x_min;

        let bbox_y_shift = if y_min.approx_zero_ulps(4) {
            // For unknown reasons, using Apple Color Emoji will lead to a vertical shift on MacOS, but this shift
            // doesn't seem to be coming from the font and most likely is somehow hardcoded. On Windows,
            // this shift will not be applied. However, if this shift is not applied the emojis are a bit
            // too high up when being together with other text, so we try to imitate this.
            // See also https://github.com/harfbuzz/harfbuzz/issues/2679#issuecomment-1345595425
            // So whenever the y-shift is 0, we approximate this vertical shift that seems to be produced by it.
            // This value seems to be pretty close to what is happening on MacOS.
            // We can still remove this if it turns out to be a problem, but Apple Color Emoji is pretty
            // much the only `sbix` font out there and they all seem to have a y-shift of 0, so it
            // makes sense to keep it.
            0.128 * self.units_per_em as f32
        } else {
            -y_min
        };

        self.transform()
            .pre_concat(Transform::from_translate(bbox_x_shift, bbox_y_shift))
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

/// A span contains a number of layouted glyphs that share the same fill, stroke, paint order and
/// visibility.
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
    /// Font variation settings for variable fonts.
    pub variations: Vec<crate::FontVariation>,
    /// Font optical sizing mode.
    pub font_optical_sizing: crate::FontOpticalSizing,
    /// The visibility of the span.
    pub visible: bool,
    /// The glyphs that make up the span.
    pub positioned_glyphs: Vec<PositionedGlyph>,
    /// An underline text decoration of the span.
    /// Needs to be rendered before all glyphs.
    pub underline: Option<Path>,
    /// An overline text decoration of the span.
    /// Needs to be rendered before all glyphs.
    pub overline: Option<Path>,
    /// A line-through text decoration of the span.
    /// Needs to be rendered after all glyphs.
    pub line_through: Option<Path>,
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
    codepoint: char,
    width: f32,
    advance: f32,
    ascent: f32,
    descent: f32,
    has_relative_shift: bool,
    glyphs: Vec<PositionedGlyph>,
    transform: Transform,
    path_transform: Transform,
    visible: bool,
}

impl GlyphCluster {
    pub(crate) fn height(&self) -> f32 {
        self.ascent - self.descent
    }

    pub(crate) fn transform(&self) -> Transform {
        self.path_transform.post_concat(self.transform)
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
        && matches!(text.chunks.as_slice(), [chunk]
            if chunk.anchor == TextAnchor::Start
                && matches!(chunk.text_flow, TextFlow::Linear)
                && chunk.spans.iter().all(|span| span.text_length.is_none()))
}

pub(crate) fn layout_text(
    text_node: &Text,
    options: &crate::Options,
    fontdb: &mut Arc<fontdb::Database>,
    probe_end_anchor: bool,
    no_primary_face: &mut bool,
) -> Option<(TextLayout, Option<TextLayout>)> {
    let mut fonts_cache: FontsCache = HashMap::new();
    let mut primary_miss = false;

    for chunk in &text_node.chunks {
        for span in &chunk.spans {
            if !fonts_cache.contains_key(&span.font) {
                if let Some(font) = (options.font_resolver.select_font)(&span.font, fontdb)
                    .and_then(|id| fontdb.load_font(id))
                {
                    fonts_cache.insert(span.font.clone(), Arc::new(font));
                } else {
                    primary_miss |= span.has_visible_paint();
                }
            }
        }
    }

    if primary_miss && !*no_primary_face {
        // A later span may have resolved the same request. Check the font cache that
        // actually controls which spans can emit glyphs.
        *no_primary_face = text_node.chunks.iter().any(|chunk| {
            chunk
                .spans
                .iter()
                .any(|span| span.has_visible_paint() && !fonts_cache.contains_key(&span.font))
        });
    }

    let mut geometry = TextGeometry::default();
    let mut end_layout = None;
    let mut char_offset = 0;
    let mut last_x = 0.0;
    let mut last_y = 0.0;
    for chunk in &text_node.chunks {
        let (x, y) = match chunk.text_flow {
            TextFlow::Linear => (chunk.x.unwrap_or(last_x), chunk.y.unwrap_or(last_y)),
            TextFlow::Path(_) => (0.0, 0.0),
        };

        let mut clusters = process_chunk(chunk, &fonts_cache, options, fontdb);
        if clusters.is_empty() {
            char_offset += chunk.text.chars().count();
            continue;
        }

        apply_writing_mode(text_node.writing_mode, &mut clusters);
        apply_letter_spacing(chunk, &mut clusters);
        apply_word_spacing(chunk, &mut clusters);

        apply_length_adjust(chunk, &mut clusters);
        let mut curr_pos = resolve_clusters_positions(
            text_node,
            chunk,
            char_offset,
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
                char_offset,
                TextAnchor::End,
                &fonts_cache,
                &mut clusters,
            );
            append_chunk_geometry(text_node, chunk, (x, y), &fonts_cache, &clusters, &mut end);
            end_layout = end.finish();
        }

        char_offset += chunk.text.chars().count();

        if text_node.writing_mode == WritingMode::TopToBottom {
            if let TextFlow::Linear = chunk.text_flow {
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

    for span in &chunk.spans {
        let font = match fonts_cache.get(&span.font) {
            Some(v) => v,
            None => continue,
        };

        let decoration_spans = collect_decoration_spans(span, clusters);

        let mut span_ts = text_ts;
        span_ts = span_ts.pre_translate(x, y);
        if let TextFlow::Linear = chunk.text_flow {
            let shift = resolve_baseline(span, font, text_node.writing_mode);

            // In case of a horizontal flow, shift transform and not clusters,
            // because clusters can be rotated and an additional shift will lead
            // to invalid results.
            span_ts = span_ts.pre_translate(0.0, shift);
        }

        let mut underline = None;
        let mut overline = None;
        let mut line_through = None;

        if let Some(decoration) = span.decoration.underline.clone() {
            // TODO: No idea what offset should be used for top-to-bottom layout.
            // There is
            // https://www.w3.org/TR/css-text-decor-3/#text-underline-position-property
            // but it doesn't go into details.
            let offset = match text_node.writing_mode {
                WritingMode::LeftToRight => -font.underline_position(span.font_size.get()),
                WritingMode::TopToBottom => font.height(span.font_size.get()) / 2.0,
            };

            if let Some(path) =
                convert_decoration(offset, span, font, decoration, &decoration_spans, span_ts)
            {
                geometry.bbox = geometry.bbox.expand(path.data.bounds());
                underline = Some(path);
            }
        }

        if let Some(decoration) = span.decoration.overline.clone() {
            let offset = match text_node.writing_mode {
                WritingMode::LeftToRight => -font.ascent(span.font_size.get()),
                WritingMode::TopToBottom => -font.height(span.font_size.get()) / 2.0,
            };

            if let Some(path) =
                convert_decoration(offset, span, font, decoration, &decoration_spans, span_ts)
            {
                geometry.bbox = geometry.bbox.expand(path.data.bounds());
                overline = Some(path);
            }
        }

        if let Some(decoration) = span.decoration.line_through.clone() {
            let offset = match text_node.writing_mode {
                WritingMode::LeftToRight => -font.line_through_position(span.font_size.get()),
                WritingMode::TopToBottom => 0.0,
            };

            if let Some(path) =
                convert_decoration(offset, span, font, decoration, &decoration_spans, span_ts)
            {
                geometry.bbox = geometry.bbox.expand(path.data.bounds());
                line_through = Some(path);
            }
        }

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

            let positioned_glyphs = span_fragments
                .into_iter()
                .flat_map(|mut gc| {
                    let cluster_ts = gc.transform();
                    gc.glyphs.iter_mut().for_each(|pg| {
                        pg.cluster_ts = cluster_ts;
                        pg.span_ts = span_ts;
                    });
                    gc.glyphs
                })
                .collect();

            append_sized_spans(
                &mut geometry.spans,
                Span {
                    fill,
                    stroke: span.stroke.clone(),
                    paint_order: span.paint_order,
                    font_size: span.font_size,
                    variations: span.font.variations.clone(),
                    font_optical_sizing: span.font_optical_sizing,
                    visible: span.visible,
                    positioned_glyphs,
                    underline,
                    overline,
                    line_through,
                },
            );
        }
    }
}

fn append_sized_spans(spans: &mut Vec<Span>, mut span: Span) {
    if span
        .positioned_glyphs
        .iter()
        .all(|glyph| glyph.font_size() == span.font_size.get())
    {
        spans.push(span);
        return;
    }

    // PDF adapters use the span size for optical sizing and glyph drawing.
    // Keep decorations around the original paint span when its glyph sizes differ.
    let first = spans.len();
    let mut glyphs = std::mem::take(&mut span.positioned_glyphs)
        .into_iter()
        .peekable();
    let underline = span.underline.take();
    let overline = span.overline.take();
    let line_through = span.line_through.take();
    while let Some(glyph) = glyphs.next() {
        let size = glyph.font_size();
        let mut positioned_glyphs = vec![glyph];
        while let Some(glyph) = glyphs.next_if(|next| next.font_size() == size) {
            positioned_glyphs.push(glyph);
        }
        spans.push(Span {
            font_size: NonZeroPositiveF32::new(size).unwrap_or(span.font_size),
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

        let mut advance = cluster.advance;
        if advance <= 0.0 {
            advance = 1.0;
        }

        // We have to calculate text bbox using font metrics and not glyph shape.
        if let Some(r) = NonZeroRect::from_xywh(0.0, -cluster.ascent, advance, cluster.height()) {
            if let Some(r) = r.transform(cluster.transform()) {
                bboxes_builder.push_rect(r.to_rect());
            }
        }
    }

    let mut bboxes = bboxes_builder.finish()?;
    bboxes = bboxes.transform(text_ts)?;
    let bbox = bboxes.compute_tight_bounds()?.to_non_zero_rect()?;

    Some((span_clusters, bbox))
}

fn collect_decoration_spans(span: &TextSpan, clusters: &[GlyphCluster]) -> Vec<DecorationSpan> {
    let mut spans = Vec::new();

    let mut started = false;
    let mut width = 0.0;
    let mut transform = Transform::default();

    for cluster in clusters {
        if span_contains(span, cluster.byte_idx) {
            if started && cluster.has_relative_shift {
                started = false;
                spans.push(DecorationSpan { width, transform });
            }

            if !started {
                width = cluster.advance;
                started = true;
                transform = cluster.transform;
            } else {
                width += cluster.advance;
            }
        } else if started {
            spans.push(DecorationSpan { width, transform });
            started = false;
        }
    }

    if started {
        spans.push(DecorationSpan { width, transform });
    }

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
    debug_assert!(!decoration_spans.is_empty());

    let thickness = font.underline_thickness(span.font_size.get());

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

    Path::new(
        String::new(),
        span.visible,
        decoration.fill.take(),
        decoration.stroke.take(),
        PaintOrder::default(),
        ShapeRendering::default(),
        Arc::new(path_data),
        Transform::default(),
    )
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
    char_offset: usize,
    anchor: TextAnchor,
    fonts_cache: &FontsCache,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let writing_mode = text.writing_mode;
    match chunk.text_flow {
        TextFlow::Linear => {
            resolve_clusters_positions_horizontal(text, chunk, char_offset, anchor, clusters)
        }
        TextFlow::Path(ref path) => resolve_clusters_positions_path(
            text,
            chunk,
            char_offset,
            path,
            writing_mode,
            fonts_cache,
            clusters,
        ),
    }
}

fn clusters_length(clusters: &[GlyphCluster]) -> f32 {
    clusters.iter().fold(0.0, |w, cluster| w + cluster.advance)
}

fn resolve_clusters_positions_horizontal(
    text: &Text,
    chunk: &TextChunk,
    offset: usize,
    anchor: TextAnchor,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let mut x = process_anchor(anchor, clusters_length(clusters));
    let mut y = 0.0;

    for cluster in clusters {
        let cp = offset + cluster.byte_idx.code_point_at(&chunk.text);
        if let (Some(dx), Some(dy)) = (text.dx.get(cp), text.dy.get(cp)) {
            if text.writing_mode == WritingMode::LeftToRight {
                x += dx;
                y += dy;
            } else {
                y -= dx;
                x += dy;
            }
            cluster.has_relative_shift = !dx.approx_zero_ulps(4) || !dy.approx_zero_ulps(4);
        }

        cluster.transform = cluster.transform.pre_translate(x, y);

        if let Some(angle) = text.rotate.get(cp).cloned() {
            if !angle.approx_zero_ulps(4) {
                cluster.transform = cluster.transform.pre_rotate(angle);
                cluster.has_relative_shift = true;
            }
        }

        x += cluster.advance;
    }

    (x, y)
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
    let mut shift = -resolve_baseline_shift(&span.baseline_shift, font, span.font_size.get());

    // TODO: support vertical layout as well
    if writing_mode == WritingMode::LeftToRight {
        if span.alignment_baseline == AlignmentBaseline::Auto
            || span.alignment_baseline == AlignmentBaseline::Baseline
        {
            shift += font.dominant_baseline_shift(span.dominant_baseline, span.font_size.get());
        } else {
            shift += font.alignment_baseline_shift(span.alignment_baseline, span.font_size.get());
        }
    }

    shift
}

fn resolve_baseline_shift(baselines: &[BaselineShift], font: &ResolvedFont, font_size: f32) -> f32 {
    let mut shift = 0.0;
    for baseline in baselines.iter().rev() {
        match baseline {
            BaselineShift::Baseline => {}
            BaselineShift::Subscript => shift -= font.subscript_offset(font_size),
            BaselineShift::Superscript => shift += font.superscript_offset(font_size),
            BaselineShift::Number(n) => shift += n,
        }
    }

    shift
}

fn resolve_clusters_positions_path(
    text: &Text,
    chunk: &TextChunk,
    char_offset: usize,
    path: &TextPath,
    writing_mode: WritingMode,
    fonts_cache: &FontsCache,
    clusters: &mut [GlyphCluster],
) -> (f32, f32) {
    let mut last_x = 0.0;
    let mut last_y = 0.0;

    let mut dy = 0.0;

    // In the text path mode, chunk's x/y coordinates provide an additional offset along the path.
    // The X coordinate is used in a horizontal mode, and Y in vertical.
    let chunk_offset = match writing_mode {
        WritingMode::LeftToRight => chunk.x.unwrap_or(0.0),
        WritingMode::TopToBottom => chunk.y.unwrap_or(0.0),
    };

    let start_offset =
        chunk_offset + path.start_offset + process_anchor(chunk.anchor, clusters_length(clusters));

    let normals = collect_normals(text, chunk, clusters, &path.path, char_offset, start_offset);
    for (cluster, normal) in clusters.iter_mut().zip(normals) {
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

        let cp = char_offset + cluster.byte_idx.code_point_at(&chunk.text);
        dy += text.dy.get(cp).cloned().unwrap_or(0.0);

        let baseline_shift = chunk_span_at(chunk, cluster.byte_idx)
            .map(|span| {
                let font = match fonts_cache.get(&span.font) {
                    Some(v) => v,
                    None => return 0.0,
                };
                -resolve_baseline(span, font, writing_mode)
            })
            .unwrap_or(0.0);

        // Shift only by `dy` since we already applied `dx`
        // during offset along the path calculation.
        if !dy.approx_zero_ulps(4) || !baseline_shift.approx_zero_ulps(4) {
            let shift = kurbo::Vec2::new(0.0, (dy - baseline_shift) as f64);
            cluster.transform = cluster
                .transform
                .pre_translate(shift.x as f32, shift.y as f32);
        }

        if let Some(angle) = text.rotate.get(cp).cloned() {
            if !angle.approx_zero_ulps(4) {
                cluster.transform = cluster.transform.pre_rotate(angle);
            }
        }

        // The possible `lengthAdjust` transform should be applied after text-on-path positioning.
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

pub(crate) struct PathNormal {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) angle: f32,
}

fn collect_normals(
    text: &Text,
    chunk: &TextChunk,
    clusters: &[GlyphCluster],
    path: &tiny_skia_path::Path,
    char_offset: usize,
    offset: f32,
) -> Vec<Option<PathNormal>> {
    let mut offsets = Vec::with_capacity(clusters.len());
    let mut normals = Vec::with_capacity(clusters.len());
    {
        let mut advance = offset;
        for cluster in clusters {
            // Clusters should be rotated by the x-midpoint x baseline position.
            let half_width = cluster.width / 2.0;

            // Include relative position.
            let cp = char_offset + cluster.byte_idx.code_point_at(&chunk.text);
            advance += text.dx.get(cp).cloned().unwrap_or(0.0);

            let offset = advance + half_width;

            // Clusters outside the path have no normals.
            if offset < 0.0 {
                normals.push(None);
            }

            offsets.push(offset as f64);
            advance += cluster.advance;
        }
    }

    let mut prev_mx = path.points()[0].x;
    let mut prev_my = path.points()[0].y;
    let mut prev_x = prev_mx;
    let mut prev_y = prev_my;

    fn create_curve_from_line(px: f32, py: f32, x: f32, y: f32) -> kurbo::CubicBez {
        let line = kurbo::Line::new(
            kurbo::Point::new(px as f64, py as f64),
            kurbo::Point::new(x as f64, y as f64),
        );
        let p1 = line.eval(0.33);
        let p2 = line.eval(0.66);
        kurbo::CubicBez {
            p0: line.p0,
            p1,
            p2,
            p3: line.p1,
        }
    }

    let mut length: f64 = 0.0;
    for seg in path.segments() {
        let curve = match seg {
            tiny_skia_path::PathSegment::MoveTo(p) => {
                prev_mx = p.x;
                prev_my = p.y;
                prev_x = p.x;
                prev_y = p.y;
                continue;
            }
            tiny_skia_path::PathSegment::LineTo(p) => {
                create_curve_from_line(prev_x, prev_y, p.x, p.y)
            }
            tiny_skia_path::PathSegment::QuadTo(p1, p) => kurbo::QuadBez {
                p0: kurbo::Point::new(prev_x as f64, prev_y as f64),
                p1: kurbo::Point::new(p1.x as f64, p1.y as f64),
                p2: kurbo::Point::new(p.x as f64, p.y as f64),
            }
            .raise(),
            tiny_skia_path::PathSegment::CubicTo(p1, p2, p) => kurbo::CubicBez {
                p0: kurbo::Point::new(prev_x as f64, prev_y as f64),
                p1: kurbo::Point::new(p1.x as f64, p1.y as f64),
                p2: kurbo::Point::new(p2.x as f64, p2.y as f64),
                p3: kurbo::Point::new(p.x as f64, p.y as f64),
            },
            tiny_skia_path::PathSegment::Close => {
                create_curve_from_line(prev_x, prev_y, prev_mx, prev_my)
            }
        };

        let arclen_accuracy = {
            let base_arclen_accuracy = 0.5;
            // Accuracy depends on a current scale.
            // When we have a tiny path scaled by a large value,
            // we have to increase out accuracy accordingly.
            let (sx, sy) = text.abs_transform.get_scale();
            // 1.0 acts as a threshold to prevent division by 0 and/or low accuracy.
            base_arclen_accuracy / (sx * sy).sqrt().max(1.0)
        };

        let curve_len = curve.arclen(arclen_accuracy as f64);

        for offset in &offsets[normals.len()..] {
            if *offset >= length && *offset <= length + curve_len {
                let mut offset = curve.inv_arclen(offset - length, arclen_accuracy as f64);
                // some rounding error may occur, so we give offset a little tolerance
                debug_assert!((-1.0e-3..=1.0 + 1.0e-3).contains(&offset));
                offset = offset.clamp(0.0, 1.0);

                let pos = curve.eval(offset);
                let d = curve.deriv().eval(offset);
                let d = kurbo::Vec2::new(-d.y, d.x); // tangent
                let angle = d.atan2().to_degrees() - 90.0;

                normals.push(Some(PathNormal {
                    x: pos.x as f32,
                    y: pos.y as f32,
                    angle: angle as f32,
                }));

                if normals.len() == offsets.len() {
                    break;
                }
            }
        }

        length += curve_len;
        prev_x = curve.p3.x as f32;
        prev_y = curve.p3.y as f32;
    }

    // If path ended and we still have unresolved normals - set them to `None`.
    for _ in 0..(offsets.len() - normals.len()) {
        normals.push(None);
    }

    normals
}

/// Converts a text chunk into a list of outlined clusters.
///
/// This function will do the BIDI reordering, text shaping and glyphs outlining,
/// but not the text layouting. So all clusters are in the 0x0 position.
fn process_chunk(
    chunk: &TextChunk,
    fonts_cache: &FontsCache,
    options: &crate::Options,
    fontdb: &mut Arc<fontdb::Database>,
) -> Vec<GlyphCluster> {
    // Reorder the full chunk once. Intersect its visual runs with shaping styles,
    // retaining ligatures and kerning across spans that only change paint.
    let bidi = unicode_bidi::BidiInfo::new(&chunk.text, Some(unicode_bidi::Level::ltr()));
    let Some(paragraph) = bidi.paragraphs.first() else {
        return Vec::new();
    };
    let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
    let default_language = options.text_fallback.language.as_deref();
    let mut styles: Vec<(&TextSpan, Range<usize>)> = Vec::new();
    for span in &chunk.spans {
        match styles.last_mut() {
            Some((previous, bytes))
                if bytes.end == span.start
                    && same_shaping_style(previous, span, default_language) =>
            {
                bytes.end = span.end;
            }
            _ => styles.push((span, span.start..span.end)),
        }
    }

    let mut glyphs = Vec::new();
    for run in runs.into_iter().filter(|run| !run.is_empty()) {
        let ltr = levels[run.start].is_ltr();
        let start = styles.partition_point(|(_, bytes)| bytes.end <= run.start);
        let end = styles.partition_point(|(_, bytes)| bytes.start < run.end);
        for index in start..end {
            let (span, bytes) = &styles[if ltr {
                index
            } else {
                end - 1 - (index - start)
            }];
            let Some(font) = fonts_cache.get(&span.font) else {
                continue;
            };
            let bytes = run.start.max(bytes.start)..run.end.min(bytes.end);
            let run = ShapingRun {
                text: &chunk.text,
                bytes: run.clone(),
                ltr,
                span,
                language: shaping_language(span, default_language),
            };
            glyphs.extend(run.shape_text(bytes, font, options, fontdb));
        }
    }

    // Convert glyphs to clusters.
    let mut clusters = Vec::new();
    for (range, byte_idx) in GlyphClusters::new(&glyphs) {
        if let Some(span) = chunk_span_at(chunk, byte_idx) {
            clusters.push(form_glyph_clusters(
                &glyphs[range],
                &chunk.text,
                span.font_size.get(),
            ));
        }
    }

    clusters
}

fn shaping_language<'a>(span: &'a TextSpan, default: Option<&'a str>) -> Option<&'a str> {
    span.language
        .as_deref()
        .or(default)
        .filter(|tag| super::is_language_tag(tag))
}

fn same_shaping_style(a: &TextSpan, b: &TextSpan, default_language: Option<&str>) -> bool {
    a.font == b.font
        && a.font_size == b.font_size
        && a.small_caps == b.small_caps
        && a.apply_kerning == b.apply_kerning
        && a.font_optical_sizing == b.font_optical_sizing
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

fn apply_length_adjust(chunk: &TextChunk, clusters: &mut [GlyphCluster]) {
    let is_horizontal = matches!(chunk.text_flow, TextFlow::Linear);

    for span in &chunk.spans {
        let target_width = match span.text_length {
            Some(v) => v,
            None => continue,
        };

        let mut width = 0.0;
        let mut cluster_indexes = Vec::new();
        for i in span.start..span.end {
            if let Some(index) = clusters.iter().position(|c| c.byte_idx.value() == i) {
                cluster_indexes.push(index);
            }
        }
        // Complex scripts can have multi-codepoint clusters therefore we have to remove duplicates.
        cluster_indexes.sort();
        cluster_indexes.dedup();

        for i in &cluster_indexes {
            // Use the original cluster `width` and not `advance`.
            // This method essentially discards any `word-spacing` and `letter-spacing`.
            width += clusters[*i].width;
        }

        if cluster_indexes.is_empty() {
            continue;
        }

        if span.length_adjust == LengthAdjust::Spacing {
            let factor = if cluster_indexes.len() > 1 {
                (target_width - width) / (cluster_indexes.len() - 1) as f32
            } else {
                0.0
            };

            for i in cluster_indexes {
                clusters[i].advance = clusters[i].width + factor;
            }
        } else {
            let factor = target_width / width;
            // Prevent multiplying by zero.
            if factor < 0.001 {
                continue;
            }

            for i in cluster_indexes {
                clusters[i].transform = clusters[i].transform.pre_scale(factor, 1.0);

                // Technically just a hack to support the current text-on-path algorithm.
                if !is_horizontal {
                    clusters[i].advance *= factor;
                    clusters[i].width *= factor;
                }
            }
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

    let num_clusters = clusters.len();
    for (i, cluster) in clusters.iter_mut().enumerate() {
        // Spacing must be applied only to characters that belongs to the script
        // that supports spacing.
        // We are checking only the first code point, since it should be enough.
        // https://www.w3.org/TR/css-text-3/#cursive-tracking
        let script = cluster.codepoint.script();
        if script_supports_letter_spacing(script) {
            if let Some(span) = chunk_span_at(chunk, cluster.byte_idx) {
                // A space after the last cluster should be ignored,
                // since it affects the bbox and text alignment.
                if i != num_clusters - 1 {
                    cluster.advance += span.letter_spacing;
                }

                // If the cluster advance became negative - clear it.
                // This is an UB so we can do whatever we want, and we mimic Chrome's behavior.
                if !cluster.advance.is_valid_length() {
                    cluster.width = 0.0;
                    cluster.advance = 0.0;
                    cluster.glyphs = vec![];
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

fn form_glyph_clusters(glyphs: &[Glyph], text: &str, font_size: f32) -> GlyphCluster {
    debug_assert!(!glyphs.is_empty());

    let mut width = 0.0;
    let mut x: f32 = 0.0;

    let mut positioned_glyphs = vec![];

    for glyph in glyphs {
        let font_size = font_size * glyph.scale;

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
            font_size,
            font: glyph.font.id,
            text: glyph.text.clone(),
            id: glyph.id,
        });

        x += glyph.width as f32;

        width += glyph.width as f32 * glyph.font.scale(font_size);
    }

    let byte_idx = glyphs[0].byte_idx;
    let font = &glyphs[0].font;
    let font_size = font_size * glyphs[0].scale;
    GlyphCluster {
        byte_idx,
        codepoint: byte_idx.char_from(text),
        width,
        advance: width,
        ascent: font.ascent(font_size),
        descent: font.descent(font_size),
        has_relative_shift: false,
        transform: Transform::default(),
        path_transform: Transform::default(),
        glyphs: positioned_glyphs,
        visible: true,
    }
}

pub(crate) trait DatabaseExt {
    fn load_font(&self, id: ID) -> Option<ResolvedFont>;
    fn has_char(&self, id: ID, c: char) -> bool;
}

impl DatabaseExt for Database {
    #[inline(never)]
    fn load_font(&self, id: ID) -> Option<ResolvedFont> {
        self.with_face_data(id, |data, face_index| -> Option<ResolvedFont> {
            let font = ttf_parser::Face::parse(data, face_index).ok()?;

            let units_per_em = NonZeroU16::new(font.units_per_em())?;

            let ascent = font.ascender();
            let descent = font.descender();

            let x_height = font
                .x_height()
                .and_then(|x| u16::try_from(x).ok())
                .and_then(NonZeroU16::new);
            let x_height = match x_height {
                Some(height) => height,
                None => {
                    // If not set - fallback to height * 45%.
                    // 45% is what Firefox uses.
                    u16::try_from((f32::from(ascent - descent) * 0.45) as i32)
                        .ok()
                        .and_then(NonZeroU16::new)?
                }
            };

            let line_through = font.strikeout_metrics();
            let line_through_position = match line_through {
                Some(metrics) => metrics.position,
                None => x_height.get() as i16 / 2,
            };

            let (underline_position, underline_thickness) = match font.underline_metrics() {
                Some(metrics) => {
                    let thickness = u16::try_from(metrics.thickness)
                        .ok()
                        .and_then(NonZeroU16::new)
                        // `ttf_parser` guarantees that units_per_em is >= 16
                        .unwrap_or_else(|| NonZeroU16::new(units_per_em.get() / 12).unwrap());

                    (metrics.position, thickness)
                }
                None => (
                    -(units_per_em.get() as i16) / 9,
                    NonZeroU16::new(units_per_em.get() / 12).unwrap(),
                ),
            };

            // 0.2 and 0.4 are generic offsets used by some applications (Inkscape/librsvg).
            let mut subscript_offset = (units_per_em.get() as f32 / 0.2).round() as i16;
            let mut superscript_offset = (units_per_em.get() as f32 / 0.4).round() as i16;
            if let Some(metrics) = font.subscript_metrics() {
                subscript_offset = metrics.y_offset;
            }

            if let Some(metrics) = font.superscript_metrics() {
                superscript_offset = metrics.y_offset;
            }

            Some(ResolvedFont {
                id,
                units_per_em,
                ascent,
                descent,
                x_height,
                underline_position,
                underline_thickness,
                line_through_position,
                subscript_offset,
                superscript_offset,
            })
        })?
    }

    #[inline(never)]
    fn has_char(&self, id: ID, c: char) -> bool {
        let res = self.with_face_data(id, |font_data, face_index| -> Option<bool> {
            let font = ttf_parser::Face::parse(font_data, face_index).ok()?;
            font.glyph_index(c)?;
            Some(true)
        });

        res == Some(Some(true))
    }
}

struct ShapingRun<'a> {
    text: &'a str,
    bytes: Range<usize>,
    ltr: bool,
    span: &'a TextSpan,
    language: Option<&'a str>,
}

impl ShapingRun<'_> {
    fn shape_text(
        &self,
        bytes: Range<usize>,
        font: &Arc<ResolvedFont>,
        options: &crate::Options,
        fontdb: &mut Arc<Database>,
    ) -> Vec<Glyph> {
        let Some(primary) = self.shape(bytes, font, 1.0, fontdb) else {
            return Vec::new();
        };
        if primary.iter().all(|glyph| !glyph.is_missing()) {
            return primary;
        }

        let mut glyphs = Vec::with_capacity(primary.len());
        for (range, start) in GlyphClusters::new(&primary) {
            let cluster = &primary[range];
            if cluster.iter().any(Glyph::is_missing) {
                let end = start.value() + cluster.iter().map(|g| g.cluster_len).max().unwrap_or(0);
                if let Some(repaired) = self.repair(start.value()..end, font.id, options, fontdb) {
                    glyphs.extend(repaired);
                    continue;
                }
                let c = start.char_from(self.text);
                log::warn!(
                    "No fonts with a {}/U+{:X} character were found.",
                    c,
                    c as u32
                );
            }
            glyphs.extend_from_slice(cluster);
        }
        glyphs
    }

    fn repair(
        &self,
        bytes: Range<usize>,
        primary: ID,
        options: &crate::Options,
        fontdb: &mut Arc<Database>,
    ) -> Option<Vec<Glyph>> {
        if bytes.is_empty() {
            return None;
        }
        let cluster = self.text.get(bytes.clone())?;
        let mut used = vec![primary];
        loop {
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
                (
                    (options.font_resolver.select_fallback)(
                        cluster.chars().next()?,
                        &used,
                        fontdb,
                    )?,
                    1.0,
                )
            };
            if used.contains(&id) {
                return None;
            }
            used.push(id);
            let size = self.span.font_size.get() * scale;
            if !size.is_finite() || size <= 0.0 {
                continue;
            }
            let Some(font) = fontdb.load_font(id) else {
                continue;
            };
            let Some(mut glyphs) = self.shape(bytes.clone(), &Arc::new(font), scale, fontdb) else {
                continue;
            };
            if glyphs.is_empty() || glyphs.iter().any(Glyph::is_missing) {
                continue;
            }
            // One source cluster can expand to multiple glyphs in the replacement
            // font. Keep its original extent for positioning and span ownership.
            for glyph in &mut glyphs {
                glyph.byte_idx = ByteIndex::new(bytes.start);
                glyph.cluster_len = bytes.len();
            }
            return Some(glyphs);
        }
    }

    fn shape(
        &self,
        bytes: Range<usize>,
        font: &Arc<ResolvedFont>,
        scale: f32,
        fontdb: &Database,
    ) -> Option<Vec<Glyph>> {
        let text = self.text.get(bytes.clone())?;
        fontdb.with_face_data(font.id, |font_data, face_index| {
            let mut rb_font = rustybuzz::Face::from_slice(font_data, face_index)?;
            super::apply_variations(
                rb_font.as_mut(),
                &self.span.font.variations,
                self.span.font_size.get() * scale,
                self.span.font_optical_sizing,
            );

            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            buffer.set_pre_context(&self.text[self.bytes.start..bytes.start]);
            buffer.set_post_context(&self.text[bytes.end..self.bytes.end]);
            buffer.set_direction(if self.ltr {
                rustybuzz::Direction::LeftToRight
            } else {
                rustybuzz::Direction::RightToLeft
            });
            if let Some(language) = self.language.and_then(|tag| tag.parse().ok()) {
                buffer.set_language(language);
            }
            let mut features = Vec::new();
            if self.span.small_caps {
                features.push(rustybuzz::Feature::new(Tag::from_bytes(b"smcp"), 1, ..));
            }
            if !self.span.apply_kerning {
                features.push(rustybuzz::Feature::new(Tag::from_bytes(b"kern"), 0, ..));
            }
            let output = rustybuzz::shape(&rb_font, &features, buffer);
            let infos = output.glyph_infos();
            let glyphs = infos
                .iter()
                .zip(output.glyph_positions())
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
                        cluster_len: end.saturating_sub(start),
                        text: text.get(start..end).unwrap_or_default().to_owned(),
                        id: GlyphId(info.glyph_id as u16),
                        dx: pos.x_offset,
                        dy: pos.y_offset,
                        width: pos.x_advance,
                        font: Arc::clone(font),
                        scale,
                    }
                })
                .collect();
            Some(glyphs)
        })?
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

    // The length of the cluster in bytes.
    pub(crate) cluster_len: usize,

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
    scale: f32,
}

impl Glyph {
    fn is_missing(&self) -> bool {
        self.id.0 == 0
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedFont {
    pub(crate) id: ID,

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
    chunk
        .spans
        .iter()
        .find(|&span| span_contains(span, byte_offset))
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
    #[inline]
    pub(crate) fn scale(&self, font_size: f32) -> f32 {
        font_size / self.units_per_em.get() as f32
    }

    #[inline]
    pub(crate) fn ascent(&self, font_size: f32) -> f32 {
        self.ascent as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn descent(&self, font_size: f32) -> f32 {
        self.descent as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn height(&self, font_size: f32) -> f32 {
        self.ascent(font_size) - self.descent(font_size)
    }

    #[inline]
    pub(crate) fn x_height(&self, font_size: f32) -> f32 {
        self.x_height.get() as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn underline_position(&self, font_size: f32) -> f32 {
        self.underline_position as f32 * self.scale(font_size)
    }

    #[inline]
    fn underline_thickness(&self, font_size: f32) -> f32 {
        self.underline_thickness.get() as f32 * self.scale(font_size)
    }

    #[inline]
    pub(crate) fn line_through_position(&self, font_size: f32) -> f32 {
        self.line_through_position as f32 * self.scale(font_size)
    }

    #[inline]
    fn subscript_offset(&self, font_size: f32) -> f32 {
        self.subscript_offset as f32 * self.scale(font_size)
    }

    #[inline]
    fn superscript_offset(&self, font_size: f32) -> f32 {
        self.superscript_offset as f32 * self.scale(font_size)
    }

    fn dominant_baseline_shift(&self, baseline: DominantBaseline, font_size: f32) -> f32 {
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

        self.alignment_baseline_shift(alignment, font_size)
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
    fn alignment_baseline_shift(&self, alignment: AlignmentBaseline, font_size: f32) -> f32 {
        match alignment {
            AlignmentBaseline::Auto => 0.0,
            AlignmentBaseline::Baseline => 0.0,
            AlignmentBaseline::BeforeEdge | AlignmentBaseline::TextBeforeEdge => {
                self.ascent(font_size)
            }
            AlignmentBaseline::Middle => self.x_height(font_size) * 0.5,
            AlignmentBaseline::Central => self.ascent(font_size) - self.height(font_size) * 0.5,
            AlignmentBaseline::AfterEdge | AlignmentBaseline::TextAfterEdge => {
                self.descent(font_size)
            }
            AlignmentBaseline::Ideographic => self.descent(font_size),
            AlignmentBaseline::Alphabetic => 0.0,
            AlignmentBaseline::Hanging => self.ascent(font_size) * 0.8,
            AlignmentBaseline::Mathematical => self.ascent(font_size) * 0.5,
        }
    }
}

pub(crate) type FontsCache = HashMap<Font, Arc<ResolvedFont>>;

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

    /// Converts byte position into a code point position.
    pub(crate) fn code_point_at(&self, text: &str) -> usize {
        text.char_indices()
            .take_while(|(i, _)| *i != self.0)
            .count()
    }

    /// Converts byte position into a character.
    pub(crate) fn char_from(&self, text: &str) -> char {
        text[self.0..].chars().next().unwrap()
    }
}
