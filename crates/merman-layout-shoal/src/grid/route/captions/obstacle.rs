use super::{Bounds, Point, SeatLine};

#[derive(Clone, Copy)]
pub(super) enum Shaft {
    Ortho(Bounds),
    Diagonal([Point; 2]),
}

impl Shaft {
    pub(super) fn new(a: Point, b: Point, bounds: Bounds) -> Self {
        if a.x == b.x || a.y == b.y {
            Self::Ortho(bounds)
        } else {
            Self::Diagonal([a, b])
        }
    }

    pub(super) fn bounds(self) -> Bounds {
        match self {
            Self::Ortho(bounds) => bounds,
            Self::Diagonal([a, b]) => Bounds::between(a, b),
        }
    }

    pub(super) fn ink_span(self, line: SeatLine) -> Option<[f64; 2]> {
        match self {
            Self::Ortho(bounds) => line.ink_span(bounds),
            Self::Diagonal(points) => {
                let diagonal = Projection::new(points, line.axis);
                let half = line.half[1 - line.axis];
                let band = [line.cross - half, line.cross + half];
                if band[0] >= diagonal.cross[1] || band[1] <= diagonal.cross[0] {
                    return None;
                }
                diagonal
                    .in_band(band)
                    .map(|along| line.expand_ink_span(along))
            }
        }
    }

    pub(super) fn foreign_span(self, line: SeatLine, reach: f64) -> Option<[f64; 2]> {
        match self {
            Self::Ortho(bounds) => line.foreign_span(bounds, reach),
            Self::Diagonal(points) => Projection::new(points, line.axis).tube_at(line.cross, reach),
        }
    }

    pub(super) fn adjacent_span(self, line: SeatLine, area: Bounds) -> Option<[f64; 2]> {
        let [along, cross] = line.axes(area);
        match self {
            Self::Ortho(bounds) => area.overlaps(bounds).then(|| line.axes(bounds)[1]),
            Self::Diagonal(points) => {
                // Only the portion of the diagonal alongside the swept label can offer a seat.
                let span = Projection::new(points, 1 - line.axis).in_band(along)?;
                (span[0] <= cross[1] && cross[0] <= span[1]).then_some(span)
            }
        }
    }
}

/// A diagonal in the coordinates of a candidate line, ordered by its cross coordinate.
struct Projection {
    along: [f64; 2],
    cross: [f64; 2],
}

impl Projection {
    fn new(points: [Point; 2], axis: usize) -> Self {
        let [a, b] = points.map(|p| [p.x, p.y]);
        let cross = 1 - axis;
        let (lo, hi) = if a[cross] <= b[cross] { (a, b) } else { (b, a) };
        Self {
            along: [lo[axis], hi[axis]],
            cross: [lo[cross], hi[cross]],
        }
    }

    fn at(&self, cross: f64) -> f64 {
        let [lo, hi] = self.cross;
        let [a, b] = self.along;
        if cross <= lo {
            a
        } else if cross >= hi {
            b
        } else {
            let ratio = (cross - lo) / (hi - lo);
            (b - a).mul_add(ratio, a).clamp(a.min(b), a.max(b))
        }
    }

    fn in_band(&self, [lo, hi]: [f64; 2]) -> Option<[f64; 2]> {
        if lo > self.cross[1] || hi < self.cross[0] {
            return None;
        }
        let (a, b) = (self.at(lo), self.at(hi));
        Some([a.min(b), a.max(b)])
    }

    fn tube_at(&self, cross: f64, reach: f64) -> Option<[f64; 2]> {
        if reach == 0.0 {
            return self.in_band([cross; 2]);
        }
        let [lo, hi] = self.cross;
        if (lo - cross).max(cross - hi) > reach {
            return None;
        }
        // The boundary of the distance field is two parallel offsets and two circular caps.
        // Intersect them with the candidate line to get the entire forbidden interval.
        let [a, b] = self.along;
        let dx = b - a;
        let dy = hi - lo;
        let length = dx.hypot(dy);
        let shift_along = reach * (dy / length);
        let shift_cross = reach * (-dx / length);
        let mut span = [f64::INFINITY, f64::NEG_INFINITY];
        for (end, at) in [(lo, a), (hi, b)] {
            let ratio = (cross - end).abs() / reach;
            if ratio <= 1.0 {
                let radius = reach * ((1.0 - ratio) * (1.0 + ratio)).sqrt();
                span = [span[0].min(at - radius), span[1].max(at + radius)];
            }
        }
        for direction in [-1.0, 1.0] {
            let at = shift_cross.mul_add(-direction, cross);
            if at >= lo && at <= hi {
                let along = shift_along.mul_add(direction, self.at(at));
                span = [span[0].min(along), span[1].max(along)];
            }
        }
        (span[0] <= span[1]).then_some(span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_projects_ink_affinity_and_candidate_boundaries_in_both_axes() {
        let a = Point { x: 0.0, y: 100.0 };
        let b = Point { x: 100.0, y: 0.0 };
        for points in [[a, b], [b, a]] {
            for axis in [0, 1] {
                let shaft = Shaft::Diagonal(points);
                let line = SeatLine {
                    axis,
                    span: [0.0, 100.0],
                    cross: 40.0,
                    anchor_cross: 40.0,
                    half: [5.0; 2],
                };
                assert_eq!(
                    shaft.ink_span(line),
                    Some([50.0_f64.next_up(), 70.0_f64.next_down()])
                );
                let [lo, hi] = shaft.foreign_span(line, 10.0).unwrap();
                let radius = 10.0 * std::f64::consts::SQRT_2;
                assert!((lo - (60.0 - radius)).abs() < 1e-12);
                assert!((hi - (60.0 + radius)).abs() < 1e-12);
                let area = line.bounds([30.0, 40.0], [20.0, 80.0]);
                assert_eq!(shaft.adjacent_span(line, area), Some([60.0, 70.0]));
                assert_eq!(
                    shaft.adjacent_span(line, line.bounds([30.0, 40.0], [20.0, 50.0])),
                    None
                );
                let beside_end = SeatLine {
                    cross: -6.0,
                    ..line
                };
                assert_eq!(shaft.foreign_span(beside_end, 6.0), Some([100.0; 2]));
            }
        }
    }
}
