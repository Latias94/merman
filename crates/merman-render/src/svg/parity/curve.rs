#![allow(clippy::too_many_arguments)]

use super::path_bounds::{SvgPathBounds, svg_path_bounds_include_cubic};
use super::*;

#[derive(Debug, Default)]
struct BoundsBuilder {
    bounds: Option<SvgPathBounds>,
    cur: Option<(f64, f64)>,
}

impl BoundsBuilder {
    fn include_point(&mut self, x: f64, y: f64) {
        if let Some(b) = &mut self.bounds {
            b.min_x = b.min_x.min(x);
            b.min_y = b.min_y.min(y);
            b.max_x = b.max_x.max(x);
            b.max_y = b.max_y.max(y);
        } else {
            self.bounds = Some(SvgPathBounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            });
        }
    }

    fn on_pair(&mut self, cmd: char, x: f64, y: f64) {
        match cmd {
            'M' | 'L' => {
                self.include_point(x, y);
                self.cur = Some((x, y));
            }
            _ => {}
        }
    }

    fn on_cubic(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        let Some((x0, y0)) = self.cur else {
            // Defensive: cubic without a current point is invalid; fall back to including end point.
            self.include_point(x, y);
            self.cur = Some((x, y));
            return;
        };

        if self.bounds.is_none() {
            self.bounds = Some(SvgPathBounds {
                min_x: x0,
                min_y: y0,
                max_x: x0,
                max_y: y0,
            });
        }
        if let Some(b) = &mut self.bounds {
            svg_path_bounds_include_cubic(b, x0, y0, x1, y1, x2, y2, x, y);
        }
        self.cur = Some((x, y));
    }
}

#[allow(dead_code)]
fn emit_cmd_pair(out: &mut String, cmd: char, x: f64, y: f64) {
    emit_cmd_pair_impl(out, None, cmd, x, y);
}

fn emit_cmd_pair_impl(
    out: &mut String,
    bounds: Option<&mut BoundsBuilder>,
    cmd: char,
    x: f64,
    y: f64,
) {
    out.push(cmd);
    fmt_path_into(out, x);
    out.push(',');
    fmt_path_into(out, y);
    if let Some(b) = bounds {
        b.on_pair(cmd, x, y);
    }
}

#[allow(dead_code)]
fn emit_cmd_cubic(out: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
    emit_cmd_cubic_impl(out, None, x1, y1, x2, y2, x, y);
}

fn emit_cmd_cubic_impl(
    out: &mut String,
    bounds: Option<&mut BoundsBuilder>,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
) {
    out.push('C');
    fmt_path_into(out, x1);
    out.push(',');
    fmt_path_into(out, y1);
    out.push(',');
    fmt_path_into(out, x2);
    out.push(',');
    fmt_path_into(out, y2);
    out.push(',');
    fmt_path_into(out, x);
    out.push(',');
    fmt_path_into(out, y);
    if let Some(b) = bounds {
        b.on_cubic(x1, y1, x2, y2, x, y);
    }
}

pub(super) fn curve_monotone_path_d(points: &[crate::model::LayoutPoint], swap_xy: bool) -> String {
    curve_monotone_path_d_impl(points, swap_xy, None)
}

pub(super) fn curve_monotone_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
    swap_xy: bool,
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_monotone_path_d_impl(points, swap_xy, Some(&mut b));
    (d, b.bounds)
}

