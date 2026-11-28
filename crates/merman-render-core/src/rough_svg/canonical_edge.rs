use crate::resources::OperationWorkMeter;
use crate::route_types::{MIN_MARKER_LEAD, Marker};
use kurbo::Point;
use roughr::PathSegment;
use roughr::core::{Op, OpSet, OpType, OptionsBuilder, RoughRandomness};
use roughr::work::WorkError;

use super::{controlled, ops_to_svg_path_d};

pub fn constrained_rough_edge_path(
    path: &str,
    markers: (Marker, Marker),
    meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> crate::Result<String> {
    let stroke = constrained_rough_edge_stroke(path, markers, meter, randomness, false)?;
    Ok(ops_to_svg_path_d(&stroke))
}

/// Produce the SVG shaft and the same centerlines for caption ownership and bounds checks.
#[cfg(feature = "layout-shoal")]
pub fn constrained_rough_edge_with_geometry(
    path: &str,
    markers: (Marker, Marker),
    meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> crate::Result<(String, kurbo::BezPath)> {
    use crate::number_format::canonicalize_number;
    use kurbo::PathEl;

    let mut stroke = constrained_rough_edge_stroke(path, markers, meter, randomness, true)?;
    meter.charge_emit_work(stroke.ops.len())?;
    let invalid = || crate::Error::InvalidModel {
        message: "invalid rough edge geometry".into(),
    };
    let mut elements = Vec::new();
    elements
        .try_reserve_exact(stroke.ops.len())
        .map_err(|_| crate::Error::InvalidModel {
            message: "could not allocate rough edge geometry".into(),
        })?;
    let count = stroke.ops.len();
    for (index, op) in stroke.ops.iter_mut().enumerate() {
        let interior = (4..count.saturating_sub(4)).contains(&index);
        for (component, coordinate) in op.data.iter_mut().enumerate() {
            if !coordinate.is_finite() {
                return Err(invalid());
            }
            if interior && op.op == OpType::BCurveTo && component < 4 && coordinate.abs() < 1e12 {
                *coordinate = (*coordinate * 100.0).round() / 100.0;
            }
            *coordinate = canonicalize_number(*coordinate);
        }
        let item = match (&op.op, op.data.as_slice()) {
            (OpType::Move, &[x, y]) => PathEl::MoveTo(Point::new(x, y)),
            (OpType::LineTo, &[x, y]) => PathEl::LineTo(Point::new(x, y)),
            (OpType::BCurveTo, &[x1, y1, x2, y2, x3, y3]) => {
                PathEl::CurveTo(Point::new(x1, y1), Point::new(x2, y2), Point::new(x3, y3))
            }
            _ => return Err(invalid()),
        };
        elements.push(item);
    }
    Ok((
        ops_to_svg_path_d(&stroke),
        kurbo::BezPath::from_vec(elements),
    ))
}

#[cfg(all(test, feature = "layout-shoal"))]
#[test]
fn compact_edge_keeps_exact_contacts_and_matches_its_painted_geometry() {
    use crate::resources::RenderResourcePolicy;
    use kurbo::{BezPath, ParamCurve, PathSeg};
    use roughr::core::{RoughJsSeed, RoughMathRandom};

    let meter = OperationWorkMeter::new(RenderResourcePolicy::constrained());
    let randomness = RoughRandomness::new(RoughJsSeed::new(7.0), RoughMathRandom::new(3));
    let route = "M3.1234 5.7891L12.1234 5.7891L20.4321 5.7891L20.4321 40.9765L35.7654 40.9765L45.7654 40.9765";
    let (svg, geometry) =
        constrained_rough_edge_with_geometry(route, Default::default(), &meter, &randomness)
            .unwrap();
    assert_eq!(BezPath::from_svg(&svg).unwrap(), geometry);
    let start = geometry.segments().next().unwrap();
    let end = geometry.segments().last().unwrap();
    assert_eq!(start.start(), Point::new(3.1234, 5.7891));
    assert_eq!(end.end(), Point::new(45.7654, 40.9765));
    for (segment, y) in [(start, 5.7891), (end, 40.9765)] {
        let PathSeg::Line(line) = segment else {
            panic!("expected a straight terminal shaft");
        };
        assert_eq!([line.p0.y, line.p1.y], [y; 2]);
    }
}

fn constrained_rough_edge_stroke(
    path: &str,
    markers: (Marker, Marker),
    meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
    compact: bool,
) -> crate::Result<OpSet<f64>> {
    controlled::generate(meter, merman_model::OperationPhase::Emit, |control| {
        let mut segments = roughr::points_on_path::normalized_path_with_control(path, control)?;
        split_direct_shaft(&mut segments, markers);
        let mut options = OptionsBuilder::default()
            .randomness(randomness.clone())
            .roughness(0.3)
            .preserve_vertices(true)
            .build()
            .map_err(|_| WorkError::InvalidGeometry)?;
        let mut stroke = roughr::renderer::svg_normalized_segments_with_control(
            &segments,
            &mut options,
            control,
        )?;
        let (passes, _) = stroke.ops.as_chunks_mut::<2>();
        match segments.get(1) {
            Some(PathSegment::LineTo { .. }) => {
                for pass in passes.iter_mut().take(2) {
                    straighten_shaft(pass, compact)?;
                }
            }
            Some(PathSegment::CurveTo { x1, y1, .. }) => {
                for pass in passes.iter_mut().take(2) {
                    align_curve_start(pass, Point::new(*x1, *y1))?;
                }
            }
            _ => {}
        }
        match segments.last() {
            Some(PathSegment::LineTo { .. }) => {
                for pass in passes.iter_mut().rev().take(2) {
                    straighten_shaft(pass, compact)?;
                }
            }
            Some(PathSegment::CurveTo { x2, y2, .. }) => {
                for pass in passes.iter_mut().rev().take(2) {
                    align_curve_end(pass, Point::new(*x2, *y2))?;
                }
            }
            _ => {}
        }
        Ok(stroke)
    })
}

fn split_direct_shaft(segments: &mut Vec<PathSegment>, markers: (Marker, Marker)) {
    let [
        PathSegment::MoveTo {
            x: from_x,
            y: from_y,
            ..
        },
        PathSegment::LineTo {
            x: to_x, y: to_y, ..
        },
    ] = segments.as_slice()
    else {
        return;
    };
    let start = Point::new(*from_x, *from_y);
    let end = Point::new(*to_x, *to_y);
    let delta = end - start;
    let length = delta.hypot();
    let start_lead = MIN_MARKER_LEAD.max(markers.0.depth);
    let end_lead = MIN_MARKER_LEAD.max(markers.1.depth);
    if length <= start_lead + end_lead {
        return;
    }
    let unit = delta / length;
    let line = |point: Point| PathSegment::LineTo {
        abs: true,
        x: point.x,
        y: point.y,
    };
    segments[1] = line(start + unit * start_lead);
    segments.push(line(end - unit * end_lead));
    segments.push(line(end));
}

fn straighten_shaft(pass: &mut [Op<f64>], compact: bool) -> Result<(), WorkError> {
    let [start, curve] = pass else {
        return Err(WorkError::InvalidGeometry);
    };
    if compact && start.op == OpType::Move && curve.op == OpType::LineTo && curve.data.len() == 2 {
        return Ok(());
    }
    let ([from_x, from_y], [first_x, first_y, second_x, second_y, end_x, end_y]) =
        (start.data.as_slice(), curve.data.as_mut_slice())
    else {
        return Err(WorkError::InvalidGeometry);
    };
    if start.op != OpType::Move || curve.op != OpType::BCurveTo {
        return Err(WorkError::InvalidGeometry);
    }
    if compact {
        curve.op = OpType::LineTo;
        (*first_x, *first_y) = (*end_x, *end_y);
        curve.data.truncate(2);
        return Ok(());
    }
    let start = Point::new(*from_x, *from_y);
    let end = Point::new(*end_x, *end_y);
    let first = start.lerp(end, 1.0 / 3.0);
    let second = start.lerp(end, 2.0 / 3.0);
    (*first_x, *first_y) = (first.x, first.y);
    (*second_x, *second_y) = (second.x, second.y);
    Ok(())
}

fn align_curve_start(pass: &mut [Op<f64>], control: Point) -> Result<(), WorkError> {
    let [start, curve] = pass else {
        return Err(WorkError::InvalidGeometry);
    };
    let [first_x, first_y, _, _, _, _] = curve.data.as_mut_slice() else {
        return Err(WorkError::InvalidGeometry);
    };
    if start.op != OpType::Move || curve.op != OpType::BCurveTo {
        return Err(WorkError::InvalidGeometry);
    }
    (*first_x, *first_y) = (control.x, control.y);
    Ok(())
}

fn align_curve_end(pass: &mut [Op<f64>], control: Point) -> Result<(), WorkError> {
    let [start, curve] = pass else {
        return Err(WorkError::InvalidGeometry);
    };
    let [_, _, second_x, second_y, _, _] = curve.data.as_mut_slice() else {
        return Err(WorkError::InvalidGeometry);
    };
    if start.op != OpType::Move || curve.op != OpType::BCurveTo {
        return Err(WorkError::InvalidGeometry);
    }
    (*second_x, *second_y) = (control.x, control.y);
    Ok(())
}
