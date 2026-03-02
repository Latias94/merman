use super::super::*;

fn splitmix64_next(state: &mut u64) -> u64 {
    // Deterministic PRNG for "rough-like" stroke paths.
    // (We do not use OS randomness to keep SVG output stable.)
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn splitmix64_f64(state: &mut u64) -> f64 {
    let v = splitmix64_next(state);
    // Convert to [0,1).
    (v as f64) / ((u64::MAX as f64) + 1.0)
}

pub(super) fn class_rough_seed(diagram_id: &str, dom_id: &str) -> u64 {
    // FNV-1a 64-bit.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in diagram_id.as_bytes().iter().chain(dom_id.as_bytes().iter()) {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub(super) fn class_rough_line_double_path_and_bounds(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    mut seed: u64,
) -> (String, super::super::path_bounds::SvgPathBounds) {
    let dx = x2 - x1;
    let dy = y2 - y1;

    fn make_pair(seed: &mut u64, a0: f64, a1: f64, b0: f64, b1: f64) -> (f64, f64) {
        let mut a = a0 + (a1 - a0) * splitmix64_f64(seed);
        let mut b = b0 + (b1 - b0) * splitmix64_f64(seed);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        (a, b)
    }

    let (t1, t2) = make_pair(&mut seed, 0.20, 0.50, 0.55, 0.90);
    let (t3, t4) = make_pair(&mut seed, 0.15, 0.55, 0.40, 0.95);

    let c1x = x1 + dx * t1;
    let c1y = y1 + dy * t1;
    let c2x = x1 + dx * t2;
    let c2y = y1 + dy * t2;

    let c3x = x1 + dx * t3;
    let c3y = y1 + dy * t3;
    let c4x = x1 + dx * t4;
    let c4y = y1 + dy * t4;

    let d = format!(
        "M{} {} C{} {}, {} {}, {} {} M{} {} C{} {}, {} {}, {} {}",
        fmt(x1),
        fmt(y1),
        fmt(c1x),
        fmt(c1y),
        fmt(c2x),
        fmt(c2y),
        fmt(x2),
        fmt(y2),
        fmt(x1),
        fmt(y1),
        fmt(c3x),
        fmt(c3y),
        fmt(c4x),
        fmt(c4y),
        fmt(x2),
        fmt(y2),
    );

    let mut pb = super::super::path_bounds::SvgPathBounds {
        min_x: x1,
        min_y: y1,
        max_x: x1,
        max_y: y1,
    };
    super::super::path_bounds::svg_path_bounds_include_cubic(
        &mut pb, x1, y1, c1x, c1y, c2x, c2y, x2, y2,
    );
    super::super::path_bounds::svg_path_bounds_include_cubic(
        &mut pb, x1, y1, c3x, c3y, c4x, c4y, x2, y2,
    );

    (d, pb)
}

pub(super) fn class_rough_rect_stroke_path_and_bounds(
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    seed: u64,
) -> (String, super::super::path_bounds::SvgPathBounds) {
    let right = left + width;
    let bottom = top + height;

    let mut out = String::new();
    let (d1, mut pb) = class_rough_line_double_path_and_bounds(left, top, right, top, seed ^ 0x01);
    out.push_str(&d1);
    let (d2, pb2) = class_rough_line_double_path_and_bounds(right, top, right, bottom, seed ^ 0x02);
    out.push_str(&d2);
    pb.min_x = pb.min_x.min(pb2.min_x);
    pb.min_y = pb.min_y.min(pb2.min_y);
    pb.max_x = pb.max_x.max(pb2.max_x);
    pb.max_y = pb.max_y.max(pb2.max_y);

    let (d3, pb3) =
        class_rough_line_double_path_and_bounds(right, bottom, left, bottom, seed ^ 0x03);
    out.push_str(&d3);
    pb.min_x = pb.min_x.min(pb3.min_x);
    pb.min_y = pb.min_y.min(pb3.min_y);
    pb.max_x = pb.max_x.max(pb3.max_x);
    pb.max_y = pb.max_y.max(pb3.max_y);

    let (d4, pb4) = class_rough_line_double_path_and_bounds(left, bottom, left, top, seed ^ 0x04);
    out.push_str(&d4);
    pb.min_x = pb.min_x.min(pb4.min_x);
    pb.min_y = pb.min_y.min(pb4.min_y);
    pb.max_x = pb.max_x.max(pb4.max_x);
    pb.max_y = pb.max_y.max(pb4.max_y);

    (out, pb)
}
