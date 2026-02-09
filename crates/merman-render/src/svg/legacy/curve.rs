use super::*;

pub(super) fn curve_monotone_path_d(points: &[crate::model::LayoutPoint], swap_xy: bool) -> String {
    fn sign(v: f64) -> f64 {
        if v < 0.0 { -1.0 } else { 1.0 }
    }

    fn get_x(p: &crate::model::LayoutPoint, swap_xy: bool) -> f64 {
        if swap_xy { p.y } else { p.x }
    }
    fn get_y(p: &crate::model::LayoutPoint, swap_xy: bool) -> f64 {
        if swap_xy { p.x } else { p.y }
    }

    fn emit_move_to(out: &mut String, x: f64, y: f64, swap_xy: bool) {
        if swap_xy {
            let _ = write!(out, "M{},{}", fmt_path(y), fmt_path(x));
        } else {
            let _ = write!(out, "M{},{}", fmt_path(x), fmt_path(y));
        }
    }
    fn emit_line_to(out: &mut String, x: f64, y: f64, swap_xy: bool) {
        if swap_xy {
            let _ = write!(out, "L{},{}", fmt_path(y), fmt_path(x));
        } else {
            let _ = write!(out, "L{},{}", fmt_path(x), fmt_path(y));
        }
    }
    fn emit_cubic_to(
        out: &mut String,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
        swap_xy: bool,
    ) {
        if swap_xy {
            let _ = write!(
                out,
                "C{},{},{},{},{},{}",
                fmt_path(y1),
                fmt_path(x1),
                fmt_path(y2),
                fmt_path(x2),
                fmt_path(y),
                fmt_path(x)
            );
        } else {
            let _ = write!(
                out,
                "C{},{},{},{},{},{}",
                fmt_path(x1),
                fmt_path(y1),
                fmt_path(x2),
                fmt_path(y2),
                fmt_path(x),
                fmt_path(y)
            );
        }
    }

    fn slope3(x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let h0 = x1 - x0;
        let h1 = x2 - x1;
        let denom0 = if h0 != 0.0 {
            h0
        } else if h1 < 0.0 {
            -0.0
        } else {
            0.0
        };
        let denom1 = if h1 != 0.0 {
            h1
        } else if h0 < 0.0 {
            -0.0
        } else {
            0.0
        };
        let s0 = (y1 - y0) / denom0;
        let s1 = (y2 - y1) / denom1;
        let p = (s0 * h1 + s1 * h0) / (h0 + h1);
        let v = (sign(s0) + sign(s1)) * s0.abs().min(s1.abs()).min(0.5 * p.abs());
        if v.is_finite() { v } else { 0.0 }
    }

    fn slope2(x0: f64, y0: f64, x1: f64, y1: f64, t: f64) -> f64 {
        let h = x1 - x0;
        if h != 0.0 {
            (3.0 * (y1 - y0) / h - t) / 2.0
        } else {
            t
        }
    }

    fn hermite_segment(
        out: &mut String,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        t0: f64,
        t1: f64,
        swap_xy: bool,
    ) {
        // dx is in the monotone coordinate system; we swap at emit-time if needed.
        let dx = (x1 - x0) / 3.0;
        emit_cubic_to(
            out,
            x0 + dx,
            y0 + dx * t0,
            x1 - dx,
            y1 - dx * t1,
            x1,
            y1,
            swap_xy,
        );
    }

    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let mut point_state: u8 = 0;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;
    let mut t0 = f64::NAN;

    for p in points {
        let x = get_x(p, swap_xy);
        let y = get_y(p, swap_xy);

        if x == x1 && y == y1 {
            continue;
        }

        let mut t1 = f64::NAN;
        match point_state {
            0 => {
                point_state = 1;
                emit_move_to(&mut out, x, y, swap_xy);
            }
            1 => {
                point_state = 2;
            }
            2 => {
                point_state = 3;
                t1 = slope3(x0, y0, x1, y1, x, y);
                let t0_local = slope2(x0, y0, x1, y1, t1);
                hermite_segment(&mut out, x0, y0, x1, y1, t0_local, t1, swap_xy);
            }
            _ => {
                t1 = slope3(x0, y0, x1, y1, x, y);
                hermite_segment(&mut out, x0, y0, x1, y1, t0, t1, swap_xy);
            }
        }

        x0 = x1;
        y0 = y1;
        x1 = x;
        y1 = y;
        t0 = t1;
    }

    match point_state {
        2 => emit_line_to(&mut out, x1, y1, swap_xy),
        3 => {
            let t1 = slope2(x0, y0, x1, y1, t0);
            hermite_segment(&mut out, x0, y0, x1, y1, t0, t1, swap_xy);
        }
        _ => {}
    }

    out
}

fn curve_monotone_x_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, false)
}

