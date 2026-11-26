//! Descriptive metrics for the painted gallery paths at intrinsic SVG size.
//!
//! Command counts include the initial move. Micro kinks count same-direction, non-collinear
//! shaft steps with an endpoint offset strictly between 0.01 and 2 px across the flow axis.
//! A seam bump measures transverse travel that reverses between named landmarks. Marker pitch
//! uses adjacent references on one body face and the sum of their effective half-widths.
//! Turn metrics follow the painted shaft away from the shared face, including at joins.

use super::{ARCLEN_ACCURACY, CrossAxis, Edge, End, LabelOwner, Scene, Shaft, start_tangent};
use kurbo::{
    CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveExtrema, PathSeg, Point, Rect, Shape,
    Vec2,
};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use svgtypes::PathParser;

const JOIN_EPSILON: f64 = 1e-7;

pub(super) enum Landmark {
    Plane(&'static str, f64),
    ShaftEndpoint(&'static str, End),
}

pub(super) struct Seam {
    pub edge: usize,
    pub landmarks: Vec<Landmark>,
}

pub(super) struct Metrics<'a> {
    pub(super) cubic_count: usize,
    pub(super) max_commands_per_edge: usize,
    pub(super) micro_kink_count: usize,
    pub(super) border_seam_bump: Option<f64>,
    pub(super) seams: Vec<SeamMetrics<'a>>,
    pub(super) marker_faces: Vec<MarkerCrowding<'a>>,
    pub(super) label_optical_balance_ratio: Option<f64>,
    pub(super) label_optical_balance: Vec<LabelBalance<'a>>,
    pub(super) max_s_curve_handle_ratio: Option<f64>,
    pub(super) max_s_curve_flatness: Option<f64>,
    pub(super) max_corner_arm_px: Option<f64>,
    pub(super) s_curves: Vec<SCurve<'a>>,
    pub(super) face_turn_coherence: Vec<FaceCoherence<'a>>,
    pub(super) sibling_departure_symmetry: Vec<SiblingSymmetry<'a>>,
    pub(super) large_fan_cubic_ratio: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(super) struct LabelBalance<'a> {
    pub(super) edge: &'a str,
    /// Exposed lengths before and after the painted label, in path order.
    pub(super) clear_shaft_px: Option<[f64; 2]>,
    pub(super) ratio: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(super) struct SCurve<'a> {
    pub(super) edge: &'a str,
    segment: usize,
    pub(super) handle_ratio: f64,
    pub(super) flatness: f64,
    pub(super) ordered_controls: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct FaceCoherence<'a> {
    pub(super) body: &'a str,
    face: &'static str,
    edges: [&'a str; 2],
    reaches_px: [f64; 2],
    pub(super) max_adjacent_delta_px: f64,
}

#[derive(Debug, Serialize)]
pub(super) struct SiblingSymmetry<'a> {
    pub(super) body: &'a str,
    face: &'static str,
    pub(super) end: &'static str,
    edges: [&'a str; 2],
    pub(super) turn_start_delta_px: f64,
    pub(super) landing_horizon_delta_px: f64,
    pub(super) turn_span_ratio: Option<f64>,
    pub(super) mirror_tangent_error_deg: f64,
}

struct FaceSample<'a> {
    edge: &'a str,
    end: End,
    other_body: usize,
    position: f64,
    normal: Vec2,
    turn: Option<Turn>,
    marker: Option<MarkerSample<'a>>,
}

struct Turn {
    start: Point,
    finish: Point,
    tangent: Vec2,
    reach_px: f64,
}

#[derive(Serialize)]
pub(super) struct SeamMetrics<'a> {
    edge: &'a str,
    bump_px: f64,
    cubic_count: usize,
    landmarks: Vec<LandmarkPosition>,
    pub(super) legs: Vec<SeamLeg>,
}

#[derive(Serialize)]
struct LandmarkPosition {
    name: &'static str,
    x: f64,
    y: f64,
}

#[derive(Serialize)]
pub(super) struct SeamLeg {
    pub(super) from: &'static str,
    pub(super) to: &'static str,
    pub(super) cubic_count: usize,
}

#[derive(Clone, Copy)]
struct MarkerSample<'a> {
    edge: &'a str,
    position: f64,
    width: f64,
}

#[derive(Serialize)]
pub(super) struct MarkerCrowding<'a> {
    pub(super) body: &'a str,
    pub(super) face: &'static str,
    pub(super) marker_count: usize,
    edges: [&'a str; 2],
    pitch_px: f64,
    marker_widths_px: [f64; 2],
    pub(super) pitch_over_width: f64,
}

