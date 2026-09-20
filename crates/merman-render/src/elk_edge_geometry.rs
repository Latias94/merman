//! Shared shape projection of Mermaid 12 ELK's `geometry.ts` and `render.ts`.
//!
//! Provider routes remain immutable. These helpers operate on the separate paint projection.

use crate::Result;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{LayoutNode, LayoutPoint as P};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outline {
    Rect,
    Ellipse,
    Diamond,
}

#[derive(Clone, Copy)]
pub(crate) struct Shape<'a> {
    pub node: &'a LayoutNode,
    pub outline: Outline,
}

impl Shape<'_> {
    fn center(self) -> P {
        P {
            x: self.node.x,
            y: self.node.y,
        }
    }

    pub(crate) fn intersect(self, point: &P) -> P {
        let node = self.node;
        let dx = point.x - node.x;
        let dy = point.y - node.y;
        let w = node.width / 2.0;
        let h = node.height / 2.0;
        if self.outline == Outline::Ellipse {
            let det = (w * w * dy * dy + h * h * dx * dx).sqrt();
            return P {
                x: node.x + w * h * dx / det,
                y: node.y + w * h * dy / det,
            };
        }
        if self.outline == Outline::Diamond {
            // Mermaid 12 polygon intersection uses floating division without the old
            // half-pixel rounding bias. A diamond's center ray has this closed form.
            let scale = dx.abs() / w + dy.abs() / h;
            return if scale == 0.0 {
                self.center()
            } else {
                P {
                    x: node.x + dx / scale,
                    y: node.y + dy / scale,
                }
            };
        }
        if dy.abs() * w > dx.abs() * h {
            let h = if dy < 0.0 { -h } else { h };
            P {
                x: node.x + if dy == 0.0 { 0.0 } else { h * dx / dy },
                y: node.y + h,
            }
        } else {
            let w = if dx < 0.0 { -w } else { w };
            P {
                x: node.x + w,
                y: node.y + if dx == 0.0 { 0.0 } else { w * dy / dx },
            }
        }
    }

    fn outside(self, point: &P) -> bool {
        (point.x - self.node.x).abs() >= self.node.width / 2.0
            || (point.y - self.node.y).abs() >= self.node.height / 2.0
    }

    fn border(self, point: &P, tolerance: f64) -> bool {
        let dx = (point.x - self.node.x).abs();
        let dy = (point.y - self.node.y).abs();
        ((dx - self.node.width / 2.0).abs() <= tolerance
            && dy <= self.node.height / 2.0 + tolerance)
            || ((dy - self.node.height / 2.0).abs() <= tolerance
                && dx <= self.node.width / 2.0 + tolerance)
    }

    fn inside_outline(self, point: &P) -> bool {
        let crossing = self.intersect(point);
        distance(point, &self.center()) <= distance(&crossing, &self.center()) + 1e-9
    }

    fn departure(self, port: &P, next: &P) -> Option<P> {
        if self.node.is_cluster {
            return None;
        }
        let dx = next.x - port.x;
        let dy = next.y - port.y;
        if (dx == 0.0 && dy == 0.0) || (dx.abs() > 1e-6 && dy.abs() > 1e-6) {
            return None;
        }
        let horizontal = dx.abs() > dy.abs();
        let along = |t| {
            if horizontal {
                P { x: t, y: port.y }
            } else {
                P { x: port.x, y: t }
            }
        };
        let mut inner = if horizontal { self.node.x } else { self.node.y };
        let mut outer = if horizontal { port.x } else { port.y };
        if !self.inside_outline(&along(inner)) {
            return None;
        }
        if self.inside_outline(&along(outer)) {
            return Some(port.clone());
        }
        for _ in 0..20 {
            let mid = (inner + outer) / 2.0;
            if self.inside_outline(&along(mid)) {
                inner = mid;
            } else {
                outer = mid;
            }
        }
        Some(along(inner))
    }

    fn compute_intersection(self, outside: &P, center: &P) -> P {
        let node = self.node;
        let mut outside = outside.clone();
        let dx = outside.x - node.x;
        let dy = outside.y - node.y;
        if (dx.abs() - node.width / 2.0).abs() < 1.0 || (dy.abs() - node.height / 2.0).abs() < 1.0 {
            let length = dx.hypot(dy);
            if length > 0.0 {
                outside = P {
                    x: node.x + dx / length * (length + 10.0),
                    y: node.y + dy / length * (length + 10.0),
                };
            }
        }
        let crossing = self.intersect(&outside);
        let wrong_side = (outside.x < node.x && crossing.x > node.x)
            || (outside.x > node.x && crossing.x < node.x);
        // Match JS comparisons for a degenerate ellipse's NaN intersection;
        // sanitize() owns the finite-point fallback after endpoint clipping.
        let rejected = wrong_side
            || (self.outline != Outline::Ellipse && distance(&outside, &crossing) <= 1.0);
        if !rejected {
            return crossing;
        }
        let inside = P {
            x: if (outside.x - node.x).abs() < 1.0 {
                outside.x
            } else if outside.x < node.x {
                node.x - node.width / 4.0
            } else {
                node.x + node.width / 4.0
            },
            y: if (outside.y - node.y).abs() < 1.0 {
                outside.y
            } else {
                center.y
            },
        };
        rect_segment_intersection(node, &outside, &inside)
    }
}

