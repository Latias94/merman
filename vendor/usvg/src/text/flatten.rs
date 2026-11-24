// Copyright 2022 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::mem;
use std::sync::Arc;

use fontdb::{Database, ID};
use rustybuzz::ttf_parser;
use rustybuzz::ttf_parser::{GlyphId, RasterImageFormat, RgbaColor};
use tiny_skia_path::{NonZeroRect, Size, Transform};
use xmlwriter::XmlWriter;

use crate::text::colr::GlyphPainter;
use crate::*;

fn resolve_rendering_mode(mode: TextRendering) -> ShapeRendering {
    match mode {
        TextRendering::OptimizeSpeed => ShapeRendering::CrispEdges,
        TextRendering::OptimizeLegibility => ShapeRendering::GeometricPrecision,
        TextRendering::GeometricPrecision => ShapeRendering::GeometricPrecision,
    }
}

fn push_outline_paths(
    span: &layout::Span,
    builder: &mut tiny_skia_path::PathBuilder,
    new_children: &mut Vec<Node>,
    rendering_mode: ShapeRendering,
) {
    let builder = mem::replace(builder, tiny_skia_path::PathBuilder::new());

    if let Some(path) = builder.finish().and_then(|p| {
        Path::new(
            String::new(),
            span.visible,
            span.fill.clone(),
            span.stroke.clone(),
            span.paint_order,
            rendering_mode,
            Arc::new(p),
            Transform::default(),
        )
    }) {
        new_children.push(Node::Path(Box::new(path)));
    }
}

pub(crate) fn flatten(
    id: &str,
    mode: TextRendering,
    spans: &[layout::Span],
    cache: &mut Cache,
) -> Option<(Group, NonZeroRect)> {
    let mut new_children = vec![];

    let rendering_mode = resolve_rendering_mode(mode);

    for span in spans {
        if let Some(path) = span.overline.as_ref() {
            let mut path = path.clone();
            path.rendering_mode = rendering_mode;
            new_children.push(Node::Path(Box::new(path)));
        }

        if let Some(path) = span.underline.as_ref() {
            let mut path = path.clone();
            path.rendering_mode = rendering_mode;
            new_children.push(Node::Path(Box::new(path)));
        }

        // Instead of always processing each glyph separately, we always collect
        // as many outline glyphs as possible by pushing them into the span_builder
        // and only if we encounter a different glyph, or we reach the very end of the
        // span to we push the actual outline paths into new_children. This way, we don't need
        // to create a new path for every glyph if we have many consecutive glyphs
        // with just outlines (which is the most common case).
        let mut span_builder = tiny_skia_path::PathBuilder::new();

        let variations = &span.variations;
        for glyph in &span.positioned_glyphs {
            let colr = if variations.is_empty() {
                cache.fontdb_colr(glyph.font, glyph.id)
            } else if cache.font_has_colr(glyph.font) {
                cache
                    .fontdb
                    .colr_with_variations(glyph.font, glyph.id, variations)
            } else {
                None
            };
            // A (best-effort conversion of a) COLR glyph.
            if let Some(tree) = colr {
                push_outline_paths(span, &mut span_builder, &mut new_children, rendering_mode);

                let mut group = Group {
                    transform: glyph.colr_transform(),
                    ..Group::empty()
                };
                // TODO: Probably need to update abs_transform of children?
                group.children.push(Node::Group(Box::new(tree.root)));
                group.calculate_bounding_boxes();

                new_children.push(Node::Group(Box::new(group)));
            }
            // An SVG glyph. Will return the usvg node containing the glyph descriptions.
            else if let Some(node) = cache.fontdb_svg(glyph.font, glyph.id) {
                push_outline_paths(span, &mut span_builder, &mut new_children, rendering_mode);

                let mut group = Group {
                    transform: glyph.svg_transform(),
                    ..Group::empty()
                };
                // TODO: Probably need to update abs_transform of children?
                group.children.push(node);
                group.calculate_bounding_boxes();

                new_children.push(Node::Group(Box::new(group)));
            }
            // A bitmap glyph.
            else if let Some(img) = cache.fontdb_raster(glyph.font, glyph.id) {
                push_outline_paths(span, &mut span_builder, &mut new_children, rendering_mode);

                let transform = if img.is_sbix {
                    let bbox = if variations.is_empty() {
                        img.glyph_bbox
                    } else {
                        cache.fontdb.glyph_bounds(glyph.font, glyph.id, variations)
                    };
                    glyph.sbix_transform(
                        img.x as f32,
                        img.y as f32,
                        bbox.map(|bbox| bbox.x_min).unwrap_or(0) as f32,
                        bbox.map(|bbox| bbox.y_min).unwrap_or(0) as f32,
                        img.pixels_per_em as f32,
                        img.image.size.height(),
                    )
                } else {
                    glyph.cbdt_transform(
                        img.x as f32,
                        img.y as f32,
                        img.pixels_per_em as f32,
                        img.image.size.height(),
                    )
                };

                let mut group = Group {
                    transform,
                    ..Group::empty()
                };
                group.children.push(Node::Image(Box::new(img.image)));
                group.calculate_bounding_boxes();

                new_children.push(Node::Group(Box::new(group)));
            } else {
                // The outline cache stores the default instance.
                let outline = if variations.is_empty() {
                    cache.fontdb_outline(glyph.font, glyph.id)
                } else {
                    cache
                        .fontdb
                        .outline_with_variations(glyph.font, glyph.id, variations)
                };

                if let Some(outline) = outline.and_then(|p| p.transform(glyph.outline_transform()))
                {
                    span_builder.push_path(&outline);
                }
            }
        }

        push_outline_paths(span, &mut span_builder, &mut new_children, rendering_mode);

        if let Some(path) = span.line_through.as_ref() {
            let mut path = path.clone();
            path.rendering_mode = rendering_mode;
            new_children.push(Node::Path(Box::new(path)));
        }
    }

    let mut group = Group {
        id: id.to_owned(),
        ..Group::empty()
    };

    for child in new_children {
        group.children.push(child);
    }

    group.calculate_bounding_boxes();
    let stroke_bbox = group.stroke_bounding_box().to_non_zero_rect()?;
    Some((group, stroke_bbox))
}

