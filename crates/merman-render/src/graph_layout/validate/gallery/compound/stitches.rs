use super::{Capture, FLATNESS, Shaft, ancestors};
use kurbo::{BezPath, Line, ParamCurve, ParamCurveNearest, Point, Rect, Shape};

fn on_same_face(rect: Rect, declared: Point, projected: Point) -> bool {
    let tolerance = 1e-6;
    if !rect.inflate(tolerance, tolerance).contains(declared) {
        return false;
    }
    let center = rect.center();
    ((declared.y - projected.y).abs() <= FLATNESS
        && ((declared.x - rect.x0).abs() <= tolerance && projected.x <= center.x + tolerance
            || (declared.x - rect.x1).abs() <= tolerance && projected.x >= center.x - tolerance))
        || ((declared.x - projected.x).abs() <= FLATNESS
            && ((declared.y - rect.y0).abs() <= tolerance && projected.y <= center.y + tolerance
                || (declared.y - rect.y1).abs() <= tolerance
                    && projected.y >= center.y - tolerance))
}

fn at(lines: &[Line], position: f64) -> Point {
    let index = (position.floor() as usize).min(lines.len() - 1);
    lines[index].eval(position - index as f64)
}

pub(super) fn transitions(shaft: &Shaft, outline: &BezPath) -> Vec<(f64, Point, bool)> {
    let mut cuts = Vec::new();
    for (index, line) in shaft.lines.iter().enumerate() {
        for segment in outline.segments() {
            for hit in segment.intersect_line(*line) {
                cuts.push(index as f64 + hit.line_t.clamp(0., 1.));
            }
        }
    }
    cuts.sort_unstable_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    let length = shaft.lines.len() as f64;
    cuts.iter()
        .enumerate()
        .filter_map(|(index, &cut)| {
            let previous = index.checked_sub(1).map_or(0., |i| cuts[i]);
            let next = cuts.get(index + 1).copied().unwrap_or(length);
            let inside = |t| outline.winding(at(&shaft.lines, t)) != 0;
            let (before, after) = (inside(previous.midpoint(cut)), inside(next.midpoint(cut)));
            (before != after).then(|| (cut, at(&shaft.lines, cut), after))
        })
        .collect()
}

fn rough_crossings(shaft: &Shaft, outline: &BezPath) -> Vec<Point> {
    let mut points = Vec::new();
    for line in &shaft.lines {
        for border in outline.segments() {
            for hit in border.intersect_line(*line) {
                let point = line.eval(hit.line_t);
                let tangent = border.eval((hit.segment_t + 1e-5).min(1.))
                    - border.eval((hit.segment_t - 1e-5).max(0.));
                let side = |p| tangent.cross(p - point);
                let mut positive = false;
                let mut negative = false;
                for stroke in &shaft.lines {
                    if stroke.nearest(point, 1e-6).distance_sq <= FLATNESS * FLATNESS {
                        for end in [stroke.p0, stroke.p1] {
                            positive |= side(end) > 0.;
                            negative |= side(end) < 0.;
                        }
                    }
                }
                if positive && negative {
                    points.push(point);
                }
            }
        }
    }
    points
}

pub(super) fn check(capture: &mut Capture<'_>, index: usize, shaft: &Shaft, name: &str) {
    let edge = &capture.scene.edges[index];
    for pair in shaft.segments.windows(2) {
        let gap = pair[0].0.end().distance(pair[1].0.start());
        if gap > 1e-7 {
            capture.failures.push(format!(
                "{name}: disconnected painted subpaths, gap {gap:.6}px"
            ));
        }
    }
    let from = ancestors(&capture.scene, edge.start.body);
    let to = ancestors(&capture.scene, edge.end.body);
    let exits = from
        .iter()
        .filter(|id| **id != edge.end.body && !to.contains(id))
        .map(|&id| (id, false));
    let entries = to
        .iter()
        .rev()
        .filter(|id| **id != edge.start.body && !from.contains(id))
        .map(|&id| (id, true));
    let mut last_cut = -1.;
    let knots = &capture.edge_knots[index];
    let mut next_knot = 1;
    for (group, entering) in exits.chain(entries) {
        capture.counts.stitches += 1;
        let ink = capture.scene.bodies[group].contour.as_ref().unwrap();
        let nominal = capture.rough_group_frames.get(&group);
        let outline = nominal.unwrap_or(ink);
        let owner = &capture.body_names[group];
        let observed = transitions(shaft, outline);
        let [(cut, point, enters)] = observed.as_slice() else {
            capture.failures.push(format!(
                "{name}: group {owner} expected one border crossing, painted {observed:?}"
            ));
            continue;
        };
        if *enters != entering || *cut <= last_cut {
            capture.failures.push(format!(
                "{name}: group {owner} crossed out of semantic order at {point:?}"
            ));
        }
        if nominal.is_some() {
            if let Some(distance) = rough_crossings(shaft, ink)
                .iter()
                .map(|p| p.distance(*point))
                .min_by(f64::total_cmp)
            {
                capture.ink.border_crossing = capture.ink.border_crossing.max(distance);
            } else {
                capture.failures.push(format!(
                    "{name}: group {owner} has no crossing through its painted outline stroke"
                ));
            }
        }
        last_cut = *cut;
        let remaining = &knots[next_knot..knots.len() - 1];
        if let Some(offset) = remaining.iter().enumerate().position(|(offset, raw)| {
            let knot = capture.painted_knots[index][next_knot + offset];
            on_same_face(
                capture.model_boxes[group],
                capture.to_layout * *raw,
                capture.to_layout * knot,
            ) && knot.distance(*point) <= FLATNESS
                && outline
                    .segments()
                    .any(|s| s.nearest(knot, 1e-6).distance_sq <= FLATNESS * FLATNESS)
        }) {
            next_knot += offset + 1;
        } else {
            let nearest = remaining
                .iter()
                .min_by(|a, b| a.distance(*point).total_cmp(&b.distance(*point)));
            capture.failures.push(format!(
                    "{name}: group {owner} missing shared layout/painted border stitch at {point:?}, nearest remaining knot {nearest:?}"
                ));
        }
    }
}
