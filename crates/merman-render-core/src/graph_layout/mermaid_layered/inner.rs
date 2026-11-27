//! Edges between a group and one of its members, drawn after its other edges.

use super::compound::CNode;
use super::measured::{CORNER, EdgePaint, PORT_SPACING, PlacedBox};
use super::work::{Work, add, cmp, geometry, js_max, js_min, mul};
use super::{Direction, Point, Result, RoutePoint, Size};
use super::{DirectionExt, PointExt, RoutePointExt, SideExt, SizeExt, route_point};

const LABEL_CLEAR: f64 = 4.0;
const LABEL_HOLD: f64 = 4.0;

#[derive(Clone, Copy)]
pub(crate) struct InnerEdge {
    pub(crate) group: usize,
    pub(crate) member: usize,
    out: bool,
    direction: Direction,
    band: f64,
    skip: f64,
    title_width: f64,
    label_len: f64,
}

impl InnerEdge {
    pub(crate) fn new(
        [group, member]: [usize; 2],
        out: bool,
        direction: Direction,
        node: &CNode,
        measured: &super::super::MeasuredGroup,
        label: Option<Size>,
        pad_scale: f64,
    ) -> Self {
        let at_top = direction.vertical() && out == (direction == Direction::Down);
        let band = if !direction.vertical() {
            node.pad_x
        } else if at_top {
            node.pad_top
        } else {
            node.pad_bottom
        };
        let (skip, title_width) = match measured.title.filter(|_| at_top) {
            Some(title) => (
                measured.padding.y0 * pad_scale / 2.0 + title.height,
                title.width,
            ),
            None => (0.0, 0.0),
        };
        Self {
            group,
            member,
            out,
            direction,
            band,
            skip,
            title_width,
            label_len: label.map_or(0.0, |size| size.in_axes(direction).height),
        }
    }

    pub(crate) fn reserve<E>(&self, node: &mut CNode, head_room: f64) -> Result<(), E> {
        if self.label_len == 0.0 && self.skip == 0.0 {
            return Ok(());
        }
        let room = self.band + self.label_len + head_room;
        if !self.direction.vertical() {
            node.min_width += 2.0 * js_max(0.0, room - node.pad_x);
            node.pad_x = js_max(node.pad_x, room);
        } else if self.out == (self.direction == Direction::Down) {
            node.pad_top = js_max(node.pad_top, room);
        } else {
            node.pad_bottom = js_max(node.pad_bottom, room);
        }
        geometry(room.is_finite() && node.min_width.is_finite())
    }

    pub(crate) fn layout<'a, E>(
        self,
        group: PlacedBox,
        member: PlacedBox,
        label: Option<Size>,
        drawn: &[usize],
        route: impl Fn(usize) -> Option<&'a [RoutePoint]>,
        work: &mut Work<'_, E>,
    ) -> Result<EdgePaint, E> {
        let down = self.direction.vertical();
        let forward = if self.direction.flipped() { -1.0 } else { 1.0 };
        let side = if self.out { -forward } else { forward };
        let face =
            along(member.center, down) + side * member.size.in_axes(self.direction).height / 2.0;
        let center = across(member.center, down);
        let half = member.size.in_axes(self.direction).width / 2.0;
        let mut taken = Vec::new();
        work.price(drawn.len())?;
        for (i, &slot) in drawn.iter().enumerate() {
            work.poll(i)?;
            let Some(points) = route(slot) else {
                continue;
            };
            for p in [points.first(), points.last()].into_iter().flatten() {
                let x = across(p.position(), down);
                if (along(p.position(), down) - face).abs() <= 1.0
                    && (x - center).abs() <= half + 1.0
                {
                    work.push(&mut taken, x)?;
                }
            }
        }
        let edge_at = free_across(center, half, &taken, work)?;
        let label_at = label
            .map(|label| label_across(group, self.direction, label, edge_at, &taken, work))
            .transpose()?;
        let border =
            along(group.center, down) + side * group.size.in_axes(self.direction).height / 2.0;
        let under_title = down && (edge_at - group.center.x).abs() < self.title_width / 2.0;
        let edge = if under_title {
            border - side * self.skip
        } else {
            border
        };
        let coords = if self.out { [edge, face] } else { [face, edge] };
        let mut points = work.reserve(2)?;
        for along in coords {
            let point = physical(edge_at, along, down);
            geometry(point.is_finite())?;
            points.push(route_point(point, self.direction.axis()));
        }
        let label = label_at
            .map(|at| physical(at, border - side * (self.band + self.label_len / 2.0), down));
        geometry(label.is_none_or(|point| point.is_finite()))?;
        let side = if self.out {
            self.direction.start()
        } else {
            self.direction.start().opposite()
        };
        Ok((points, label, [side; 2]))
    }
}