struct PathBuilder {
    builder: tiny_skia_path::PathBuilder,
}

impl ttf_parser::OutlineBuilder for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

pub(crate) trait DatabaseExt {
    fn outline(&self, id: ID, glyph_id: GlyphId) -> Option<tiny_skia_path::Path>;
    fn outline_with_variations(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[FontVariation],
    ) -> Option<tiny_skia_path::Path>;
    fn glyph_bounds(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[FontVariation],
    ) -> Option<ttf_parser::Rect>;
    fn raster(&self, id: ID, glyph_id: GlyphId) -> Option<BitmapImage>;
    fn svg(&self, id: ID, glyph_id: GlyphId) -> Option<Node>;
    fn has_colr(&self, id: ID) -> bool;
    fn colr(&self, id: ID, glyph_id: GlyphId) -> Option<Tree>;
    fn colr_with_variations(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[FontVariation],
    ) -> Option<Tree>;
}

#[derive(Clone)]
pub(crate) struct BitmapImage {
    image: Image,
    x: i16,
    y: i16,
    pixels_per_em: u16,
    glyph_bbox: Option<ttf_parser::Rect>,
    is_sbix: bool,
}

impl DatabaseExt for Database {
    #[inline(never)]
    fn outline(&self, id: ID, glyph_id: GlyphId) -> Option<tiny_skia_path::Path> {
        self.outline_with_variations(id, glyph_id, &[])
    }

    #[inline(never)]
    fn outline_with_variations(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[FontVariation],
    ) -> Option<tiny_skia_path::Path> {
        self.with_face_data(id, |data, face_index| -> Option<tiny_skia_path::Path> {
            let mut font = ttf_parser::Face::parse(data, face_index).ok()?;

            super::apply_variations(&mut font, variations);

            let mut builder = PathBuilder {
                builder: tiny_skia_path::PathBuilder::new(),
            };

            font.outline_glyph(glyph_id, &mut builder)?;
            builder.builder.finish()
        })?
    }

    fn glyph_bounds(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[FontVariation],
    ) -> Option<ttf_parser::Rect> {
        self.with_face_data(id, |data, face_index| {
            let mut font = ttf_parser::Face::parse(data, face_index).ok()?;
            super::apply_variations(&mut font, variations);
            font.glyph_bounding_box(glyph_id)
        })?
    }