fn distance(a: &P, b: &P) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn rect_segment_intersection(node: &LayoutNode, outside: &P, inside: &P) -> P {
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    let q_abs = (outside.y - inside.y).abs();
    let r_abs = (outside.x - inside.x).abs();
    let mut result;
    if (node.y - outside.y).abs() * w > (node.x - outside.x).abs() * h {
        let q = if inside.y < outside.y {
            outside.y - h - node.y
        } else {
            node.y - h - outside.y
        };
        let r = r_abs * q / q_abs;
        result = P {
            x: if inside.x < outside.x {
                inside.x + r
            } else {
                inside.x - r_abs + r
            },
            y: if inside.y < outside.y {
                inside.y + q_abs - q
            } else {
                inside.y - q_abs + q
            },
        };
    } else {
        let r = if inside.x < outside.x {
            outside.x - w - node.x
        } else {
            node.x - w - outside.x
        };
        let q = q_abs * r / r_abs;
        result = P {
            x: if inside.x < outside.x {
                inside.x + r_abs - r
            } else {
                inside.x - r_abs + r
            },
            y: if inside.y < outside.y {
                inside.y + q
            } else {
                inside.y - q
            },
        };
    }
    if r_abs == 0.0 {
        result.x = outside.x;
    }
    if q_abs == 0.0 {
        result.y = outside.y;
    }
    result
}

fn replace_endpoint(points: &mut Vec<P>, start: bool, value: P) {
    if points.is_empty() {
        return;
    }
    let index = if start { 0 } else { points.len() - 1 };
    if points.len() > 2
        && (points[index].x - value.x).abs() < 0.1
        && (points[index].y - value.y).abs() < 0.1
    {
        points.remove(index);
    } else {
        points[index] = value;
    }
}

fn group_clip(points: &mut Vec<P>, shape: Shape<'_>, start: bool) -> bool {
    let outside = if start {
        points.iter().position(|p| shape.outside(p))
    } else {
        points.iter().rposition(|p| shape.outside(p))
    };
    let Some(index) = outside else {
        return false;
    };
    if (start && index == 0) || (!start && index == points.len() - 1) {
        return false;
    }
    let outside = &points[index];
    let inside = &points[if start { index - 1 } else { index + 1 }];
    let dx = outside.x - inside.x;
    let dy = outside.y - inside.y;
    let tx = if dx == 0.0 {
        f64::INFINITY
    } else {
        (shape.node.x + dx.signum() * shape.node.width / 2.0 - inside.x) / dx
    };
    let ty = if dy == 0.0 {
        f64::INFINITY
    } else {
        (shape.node.y + dy.signum() * shape.node.height / 2.0 - inside.y) / dy
    };
    let t = tx.min(ty);
    let crossing = P {
        x: inside.x + t * dx,
        y: inside.y + t * dy,
    };
    if start {
        points.splice(..index, [crossing]);
    } else {
        points.splice(index + 1.., [crossing]);
    }
    true
}

