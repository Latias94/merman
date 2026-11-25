use super::super::layout::C4Measurement;
use super::super::typography::C4PreparedFont;
use super::super::{C4_TECHNOLOGY_GAP, c4_relation_offset, c4_text_row_offset};
use crate::Result;
use crate::graph_layout::canonical::CanonicalPaintedEdge;
use crate::graph_layout::ir::Size;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{C4BoundaryLayout, C4RelLayout, LayoutLabel, LayoutPoint};
use crate::text::{split_mermaid_common_br_lines, svg_title_bbox_vertical_extents_px};
use kurbo::{BezPath, Point, Rect, Shape, Vec2};

const SHAFT_CLEARANCE: f64 = 2.0;

/// Bounds relative to the common SVG text origin. Mirroring the authored offset in the
/// reservation lets both the router and the label placer test the shifted ink.
pub(super) struct RelationLabel {
    ink: Rect,
    offset: Vec2,
}

impl RelationLabel {
    pub(super) fn measure(rel: &C4RelLayout, ctx: &C4Measurement<'_>) -> Result<Option<Self>> {
        let font = ctx.typography.message_font();
        let mut ink = text_ink(&rel.label.text, 0.0, font, false, ctx)?;
        if let Some(techn) = &rel.techn
            && !techn.text.trim().is_empty()
        {
            let y = font.css_font_size_px.expect("C4 relationship font size") + C4_TECHNOLOGY_GAP;
            let technology = text_ink(&format!("[{}]", techn.text), y, font, true, ctx)?;
            ink = union(ink, technology);
        }
        let offset = |value| if f64::is_finite(value) { value } else { 0.0 };
        Ok(ink.map(|ink| Self {
            ink,
            offset: Vec2::new(
                offset(c4_relation_offset(rel.offset_x.as_ref())),
                offset(c4_relation_offset(rel.offset_y.as_ref())),
            ),
        }))
    }

    pub(super) fn size(&self) -> Size {
        Size {
            width: self.ink.width() + 2.0 * self.offset.x.abs(),
            height: self.ink.height() + 2.0 * (self.offset.y.abs() + self.side_shift()),
        }
    }

    fn side_shift(&self) -> f64 {
        self.ink.height() / 2.0 + SHAFT_CLEARANCE
    }

    pub(super) fn origin(
        &self,
        label: &LayoutLabel,
        route: &CanonicalPaintedEdge,
        stroke_outset: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<LayoutPoint> {
        work.charge_adapter(route.path.d.len())?;
        let path =
            BezPath::from_svg(&route.path.d).map_err(|error| crate::Error::InvalidModel {
                message: format!("c4: unable to check the painted shaft: {error}"),
            })?;
        let marker = |contact: Option<&LayoutPoint>, lead: &LayoutPoint, width: f64| {
            contact.filter(|_| width > 0.0).map(|contact| {
                Rect::from_points((contact.x, contact.y), (lead.x, lead.y))
                    .inflate(width / 2.0, width / 2.0)
            })
        };
        let markers = [
            marker(
                route.data_points.first(),
                &route.marker_points[1],
                route.markers.0.width,
            ),
            marker(
                route.data_points.last(),
                &route.marker_points[2],
                route.markers.1.width,
            ),
        ];
        let preferred = Point::new(label.x, label.y) + self.offset - self.ink.center().to_vec2();
        let shift = self.side_shift();
        let origin = [
            preferred,
            preferred - Vec2::new(0.0, shift),
            preferred + Vec2::new(0.0, shift),
        ]
        .into_iter()
        .find(|origin| {
            let ink = self.ink + origin.to_vec2();
            let overlaps = |obstacle: Rect| obstacle.intersect(ink).area() > 0.0;
            !markers.iter().flatten().copied().any(overlaps)
                && path.segments().all(|segment| {
                    !overlaps(segment.bounding_box().inflate(stroke_outset, stroke_outset))
                })
        })
        .unwrap_or(preferred);
        Ok(LayoutPoint {
            x: origin.x,
            y: origin.y,
        })
    }
}

pub(super) fn title_ink(
    boundary: &C4BoundaryLayout,
    ctx: &C4Measurement<'_>,
) -> Result<Option<Rect>> {
    let fonts = ctx.typography.boundary_fonts();
    let mut ink = text_ink(
        &boundary.label.text,
        boundary.label.y,
        &fonts.name,
        false,
        ctx,
    )?;
    for (text, font) in [(&boundary.ty, &fonts.ty), (&boundary.descr, &fonts.descr)] {
        if let Some(block) = text {
            ink = union(ink, text_ink(&block.text, block.y, font, false, ctx)?);
        }
    }
    Ok(ink)
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

fn text_ink(
    text: &str,
    y: f64,
    font: &C4PreparedFont,
    italic: bool,
    ctx: &C4Measurement<'_>,
) -> Result<Option<Rect>> {
    let mut style = font.measurement_style.clone();
    if style.font_size <= 0.0 {
        return Ok(None);
    }
    if italic {
        style.font_style = Some("italic".into());
    }
    let rows = split_mermaid_common_br_lines(text);
    let font_size = font.css_font_size_px.expect("legacy C4 text row size");
    let mut ink = None;
    for (row, content) in rows.iter().enumerate() {
        ctx.work.charge(content.len().saturating_add(1))?;
        if content.trim().is_empty() {
            continue;
        }
        let half_width = ctx
            .measurer
            .measure_svg_tspan_text_bbox_width_px(content, &style)
            / 2.0;
        let height = ctx
            .measurer
            .measure_svg_tspan_text_bbox_height_px(content, &style);
        let middle = ctx
            .measurer
            .measure_svg_raw_text_middle_bbox_y_offset_px(content, &style);
        let (top, bottom) = middle.map_or_else(
            || {
                // When the profile has no middle-baseline metric, reserve both the
                // centered row and the usual ascent/descent around its baseline.
                let (ascent, descent) = svg_title_bbox_vertical_extents_px(&style);
                (-ascent.max(height / 2.0), descent.max(height / 2.0))
            },
            |top| (top, top + height),
        );
        let at = y + c4_text_row_offset(row, rows.len(), font_size);
        let row = Rect::new(-half_width, at + top, half_width, at + bottom);
        if row.width() > 0.0 && row.height() > 0.0 {
            ink = union(ink, Some(row));
        }
    }
    Ok(ink)
}