fn across(point: Point, down: bool) -> f64 {
    if down { point.x } else { point.y }
}

fn along(point: Point, down: bool) -> f64 {
    if down { point.y } else { point.x }
}

fn physical(across: f64, along: f64, down: bool) -> Point {
    if down {
        Point {
            x: across,
            y: along,
        }
    } else {
        Point {
            x: along,
            y: across,
        }
    }
}

fn gap_to<E>(x: f64, taken: &[f64], work: &mut Work<'_, E>) -> Result<f64, E> {
    work.price(taken.len())?;
    let mut gap = f64::INFINITY;
    for (i, &other) in taken.iter().enumerate() {
        work.poll(i)?;
        gap = js_min(gap, (x - other).abs());
    }
    Ok(gap)
}

fn free_across<E>(center: f64, half: f64, taken: &[f64], work: &mut Work<'_, E>) -> Result<f64, E> {
    let mut best = center;
    let mut distance = gap_to(center, taken, work)?;
    if distance >= PORT_SPACING {
        return Ok(center);
    }
    let reach = js_max(0.0, half - js_min(CORNER, half / 2.0));
    let mut step = 1;
    while (step as f64) * PORT_SPACING <= reach + 1e-6 {
        work.price(1)?;
        let dx = (step as f64) * PORT_SPACING;
        for x in [center + dx, center - dx] {
            let gap = gap_to(x, taken, work)?;
            if gap >= PORT_SPACING {
                return Ok(x);
            }
            if gap > distance {
                (best, distance) = (x, gap);
            }
        }
        step = add(step, 1)?;
    }
    for x in [center + reach, center - reach] {
        let gap = gap_to(x, taken, work)?;
        if gap > distance {
            (best, distance) = (x, gap);
        }
    }
    Ok(best)
}

fn label_across<E>(
    group: PlacedBox,
    direction: Direction,
    label: Size,
    at: f64,
    taken: &[f64],
    work: &mut Work<'_, E>,
) -> Result<f64, E> {
    if taken.is_empty() {
        return Ok(at);
    }
    let half = label.in_axes(direction).width / 2.0;
    let clear = half + LABEL_CLEAR;
    let room = group.size.in_axes(direction).width / 2.0 - half;
    let center = across(group.center, direction.vertical());
    let (lo, hi) = if room >= 0.0 {
        (center - room, center + room)
    } else {
        (center, center)
    };
    let slide = js_max(0.0, half - LABEL_HOLD);
    let fits = |x: f64| x >= lo - 1e-6 && x <= hi + 1e-6 && (x - at).abs() <= slide + 1e-6;
    if gap_to(at, taken, work)? >= clear && fits(at) {
        return Ok(at);
    }
    let mut options = work.reserve(add(1, mul(2, taken.len())?)?)?;
    if fits(at) {
        options.push((at, 0));
    }
    work.price(taken.len())?;
    for (i, &x) in taken.iter().enumerate() {
        work.poll(i)?;
        for option in [x - clear, x + clear] {
            if fits(option) {
                options.push((option, options.len()));
            }
        }
    }
    work.sort_rows(&mut options, |a, b| {
        cmp((a.0 - at).abs(), (b.0 - at).abs()).then(a.1.cmp(&b.1))
    })?;
    let mut best = js_min(hi, js_max(lo, at));
    let mut distance = gap_to(best, taken, work)?;
    for (x, _) in options {
        let gap = gap_to(x, taken, work)?;
        if gap >= clear {
            return Ok(x);
        }
        if gap > distance {
            (best, distance) = (x, gap);
        }
    }
    Ok(best)
}