pub(super) fn emit<'a>(
    case: &str,
    direction: &str,
    scene: &Scene<'_>,
    body_names: &'a [impl AsRef<str>],
    edge_names: &'a [impl AsRef<str>],
    seams: &[Seam],
) -> Metrics<'a> {
    let mut cubic_count = 0;
    let mut max_commands_per_edge = 0;
    let mut micro_kink_count = 0;
    let mut micro_kink_edges = Vec::new();
    let mut border_seams = Vec::new();
    let mut face_samples = BTreeMap::<_, Vec<_>>::new();
    let mut label_optical_balance = Vec::new();
    let mut s_curves = Vec::new();
    let mut max_corner_arm_px: Option<f64> = None;
    let mut edge_labels = BTreeMap::new();
    for label in &scene.labels {
        if let LabelOwner::Edge(index) = label.owner {
            edge_labels
                .entry(index)
                .and_modify(|bounds| *bounds = None)
                .or_insert(Some(label.bounds));
        }
    }
    for (index, edge) in scene.edges.iter().enumerate() {
        let name = edge_names[index].as_ref();
        let shaft = Shaft::read(edge).unwrap_or_else(|error| panic!("{case}: {name}: {error}"));
        let commands = PathParser::from(edge.d).count();
        max_commands_per_edge = max_commands_per_edge.max(commands);
        for (segment, &(path, _)) in shaft.segments.iter().enumerate() {
            if let PathSeg::Cubic(curve) = path {
                cubic_count += 1;
                if let Some((handle_ratio, flatness, ordered_controls)) =
                    s_curve_handles(curve, edge.cross_axis)
                {
                    s_curves.push(SCurve {
                        edge: name,
                        segment,
                        handle_ratio,
                        flatness,
                        ordered_controls,
                    });
                } else if let Some(arm) = right_angle_corner_arm(curve) {
                    max_corner_arm_px = Some(max_corner_arm_px.map_or(arm, |old| old.max(arm)));
                }
            }
        }
        if let Some(Some(bounds)) = edge_labels.get(&index) {
            let clear_shaft_px = label_clear_shafts(&shaft, edge, *bounds);
            label_optical_balance.push(LabelBalance {
                edge: name,
                clear_shaft_px,
                ratio: clear_shaft_px.and_then(|[before, after]| length_ratio(before, after)),
            });
        }
        let kinks = micro_kinks(&shaft, edge.cross_axis);
        micro_kink_count += kinks;
        if kinks > 0 {
            micro_kink_edges.push(json!({"edge": name, "count": kinks}));
        }
        for seam in seams.iter().filter(|seam| seam.edge == index) {
            border_seams.push(seam_metrics(&shaft, edge.cross_axis, name, &seam.landmarks));
        }
        for (end, endpoint, other_body) in [
            (End::Start, edge.start, edge.end.body),
            (End::End, edge.end, edge.start.body),
        ] {
            let Some(normal) = endpoint.outward_normal else {
                continue;
            };
            let normal = normal.normalize();
            let first = shaft.segment_from(end, 0).0;
            let contact = first.start() - start_tangent(first) * endpoint.setback;
            let (face, across) = if normal.x.abs() > normal.y.abs() {
                (if normal.x < 0. { "left" } else { "right" }, CrossAxis::Y)
            } else {
                (if normal.y < 0. { "top" } else { "bottom" }, CrossAxis::X)
            };
            face_samples
                .entry((endpoint.body, face))
                .or_default()
                .push(FaceSample {
                    edge: name,
                    end,
                    other_body,
                    position: across.coordinate(contact),
                    normal,
                    turn: first_turn(&shaft, end, contact, normal),
                    marker: endpoint.marker_width.map(|width| MarkerSample {
                        edge: name,
                        position: across.coordinate(first.start()),
                        width,
                    }),
                });
        }
    }
    let mut marker_crowding_faces = Vec::new();
    let mut face_turn_coherence = Vec::new();
    let mut sibling_departure_symmetry = Vec::new();
    for ((body, face), mut samples) in face_samples {
        if samples.len() < 2 {
            continue;
        }
        let body_geometry = &scene.bodies[body];
        let center = body_geometry.contour.as_ref().map_or_else(
            || body_geometry.bounds.center(),
            |outline| outline.bounding_box().center(),
        );
        let name = body_names[body].as_ref();
        samples.sort_by(|a, b| a.position.total_cmp(&b.position));
        let markers = samples.iter().filter_map(|sample| sample.marker).collect();
        marker_crowding_faces.extend(crowding(name, face, markers));
        face_turn_coherence.extend(face_coherence(name, face, &samples));
        for end in [End::Start, End::End] {
            sibling_departure_symmetry.extend(sibling_symmetry(name, face, center, &samples, end));
        }
    }
    let marker_crowding_min_pitch = marker_crowding_faces
        .iter()
        .map(|crowding| crowding.pitch_over_width)
        .min_by(f64::total_cmp);
    let border_seam_bump = border_seams
        .iter()
        .map(|seam| seam.bump_px)
        .max_by(f64::total_cmp);
    let label_optical_balance_ratio = label_optical_balance
        .iter()
        .filter_map(|label| label.ratio)
        .max_by(f64::total_cmp);
    let worst_s_curve = s_curves
        .iter()
        .max_by(|a, b| a.handle_ratio.total_cmp(&b.handle_ratio));
    let max_s_curve_handle_ratio = worst_s_curve.map(|curve| curve.handle_ratio);
    let max_s_curve_flatness = s_curves
        .iter()
        .map(|curve| curve.flatness)
        .max_by(f64::total_cmp);
    let unordered_s_curves: Vec<_> = s_curves
        .iter()
        .filter(|curve| !curve.ordered_controls)
        .collect();
    let large_fan_cubic_ratio = fan_cubic_ratio(&scene.edges, cubic_count);
    let row = json!({
        "case": case,
        "direction": direction,
        "edges": scene.edges.len(),
        "cubic_count": cubic_count,
        "max_commands_per_edge": max_commands_per_edge,
        "micro_kink_count": micro_kink_count,
        "micro_kink_edges": micro_kink_edges,
        "border_seam_bump": border_seam_bump,
        "border_seams": border_seams,
        "marker_crowding_min_pitch": marker_crowding_min_pitch,
        "marker_crowding_faces": marker_crowding_faces,
        "label_optical_balance_ratio": label_optical_balance_ratio,
        "label_optical_balance": label_optical_balance,
        "max_s_curve_handle_ratio": max_s_curve_handle_ratio,
        "max_s_curve_flatness": max_s_curve_flatness,
        "max_corner_arm_px": max_corner_arm_px,
        "s_curve_count": s_curves.len(),
        "worst_s_curve": worst_s_curve,
        "unordered_s_curves": unordered_s_curves,
        "face_turn_coherence": face_turn_coherence,
        "sibling_departure_symmetry": sibling_departure_symmetry,
        "large_fan_cubic_ratio": large_fan_cubic_ratio,
    });
    eprintln!("shoal-aesthetic {row}");
    Metrics {
        cubic_count,
        max_commands_per_edge,
        micro_kink_count,
        border_seam_bump,
        seams: border_seams,
        marker_faces: marker_crowding_faces,
        label_optical_balance_ratio,
        label_optical_balance,
        max_s_curve_handle_ratio,
        max_s_curve_flatness,
        max_corner_arm_px,
        s_curves,
        face_turn_coherence,
        sibling_departure_symmetry,
        large_fan_cubic_ratio,
    }
}