fn curve_monotone_path_d_impl(
    points: &[crate::model::LayoutPoint],
    swap_xy: bool,
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    fn sign(v: f64) -> f64 {
        if v < 0.0 { -1.0 } else { 1.0 }
    }

    fn get_x(p: &crate::model::LayoutPoint, swap_xy: bool) -> f64 {
        if swap_xy { p.y } else { p.x }
    }
    fn get_y(p: &crate::model::LayoutPoint, swap_xy: bool) -> f64 {
        if swap_xy { p.x } else { p.y }
    }

    fn emit_move_to(
        out: &mut String,
        x: f64,
        y: f64,
        swap_xy: bool,
        bounds: Option<&mut BoundsBuilder>,
    ) {
        if swap_xy {
            emit_cmd_pair_impl(out, bounds, 'M', y, x);
        } else {
            emit_cmd_pair_impl(out, bounds, 'M', x, y);
        }
    }
    fn emit_line_to(
        out: &mut String,
        x: f64,
        y: f64,
        swap_xy: bool,
        bounds: Option<&mut BoundsBuilder>,
    ) {
        if swap_xy {
            emit_cmd_pair_impl(out, bounds, 'L', y, x);
        } else {
            emit_cmd_pair_impl(out, bounds, 'L', x, y);
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
        bounds: Option<&mut BoundsBuilder>,
    ) {
        if swap_xy {
            emit_cmd_cubic_impl(out, bounds, y1, x1, y2, x2, y, x);
        } else {
            emit_cmd_cubic_impl(out, bounds, x1, y1, x2, y2, x, y);
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
        bounds: Option<&mut BoundsBuilder>,
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
            bounds,
        );
    }

    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(64));
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
                emit_move_to(&mut out, x, y, swap_xy, bounds.as_deref_mut());
            }
            1 => {
                point_state = 2;
            }
            2 => {
                point_state = 3;
                t1 = slope3(x0, y0, x1, y1, x, y);
                let t0_local = slope2(x0, y0, x1, y1, t1);
                hermite_segment(
                    &mut out,
                    x0,
                    y0,
                    x1,
                    y1,
                    t0_local,
                    t1,
                    swap_xy,
                    bounds.as_deref_mut(),
                );
            }
            _ => {
                t1 = slope3(x0, y0, x1, y1, x, y);
                hermite_segment(
                    &mut out,
                    x0,
                    y0,
                    x1,
                    y1,
                    t0,
                    t1,
                    swap_xy,
                    bounds.as_deref_mut(),
                );
            }
        }

        x0 = x1;
        y0 = y1;
        x1 = x;
        y1 = y;
        t0 = t1;
    }

    match point_state {
        2 => emit_line_to(&mut out, x1, y1, swap_xy, bounds.as_deref_mut()),
        3 => {
            let t1 = slope2(x0, y0, x1, y1, t0);
            hermite_segment(
                &mut out,
                x0,
                y0,
                x1,
                y1,
                t0,
                t1,
                swap_xy,
                bounds.as_deref_mut(),
            );
        }
        _ => {}
    }

    out
}

#[allow(dead_code)]
fn curve_monotone_x_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, false)
}

#[allow(dead_code)]
fn curve_monotone_y_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_monotone_path_d(points, true)
}

// Ported from D3 `curveBasis` (d3-shape v3.x), used by Mermaid ER renderer `@11.12.2`.
pub(super) fn curve_basis_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_basis_path_d_impl(points, None)
}

pub(super) fn curve_basis_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_basis_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_basis_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(64));
    if points.is_empty() {
        return out;
    }

    let mut p = 0u8;
    let mut x0 = f64::NAN;
    let mut y0 = f64::NAN;
    let mut x1 = f64::NAN;
    let mut y1 = f64::NAN;

    fn basis_point(
        out: &mut String,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x: f64,
        y: f64,
        bounds: Option<&mut BoundsBuilder>,
    ) {
        let c1x = (2.0 * x0 + x1) / 3.0;
        let c1y = (2.0 * y0 + y1) / 3.0;
        let c2x = (x0 + 2.0 * x1) / 3.0;
        let c2y = (y0 + 2.0 * y1) / 3.0;
        let ex = (x0 + 4.0 * x1 + x) / 6.0;
        let ey = (y0 + 4.0 * y1 + y) / 6.0;
        emit_cmd_cubic_impl(out, bounds, c1x, c1y, c2x, c2y, ex, ey);
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', x, y);
            }
            1 => {
                p = 2;
            }
            2 => {
                p = 3;
                let lx = (5.0 * x0 + x1) / 6.0;
                let ly = (5.0 * y0 + y1) / 6.0;
                emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', lx, ly);
                basis_point(&mut out, x0, y0, x1, y1, x, y, bounds.as_deref_mut());
            }
            _ => {
                basis_point(&mut out, x0, y0, x1, y1, x, y, bounds.as_deref_mut());
            }
        }
        x0 = x1;
        x1 = x;
        y0 = y1;
        y1 = y;
    }

    match p {
        3 => {
            basis_point(&mut out, x0, y0, x1, y1, x1, y1, bounds.as_deref_mut());
            emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', x1, y1);
        }
        2 => {
            emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', x1, y1);
        }
        _ => {}
    }

    out
}

pub(super) fn curve_linear_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_linear_path_d_impl(points, None)
}

pub(super) fn curve_linear_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_linear_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_linear_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(32));
    let Some(first) = points.first() else {
        return out;
    };
    emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', first.x, first.y);
    for p in points.iter().skip(1) {
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', p.x, p.y);
    }
    out
}

