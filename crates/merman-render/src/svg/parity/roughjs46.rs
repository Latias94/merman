//! Rough.js 4.6.x-compatible path helpers.
//!
//! Mermaid uses Rough.js for some shapes even in non-handDrawn look (roughness=0). In that mode,
//! Rough.js still consumes RNG values and produces Bézier curves (via `bcurveTo`) rather than
//! simple `L` segments. The output is deterministic when `seed` is pinned.
//!
//! This module implements the specific subset needed for Mermaid parity rendering of polyline
//! paths (MoveTo + LineTo + Close), matching Rough.js 4.6.6 `linearPath(...)` / `_doubleLine(...)`
//! behavior and its Park–Miller-style RNG stream.

use std::fmt::Write as _;

#[derive(Debug, Clone)]
struct Random {
    seed: i64,
}

impl Random {
    fn new(seed: i64) -> Self {
        Self { seed }
    }

    fn next_f64(&mut self) -> f64 {
        // Rough.js `Random.next()`:
        //   ((2**31 - 1) & (seed = Math.imul(48271, seed))) / 2**31
        //
        // Note: Rough.js uses Math.random() when seed is zero; for parity rendering we always
        // pin a non-zero seed.
        if self.seed == 0 {
            return 0.0;
        }
        let next = (self.seed * 48271) & 0x7fffffff;
        self.seed = next;
        (next as f64) / 2147483648.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpType {
    Move,
    BCurveTo,
    LineTo,
}

#[derive(Debug, Clone)]
struct Op {
    op: OpType,
    data: [f64; 6],
    len: usize,
}

impl Op {
    fn moveto(x: f64, y: f64) -> Self {
        Self {
            op: OpType::Move,
            data: [x, y, 0.0, 0.0, 0.0, 0.0],
            len: 2,
        }
    }

    fn lineto(x: f64, y: f64) -> Self {
        Self {
            op: OpType::LineTo,
            data: [x, y, 0.0, 0.0, 0.0, 0.0],
            len: 2,
        }
    }

    fn bcurveto(x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> Self {
        Self {
            op: OpType::BCurveTo,
            data: [x1, y1, x2, y2, x, y],
            len: 6,
        }
    }
}

#[derive(Debug, Clone)]
struct RoughJs46Options {
    // Public knobs (subset)
    max_randomness_offset: f64,
    roughness: f64,
    bowing: f64,
    seed: i64,
    disable_multi_stroke: bool,
    disable_multi_stroke_fill: bool,
    preserve_vertices: bool,

    // Internal state: created lazily and advanced across calls
    randomizer: Option<Random>,
}

impl RoughJs46Options {
    fn new(seed: u64) -> Self {
        Self {
            max_randomness_offset: 2.0,
            roughness: 0.0,
            bowing: 1.0,
            seed: seed as i64,
            disable_multi_stroke: false,
            disable_multi_stroke_fill: false,
            preserve_vertices: false,
            randomizer: None,
        }
    }

    fn random(&mut self) -> f64 {
        if self.randomizer.is_none() {
            let seed = if self.seed == 0 { 0 } else { self.seed };
            self.randomizer = Some(Random::new(seed));
        }
        self.randomizer
            .as_mut()
            .map(|r| r.next_f64())
            .unwrap_or(0.0)
    }
}

fn offset(min: f64, max: f64, o: &mut RoughJs46Options, roughness_gain: f64) -> f64 {
    // Rough.js `_offset(...)` always consumes RNG, even when `roughness == 0`.
    let r = o.random();
    o.roughness * roughness_gain * ((r * (max - min)) + min)
}

fn offset_opt(x: f64, o: &mut RoughJs46Options, roughness_gain: f64) -> f64 {
    offset(-x, x, o, roughness_gain)
}

fn line_ops(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    o: &mut RoughJs46Options,
    move_to: bool,
    overlay: bool,
) -> Vec<Op> {
    // Port of Rough.js 4.6.6 `renderer._line(...)`.
    let length_sq = (x1 - x2).powi(2) + (y1 - y2).powi(2);
    let length = length_sq.sqrt();
    let roughness_gain = if length < 200.0 {
        1.0
    } else if length > 500.0 {
        0.4
    } else {
        (-0.0016668) * length + 1.233334
    };

    let mut off = o.max_randomness_offset.max(0.0);
    if (off * off * 100.0) > length_sq {
        off = length / 10.0;
    }
    let half_off = off / 2.0;

    let diverge_point = 0.2 + o.random() * 0.2;

    let mut mid_disp_x = o.bowing * o.max_randomness_offset * (y2 - y1) / 200.0;
    let mut mid_disp_y = o.bowing * o.max_randomness_offset * (x1 - x2) / 200.0;
    mid_disp_x = offset_opt(mid_disp_x, o, roughness_gain);
    mid_disp_y = offset_opt(mid_disp_y, o, roughness_gain);

    let preserve_vertices = o.preserve_vertices;

    let mut ops: Vec<Op> = Vec::new();
    if move_to {
        if overlay {
            let dx = if preserve_vertices {
                0.0
            } else {
                offset_opt(half_off, o, roughness_gain)
            };
            let dy = if preserve_vertices {
                0.0
            } else {
                offset_opt(half_off, o, roughness_gain)
            };
            ops.push(Op::moveto(x1 + dx, y1 + dy));
        } else {
            let dx = if preserve_vertices {
                0.0
            } else {
                offset_opt(off, o, roughness_gain)
            };
            let dy = if preserve_vertices {
                0.0
            } else {
                offset_opt(off, o, roughness_gain)
            };
            ops.push(Op::moveto(x1 + dx, y1 + dy));
        }
    }

    let (rand_a, _rand_b) = if overlay {
        (half_off, half_off)
    } else {
        (off, off)
    };
    let rand = |o: &mut RoughJs46Options| -> f64 {
        if preserve_vertices {
            0.0
        } else {
            offset_opt(rand_a, o, roughness_gain)
        }
    };

    let c1x = mid_disp_x + x1 + (x2 - x1) * diverge_point + rand(o);
    let c1y = mid_disp_y + y1 + (y2 - y1) * diverge_point + rand(o);
    let c2x = mid_disp_x + x1 + 2.0 * (x2 - x1) * diverge_point + rand(o);
    let c2y = mid_disp_y + y1 + 2.0 * (y2 - y1) * diverge_point + rand(o);
    let ex = x2 + rand(o);
    let ey = y2 + rand(o);
    ops.push(Op::bcurveto(c1x, c1y, c2x, c2y, ex, ey));
    ops
}

fn double_line_ops(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    o: &mut RoughJs46Options,
    filling: bool,
) -> Vec<Op> {
    // Port of Rough.js 4.6.6 `renderer._doubleLine(...)`.
    let single_stroke = if filling {
        o.disable_multi_stroke_fill
    } else {
        o.disable_multi_stroke
    };
    let mut ops = line_ops(x1, y1, x2, y2, o, true, false);
    if !single_stroke {
        ops.extend(line_ops(x1, y1, x2, y2, o, true, true));
    }
    ops
}

fn linear_path_ops(points: &[(f64, f64)], close: bool, o: &mut RoughJs46Options) -> Vec<Op> {
    // Rough.js `linearPath(points, close, o)` for `len > 2`.
    let len = points.len();
    if len < 2 {
        return Vec::new();
    }
    if len == 2 {
        let (x1, y1) = points[0];
        let (x2, y2) = points[1];
        return double_line_ops(x1, y1, x2, y2, o, false);
    }

    let mut ops: Vec<Op> = Vec::new();
    for i in 0..(len - 1) {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        ops.extend(double_line_ops(x1, y1, x2, y2, o, false));
    }
    if close {
        let (x1, y1) = points[len - 1];
        let (x2, y2) = points[0];
        ops.extend(double_line_ops(x1, y1, x2, y2, o, false));
    }
    ops
}

fn merged_shape_ops(input: Vec<Op>) -> Vec<Op> {
    // Rough.js `generator._mergedShape(...)`: keep first op, drop subsequent `move` ops.
    input
        .into_iter()
        .enumerate()
        .filter(|(i, op)| *i == 0 || op.op != OpType::Move)
        .map(|(_, op)| op)
        .collect()
}

fn ops_to_path_d(ops: &[Op]) -> String {
    let mut out = String::new();
    for op in ops {
        match op.op {
            OpType::Move => {
                let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
            }
            OpType::LineTo => {
                let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
            }
            OpType::BCurveTo => {
                let _ = write!(
                    &mut out,
                    "C{} {}, {} {}, {} {} ",
                    op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                );
            }
        }
    }
    out.trim_end().to_string()
}

/// Rough.js 4.6.6 `generator.path(...)` (solid fill, sets.length==1) for a polyline path.
///
/// - `points` are the vertex list implied by `M ... L ... Z`.
/// - Returns `(fill_d, stroke_d)` where `stroke_d` is `None` if `has_stroke == false`.
pub(in crate::svg::parity) fn roughjs46_solid_fill_paths_for_closed_polyline_path(
    points: &[(f64, f64)],
    seed: u64,
    has_stroke: bool,
) -> (String, Option<String>) {
    // First pass: `shape = svgPath(d, o)` (even if stroke is `none`).
    let mut o = RoughJs46Options::new(seed);
    let stroke_ops = linear_path_ops(points, true, &mut o);
    let stroke_d = has_stroke.then(|| ops_to_path_d(&stroke_ops));

    // Fill pass: `fillShape = svgPath(d, { ...o, disableMultiStroke: true, roughness: 0 })`
    let mut fill_o = o.clone();
    fill_o.disable_multi_stroke = true;
    let fill_ops = merged_shape_ops(linear_path_ops(points, true, &mut fill_o));
    (ops_to_path_d(&fill_ops), stroke_d)
}