fn fan_cubic_ratio(edges: &[Edge<'_>], cubic_count: usize) -> Option<f64> {
    let [first, _, ..] = edges else { return None };
    let is_fan = |end| {
        let mut leaves = BTreeSet::new();
        edges.iter().all(|edge| {
            let (hub, leaf, expected_hub) = match end {
                End::Start => (edge.start.body, edge.end.body, first.start.body),
                End::End => (edge.end.body, edge.start.body, first.end.body),
            };
            hub == expected_hub && leaf != hub && leaves.insert(leaf)
        })
    };
    (is_fan(End::Start) || is_fan(End::End)).then(|| cubic_count as f64 / edges.len() as f64)
}

fn length_ratio(a: f64, b: f64) -> Option<f64> {
    (a.min(b) > JOIN_EPSILON).then(|| a.max(b) / a.min(b))
}

fn opposite_signs(a: f64, b: f64) -> bool {
    a.abs().min(b.abs()) > JOIN_EPSILON && a.signum() != b.signum()
}

fn label_clear_shafts(shaft: &Shaft, edge: &Edge<'_>, bounds: Rect) -> Option<[f64; 2]> {
    // An arrow's base covers the last portion of the path behind its SVG reference point.
    // Circle and cross markers have no point-marker base to measure.
    let marker_depth = match (edge.end.marker, edge.end.marker_width) {
        (Some(marker), _) => marker.depth,
        (None, None) => 0.,
        (None, Some(_)) => return None,
    };
    let [mut first, mut last] = [f64::INFINITY, f64::NEG_INFINITY];
    let mut offset = 0.;
    for &(segment, length) in &shaft.segments {
        let mut cuts = vec![0., 1.];
        for side in bounds.path_segments(0.) {
            let PathSeg::Line(side) = side else {
                unreachable!("rectangle side")
            };
            cuts.extend(
                segment
                    .intersect_line(side)
                    .into_iter()
                    .map(|hit| hit.segment_t.clamp(0., 1.)),
            );
        }
        cuts.sort_by(f64::total_cmp);
        for pair in cuts.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            if hi - lo <= JOIN_EPSILON {
                continue;
            }
            let p = segment.eval(lo.midpoint(hi));
            if bounds.x0 < p.x && p.x < bounds.x1 && bounds.y0 < p.y && p.y < bounds.y1 {
                let at = |t| offset + segment.subsegment(0.0..t).arclen(ARCLEN_ACCURACY);
                first = first.min(at(lo));
                last = last.max(at(hi));
            }
        }
        offset += length;
    }
    // Measure the outer legs from the restored source contact to the first label entry and
    // from the last label exit to the marker base.
    first.is_finite().then(|| {
        [
            first + edge.start.setback,
            (offset - last - marker_depth).max(0.),
        ]
    })
}

