use super::*;

/// Closed rectangle bounds; crossing or overlapping the open interior is a collision.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

impl Bounds {
    pub(super) fn of(rect: Rect) -> LayoutResult<Self> {
        let b = Self {
            x0: rect.x,
            x1: finite(rect.x + rect.width)?,
            y0: rect.y,
            y1: finite(rect.y + rect.height)?,
        };
        if (rect.width > 0.0 && b.x1 <= b.x0) || (rect.height > 0.0 && b.y1 <= b.y0) {
            return Err(PRECISION);
        }
        Ok(b)
    }
    pub(super) fn centered(p: Point, size: Size) -> LayoutResult<Self> {
        let b = Self {
            x0: finite(p.x - size.width / 2.0)?,
            x1: finite(p.x + size.width / 2.0)?,
            y0: finite(p.y - size.height / 2.0)?,
            y1: finite(p.y + size.height / 2.0)?,
        };
        if (size.width > 0.0 && (p.x <= b.x0 || b.x1 <= p.x))
            || (size.height > 0.0 && (p.y <= b.y0 || b.y1 <= p.y))
        {
            return Err(PRECISION);
        }
        Ok(b)
    }
    // A route leg can have zero width or height. Its envelope still obstructs a label
    // whenever the leg lies strictly inside the label's two axis intervals.
    pub(super) fn intersects_label(self, label: Self) -> bool {
        label.x0 < label.x1
            && label.y0 < label.y1
            && self.x0 < label.x1
            && label.x0 < self.x1
            && self.y0 < label.y1
            && label.y0 < self.y1
    }
    pub(super) fn blocks(self, a: Point, b: Point) -> bool {
        if a.x == b.x {
            self.x0 < a.x && a.x < self.x1 && a.y.min(b.y).max(self.y0) < a.y.max(b.y).min(self.y1)
        } else {
            debug_assert_eq!(a.y, b.y);
            self.y0 < a.y && a.y < self.y1 && a.x.min(b.x).max(self.x0) < a.x.max(b.x).min(self.x1)
        }
    }
    pub(super) fn along(self, s: Side) -> (f64, f64) {
        match s {
            Side::Right => (self.x0, self.x1),
            Side::Left => (-self.x1, -self.x0),
            Side::Bottom => (self.y0, self.y1),
            Side::Top => (-self.y1, -self.y0),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Owner {
    Node,
    Middle(u32),
    Marker(u32),
    LaneMarker(u32, Side),
    Stroke(u32),
    End,
}

pub(super) struct Obstacles {
    boxes: Vec<(Owner, Bounds)>,
    strokes: Vec<(u32, Bounds)>,
}

impl Obstacles {
    pub(super) fn new() -> Self {
        Self {
            boxes: Vec::new(),
            strokes: Vec::new(),
        }
    }
    pub(super) fn add(
        &mut self,
        owner: Owner,
        bounds: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.charge(1)?;
        push(&mut self.boxes, (owner, bounds))
    }
    fn iter(&self) -> impl Iterator<Item = (Owner, Bounds)> + '_ {
        self.boxes
            .iter()
            .copied()
            .chain(self.strokes.iter().map(|&(id, b)| (Owner::Stroke(id), b)))
    }
    pub(super) fn exterior(
        &self,
        mid: Point,
        size: Size,
        gap: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<Point> {
        let (mut left, mut right) = (mid.x, mid.x);
        for (_, r) in self.iter() {
            work.visit(1)?;
            left = left.min(r.x0);
            right = right.max(r.x1);
        }
        let room = finite(size.width / 2.0 + gap)?;
        let at = if mid.x - left <= right - mid.x {
            finite((left - room).next_down())?
        } else {
            finite((right + room).next_up())?
        };
        Ok(Point { x: at, ..mid })
    }
    pub(super) fn for_route(
        &self,
        edge: u32,
        ports: [Port; 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<RouteWalls> {
        work.charge(self.boxes.len())?;
        let mut rects = reserved(self.boxes.len())?;
        for &(owner, b) in &self.boxes {
            work.visit(1)?;
            let skip = match owner {
                Owner::Middle(id) => id == edge,
                // A shared marker shaft occupies the same outward ray. The other edge
                // must also be able to leave this port. Label placement sees both boxes.
                Owner::Marker(id) | Owner::LaneMarker(id, _) => {
                    id == edge
                        || ports.iter().any(|p| match p.side {
                            Side::Right | Side::Left => {
                                p.at.y >= b.y0
                                    && p.at.y <= b.y1
                                    && p.at.x == if p.side == Side::Right { b.x0 } else { b.x1 }
                            }
                            Side::Bottom | Side::Top => {
                                p.at.x >= b.x0
                                    && p.at.x <= b.x1
                                    && p.at.y == if p.side == Side::Bottom { b.y0 } else { b.y1 }
                            }
                        })
                }
                _ => false,
            };
            if !skip {
                rects.push(b);
            }
        }
        Ok(RouteWalls(rects))
    }

    pub(super) fn clear_lane_marker(
        &self,
        edge: u32,
        side: Side,
        ink: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        for &(owner, wall) in &self.boxes {
            work.visit(1)?;
            // Busy faces can share their normal shafts when their usable span cannot fit another
            // marker. Measured bodies, labels and markers on other faces still obstruct them.
            let same_face = matches!(owner, Owner::LaneMarker(id, face)
                if id == edge || (face == side && wall.along(side).0 == ink.along(side).0));
            if !same_face && wall.intersects_label(ink) {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub(super) fn add_stroke(
        &mut self,
        id: u32,
        edge: &EdgeGeometry,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let count = edge.points.len().saturating_sub(1);
        work.charge(count)?;
        self.strokes
            .try_reserve(count)
            .map_err(|_| LayoutError::AllocationLimit)?;
        for pair in edge.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            work.visit(1)?;
            self.strokes.push((
                id,
                Bounds {
                    x0: a.x.min(b.x),
                    x1: a.x.max(b.x),
                    y0: a.y.min(b.y),
                    y1: a.y.max(b.y),
                },
            ));
        }
        Ok(())
    }
    pub(super) fn fits(
        &self,
        b: Bounds,
        own_stroke: Option<u32>,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        for (owner, wall) in self.iter() {
            work.visit(1)?;
            if matches!(owner,Owner::Stroke(id) if Some(id)==own_stroke) {
                continue;
            }
            if wall.intersects_label(b) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Slide a measured box along a ray. Intersecting walls project onto forbidden open
    /// intervals for its center. Sorting once also handles obstacles hidden behind others.
    pub(super) fn slide(
        &self,
        origin: Point,
        size: Size,
        side: Side,
        own_stroke: Option<u32>,
        work: &mut Work<'_>,
    ) -> LayoutResult<(Point, f64)> {
        let initial = Bounds::centered(origin, size)?;
        let (start, half) = match side {
            Side::Top => (-origin.y, size.height / 2.0),
            Side::Bottom => (origin.y, size.height / 2.0),
            Side::Left => (-origin.x, size.width / 2.0),
            Side::Right => (origin.x, size.width / 2.0),
        };
        let total = self
            .boxes
            .len()
            .checked_add(self.strokes.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(total)?;
        let mut intervals = reserved(total)?;
        for (owner, b) in self.iter() {
            work.visit(1)?;
            if matches!(owner,Owner::Stroke(id) if Some(id)==own_stroke) {
                continue;
            }
            let across = match side.axis() {
                Axis::Vertical => initial.x0 < b.x1 && b.x0 < initial.x1,
                Axis::Horizontal => initial.y0 < b.y1 && b.y0 < initial.y1,
            };
            if across {
                let (lo, hi) = b.along(side);
                if half > 0.0 && finite(hi + half)? > start {
                    intervals.push((finite(lo - half)?, finite(hi + half)?));
                }
            }
        }
        super::super::validate::charge_sort(intervals.len(), work)?;
        intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let mut at = start;
        for (lo, hi) in intervals {
            work.visit(1)?;
            if at <= lo {
                break;
            }
            if at < hi {
                at = finite(hi.next_up())?;
            }
        }
        let center = match side {
            Side::Top => Point { y: -at, ..origin },
            Side::Bottom => Point { y: at, ..origin },
            Side::Left => Point { x: -at, ..origin },
            Side::Right => Point { x: at, ..origin },
        };
        Ok((center, finite(at - start)?))
    }
}

pub(super) struct RouteWalls(pub(super) Vec<Bounds>);

impl RouteWalls {
    pub(super) fn clear_leg(&self, a: Point, b: Point, work: &mut Work<'_>) -> LayoutResult<bool> {
        if a.x != b.x && a.y != b.y {
            return Ok(false);
        }
        for &wall in &self.0 {
            work.visit(1)?;
            if wall.blocks(a, b) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