fn apply_intersection(points: &mut Vec<P>, shape: Shape<'_>, start: bool) {
    let outside = if start {
        points.iter().find(|p| shape.outside(p))
    } else {
        points.iter().rfind(|p| shape.outside(p))
    };
    let center = if start { points.first() } else { points.last() };
    if let (Some(outside), Some(center)) = (outside, center) {
        let crossing = shape.compute_intersection(outside, center);
        replace_endpoint(points, start, crossing);
    }
}

fn cutter(points: &[P], start: Shape<'_>, end: Shape<'_>) -> Vec<P> {
    let mut out = points.to_vec();
    let (Some(start_center), Some(end_center)) = (points.first(), points.last()) else {
        return out;
    };
    if let Some(index) = out.iter().position(|p| start.outside(p)) {
        let intersection = out
            .get(index + 1)
            .and_then(|next| {
                if start.outline == Outline::Ellipse {
                    None
                } else {
                    start.departure(&out[index], next)
                }
            })
            .unwrap_or_else(|| start.compute_intersection(&out[index], start_center));
        replace_endpoint(&mut out, true, intersection);
    }
    let outside = out
        .iter()
        .rposition(|p| end.outside(p))
        .or_else(|| out.len().checked_sub(2));
    if let Some(index) = outside {
        let intersection = index
            .checked_sub(1)
            .and_then(|next| {
                if end.outline == Outline::Ellipse {
                    None
                } else {
                    end.departure(&out[index], &out[next])
                }
            })
            .unwrap_or_else(|| end.compute_intersection(&out[index], end_center));
        replace_endpoint(&mut out, false, intersection);
    }
    if out.len() > 1 && distance(&out[out.len() - 1], &out[out.len() - 2]) < 2.0 {
        out.pop();
    }
    out
}

pub(crate) fn sanitize(points: &[P], start: Shape<'_>, end: Shape<'_>) -> Vec<P> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut previous = points.to_vec();
    let center_approx = |point: &P, shape: Shape<'_>| {
        (point.x - shape.node.x).abs() < 1e-6 && (point.y - shape.node.y).abs() < 1e-6
    };
    let start_center = center_approx(&previous[0], start);
    let end_center = center_approx(&previous[previous.len() - 1], end);
    let start_candidate = if start_center && previous.len() > 1 {
        &previous[1]
    } else {
        &previous[0]
    };
    let end_candidate = if end_center && previous.len() > 1 {
        &previous[previous.len() - 2]
    } else {
        &previous[previous.len() - 1]
    };
    let mut skip_start = start.node.is_cluster && start.border(start_candidate, 0.5);
    let mut skip_end = end.node.is_cluster && end.border(end_candidate, 0.5);
    if skip_start && start_center {
        previous.remove(0);
    }
    if skip_end && end_center {
        previous.pop();
    }
    if start.node.is_cluster && !skip_start {
        skip_start = group_clip(&mut previous, start, true);
    }
    if end.node.is_cluster && !skip_end {
        skip_end = group_clip(&mut previous, end, false);
    }
    let mut clipped = if skip_start || skip_end {
        if !skip_start {
            apply_intersection(&mut previous, start, true);
        }
        if !skip_end {
            apply_intersection(&mut previous, end, false);
        }
        previous.clone()
    } else {
        cutter(&previous, start, end)
    };
    if clipped.len() < 2 || clipped.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        let cleaned: Vec<_> = previous
            .iter()
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            .cloned()
            .collect();
        clipped = if cleaned.len() >= 2 {
            cleaned
        } else {
            previous
        };
    }
    // Source compares with the original predecessor, not the previously retained point.
    clipped
        .iter()
        .enumerate()
        .filter(|(index, point)| {
            *index == 0
                || (point.x - clipped[index - 1].x).abs() > 1e-6
                || (point.y - clipped[index - 1].y).abs() > 1e-6
        })
        .map(|(_, p)| p.clone())
        .collect()
}