fn s_curve_handles(curve: CubicBez, axis: CrossAxis) -> Option<(f64, f64, bool)> {
    let chord = curve.p3 - curve.p0;
    let along = |point: Point| match axis {
        CrossAxis::X => point.y,
        CrossAxis::Y => point.x,
    };
    let span = along(curve.p3) - along(curve.p0);
    let across = axis.coordinate(curve.p3) - axis.coordinate(curve.p0);
    if span.abs() <= JOIN_EPSILON || across.abs() <= JOIN_EPSILON {
        return None;
    }
    // Offset sweeps straddle their chord. Rounded corners place both controls on one side.
    let side = |p| chord.cross(p - curve.p0) / chord.hypot();
    if !opposite_signs(side(curve.p1), side(curve.p2)) {
        return None;
    }
    let [first, second] = [curve.p1, curve.p2].map(|p| (along(p) - along(curve.p0)) / span);
    let ordered =
        first >= -JOIN_EPSILON && first <= second + JOIN_EPSILON && second <= 1. + JOIN_EPSILON;
    Some((
        first.abs().max((1. - second).abs()),
        across.abs() / span.abs(),
        ordered,
    ))
}

fn right_angle_corner_arm(curve: CubicBez) -> Option<f64> {
    let chord = curve.p3 - curve.p0;
    if chord.x.abs() <= JOIN_EPSILON || chord.y.abs() <= JOIN_EPSILON {
        return None;
    }
    let side = |p: Point| chord.cross(p - curve.p0) / chord.hypot();
    let (s1, s2) = (side(curve.p1), side(curve.p2));
    if s1.abs().min(s2.abs()) <= JOIN_EPSILON || s1.signum() != s2.signum() {
        return None;
    }
    let t0 = curve.p1 - curve.p0;
    let t1 = curve.p3 - curve.p2;
    let ortho = t0.hypot() > JOIN_EPSILON
        && t1.hypot() > JOIN_EPSILON
        && t0.dot(t1).abs() <= 1e-3 * t0.hypot() * t1.hypot();
    ortho.then(|| chord.x.abs().max(chord.y.abs()))
}

fn first_turn(shaft: &Shaft, end: End, contact: Point, normal: Vec2) -> Option<Turn> {
    let mut previous = shaft.segment_from(end, 0).0.start();
    for offset in 0..shaft.segments.len() {
        let (segment, _) = shaft.segment_from(end, offset);
        let start = segment.start();
        if previous.distance(start) > JOIN_EPSILON {
            return None;
        }
        let follows_normal = |point| normal.cross(point - start).abs() <= JOIN_EPSILON;
        let straight = follows_normal(segment.end())
            && match segment {
                PathSeg::Line(_) => true,
                PathSeg::Quad(curve) => follows_normal(curve.p1),
                PathSeg::Cubic(curve) => follows_normal(curve.p1) && follows_normal(curve.p2),
            };
        if !straight || normal.dot(segment.end() - start) < 0. {
            return Some(Turn {
                start,
                finish: segment.end(),
                tangent: start_tangent(segment),
                reach_px: (start - contact).dot(normal),
            });
        }
        previous = segment.end();
    }
    None
}