// Ported from D3 `curveNatural` (d3-shape v3.x).
//
// This is used by Mermaid flowchart edge-id curve overrides (e.g. `e1@{ curve: natural }`).
pub(super) fn curve_natural_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_natural_path_d_impl(points, None)
}

pub(super) fn curve_natural_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_natural_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_natural_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    fn compute_control_points(coords: &[f64]) -> (Vec<f64>, Vec<f64>) {
        // `coords` contains the knot coordinates for points[0..=n], where n = segment count.
        let n = coords.len().saturating_sub(1);
        let mut c1 = vec![0.0f64; n];
        let mut c2 = vec![0.0f64; n];
        if n == 0 {
            return (c1, c2);
        }

        // Tridiagonal solve for first control points (Thomas algorithm).
        let mut a = vec![0.0f64; n];
        let mut b = vec![0.0f64; n];
        let mut c = vec![0.0f64; n];
        let mut rhs = vec![0.0f64; n];

        b[0] = 2.0;
        c[0] = 1.0;
        rhs[0] = coords[0] + 2.0 * coords[1];

        for i in 1..n.saturating_sub(1) {
            a[i] = 1.0;
            b[i] = 4.0;
            c[i] = 1.0;
            rhs[i] = 4.0 * coords[i] + 2.0 * coords[i + 1];
        }

        if n > 1 {
            a[n - 1] = 2.0;
            b[n - 1] = 7.0;
            rhs[n - 1] = 8.0 * coords[n - 1] + coords[n];
        } else {
            // Single segment (2 points): not used (caller returns a line), but keep the solver
            // stable anyway.
            b[0] = 2.0;
            rhs[0] = coords[0] + coords[1];
        }

        for i in 1..n {
            let m = a[i] / b[i - 1];
            b[i] -= m * c[i - 1];
            rhs[i] -= m * rhs[i - 1];
        }

        c1[n - 1] = rhs[n - 1] / b[n - 1];
        for i in (0..n.saturating_sub(1)).rev() {
            c1[i] = (rhs[i] - c[i] * c1[i + 1]) / b[i];
        }

        for i in 0..n.saturating_sub(1) {
            c2[i] = 2.0 * coords[i + 1] - c1[i + 1];
        }
        c2[n - 1] = (coords[n] + c1[n - 1]) / 2.0;

        (c1, c2)
    }

    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(64));
    let Some(first) = points.first() else {
        return out;
    };
    emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', first.x, first.y);
    if points.len() == 1 {
        return out;
    }
    if points.len() == 2 {
        let p1 = &points[1];
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', p1.x, p1.y);
        return out;
    }

    let mut xs: Vec<f64> = Vec::with_capacity(points.len());
    let mut ys: Vec<f64> = Vec::with_capacity(points.len());
    for p in points {
        xs.push(p.x);
        ys.push(p.y);
    }

    let (x1, x2) = compute_control_points(&xs);
    let (y1, y2) = compute_control_points(&ys);

    for i in 0..points.len().saturating_sub(1) {
        let p = &points[i + 1];
        emit_cmd_cubic_impl(
            &mut out,
            bounds.as_deref_mut(),
            x1[i],
            y1[i],
            x2[i],
            y2[i],
            p.x,
            p.y,
        );
    }

    out
}

// Ported from D3 `curveStepAfter` (d3-shape v3.x).
pub(super) fn curve_step_after_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_step_after_path_d_impl(points, None)
}

pub(super) fn curve_step_after_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_step_after_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_step_after_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(32));
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_y = first.y;
    emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', first.x, first.y);
    for p in points.iter().skip(1) {
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', p.x, prev_y);
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', p.x, p.y);
        prev_y = p.y;
    }
    out
}

// Ported from D3 `curveStepBefore` (d3-shape v3.x).
pub(super) fn curve_step_before_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_step_before_path_d_impl(points, None)
}

pub(super) fn curve_step_before_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_step_before_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_step_before_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(32));
    let Some(first) = points.first() else {
        return out;
    };
    let mut prev_x = first.x;
    emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', first.x, first.y);
    for p in points.iter().skip(1) {
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', prev_x, p.y);
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', p.x, p.y);
        prev_x = p.x;
    }
    out
}

// Ported from D3 `curveStep` (d3-shape v3.x).
pub(super) fn curve_step_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_step_path_d_impl(points, None)
}

