use crate::Result;
use crate::graph_layout::ir::{Marker, MeasuredGraph, RoutePoint, Side, SolvedGraph};
use crate::graph_layout::route_paint::marker_bounds;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use kurbo::{Line, ParamCurveNearest, Point, Rect, Shape, Vec2};
use merman_layout_shoal::geom::{Bounds, BoundsIndex, Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

const MAX_TERMINAL_DISTANCE: f64 = 56.0;

#[derive(Clone, Copy)]
enum Obstacle {
    Box(Rect),
    Terminal((usize, bool), Rect),
    Shaft(usize, Line),
}

impl Obstacle {
    fn bounds(self) -> Rect {
        match self {
            Self::Box(rect) | Self::Terminal(_, rect) => rect,
            Self::Shaft(_, line) => line.bounding_box(),
        }
    }
}

pub(super) fn seat(
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    shaft_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(graph.edges.len())?;
    if !graph
        .edges
        .iter()
        .any(|edge| edge.start_label.is_some() || edge.end_label.is_some())
    {
        return Ok(());
    }
    work.charge_adapter(work.checked_add(solved.nodes.len(), solved.groups.len())?)?;
    let mut obstacles = Vec::new();
    obstacles.extend(solved.nodes.iter().map(|node| {
        Obstacle::Box(Rect::new(
            node.x,
            node.y,
            node.x + node.width,
            node.y + node.height,
        ))
    }));
    obstacles.extend(solved.groups.iter().filter_map(|group| {
        group
            .title
            .as_ref()
            .map(|label| Obstacle::Box(label_rect(label)))
    }));
    for (index, (edge, spec)) in solved.edges.iter().zip(&graph.edges).enumerate() {
        work.charge_adapter(work.checked_add(edge.points.len(), 5)?)?;
        if let Some(label) = &edge.label {
            obstacles.push(Obstacle::Box(label_rect(label)));
        }
        for (start, label, port, side, marker) in [
            (
                true,
                &edge.start_label,
                edge.points.first(),
                edge.ports.0,
                spec.start_marker,
            ),
            (
                false,
                &edge.end_label,
                edge.points.last(),
                edge.ports.1,
                spec.end_marker,
            ),
        ] {
            if let Some(label) = label {
                obstacles.push(Obstacle::Terminal((index, start), label_rect(label)));
            }
            if let Some(rect) = port.and_then(|&port| marker_bounds(port, side, marker)) {
                obstacles.push(Obstacle::Box(rect));
            }
        }
        obstacles.extend(
            edge.points
                .windows(2)
                .map(|pair| Obstacle::Shaft(index, Line::new(point(pair[0]), point(pair[1])))),
        );
    }
    let fixed = Bvh::build_controlled(obstacles, |obstacle| bounds(obstacle.bounds()), work)
        .map_err(|error| work.map_shoal_error(error))?;
    let drawing = Rect::new(0.0, 0.0, solved.bounds.width, solved.bounds.height);
    work.charge_adapter(solved.edges.len())?;
    let mut seating = Seating {
        fixed,
        occupied: BoundsIndex::default(),
        handled: vec![[false; 2]; solved.edges.len()],
        drawing,
        shaft_outset,
    };
    for (index, (edge, spec)) in solved.edges.iter_mut().zip(&graph.edges).enumerate() {
        work.charge_adapter(0)?;
        let [first, .., last] = edge.points.as_slice() else {
            continue;
        };
        let span = point(*first).distance(point(*last));
        let mut paired = None;
        for (start, label, port, side, marker) in [
            (
                true,
                &mut edge.start_label,
                *first,
                edge.ports.0,
                spec.start_marker,
            ),
            (
                false,
                &mut edge.end_label,
                *last,
                edge.ports.1,
                spec.end_marker,
            ),
        ] {
            let Some(current) = label.take() else {
                continue;
            };
            let port = point(port);
            let outward = outward(side);
            let lead = if start {
                straight_lead(edge.points.iter().skip(1), port, outward, work)?
            } else {
                straight_lead(edge.points.iter().rev().skip(1), port, outward, work)?
            };
            *label = seating.place(
                current,
                Terminal {
                    owner: (index, start),
                    port,
                    outward,
                    lead,
                    span,
                    marker,
                },
                paired,
                work,
            )?;
            if start {
                paired = label.as_ref().map(label_rect);
            }
        }
    }
    Ok(())
}

struct Seating {
    fixed: Bvh<Obstacle>,
    occupied: BoundsIndex,
    // A handled label's original box is obsolete, even if the label had no seat.
    handled: Vec<[bool; 2]>,
    drawing: Rect,
    shaft_outset: f64,
}

struct Terminal {
    owner: (usize, bool),
    port: Point,
    outward: Vec2,
    lead: f64,
    span: f64,
    marker: Marker,
}

struct Clearance {
    overlap: f64,
    foreign_shaft_overlap: f64,
    nearest_shaft: f64,
}

impl Seating {
    fn place(
        &mut self,
        mut label: LayoutLabel,
        terminal: Terminal,
        paired: Option<Rect>,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<LayoutLabel>> {
        let seat = self.choose(&label, &terminal, paired, work)?;
        let (edge, start) = terminal.owner;
        self.handled[edge][usize::from(start)] = true;
        let Some(rect) = seat else {
            // No candidate in the local window cleared the hard obstacles.
            return Ok(None);
        };
        self.occupied
            .insert(bounds(rect), work)
            .map_err(|error| work.map_shoal_error(error))?;
        let center = rect.center();
        (label.x, label.y) = (center.x, center.y);
        Ok(Some(label))
    }

    fn choose(
        &self,
        label: &LayoutLabel,
        terminal: &Terminal,
        paired: Option<Rect>,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Rect>> {
        let Terminal {
            owner,
            port,
            outward,
            lead,
            span,
            marker,
        } = *terminal;
        let across = Vec2::new(-outward.y, outward.x);
        let half = Vec2::new(label.width / 2.0, label.height / 2.0);
        let along_half = half.dot(Vec2::new(outward.x.abs(), outward.y.abs()));
        let across_half = half.dot(Vec2::new(across.x.abs(), across.y.abs()));
        let minimum = along_half + 2.0;
        let preferred_along = 16.0_f64.max(minimum);
        let existing = Point::new(label.x, label.y);
        let preferred = if (existing - port).dot(across) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let adjacent = across_half + self.shaft_outset + 1.0;
        let between_shafts = across_half + self.shaft_outset + 0.25;
        let past_marker = marker.trim + marker.depth + along_half + 1.0;
        let offsets = [
            adjacent,
            between_shafts,
            adjacent.max(across_half + marker.width / 2.0 + 1.0),
        ];
        let mut radius = (0.35 * span)
            .min(MAX_TERMINAL_DISTANCE)
            .max(minimum.hypot(between_shafts));
        // Very wide authored text can need more room just to sit beside the lead.
        let preferred_radius = preferred_along.hypot(adjacent);
        if preferred_radius > MAX_TERMINAL_DISTANCE {
            radius = radius.max(preferred_radius);
        }
        let assess = |center: Point, offset: f64, work: &mut OperationLayoutWorkControl| {
            work.charge_adapter(1)?;
            let rect = Rect::from_center_size(center, (label.width, label.height));
            if self.drawing.intersect(rect) != rect
                || paired.is_some_and(|paired| rect.intersect(paired).area() > 0.0)
            {
                return Ok(None);
            }
            self.clearance(rect, owner, offset, work)
                .map(|clearance| clearance.map(|clearance| (rect, clearance)))
        };

        let lead_limit = lead - along_half;
        if lead_limit >= minimum {
            let near = preferred_along.min(lead_limit);
            let far = (near + 8.0).min(lead_limit);
            // Preserve the close, shaft-owned seat whenever there is room beside the lead.
            for offset in offsets {
                for distance in [near, past_marker.clamp(near, far), far] {
                    if distance.hypot(offset) > radius {
                        continue;
                    }
                    let mut best = None::<(Rect, f64)>;
                    for sign in [preferred, -preferred] {
                        let center = port + outward * distance + across * (sign * offset);
                        if let Some((rect, clearance)) = assess(center, offset, work)?
                            && clearance.overlap == 0.0
                            && clearance.nearest_shaft > offset
                            && best.is_none_or(|(_, previous)| clearance.nearest_shaft > previous)
                        {
                            best = Some((rect, clearance.nearest_shaft));
                        }
                    }
                    if let Some((rect, _)) = best {
                        return Ok(Some(rect));
                    }
                }
            }
        }

        // Nearby labels and shafts can compete for the same channel. Keep boxes and markers
        // hard, and prefer covering our own shaft (which SVG clips under the label) to a peer's.
        let mut best = None::<(Rect, [f64; 4])>;
        let mut consider = |center: Point, offset: f64, work: &mut OperationLayoutWorkControl| {
            if let Some((rect, clearance)) = assess(center, offset, work)? {
                let rank = [
                    clearance.foreign_shaft_overlap,
                    clearance.overlap,
                    (offset - clearance.nearest_shaft).max(0.0),
                    center.distance(port),
                ];
                if best.is_none_or(|(_, previous)| rank < previous) {
                    best = Some((rect, rank));
                }
            }
            Ok::<_, crate::Error>(())
        };
        if existing.distance(port) <= radius {
            consider(existing, (existing - port).dot(across).abs(), work)?;
        }
        for offset in offsets
            .into_iter()
            .chain([adjacent / 2.0, 0.0, adjacent + across_half])
        {
            let far = (radius * radius - offset * offset).max(0.0).sqrt();
            if far < minimum {
                continue;
            }
            let near = preferred_along.min(far);
            let distances = [
                near,
                past_marker.clamp(minimum, far),
                near.midpoint(far),
                far,
                minimum,
            ];
            for (index, &distance) in distances.iter().enumerate() {
                if distances[..index].contains(&distance) {
                    continue;
                }
                for sign in [preferred, -preferred] {
                    let center = port + outward * distance + across * (sign * offset);
                    consider(center, offset, work)?;
                    if offset == 0.0 {
                        break;
                    }
                }
            }
        }
        Ok(best.map(|(rect, _)| rect))
    }

    fn clearance(
        &self,
        rect: Rect,
        owner: (usize, bool),
        own_distance: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Clearance>> {
        let terminal_ink = rect.inflate(1.0, 1.0);
        let center = rect.center();
        let shaft_ink = rect.inflate(self.shaft_outset, self.shaft_outset);
        // A competing shaft can be nearer to a wide label's center without nearing its ink.
        let ownership = Rect::from_center_size(center, (2.0 * own_distance, 2.0 * own_distance));
        let halo = rect.inflate(24.0, 24.0).union(shaft_ink).union(ownership);
        let mut clearance = Clearance {
            overlap: 0.0,
            foreign_shaft_overlap: 0.0,
            nearest_shaft: f64::INFINITY,
        };
        let result = self.fixed.visit(|entry| -> Result<_> {
            work.charge_adapter(1)?;
            let obstacle = match entry {
                BvhEntry::Node(Bounds {
                    x: [x0, x1],
                    y: [y0, y1],
                }) => {
                    return Ok(Continue(halo.overlaps(Rect::new(x0, y0, x1, y1))));
                }
                BvhEntry::Item(&obstacle) => obstacle,
            };
            let ink = obstacle.bounds();
            if !halo.overlaps(ink) {
                return Ok(Continue(true));
            }
            let blocked = match obstacle {
                Obstacle::Shaft(edge, line) => {
                    if edge != owner.0 {
                        clearance.nearest_shaft = clearance
                            .nearest_shaft
                            .min(line.nearest(center, 0.0).distance_sq.sqrt());
                    }
                    let hits = shaft_ink.contains(line.p0)
                        || shaft_ink.contains(line.p1)
                        || (shaft_ink.overlaps(ink)
                            && shaft_ink
                                .path_segments(0.0)
                                .any(|side| !side.intersect_line(line).is_empty()));
                    if hits {
                        let area = shaft_ink
                            .intersect(ink.inflate(self.shaft_outset, self.shaft_outset))
                            .area()
                            .max(f64::EPSILON);
                        clearance.overlap += area;
                        if edge != owner.0 {
                            clearance.foreign_shaft_overlap += area;
                        }
                    }
                    false
                }
                Obstacle::Terminal(terminal, ink) => {
                    if terminal != owner && !self.handled[terminal.0][usize::from(terminal.1)] {
                        clearance.overlap += terminal_ink.intersect(ink).area();
                    }
                    false
                }
                Obstacle::Box(ink) => rect.intersect(ink).area() > 0.0,
            };
            Ok(if blocked { Break(()) } else { Continue(true) })
        })?;
        if result.is_break() {
            return Ok(None);
        }
        clearance.overlap += self
            .occupied
            .overlap_area(bounds(terminal_ink), work)
            .map_err(|error| work.map_shoal_error(error))?;
        Ok(Some(clearance))
    }
}

fn straight_lead<'a>(
    points: impl Iterator<Item = &'a RoutePoint>,
    port: Point,
    outward: Vec2,
    work: &mut OperationLayoutWorkControl,
) -> Result<f64> {
    let mut lead = 0.0;
    for &next in points {
        work.charge_adapter(1)?;
        let delta = point(next) - port;
        let along = delta.dot(outward);
        if delta.cross(outward).abs() > 1e-6 || along < lead {
            break;
        }
        lead = along;
    }
    Ok(lead)
}

fn outward(side: Side) -> Vec2 {
    match side {
        Side::Top => Vec2::new(0.0, -1.0),
        Side::Right => Vec2::new(1.0, 0.0),
        Side::Bottom => Vec2::new(0.0, 1.0),
        Side::Left => Vec2::new(-1.0, 0.0),
    }
}

fn point(point: RoutePoint) -> Point {
    Point::new(point.x, point.y)
}

fn label_rect(label: &LayoutLabel) -> Rect {
    Rect::from_center_size((label.x, label.y), (label.width, label.height))
}

fn bounds(rect: Rect) -> Bounds {
    Bounds {
        x: [rect.x0, rect.x1],
        y: [rect.y0, rect.y1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    #[test]
    fn wide_terminal_caption_checks_ownership_beyond_its_ink_halo() {
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let label = LayoutLabel {
            x: 118.5,
            y: 116.0,
            width: 160.0,
            height: 12.0,
        };
        let owner = Line::new((200.0, 100.0), (200.0, 140.0));
        let foreign = Line::new((100.0, 150.0), (130.0, 150.0));
        let ink = label_rect(&label);
        assert!(!ink.inflate(24.0, 24.0).overlaps(foreign.bounding_box()));
        assert!(foreign.nearest(ink.center(), 0.0).distance_sq < 81.5_f64.powi(2));
        let fixed = Bvh::build_controlled(
            vec![Obstacle::Shaft(0, owner), Obstacle::Shaft(1, foreign)],
            |obstacle| bounds(obstacle.bounds()),
            &mut work,
        )
        .unwrap();
        let mut seating = Seating {
            fixed,
            occupied: BoundsIndex::default(),
            handled: vec![[false; 2]; 2],
            drawing: Rect::new(0.0, 0.0, 400.0, 400.0),
            shaft_outset: 0.5,
        };
        let seated = seating
            .place(
                label,
                Terminal {
                    owner: (0, true),
                    port: owner.p0,
                    outward: outward(Side::Bottom),
                    lead: 40.0,
                    span: owner.p0.distance(owner.p1),
                    marker: Marker::default(),
                },
                None,
                &mut work,
            )
            .unwrap()
            .unwrap();
        assert_eq!(seated.x, 281.5);
    }

    #[test]
    fn terminal_clearance_follows_the_label_and_also_protects_unseated_labels() {
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let make_label = |x, y| LayoutLabel {
            x,
            y,
            width: 8.0,
            height: 8.0,
        };
        let moving = make_label(20.0, 40.0);
        let unseated = make_label(60.0, 40.0);
        let old_ink = label_rect(&moving);
        let fixed = Bvh::build_controlled(
            vec![
                Obstacle::Terminal((0, true), old_ink),
                Obstacle::Terminal((0, false), label_rect(&unseated)),
            ],
            |obstacle| bounds(obstacle.bounds()),
            &mut work,
        )
        .unwrap();
        let mut seating = Seating {
            fixed,
            occupied: BoundsIndex::default(),
            handled: vec![[false; 2]; 2],
            drawing: Rect::new(0.0, 0.0, 150.0, 100.0),
            shaft_outset: 0.5,
        };
        let moving = seating
            .place(
                moving,
                Terminal {
                    owner: (0, true),
                    port: Point::new(90.0, 40.0),
                    outward: outward(Side::Right),
                    lead: 40.0,
                    span: 40.0,
                    marker: Marker::default(),
                },
                None,
                &mut work,
            )
            .unwrap()
            .unwrap();

        let mut can_place = |ink| {
            seating
                .clearance(ink, (1, true), 0.0, &mut work)
                .unwrap()
                .is_some_and(|clearance| clearance.overlap == 0.0)
        };
        assert!(can_place(old_ink));
        for current in [&moving, &unseated] {
            let ink = label_rect(current);
            assert!(!can_place(ink + Vec2::new(8.5, 0.0)));
            assert!(can_place(ink + Vec2::new(9.0, 0.0)));
        }
    }

    #[test]
    fn blocked_terminal_uses_its_own_shaft_locally_without_covering_nodes() {
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let shaft = Line::new((100.0, 40.0), (180.0, 40.0));
        let nodes = [
            Rect::new(98.0, 28.0, 180.0, 36.0),
            Rect::new(98.0, 44.0, 180.0, 52.0),
            Rect::new(110.0, 36.0, 126.0, 44.0),
        ];
        let fixed = Bvh::build_controlled(
            nodes
                .into_iter()
                .map(Obstacle::Box)
                .chain([Obstacle::Shaft(0, shaft)])
                .collect(),
            |obstacle| bounds(obstacle.bounds()),
            &mut work,
        )
        .unwrap();
        let mut seating = Seating {
            fixed,
            occupied: BoundsIndex::default(),
            handled: vec![[false; 2]],
            drawing: Rect::new(0.0, 0.0, 400.0, 100.0),
            shaft_outset: 0.5,
        };
        let label = LayoutLabel {
            x: 350.0,
            y: 40.0,
            width: 16.0,
            height: 8.0,
        };
        let seated = seating
            .place(
                label,
                Terminal {
                    owner: (0, true),
                    port: shaft.p0,
                    outward: outward(Side::Right),
                    lead: 80.0,
                    span: 200.0,
                    marker: Marker::default(),
                },
                None,
                &mut work,
            )
            .unwrap()
            .unwrap();
        let ink = label_rect(&seated);
        assert_eq!(ink.center().y, shaft.p0.y);
        assert!(ink.center().distance(shaft.p0) <= 56.0);
        assert!(nodes.iter().all(|node| ink.intersect(*node).area() == 0.0));
    }
}