fn face_coherence<'a>(
    body: &'a str,
    face: &'static str,
    samples: &[FaceSample<'a>],
) -> Option<FaceCoherence<'a>> {
    // A straight shaft has no turn reach to compare with its neighbors.
    let mut turning = samples.iter().filter_map(|sample| {
        sample
            .turn
            .as_ref()
            .map(|turn| (sample.edge, turn.reach_px))
    });
    let mut previous = turning.next()?;
    let mut worst: Option<FaceCoherence<'_>> = None;
    for next in turning {
        let delta = (previous.1 - next.1).abs();
        if worst
            .as_ref()
            .is_none_or(|pair| delta > pair.max_adjacent_delta_px)
        {
            worst = Some(FaceCoherence {
                body,
                face,
                edges: [previous.0, next.0],
                reaches_px: [previous.1, next.1],
                max_adjacent_delta_px: delta,
            });
        }
        previous = next;
    }
    worst
}

fn sibling_symmetry<'a>(
    body: &'a str,
    face: &'static str,
    center: Point,
    samples: &[FaceSample<'a>],
    end: End,
) -> Option<SiblingSymmetry<'a>> {
    let mut same_end = samples.iter().filter(|sample| sample.end == end);
    let (a, b) = (same_end.next()?, same_end.next()?);
    if same_end.next().is_some() || a.other_body == b.other_body {
        return None;
    }
    let (first, second) = (a.turn.as_ref()?, b.turn.as_ref()?);
    let (first_span, second_span) = (first.finish - first.start, second.finish - second.start);
    let normal = a.normal;
    // A sibling fork or join turns in opposite lateral directions around the shared body.
    if !opposite_signs(normal.cross(first_span), normal.cross(second_span))
        || !opposite_signs(
            normal.cross(first.finish - center),
            normal.cross(second.finish - center),
        )
    {
        return None;
    }
    let mirrored = normal * (2. * first.tangent.dot(normal)) - first.tangent;
    Some(SiblingSymmetry {
        body,
        face,
        end: match end {
            End::Start => "start",
            End::End => "end",
        },
        edges: [a.edge, b.edge],
        turn_start_delta_px: normal.dot(first.start - second.start).abs(),
        landing_horizon_delta_px: normal.dot(first.finish - second.finish).abs(),
        turn_span_ratio: length_ratio(normal.dot(first_span).abs(), normal.dot(second_span).abs()),
        mirror_tangent_error_deg: mirrored
            .cross(second.tangent)
            .abs()
            .atan2(mirrored.dot(second.tangent))
            .to_degrees(),
    })
}

fn micro_kinks(shaft: &Shaft, axis: CrossAxis) -> usize {
    let mut kinks = vec![false; shaft.segments.len()];
    let micro_offset = |delta: f64| 0.01 < delta.abs() && delta.abs() < 2.0;
    let components = |v: Vec2| match axis {
        CrossAxis::X => (v.x, v.y),
        CrossAxis::Y => (v.y, v.x),
    };
    for (index, pair) in shaft.segments.windows(2).enumerate() {
        let (first, second) = (pair[0].0, pair[1].0);
        let (a, b) = (first.end() - first.start(), second.end() - second.start());
        let ((a_cross, a_main), (b_cross, b_main)) = (components(a), components(b));
        if a_main * b_main > 0.
            && first.end().distance(second.start()) <= JOIN_EPSILON
            && a.cross(b).abs() > JOIN_EPSILON * a.hypot() * b.hypot()
        {
            kinks[index] |= micro_offset(a_cross);
            kinks[index + 1] |= micro_offset(b_cross);
        }
    }
    for (index, triple) in shaft.segments.windows(3).enumerate() {
        let (first, middle, third) = (triple[0].0, triple[1].0, triple[2].0);
        if first.end().distance(middle.start()) > JOIN_EPSILON
            || middle.end().distance(third.start()) > JOIN_EPSILON
        {
            continue;
        }
        let (_, a_main) = components(first.end() - first.start());
        let (m_cross, m_main) = components(middle.end() - middle.start());
        let (_, c_main) = components(third.end() - third.start());
        if a_main * c_main > 0. && m_main.abs() <= JOIN_EPSILON && micro_offset(m_cross) {
            kinks[index + 1] = true;
        }
    }
    kinks.into_iter().filter(|&kink| kink).count()
}