pub(super) fn curve_step_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_step_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_step_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(32));
    let Some(first) = points.first() else {
        return out;
    };
    emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', first.x, first.y);
    let mut prev = first;
    for p in points.iter().skip(1) {
        let mid_x = (prev.x + p.x) / 2.0;
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', mid_x, prev.y);
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', mid_x, p.y);
        emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', p.x, p.y);
        prev = p;
    }
    out
}

// Ported from D3 `curveCardinal` (d3-shape v3.x).
pub(super) fn curve_cardinal_path_d(points: &[crate::model::LayoutPoint], tension: f64) -> String {
    curve_cardinal_path_d_impl(points, tension, None)
}

pub(super) fn curve_cardinal_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
    tension: f64,
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_cardinal_path_d_impl(points, tension, Some(&mut b));
    (d, b.bounds)
}

fn curve_cardinal_path_d_impl(
    points: &[crate::model::LayoutPoint],
    tension: f64,
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::with_capacity(points.len().saturating_mul(64));
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
        bounds: Option<&mut BoundsBuilder>,
    ) {
        let c1x = x1 + k * (x2 - x0);
        let c1y = y1 + k * (y2 - y0);
        let c2x = x2 + k * (x1 - x);
        let c2y = y2 + k * (y1 - y);
        emit_cmd_cubic_impl(out, bounds, c1x, c1y, c2x, c2y, x2, y2);
    }

    for pt in points {
        let x = pt.x;
        let y = pt.y;
        match p {
            0 => {
                p = 1;
                emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', x, y);
            }
            1 => {
                p = 2;
                x1 = x;
                y1 = y;
            }
            2 => {
                p = 3;
                cardinal_point(
                    &mut out,
                    k,
                    x0,
                    y0,
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                    bounds.as_deref_mut(),
                );
            }
            _ => {
                cardinal_point(
                    &mut out,
                    k,
                    x0,
                    y0,
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                    bounds.as_deref_mut(),
                );
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
            emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'L', x2, y2);
        }
        3 => {
            cardinal_point(
                &mut out,
                k,
                x0,
                y0,
                x1,
                y1,
                x2,
                y2,
                x1,
                y1,
                bounds.as_deref_mut(),
            );
        }
        _ => {}
    }

    out
}

// Ported from D3 `curveBumpY` (d3-shape v3.x).
//
// This is used by Mermaid flowchart edge-id curve overrides (e.g. `e1@{ curve: bumpY }`).
pub(super) fn curve_bump_y_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_bump_y_path_d_impl(points, None)
}

pub(super) fn curve_bump_y_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_bump_y_path_d_impl(points, Some(&mut b));
    (d, b.bounds)
}

fn curve_bump_y_path_d_impl(
    points: &[crate::model::LayoutPoint],
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    let mut bounds = bounds;
    let mut out = String::new();
    let Some(first) = points.first() else {
        return out;
    };

    let mut x0 = first.x;
    let mut y0 = first.y;
    emit_cmd_pair_impl(&mut out, bounds.as_deref_mut(), 'M', x0, y0);

    for p in points.iter().skip(1) {
        let x = p.x;
        let y = p.y;
        let y_mid = (y0 + y) / 2.0;
        emit_cmd_cubic_impl(&mut out, bounds.as_deref_mut(), x0, y_mid, x, y_mid, x, y);
        x0 = x;
        y0 = y;
    }

    out
}

// Ported from D3 `curveCatmullRom` (d3-shape v3.x), with the default alpha=0.5.
//
// This is used by Mermaid flowchart edge-id curve overrides (e.g. `e1@{ curve: catmullRom }`).
pub(super) fn curve_catmull_rom_path_d(points: &[crate::model::LayoutPoint]) -> String {
    curve_catmull_rom_path_d_with_alpha(points, 0.5)
}

pub(super) fn curve_catmull_rom_path_d_and_bounds(
    points: &[crate::model::LayoutPoint],
) -> (String, Option<SvgPathBounds>) {
    let mut b = BoundsBuilder::default();
    let d = curve_catmull_rom_path_d_with_alpha_impl(points, 0.5, Some(&mut b));
    (d, b.bounds)
}

fn curve_catmull_rom_path_d_with_alpha(points: &[crate::model::LayoutPoint], alpha: f64) -> String {
    curve_catmull_rom_path_d_with_alpha_impl(points, alpha, None)
}