fn curve_monotone_y_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, true)
}

// Ported from D3 `curveBasis` (d3-shape v3.x), used by Mermaid ER renderer `@11.12.2`.
pub(super) fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;

    fn basis_point(out: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
        let c1x = (2.0 * x0 + x1) / 3.0;
        let c1y = (2.0 * y0 + y1) / 3.0;
        let c2x = (x0 + 2.0 * x1) / 3.0;
        let c2y = (y0 + 2.0 * y1) / 3.0;
        let ex = (x0 + 4.0 * x1 + x) / 6.0;
        let ey = (y0 + 4.0 * y1 + y) / 6.0;
        let _ = write!(
            out,
            "C{},{},{},{},{},{}",
            fmt_path(c1x),
            fmt_path(c1y),
            fmt_path(c2x),
            fmt_path(c2y),
            fmt_path(ex),
            fmt_path(ey)
        );
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                let _ = write!(&mut out, "M{},{}", fmt_path(x), fmt_path(y));
            }
            1 => {
                p = 2;
            }
            2 => {
                p = 3;
                let lx = (5.0 * x0 + x1) / 6.0;
                let ly = (5.0 * y0 + y1) / 6.0;
                let _ = write!(&mut out, "L{},{}", fmt_path(lx), fmt_path(ly));
                basis_point(&mut out, x0, y0, x1, y1, x, y);
            }
            _ => {
                basis_point(&mut out, x0, y0, x1, y1, x, y);
            }
        }
        x0 = x1;
        x1 = x;
        y0 = y1;
        y1 = y;
    }

    match p {
        3 => {
            basis_point(&mut out, x0, y0, x1, y1, x1, y1);
            let _ = write!(&mut out, "L{},{}", fmt_path(x1), fmt_path(y1));
        }
        2 => {
            let _ = write!(&mut out, "L{},{}", fmt_path(x1), fmt_path(y1));
        }
        _ => {}
    }

    out
}

pub(super) fn curve_linear_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
    }
    out
}

// Ported from D3 `curveStepAfter` (d3-shape v3.x).
pub(super) fn curve_step_after_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_y = first.y;
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(prev_y));
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
        prev_y = p.y;
    }
    out
}

// Ported from D3 `curveStepBefore` (d3-shape v3.x).
pub(super) fn curve_step_before_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_x = first.x;
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    for p in points.iter().skip(1) {
        let _ = write!(&mut out, "L{},{}", fmt_path(prev_x), fmt_path(p.y));
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
        prev_x = p.x;
    }
    out
}

// Ported from D3 `curveStep` (d3-shape v3.x).
pub(super) fn curve_step_path_d(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };
    let _ = write!(&mut out, "M{},{}", fmt_path(first.x), fmt_path(first.y));
    let mut prev = first;
    for p in points.iter().skip(1) {
        let mid_x = (prev.x + p.x) / 2.0;
        let _ = write!(&mut out, "L{},{}", fmt_path(mid_x), fmt_path(prev.y));
        let _ = write!(&mut out, "L{},{}", fmt_path(mid_x), fmt_path(p.y));
        let _ = write!(&mut out, "L{},{}", fmt_path(p.x), fmt_path(p.y));
        prev = p;
    }
    out
}

// Ported from D3 `curveCardinal` (d3-shape v3.x).
pub(super) fn curve_cardinal_path_d(points: &[crate::model::LayoutPoint], tension: f64) -> String {
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let k = (1.0 - tension) / 6.0;

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;
    let mut x2 = f64::NAN;
    let mut y2 = f64::NAN;

    fn cardinal_point(
        out: &mut String,
        k: f64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    ) {
        let c1x = x1 + k * (x2 - x0);
        let c1y = y1 + k * (y2 - y0);
        let c2x = x2 + k * (x1 - x);
        let c2y = y2 + k * (y1 - y);
        let _ = write!(
            out,
            "C{},{},{},{},{},{}",
            fmt_path(c1x),
            fmt_path(c1y),
            fmt_path(c2x),
            fmt_path(c2y),
            fmt_path(x2),
            fmt_path(y2)
        );
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                let _ = write!(&mut out, "M{},{}", fmt_path(x), fmt_path(y));
            }
            1 => {
                p = 2;
                x1 = x;
                y1 = y;
            }
            2 => {
                p = 3;
                cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x, y);
            }
            _ => {
                cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x, y);
            }
        }

        x0 = x1;
        x1 = x2;
        x2 = x;
        y0 = y1;
        y1 = y2;
        y2 = y;
    }

    match p {
        2 => {
            let _ = write!(&mut out, "L{},{}", fmt_path(x2), fmt_path(y2));
        }
        3 => {
            cardinal_point(&mut out, k, x0, y0, x1, y1, x2, y2, x1, y1);
        }
        _ => {}
    }

    out
}
