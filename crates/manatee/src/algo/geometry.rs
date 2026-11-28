/// A top-left-anchored layout-base rectangle.
#[derive(Clone, Copy)]
pub(super) struct Rect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl Rect {
    pub(super) const fn new(left: f64, top: f64, width: f64, height: f64) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    fn right(self) -> f64 {
        self.left + self.width
    }

    fn bottom(self) -> f64 {
        self.top + self.height
    }

    #[inline]
    pub(super) fn intersects(self, other: Self) -> bool {
        // Keep the ordered separation tests from RectangleD. Touch counts as overlap, and an
        // unordered comparison does not by itself prove separation.
        !(self.right() < other.left
            || self.bottom() < other.top
            || other.right() < self.left
            || other.bottom() < self.top)
    }
}

pub(super) struct RectOverlap {
    x: f64,
    y: f64,
}

impl RectOverlap {
    /// Measures overlap, including containment, as in layout-base `IGeometry.calcSeparationAmount`.
    #[inline]
    pub(super) fn between(a: Rect, b: Rect) -> Self {
        let mut x = a.right().min(b.right()) - a.left.max(b.left);
        let mut y = a.bottom().min(b.bottom()) - a.top.max(b.top);

        if (a.left <= b.left) && (a.right() >= b.right()) {
            x += (b.left - a.left).min(a.right() - b.right());
        } else if (b.left <= a.left) && (b.right() >= a.right()) {
            x += (a.left - b.left).min(b.right() - a.right());
        }
        if (a.top <= b.top) && (a.bottom() >= b.bottom()) {
            y += (b.top - a.top).min(a.bottom() - b.bottom());
        } else if (b.top <= a.top) && (b.bottom() >= a.bottom()) {
            y += (a.top - b.top).min(b.bottom() - a.bottom());
        }

        Self { x, y }
    }

    /// Returns the buffered half-separation using the caller's center slope and direction signs.
    #[inline]
    pub(super) fn separation(
        self,
        slope: f64,
        (dir_x, dir_y): (f64, f64),
        separation_buffer: f64,
    ) -> (f64, f64) {
        let mut move_by_y = slope * self.x;
        let mut move_by_x = self.y / slope;
        if self.x < move_by_x {
            move_by_x = self.x;
        } else {
            move_by_y = self.y;
        }

        let dx = -dir_x * ((move_by_x / 2.0) + separation_buffer);
        let dy = -dir_y * ((move_by_y / 2.0) + separation_buffer);
        (dx, dy)
    }
}

/// Returns both input centers on overlap, or clips their connecting line to the rectangles.
#[inline]
pub(super) fn clip_points(
    a: Rect,
    b: Rect,
    center_a: (f64, f64),
    center_b: (f64, f64),
) -> (f64, f64, f64, f64, bool) {
    if a.intersects(b) {
        return (center_a.0, center_a.1, center_b.0, center_b.1, true);
    }
    let (ax, ay, bx, by) = clip_points_disjoint(a, b, center_a, center_b);
    (ax, ay, bx, by, false)
}

