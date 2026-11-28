use crate::Result;
use crate::graph_layout::ir::{Marker, MeasuredGraph, RoutePoint, Side, SolvedGraph};
use crate::graph_layout::route_paint::marker_bounds;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use kurbo::{Line, ParamCurveNearest, Point, Rect, Shape, Vec2};
use merman_layout_shoal::geom::{Bounds, BoundsIndex, Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

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
    let mut seating = Seating {
        fixed,
        occupied: BoundsIndex::default(),
        drawing,
        shaft_outset,
    };
    for (index, (edge, spec)) in solved.edges.iter_mut().zip(&graph.edges).enumerate() {
        work.charge_adapter(0)?;
        let [first, .., last] = edge.points.as_slice() else {
            continue;
        };
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
            if let Some(label) = label {
                let outward = outward(side);
                let lead = if start {
                    straight_lead(edge.points.iter().skip(1), point(port), outward, work)?
                } else {
                    straight_lead(edge.points.iter().rev().skip(1), point(port), outward, work)?
                };
                seating.place(
                    label,
                    (index, start),
                    point(port),
                    outward,
                    lead,
                    marker,
                    work,
                )?;
            }
        }
    }
    Ok(())
}

struct Seating {
    fixed: Bvh<Obstacle>,
    occupied: BoundsIndex,
    drawing: Rect,
    shaft_outset: f64,
}

impl Seating {
    #[expect(
        clippy::too_many_arguments,
        reason = "Seating uses one complete terminal frame."
    )]
    fn place(
        &mut self,
        label: &mut LayoutLabel,
        owner: (usize, bool),
        port: Point,
        outward: Vec2,
        lead: f64,
        marker: Marker,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        let across = Vec2::new(-outward.y, outward.x);
        let half = Vec2::new(label.width / 2.0, label.height / 2.0);
        let along_half = half.dot(Vec2::new(outward.x.abs(), outward.y.abs()));
        let across_half = half.dot(Vec2::new(across.x.abs(), across.y.abs()));
        let limit = lead - along_half;
        let minimum = along_half + 2.0;
        if limit < minimum {
            return Ok(());
        }
        let near = 16.0_f64.max(minimum).min(limit);
        let furthest = (near + 8.0).min(limit);
        let existing = Point::new(label.x, label.y);
        let preferred = if (existing - port).dot(across) < 0.0 {
            -1.0
        } else {
            1.0
        };
        let adjacent = across_half + self.shaft_outset + 1.0;
        let between_shafts = across_half + self.shaft_outset + 0.25;
        let past_marker = (marker.trim + marker.depth + along_half + 1.0).clamp(near, furthest);
        // Keep a little whitespace beside the shaft. A tight lane can still provide ink clearance.
        for offset in [
            adjacent,
            between_shafts,
            adjacent.max(across_half + marker.width / 2.0 + 1.0),
        ] {
            for distance in [near, past_marker, furthest] {
                let mut best = None::<(Point, f64)>;
                for sign in [preferred, -preferred] {
                    let center = port + outward * distance + across * (sign * offset);
                    let rect = Rect::from_center_size(center, (label.width, label.height));
                    if self.drawing.intersect(rect) != rect
                        || self
                            .occupied
                            .any_overlap(bounds(rect.inflate(1.0, 1.0)), work)
                            .map_err(|error| work.map_shoal_error(error))?
                    {
                        continue;
                    }
                    if let Some(clearance) = self.clearance(rect, owner, offset, work)?
                        && clearance > offset
                        && best.is_none_or(|(_, previous)| clearance > previous)
                    {
                        best = Some((center, clearance));
                    }
                }
                if let Some((center, _)) = best {
                    label.x = center.x;
                    label.y = center.y;
                    self.occupied
                        .insert(bounds(label_rect(label)), work)
                        .map_err(|error| work.map_shoal_error(error))?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn clearance(
        &self,
        rect: Rect,
        owner: (usize, bool),
        own_distance: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<f64>> {
        let center = rect.center();
        let shaft_ink = rect.inflate(self.shaft_outset, self.shaft_outset);
        // A competing shaft can be nearer to a wide label's center without nearing its ink.
        let ownership = Rect::from_center_size(center, (2.0 * own_distance, 2.0 * own_distance));
        let halo = rect.inflate(24.0, 24.0).union(shaft_ink).union(ownership);
        let mut nearest = f64::INFINITY;
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
                        nearest = nearest.min(line.nearest(center, 0.0).distance_sq.sqrt());
                    }
                    shaft_ink.contains(line.p0)
                        || shaft_ink.contains(line.p1)
                        || (shaft_ink.overlaps(ink)
                            && shaft_ink
                                .path_segments(0.0)
                                .any(|side| !side.intersect_line(line).is_empty()))
                }
                Obstacle::Terminal(terminal, ink) => {
                    terminal != owner && rect.intersect(ink).area() > 0.0
                }
                Obstacle::Box(ink) => rect.intersect(ink).area() > 0.0,
            };
            Ok(if blocked { Break(()) } else { Continue(true) })
        })?;
        Ok((!result.is_break()).then_some(nearest))
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
        let mut label = LayoutLabel {
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
            drawing: Rect::new(0.0, 0.0, 400.0, 400.0),
            shaft_outset: 0.5,
        };
        seating
            .place(
                &mut label,
                (0, true),
                owner.p0,
                outward(Side::Bottom),
                40.0,
                Marker::default(),
                &mut work,
            )
            .unwrap();
        assert_eq!(label.x, 281.5);
    }
}
