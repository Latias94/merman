use merman_render_graph::c4::{SHAFT_CLEARANCE, measure_c4_relation_label};

use super::super::C4Measurement;
use crate::Result;
use crate::graph_layout::canonical::CanonicalPaintedEdge;
use crate::graph_layout::ir::Size;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{C4RelLayout, LayoutLabel, LayoutPoint};
use kurbo::{BezPath, Point, Rect, Shape, Vec2};

#[derive(Debug)]
pub(crate) struct PlacedRelationLabel {
    pub(crate) origin: LayoutPoint,
    pub(crate) shaft_gap: Option<Rect>,
}

/// Bounds relative to the common SVG text origin. Authored offsets are limited to the
/// local caption allowance so a relationship can still use the compact grid's gutters.
pub(super) struct RelationLabel {
    ink: Rect,
    offset: Vec2,
}

impl RelationLabel {
    pub(super) fn ink_at(&self, origin: &LayoutPoint) -> Rect {
        self.ink + Vec2::new(origin.x, origin.y)
    }

    pub(super) fn prepare(rel: &mut C4RelLayout, ctx: &C4Measurement<'_>) -> Result<Option<Self>> {
        Ok(measure_c4_relation_label(rel, ctx)?.map(|(ink, offset)| Self { ink, offset }))
    }

    pub(super) fn size(&self) -> Size {
        let [side_room, vertical_room] = self.room();
        Size {
            width: self.ink.width() + 2.0 * side_room,
            height: self.ink.height() + 2.0 * vertical_room,
        }
    }

    fn room(&self) -> [f64; 2] {
        [
            self.offset.x.abs().min(SHAFT_CLEARANCE),
            2.0 * SHAFT_CLEARANCE,
        ]
    }

    pub(super) fn place(
        &self,
        label: &LayoutLabel,
        route: &CanonicalPaintedEdge,
        stroke_outset: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<PlacedRelationLabel> {
        let bounds = &route.path.bounds;
        let shaft_bounds = Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y)
            .inflate(stroke_outset, stroke_outset);
        let marker = |contact: Option<&LayoutPoint>, lead: &LayoutPoint, width: f64| {
            contact.filter(|_| width > 0.0).map(|contact| {
                let half = width / 2.0;
                let (pad_x, pad_y) = if (contact.x - lead.x).abs() >= (contact.y - lead.y).abs() {
                    (0.0, half)
                } else {
                    (half, 0.0)
                };
                Rect::from_points((contact.x, contact.y), (lead.x, lead.y)).inflate(pad_x, pad_y)
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
        let anchor = Point::new(label.x, label.y) - self.ink.center().to_vec2();
        let [side_room, vertical_room] = self.room();
        let preferred = anchor
            + Vec2::new(
                self.offset.x.clamp(-side_room, side_room),
                self.offset.y.clamp(-vertical_room, vertical_room),
            );
        let positioned = |origin: Point, shaft_gap| PlacedRelationLabel {
            origin: LayoutPoint {
                x: origin.x,
                y: origin.y,
            },
            shaft_gap,
        };
        let mut on_route = None;
        let mut path: Option<BezPath> = None;
        for origin in [
            preferred,
            anchor,
            anchor - Vec2::new(0.0, vertical_room),
            anchor + Vec2::new(0.0, vertical_room),
        ] {
            let ink = self.ink + origin.to_vec2();
            let overlaps = |obstacle: Rect| obstacle.intersect(ink).area() > 0.0;
            if markers.iter().flatten().copied().any(overlaps) {
                continue;
            }
            if !overlaps(shaft_bounds) {
                return Ok(positioned(origin, None));
            }
            let path = match &mut path {
                Some(path) => path,
                cached => {
                    work.charge_adapter(route.path.d.len())?;
                    cached.insert(BezPath::from_svg(&route.path.d).map_err(|error| {
                        crate::Error::InvalidModel {
                            message: format!("c4: unable to check the painted shaft: {error}"),
                        }
                    })?)
                }
            };
            work.charge_adapter(path.elements().len())?;
            if !path.segments().any(|segment| {
                overlaps(segment.bounding_box().inflate(stroke_outset, stroke_outset))
            }) {
                return Ok(positioned(origin, None));
            }
            on_route.get_or_insert_with(|| positioned(origin, Some(ink)));
        }
        Ok(on_route.unwrap_or_else(|| positioned(anchor, None)))
    }
}

/// Collects foreign placed relation-label ink boxes that intersect `route`'s painted shaft so
/// the SVG renderer can knock out crossing wires cleanly behind captions without opaque pills.
pub(super) fn collect_foreign_shaft_cutouts(
    route_index: usize,
    route: &CanonicalPaintedEdge,
    stroke_outset: f64,
    label_inks: &[Option<Rect>],
    out: &mut Vec<Rect>,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let bounds = &route.path.bounds;
    let shaft_bounds = Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y)
        .inflate(stroke_outset, stroke_outset);
    let mut path: Option<BezPath> = None;
    for (other_index, ink) in label_inks.iter().enumerate() {
        if other_index == route_index {
            continue;
        }
        let Some(&ink) = ink.as_ref() else {
            continue;
        };
        if shaft_bounds.intersect(ink).area() <= 0.0 {
            continue;
        }
        let parsed = match &mut path {
            Some(parsed) => parsed,
            cached => {
                work.charge_adapter(route.path.d.len())?;
                cached.insert(BezPath::from_svg(&route.path.d).map_err(|error| {
                    crate::Error::InvalidModel {
                        message: format!("c4: unable to check the painted shaft: {error}"),
                    }
                })?)
            }
        };
        work.charge_adapter(parsed.elements().len())?;
        if parsed.segments().any(|segment| {
            segment
                .bounding_box()
                .inflate(stroke_outset, stroke_outset)
                .intersect(ink)
                .area()
                > 0.0
        }) {
            out.push(ink);
        }
    }
    Ok(())
}