fn crowding<'a>(
    body: &'a str,
    face: &'static str,
    mut markers: Vec<MarkerSample<'a>>,
) -> Option<MarkerCrowding<'a>> {
    markers.sort_by(|a, b| a.position.total_cmp(&b.position));
    markers
        .windows(2)
        .map(|pair| {
            let pitch_px = pair[1].position - pair[0].position;
            MarkerCrowding {
                body,
                face,
                marker_count: markers.len(),
                edges: [pair[0].edge, pair[1].edge],
                pitch_px,
                marker_widths_px: [pair[0].width, pair[1].width],
                pitch_over_width: pitch_px / pair[0].width.midpoint(pair[1].width),
            }
        })
        .min_by(|a, b| a.pitch_over_width.total_cmp(&b.pitch_over_width))
}

fn seam_metrics<'a>(
    shaft: &Shaft,
    axis: CrossAxis,
    edge: &'a str,
    landmarks: &[Landmark],
) -> SeamMetrics<'a> {
    let mut cuts = Vec::new();
    let mut positions = Vec::new();
    for landmark in landmarks {
        let (name, cut, point) = match *landmark {
            Landmark::Plane(name, coordinate) => {
                let (cut, point) = plane_cut(shaft, axis, coordinate, cuts.last().copied())
                    .unwrap_or_else(|| panic!("{edge}: shaft does not reach {name} in order"));
                (name, cut, point)
            }
            Landmark::ShaftEndpoint(name, end) => {
                let cut = match end {
                    End::Start => 0.,
                    End::End => shaft.segments.len() as f64,
                };
                (name, cut, shaft.segment_from(end, 0).0.start())
            }
        };
        cuts.push(cut);
        positions.push(LandmarkPosition {
            name,
            x: point.x,
            y: point.y,
        });
    }
    let mut increasing = 0.0;
    let mut decreasing = 0.0;
    let mut cubic_count = 0;
    for segment in clip(shaft, cuts[0], *cuts.last().unwrap()) {
        cubic_count += usize::from(matches!(segment, PathSeg::Cubic(_)));
        let mut previous = axis.coordinate(segment.start());
        for time in segment.extrema().into_iter().chain([1.0]) {
            let next = axis.coordinate(segment.eval(time));
            increasing += (next - previous).max(0.);
            decreasing += (previous - next).max(0.);
            previous = next;
        }
    }
    let legs = positions
        .windows(2)
        .zip(cuts.windows(2))
        .map(|(points, range)| SeamLeg {
            from: points[0].name,
            to: points[1].name,
            cubic_count: clip(shaft, range[0], range[1])
                .filter(|segment| matches!(segment, PathSeg::Cubic(_)))
                .count(),
        })
        .collect();
    SeamMetrics {
        edge,
        bump_px: increasing.min(decreasing),
        cubic_count,
        landmarks: positions,
        legs,
    }
}

fn plane_cut(
    shaft: &Shaft,
    axis: CrossAxis,
    coordinate: f64,
    after: Option<f64>,
) -> Option<(f64, Point)> {
    let bounds = shaft.bounds.inflate(1., 1.);
    let plane = match axis {
        CrossAxis::X => Line::new((bounds.x0, coordinate), (bounds.x1, coordinate)),
        CrossAxis::Y => Line::new((coordinate, bounds.y0), (coordinate, bounds.y1)),
    };
    shaft
        .segments
        .iter()
        .enumerate()
        .flat_map(|(index, &(segment, _))| {
            segment.intersect_line(plane).into_iter().map(move |hit| {
                let time = hit.segment_t.clamp(0., 1.);
                (index as f64 + time, segment.eval(time))
            })
        })
        .filter(|&(cut, _)| after.is_none_or(|previous| cut > previous + JOIN_EPSILON))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

fn clip(shaft: &Shaft, from: f64, to: f64) -> impl Iterator<Item = PathSeg> + '_ {
    shaft
        .segments
        .iter()
        .enumerate()
        .filter_map(move |(index, &(segment, _))| {
            let low = (from - index as f64).max(0.);
            let high = (to - index as f64).min(1.);
            (high - low > JOIN_EPSILON).then(|| segment.subsegment(low..high))
        })
}