pub(crate) fn marker_segment(
    points: &mut Vec<P>,
    shape: Shape<'_>,
    marker: Option<&str>,
    start: bool,
) {
    let offset: f64 = match marker {
        Some("arrow_point") => 4.0,
        Some("extension") => 17.25,
        Some("arrow_cross" | "arrow_circle") => 12.5,
        _ => 0.0,
    };
    if offset <= 0.0 || points.len() < 3 {
        return;
    }
    let (terminal, adjacent) = if start {
        (0, 1)
    } else {
        (points.len() - 1, points.len() - 2)
    };
    if distance(&points[terminal], &points[adjacent]) < 8.0_f64.max(offset * 2.0)
        && shape.border(&points[adjacent], 1.0)
    {
        points.remove(adjacent);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

fn axis(a: &P, b: &P) -> Option<Axis> {
    let dx = (b.x - a.x).abs();
    let dy = (b.y - a.y).abs();
    if dx > 0.01 && dy <= 0.01 {
        Some(Axis::Horizontal)
    } else if dy > 0.01 && dx <= 0.01 {
        Some(Axis::Vertical)
    } else {
        None
    }
}

fn straighten_front(points: &[P]) -> Option<Vec<P>> {
    if points.len() < 5 {
        return None;
    }
    let a = axis(&points[0], &points[1])?;
    let other = if a == Axis::Horizontal {
        Axis::Vertical
    } else {
        Axis::Horizontal
    };
    if axis(&points[2], &points[3]) != Some(a)
        || axis(&points[1], &points[2]) != Some(other)
        || distance(&points[0], &points[1]) > 30.0
    {
        return None;
    }
    let jog = if a == Axis::Horizontal {
        (points[2].y - points[1].y).abs()
    } else {
        (points[2].x - points[1].x).abs()
    };
    if !(0.01..=16.0).contains(&jog) {
        return None;
    }
    let along = |p: &P| if a == Axis::Horizontal { p.x } else { p.y };
    if (along(&points[1]) - along(&points[0])).signum()
        != (along(&points[3]) - along(&points[2])).signum()
    {
        return None;
    }
    let mut last = 3;
    while last + 1 < points.len() && axis(&points[last], &points[last + 1]) == Some(a) {
        last += 1;
    }
    if last == points.len() - 1 {
        return None;
    }
    let mut moved = points.to_vec();
    for point in &mut moved[2..=last] {
        if a == Axis::Horizontal {
            point.y = points[0].y;
        } else {
            point.x = points[0].x;
        }
    }
    moved.drain(1..3);
    Some(moved)
}

fn straighten(points: &[P]) -> Option<Vec<P>> {
    let front = straighten_front(points);
    let mut reversed = front.as_deref().unwrap_or(points).to_vec();
    reversed.reverse();
    if let Some(mut end) = straighten_front(&reversed) {
        end.reverse();
        Some(end)
    } else {
        front
    }
}

fn crossings(a: &[P], b: &[P], work: &mut OperationLayoutWorkControl) -> Result<usize> {
    let units = work.checked_mul(a.len().saturating_sub(1), b.len().saturating_sub(1))?;
    work.charge_adapter(units)?;
    let side = |o: &P, p: &P, q: &P| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let opposite = |a: f64, b: f64| (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0);
    Ok(a.windows(2)
        .map(|a| {
            b.windows(2)
                .filter(|b| {
                    opposite(side(&b[0], &b[1], &a[0]), side(&b[0], &b[1], &a[1]))
                        && opposite(side(&a[0], &a[1], &b[0]), side(&a[0], &a[1], &b[1]))
                })
                .count()
        })
        .sum())
}

pub(crate) fn straighten_routes(
    routes: &mut [Vec<P>],
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    for index in 0..routes.len() {
        work.charge_adapter(routes[index].len())?;
        let Some(candidate) = straighten(&routes[index]) else {
            continue;
        };
        let mut before = 0;
        let mut after = 0;
        for (other, route) in routes.iter().enumerate() {
            if other == index || route.len() < 2 {
                continue;
            }
            let original_crossings = crossings(&routes[index], route, work)?;
            let candidate_crossings = crossings(&candidate, route, work)?;
            before = work.checked_add(before, original_crossings)?;
            after = work.checked_add(after, candidate_crossings)?;
        }
        if after <= before {
            routes[index] = candidate;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    fn node(x: f64, y: f64, width: f64, height: f64, group: bool) -> LayoutNode {
        LayoutNode {
            id: "node".into(),
            x,
            y,
            width,
            height,
            is_cluster: group,
            label_width: None,
            label_height: None,
        }
    }

    fn points(values: &[(f64, f64)]) -> Vec<P> {
        values.iter().map(|&(x, y)| P { x, y }).collect()
    }

    fn coordinates(points: &[P]) -> Vec<(f64, f64)> {
        points.iter().map(|point| (point.x, point.y)).collect()
    }

    #[test]
    fn offset_ellipse_ports_keep_the_provider_departure_axis() {
        let start_node = node(0.0, 0.0, 100.0, 60.0, false);
        let end_node = node(200.0, 15.0, 60.0, 60.0, false);
        let start = Shape {
            node: &start_node,
            outline: Outline::Ellipse,
        };
        let end = Shape {
            node: &end_node,
            outline: Outline::Rect,
        };
        let raw = points(&[
            (0.0, 0.0),
            (50.0, 15.0),
            (100.0, 15.0),
            (170.0, 15.0),
            (200.0, 15.0),
        ]);
        let clipped = sanitize(&raw, start, end);
        assert!((clipped[0].x - 50.0 * 0.75_f64.sqrt()).abs() < 0.0001);
        assert_eq!(clipped[0].y, 15.0);
        assert_eq!(clipped[1].y, 15.0);
        assert_eq!(coordinates(&raw)[1], (50.0, 15.0));
        let reversed = sanitize(&raw.iter().rev().cloned().collect::<Vec<_>>(), end, start);
        assert!((reversed.last().unwrap().x - clipped[0].x).abs() < 0.0001);
        assert_eq!(reversed.last().unwrap().y, 15.0);
    }

    #[test]
    fn changed_group_frame_uses_actual_crossing_segment_in_both_directions() {
        // Pinned render.spec.ts: crossing lies on the frame at y=130, not on its centre ray.
        let source_node = node(-100.0, 130.0, 40.0, 40.0, true);
        let target_node = node(100.0, 100.0, 100.0, 120.0, true);
        let source = Shape {
            node: &source_node,
            outline: Outline::Rect,
        };
        let target = Shape {
            node: &target_node,
            outline: Outline::Rect,
        };
        let raw = points(&[
            (-100.0, 130.0),
            (-80.0, 130.0),
            (20.0, 130.0),
            (53.0, 130.0),
            (60.0, 130.0),
            (100.0, 100.0),
        ]);
        let expected = [(-80.0, 130.0), (20.0, 130.0), (50.0, 130.0)];
        assert_eq!(coordinates(&sanitize(&raw, source, target)), expected);
        let reversed = raw.into_iter().rev().collect::<Vec<_>>();
        assert_eq!(
            coordinates(&sanitize(&reversed, target, source)),
            expected.into_iter().rev().collect::<Vec<_>>()
        );
    }

    #[test]
    fn terminal_jog_moves_the_channel_without_moving_either_port() {
        let raw = points(&[
            (193.0, 116.25),
            (218.0, 116.25),
            (218.0, 119.5),
            (400.0, 119.5),
            (400.0, 300.0),
        ]);
        assert_eq!(
            coordinates(&straighten(&raw).unwrap()),
            [(193.0, 116.25), (400.0, 116.25), (400.0, 300.0)]
        );
        assert!(straighten(&raw[..4]).is_none());
    }

    #[test]
    fn terminal_jog_is_rejected_when_it_adds_a_crossing() {
        let raw = points(&[
            (193.0, 116.25),
            (218.0, 116.25),
            (218.0, 119.5),
            (400.0, 119.5),
            (400.0, 300.0),
        ]);
        let blocker = points(&[(300.0, 115.0), (300.0, 118.0)]);
        let mut routes = vec![raw.clone(), blocker];
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::default(),
        )));
        straighten_routes(&mut routes, &mut work).unwrap();
        assert_eq!(coordinates(&routes[0]), coordinates(&raw));
    }

    #[test]
    fn short_marker_stubs_are_removed_without_changing_ports() {
        let terminal = node(100.0, 100.0, 100.0, 100.0, false);
        let shape = Shape {
            node: &terminal,
            outline: Outline::Rect,
        };
        let mut route = points(&[(0.0, 90.0), (50.0, 90.0), (50.0, 100.0)]);
        marker_segment(&mut route, shape, Some("extension"), false);
        assert_eq!(coordinates(&route), [(0.0, 90.0), (50.0, 100.0)]);
    }
}