    fn raster(&self, id: ID, glyph_id: GlyphId) -> Option<BitmapImage> {
        self.with_face_data(id, |data, face_index| -> Option<BitmapImage> {
            let font = ttf_parser::Face::parse(data, face_index).ok()?;
            let image = font.glyph_raster_image(glyph_id, u16::MAX)?;

            if image.format == RasterImageFormat::PNG {
                let bitmap_image = BitmapImage {
                    image: Image {
                        id: String::new(),
                        visible: true,
                        size: Size::from_wh(image.width as f32, image.height as f32)?,
                        rendering_mode: ImageRendering::OptimizeQuality,
                        kind: ImageKind::PNG(Arc::new(image.data.into())),
                        abs_transform: Transform::default(),
                        abs_bounding_box: NonZeroRect::from_xywh(
                            0.0,
                            0.0,
                            image.width as f32,
                            image.height as f32,
                        )?,
                    },
                    x: image.x,
                    y: image.y,
                    pixels_per_em: image.pixels_per_em,
                    glyph_bbox: font.glyph_bounding_box(glyph_id),
                    // ttf-parser always checks sbix first, so if this table exists, it was used.
                    is_sbix: font.tables().sbix.is_some(),
                };

                return Some(bitmap_image);
            }

            None
        })?
    }

    fn svg(&self, id: ID, glyph_id: GlyphId) -> Option<Node> {
        // TODO: Technically not 100% accurate because the SVG format in a OTF font
        // is actually a subset/superset of a normal SVG, but it seems to work fine
        // for Twitter Color Emoji, so might as well use what we already have.

        // TODO: Glyph records can contain the data for multiple glyphs. We should
        // add a cache so we don't need to reparse the data every time.
        self.with_face_data(id, |data, face_index| -> Option<Node> {
            let font = ttf_parser::Face::parse(data, face_index).ok()?;
            let image = font.glyph_svg_image(glyph_id)?;
            let tree = Tree::from_data(image.data, &Options::default()).ok()?;

            // Twitter Color Emoji seems to always have one SVG record per glyph,
            // while Noto Color Emoji sometimes contains multiple ones. It's kind of hacky,
            // but the best we have for now.
            let node = if image.start_glyph_id == image.end_glyph_id {
                Node::Group(Box::new(tree.root))
            } else {
                tree.node_by_id(&format!("glyph{}", glyph_id.0))
                    .log_none(|| {
                        log::warn!("Failed to find SVG glyph node for glyph {}", glyph_id.0);
                    })
                    .cloned()?
            };

            Some(node)
        })?
    }

    fn has_colr(&self, id: ID) -> bool {
        self.with_face_data(id, |data, face_index| {
            ttf_parser::Face::parse(data, face_index).is_ok_and(|face| face.tables().colr.is_some())
        })
        .unwrap_or(false)
    }

    fn colr(&self, id: ID, glyph_id: GlyphId) -> Option<Tree> {
        self.colr_with_variations(id, glyph_id, &[])
    }

    fn colr_with_variations(
        &self,
        id: ID,
        glyph_id: GlyphId,
        variations: &[FontVariation],
    ) -> Option<Tree> {
        self.with_face_data(id, |data, face_index| -> Option<Tree> {
            let mut face = ttf_parser::Face::parse(data, face_index).ok()?;
            super::apply_variations(&mut face, variations);

            let mut svg = XmlWriter::new(xmlwriter::Options::default());

            svg.start_element("svg");
            svg.write_attribute("xmlns", "http://www.w3.org/2000/svg");
            svg.write_attribute("xmlns:xlink", "http://www.w3.org/1999/xlink");

            let mut path_buf = String::with_capacity(256);
            let gradient_index = 1;
            let clip_path_index = 1;

            svg.start_element("g");

            let mut glyph_painter = GlyphPainter {
                face: &face,
                svg: &mut svg,
                path_buf: &mut path_buf,
                gradient_index,
                clip_path_index,
                palette_index: 0,
                transform: ttf_parser::Transform::default(),
                outline_transform: ttf_parser::Transform::default(),
                transforms_stack: vec![ttf_parser::Transform::default()],
            };

            face.paint_color_glyph(
                glyph_id,
                0,
                RgbaColor::new(0, 0, 0, 255),
                &mut glyph_painter,
            )?;
            svg.end_element();

            Tree::from_data(svg.end_document().as_bytes(), &Options::default()).ok()
        })?
    }
}
