use crate::graph_layout::{Marker, route_paint::LEAD};
use crate::resources::OperationWorkMeter;
use crate::svg::parity::roughjs_common::{controlled, ops_to_svg_path_d};
use kurbo::Point;
use roughr::core::{Op, OpType, OptionsBuilder, RoughRandomness};
use roughr::work::WorkError;
use svgtypes::PathSegment;

pub(super) fn constrained_rough_edge_path(
    path: &str,
    markers: (Marker, Marker),
    meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> crate::Result<String> {
    let stroke = controlled::generate(meter, merman_model::OperationPhase::Emit, |control| {
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
        for pass in passes.iter_mut().take(2) {
            straighten_shaft(pass)?;
        }
        for pass in passes.iter_mut().rev().take(2) {
            straighten_shaft(pass)?;
        }
        Ok(stroke)
    })?;
    Ok(ops_to_svg_path_d(&stroke))
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
