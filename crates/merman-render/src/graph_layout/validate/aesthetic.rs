//! Descriptive metrics for the painted gallery paths at intrinsic SVG size.
//!
//! Command counts include the initial move. Micro kinks count same-direction, non-collinear
//! shaft steps with an endpoint offset strictly between 0.01 and 2 px across the flow axis.
//! A seam bump measures transverse travel that reverses between named landmarks. Marker pitch
//! uses adjacent references on one body face and the sum of their effective half-widths.

use super::{CrossAxis, End, Scene, Shaft};
use kurbo::{Line, ParamCurve, ParamCurveExtrema, PathSeg, Point};
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;
use svgtypes::{PathParser, PathSegment};

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

struct MarkerSample<'a> {
    edge: &'a str,
    position: f64,
    width: f64,
}

#[derive(Serialize)]
struct MarkerCrowding<'a> {
    body: &'a str,
    face: &'static str,
    marker_count: usize,
    edges: [&'a str; 2],
    pitch_px: f64,
    marker_widths_px: [f64; 2],
    pitch_over_width: f64,
}

pub(super) fn emit<'a>(
    case: &str,
    direction: &str,
    scene: &Scene<'_>,
    body_names: &[impl AsRef<str>],
    edge_names: &'a [impl AsRef<str>],
    seams: &[Seam],
) -> Metrics<'a> {
    let mut cubic_count = 0;
    let mut max_commands_per_edge = 0;
    let mut micro_kink_count = 0;
    let mut micro_kink_edges = Vec::new();
    let mut border_seams = Vec::new();
    let mut marker_faces = BTreeMap::<_, Vec<_>>::new();
    for (index, edge) in scene.edges.iter().enumerate() {
        let name = edge_names[index].as_ref();
        let shaft = Shaft::read(edge).unwrap_or_else(|error| panic!("{case}: {name}: {error}"));
        let (mut commands, mut cubics) = (0, 0);
        for command in PathParser::from(edge.d) {
            commands += 1;
            cubics += usize::from(matches!(
                command.unwrap_or_else(|error| panic!("{case}: {name}: {error}")),
                PathSegment::CurveTo { .. } | PathSegment::SmoothCurveTo { .. }
            ));
        }
        cubic_count += cubics;
        max_commands_per_edge = max_commands_per_edge.max(commands);
        let kinks = micro_kinks(&shaft, edge.cross_axis);
        micro_kink_count += kinks;
        if kinks > 0 {
            micro_kink_edges.push(json!({"edge": name, "count": kinks}));
        }
        for seam in seams.iter().filter(|seam| seam.edge == index) {
            border_seams.push(seam_metrics(&shaft, edge.cross_axis, name, &seam.landmarks));
        }
        for (end, endpoint) in [(End::Start, edge.start), (End::End, edge.end)] {
            let (Some(width), Some(normal)) = (endpoint.marker_width, endpoint.outward_normal)
            else {
                continue;
            };
            let anchor = shaft.segment_from(end, 0).0.start();
            let offset = anchor - scene.bodies[endpoint.body].bounds.center();
            let (face, position) = if normal.x.abs() > normal.y.abs() {
                (if offset.x < 0. { "left" } else { "right" }, anchor.y)
            } else {
                (if offset.y < 0. { "top" } else { "bottom" }, anchor.x)
            };
            marker_faces
                .entry((endpoint.body, face))
                .or_default()
                .push(MarkerSample {
                    edge: name,
                    position,
                    width,
                });
        }
    }
    let marker_crowding_faces: Vec<_> = marker_faces
        .into_iter()
        .filter_map(|((body, face), markers)| crowding(body_names[body].as_ref(), face, markers))
        .collect();
    let marker_crowding_min_pitch = marker_crowding_faces
        .iter()
        .map(|crowding| crowding.pitch_over_width)
        .min_by(f64::total_cmp);
    let border_seam_bump = border_seams
        .iter()
        .map(|seam| seam.bump_px)
        .max_by(f64::total_cmp);
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
    });
    eprintln!("shoal-aesthetic {row}");
    Metrics {
        cubic_count,
        max_commands_per_edge,
        micro_kink_count,
        border_seam_bump,
        seams: border_seams,
    }
}

fn micro_kinks(shaft: &Shaft, axis: CrossAxis) -> usize {
    let mut kinks = vec![false; shaft.segments.len()];
    let micro_offset = |delta: f64| 0.01 < delta.abs() && delta.abs() < 2.0;
    for (index, pair) in shaft.segments.windows(2).enumerate() {
        let (first, second) = (pair[0].0, pair[1].0);
        let (a, b) = (first.end() - first.start(), second.end() - second.start());
        let (a_cross, a_main, b_cross, b_main) = match axis {
            CrossAxis::X => (a.x, a.y, b.x, b.y),
            CrossAxis::Y => (a.y, a.x, b.y, b.x),
        };
        if a_main * b_main > 0.
            && first.end().distance(second.start()) <= JOIN_EPSILON
            && a.cross(b).abs() > JOIN_EPSILON * a.hypot() * b.hypot()
        {
            kinks[index] |= micro_offset(a_cross);
            kinks[index + 1] |= micro_offset(b_cross);
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
