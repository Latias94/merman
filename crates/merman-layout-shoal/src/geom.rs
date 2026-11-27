mod bvh;

pub use bvh::{BoundsIndex, Bvh, BvhEntry};

use crate::{MarkerMetrics, Point, Rect, Side, Size};

/// Axis-aligned bounds stored as pairs of endpoints in drawing coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: [f64; 2],
    pub y: [f64; 2],
}

impl Bounds {
    pub(crate) fn of(rect: Rect) -> Self {
        Self {
            x: [rect.x, rect.x + rect.width],
            y: [rect.y, rect.y + rect.height],
        }
    }

    pub(crate) fn centered(at: Point, size: Size) -> Self {
        Self::from_half_extents(at, size.width * 0.5, size.height * 0.5)
    }

    pub(crate) fn from_half_extents(at: Point, w: f64, h: f64) -> Self {
        Self {
            x: [at.x - w, at.x + w],
            y: [at.y - h, at.y + h],
        }
    }

    pub(crate) fn between(a: Point, b: Point) -> Self {
        Self {
            x: [a.x, b.x],
            y: [a.y, b.y],
        }
        .ordered()
    }

    pub(crate) fn ordered(self) -> Self {
        let order = |[a, b]: [f64; 2]| [a.min(b), a.max(b)];
        Self {
            x: order(self.x),
            y: order(self.y),
        }
    }

    pub(crate) fn marker(at: Point, side: Side, marker: MarkerMetrics) -> Self {
        let Point { x, y } = at;
        let half = marker.width * 0.5;
        let length = marker.length;
        let (x, y) = match side {
            Side::Top => ([x - half, x + half], [y - length, y]),
            Side::Bottom => ([x - half, x + half], [y, y + length]),
            Side::Left => ([x - length, x], [y - half, y + half]),
            Side::Right => ([x, x + length], [y - half, y + half]),
        };
        Self { x, y }
    }

    pub(crate) fn marker_footprint(at: Point, side: Side, marker: MarkerMetrics) -> Option<Self> {
        (marker.length > 0.0 && marker.width > 0.0).then(|| Self::marker(at, side, marker))
    }

    pub(crate) fn finite(self) -> bool {
        self.x.into_iter().chain(self.y).all(f64::is_finite)
    }

    pub(crate) fn has_area(self) -> bool {
        self.x[0] < self.x[1] && self.y[0] < self.y[1]
    }

    pub(crate) fn union(self, other: Self) -> Self {
        Self {
            x: [self.x[0].min(other.x[0]), self.x[1].max(other.x[1])],
            y: [self.y[0].min(other.y[0]), self.y[1].max(other.y[1])],
        }
    }

    pub(crate) fn include(&mut self, point: Point) {
        *self = self.union(Self {
            x: [point.x; 2],
            y: [point.y; 2],
        });
    }

    /// Intersection of closed bounds, including contact at the boundary.
    pub(crate) fn overlaps(self, other: Self) -> bool {
        self.x[0] <= other.x[1]
            && other.x[0] <= self.x[1]
            && self.y[0] <= other.y[1]
            && other.y[0] <= self.y[1]
    }

    /// Strict endpoint comparisons on both axes. A degenerate bound inside another can overlap it.
    pub(crate) fn overlaps_strictly(self, other: Self) -> bool {
        self.x[0] < other.x[1]
            && other.x[0] < self.x[1]
            && self.y[0] < other.y[1]
            && other.y[0] < self.y[1]
    }
}
