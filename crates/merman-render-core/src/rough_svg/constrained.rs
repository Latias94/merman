use super::{controlled, ops_to_svg_path_d};
use crate::resources::OperationWorkMeter;
#[cfg(feature = "layout-shoal")]
use crate::route_paint::LEAD;
use crate::route_types::Marker;
use kurbo::Point;
use roughr::core::{Op, OpSet, OpType, OptionsBuilder, RoughRandomness};
use roughr::work::WorkError;
use svgtypes::PathSegment;

#[cfg(not(feature = "layout-shoal"))]
const LEAD: f64 = 6.0;

pub fn constrained_rough_edge_path(
    path: &str,
    markers: (Marker, Marker),
    meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> crate::Result<String> {
    let stroke = constrained_rough_edge_stroke(path, markers, meter, randomness)?;
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

    let mut stroke = constrained_rough_edge_stroke(path, markers, meter, randomness)?;
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
    for op in &mut stroke.ops {
        for coordinate in &mut op.data {
            if !coordinate.is_finite() {
                return Err(invalid());
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

fn constrained_rough_edge_stroke(
    path: &str,
    markers: (Marker, Marker),
    meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
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
                    straighten_shaft(pass)?;
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
                    straighten_shaft(pass)?;
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
    let start_lead = LEAD.max(markers.0.depth);
    let end_lead = LEAD.max(markers.1.depth);
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

fn straighten_shaft(pass: &mut [Op<f64>]) -> Result<(), WorkError> {
    let [start, curve] = pass else {
        return Err(WorkError::InvalidGeometry);
    };
    let ([from_x, from_y], [first_x, first_y, second_x, second_y, end_x, end_y]) =
        (start.data.as_slice(), curve.data.as_mut_slice())
    else {
        return Err(WorkError::InvalidGeometry);
    };
    if start.op != OpType::Move || curve.op != OpType::BCurveTo {
        return Err(WorkError::InvalidGeometry);
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