fn curve_catmull_rom_path_d_with_alpha_impl(
    points: &[crate::model::LayoutPoint],
    alpha: f64,
    bounds: Option<&mut BoundsBuilder>,
) -> String {
    const EPSILON: f64 = 1e-12;

    #[derive(Debug, Clone, Copy)]
    struct CatmullRomState {
        alpha: f64,
        point_state: u8,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        l01_a: f64,
        l12_a: f64,
        l23_a: f64,
        l01_2a: f64,
        l12_2a: f64,
        l23_2a: f64,
    }

    impl CatmullRomState {
        fn new(alpha: f64) -> Self {
            Self {
                alpha,
                point_state: 0,
                x0: f64::NAN,
                y0: f64::NAN,
                x1: f64::NAN,
                y1: f64::NAN,
                x2: f64::NAN,
                y2: f64::NAN,
                l01_a: 0.0,
                l12_a: 0.0,
                l23_a: 0.0,
                l01_2a: 0.0,
                l12_2a: 0.0,
                l23_2a: 0.0,
            }
        }

        fn emit_segment(
            &self,
            out: &mut String,
            x: f64,
            y: f64,
            bounds: Option<&mut BoundsBuilder>,
        ) {
            let mut x1 = self.x1;
            let mut y1 = self.y1;
            let mut x2 = self.x2;
            let mut y2 = self.y2;

            if self.l01_a > EPSILON {
                let a = 2.0 * self.l01_2a + 3.0 * self.l01_a * self.l12_a + self.l12_2a;
                let n = 3.0 * self.l01_a * (self.l01_a + self.l12_a);
                if n != 0.0 && n.is_finite() {
                    x1 = (x1 * a - self.x0 * self.l12_2a + self.x2 * self.l01_2a) / n;
                    y1 = (y1 * a - self.y0 * self.l12_2a + self.y2 * self.l01_2a) / n;
                }
            }

            if self.l23_a > EPSILON {
                let b = 2.0 * self.l23_2a + 3.0 * self.l23_a * self.l12_a + self.l12_2a;
                let m = 3.0 * self.l23_a * (self.l23_a + self.l12_a);
                if m != 0.0 && m.is_finite() {
                    // Note: D3 uses the original (unadjusted) `_x1/_y1` here.
                    x2 = (x2 * b + self.x1 * self.l23_2a - x * self.l12_2a) / m;
                    y2 = (y2 * b + self.y1 * self.l23_2a - y * self.l12_2a) / m;
                }
            }

            emit_cmd_cubic_impl(out, bounds, x1, y1, x2, y2, self.x2, self.y2);
        }

        fn point(&mut self, out: &mut String, x: f64, y: f64, bounds: Option<&mut BoundsBuilder>) {
            if self.point_state != 0 {
                let dx = self.x2 - x;
                let dy = self.y2 - y;
                self.l23_2a = (dx * dx + dy * dy).powf(self.alpha);
                self.l23_a = self.l23_2a.sqrt();
            }

            match self.point_state {
                0 => {
                    self.point_state = 1;
                    emit_cmd_pair_impl(out, bounds, 'M', x, y);
                }
                1 => {
                    self.point_state = 2;
                }
                2 => {
                    self.point_state = 3;
                    self.emit_segment(out, x, y, bounds);
                }
                _ => {
                    self.emit_segment(out, x, y, bounds);
                }
            }

            self.l01_a = self.l12_a;
            self.l12_a = self.l23_a;
            self.l01_2a = self.l12_2a;
            self.l12_2a = self.l23_2a;

            self.x0 = self.x1;
            self.x1 = self.x2;
            self.x2 = x;
            self.y0 = self.y1;
            self.y1 = self.y2;
            self.y2 = y;
        }

        fn line_end(&mut self, out: &mut String, bounds: Option<&mut BoundsBuilder>) {
            match self.point_state {
                2 => {
                    emit_cmd_pair_impl(out, bounds, 'L', self.x2, self.y2);
                }
                3 => {
                    // Mirror D3's `lineEnd` behavior: `this.point(this._x2, this._y2)`.
                    self.l23_a = 0.0;
                    self.l23_2a = 0.0;
                    self.emit_segment(out, self.x2, self.y2, bounds);
                }
                _ => {}
            }
        }
    }

    let mut bounds = bounds;
    let mut out = String::new();
    if points.is_empty() {
        return out;
    }

    let mut state = CatmullRomState::new(alpha);
    for p in points {
        state.point(&mut out, p.x, p.y, bounds.as_deref_mut());
    }
    state.line_end(&mut out, bounds.as_deref_mut());

    out
}