/// Clips a pair whose overlap has already been checked by the caller.
///
/// The supplied centers retain each solver's arithmetic, including CoSE's cached half-sizes.
/// The side projections still use half-sizes calculated from the rectangle dimensions.
#[inline]
pub(super) fn clip_points_disjoint(
    a: Rect,
    b: Rect,
    (p1x, p1y): (f64, f64),
    (p2x, p2y): (f64, f64),
) -> (f64, f64, f64, f64) {
    // Preserve the branching and floating-point expressions of layout-base IGeometry.
    let left_a = a.left;
    let top_a = a.top;
    let right_a = a.right();
    let bottom_a = a.bottom();
    let half_width_a = a.width / 2.0;
    let half_height_a = a.height / 2.0;

    let left_b = b.left;
    let top_b = b.top;
    let right_b = b.right();
    let bottom_b = b.bottom();
    let half_width_b = b.width / 2.0;
    let half_height_b = b.height / 2.0;

    // Exact axis comparisons also cover coincident centers and an unordered other axis.
    if p1x == p2x {
        if p1y > p2y {
            return (p1x, top_a, p2x, bottom_b);
        } else if p1y < p2y {
            return (p1x, bottom_a, p2x, top_b);
        }
        return (p1x, p1y, p2x, p2y);
    }
    if p1y == p2y {
        if p1x > p2x {
            return (left_a, p1y, right_b, p2y);
        } else if p1x < p2x {
            return (right_a, p1y, left_b, p2y);
        }
        return (p1x, p1y, p2x, p2y);
    }

    let slope_a = a.height / a.width;
    let slope_b = b.height / b.width;
    let slope_prime = (p2y - p1y) / (p2x - p1x);

    let mut ax = 0.0;
    let mut ay = 0.0;
    let mut bx = 0.0;
    let mut by = 0.0;
    let mut clip_a_found = false;
    let mut clip_b_found = false;

    if -slope_a == slope_prime {
        if p1x > p2x {
            ax = left_a;
            ay = bottom_a;
        } else {
            ax = right_a;
            ay = top_a;
        }
        clip_a_found = true;
    } else if slope_a == slope_prime {
        if p1x > p2x {
            ax = left_a;
            ay = top_a;
        } else {
            ax = right_a;
            ay = bottom_a;
        }
        clip_a_found = true;
    }

    if -slope_b == slope_prime {
        if p2x > p1x {
            bx = left_b;
            by = bottom_b;
        } else {
            bx = right_b;
            by = top_b;
        }
        clip_b_found = true;
    } else if slope_b == slope_prime {
        if p2x > p1x {
            bx = left_b;
            by = top_b;
        } else {
            bx = right_b;
            by = bottom_b;
        }
        clip_b_found = true;
    }

    if clip_a_found && clip_b_found {
        return (ax, ay, bx, by);
    }

    let (card_a, card_b) = if p1x > p2x {
        if p1y > p2y {
            (
                get_cardinal_direction(slope_a, slope_prime, 4),
                get_cardinal_direction(slope_b, slope_prime, 2),
            )
        } else {
            (
                get_cardinal_direction(-slope_a, slope_prime, 3),
                get_cardinal_direction(-slope_b, slope_prime, 1),
            )
        }
    } else if p1y > p2y {
        (
            get_cardinal_direction(-slope_a, slope_prime, 1),
            get_cardinal_direction(-slope_b, slope_prime, 3),
        )
    } else {
        (
            get_cardinal_direction(slope_a, slope_prime, 2),
            get_cardinal_direction(slope_b, slope_prime, 4),
        )
    };

    if !clip_a_found {
        match card_a {
            1 => {
                ay = top_a;
                ax = p1x + (-half_height_a) / slope_prime;
            }
            2 => {
                ax = right_a;
                ay = p1y + half_width_a * slope_prime;
            }
            3 => {
                ay = bottom_a;
                ax = p1x + half_height_a / slope_prime;
            }
            _ => {
                ax = left_a;
                ay = p1y + (-half_width_a) * slope_prime;
            }
        }
    }

    if !clip_b_found {
        match card_b {
            1 => {
                by = top_b;
                bx = p2x + (-half_height_b) / slope_prime;
            }
            2 => {
                bx = right_b;
                by = p2y + half_width_b * slope_prime;
            }
            3 => {
                by = bottom_b;
                bx = p2x + half_height_b / slope_prime;
            }
            _ => {
                bx = left_b;
                by = p2y + (-half_width_b) * slope_prime;
            }
        }
    }

    (ax, ay, bx, by)
}

#[inline]
fn get_cardinal_direction(slope: f64, slope_prime: f64, line: i32) -> i32 {
    if slope > slope_prime {
        line
    } else {
        1 + (line % 4)
    }
}
