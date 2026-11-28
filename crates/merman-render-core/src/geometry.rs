use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Option<Self> {
        let mut it = points.into_iter();
        let (x0, y0) = it.next()?;
        let mut b = Self {
            min_x: x0,
            min_y: y0,
            max_x: x0,
            max_y: y0,
        };
        for (x, y) in it {
            b.min_x = b.min_x.min(x);
            b.min_y = b.min_y.min(y);
            b.max_x = b.max_x.max(x);
            b.max_y = b.max_y.max(y);
        }
        Some(b)
    }

    /// Forms bounds by subtracting and adding half the dimensions at the center.
    #[doc(hidden)]
    pub fn from_center(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            min_x: x - width / 2.0,
            min_y: y - height / 2.0,
            max_x: x + width / 2.0,
            max_y: y + height / 2.0,
        }
    }

    /// Tests for overlap with strict comparisons along both axes.
    #[doc(hidden)]
    pub fn overlaps_strictly(&self, other: &Self) -> bool {
        self.min_x < other.max_x
            && other.min_x < self.max_x
            && self.min_y < other.max_y
            && other.min_y < self.max_y
    }

    /// Applies the terminal-label Liang-Barsky clip. The parameter interval must have
    /// positive length. A stationary segment inside the bounds passes.
    #[doc(hidden)]
    pub fn overlaps_segment(&self, a: &LayoutPoint, b: &LayoutPoint) -> bool {
        let mut t0 = 0.0_f64;
        let mut t1 = 1.0_f64;
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        for (p, q) in [
            (-dx, a.x - self.min_x),
            (dx, self.max_x - a.x),
            (-dy, a.y - self.min_y),
            (dy, self.max_y - a.y),
        ] {
            if p == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else if p < 0.0 {
                t0 = t0.max(q / p);
            } else {
                t1 = t1.min(q / p);
            }
        }
        t0 < t1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPoint {
    pub x: f64,
    pub y: f64,
}
