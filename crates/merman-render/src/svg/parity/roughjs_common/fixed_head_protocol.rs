use roughr::core::{
    Op, OpSet, OpSetType, OpType, Options, OptionsBuilder, RoughJsSeed, RoughMathRandom,
    RoughRandomness,
};
use roughr::{PathParser, PathSegment};

pub(in crate::svg::parity) const SOURCE_COMMIT: &str = "bd1ef5b8c6e5ada9bbe544a54cd85e11e62e727a";

type Point = [f64; 2];

fn opset(kind: OpSetType, operations: Vec<Op<f64>>) -> OpSet<f64> {
    OpSet {
        op_set_type: kind,
        ops: operations,
        size: None,
        path: None,
    }
}

pub(in crate::svg::parity) fn zero_roughness_line(
    start: Point,
    end: Point,
    options: &mut Options,
) -> OpSet<f64> {
    assert_eq!(options.roughness, Some(0.0));
    let mut operations = Vec::new();
    let strokes = if options.disable_multi_stroke == Some(true) {
        1
    } else {
        2
    };
    for _ in 0..strokes {
        let diverge = f64::from(0.2_f32) + options.random() * f64::from(0.2_f32);
        for _ in 0..10 {
            options.random();
        }
        operations.push(Op {
            op: OpType::Move,
            data: start.to_vec(),
        });
        operations.push(Op {
            op: OpType::BCurveTo,
            data: vec![
                start[0] + (end[0] - start[0]) * diverge,
                start[1] + (end[1] - start[1]) * diverge,
                start[0] + 2.0 * (end[0] - start[0]) * diverge,
                start[1] + 2.0 * (end[1] - start[1]) * diverge,
                end[0],
                end[1],
            ],
        });
    }
    opset(OpSetType::Path, operations)
}

fn svg_operations(segments: &[PathSegment], options: &mut Options) -> OpSet<f64> {
    assert_eq!(options.roughness, Some(0.0));
    let mut operations = Vec::new();
    let mut first = [0.0, 0.0];
    let mut current = first;
    for segment in segments {
        match *segment {
            PathSegment::MoveTo { abs: true, x, y } => {
                current = [x, y];
                first = current;
            }
            PathSegment::LineTo { abs: true, x, y } => {
                operations.extend(zero_roughness_line(current, [x, y], options).ops);
                current = [x, y];
            }
            PathSegment::CurveTo {
                abs: true,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let strokes = if options.disable_multi_stroke == Some(true) {
                    1
                } else {
                    2
                };
                for stroke in 0..strokes {
                    for _ in 0..if stroke == 0 { 6 } else { 8 } {
                        options.random();
                    }
                    operations.push(Op {
                        op: OpType::Move,
                        data: current.to_vec(),
                    });
                    operations.push(Op {
                        op: OpType::BCurveTo,
                        data: vec![x1, y1, x2, y2, x, y],
                    });
                }
                current = [x, y];
            }
            PathSegment::ClosePath { abs: true } => {
                operations.extend(zero_roughness_line(current, first, options).ops);
                current = first;
            }
            _ => panic!("fixed HEAD fixture must use absolute M/L/C/Z commands"),
        }
    }
    opset(OpSetType::Path, operations)
}

fn distance_squared(first: Point, second: Point) -> f64 {
    ((first[0] - second[0]).powi(2) + (first[1] - second[1]).powi(2))
        .sqrt()
        .powi(2)
}

fn interpolate(first: Point, second: Point, weight: f64) -> Point {
    [
        first[0] * (1.0 - weight) + second[0] * weight,
        first[1] * (1.0 - weight) + second[1] * weight,
    ]
}

fn simplify(points: &[Point], start: usize, end: usize, output: &mut Vec<Point>) {
    let first = points[start];
    let last = points[end - 1];
    let length = distance_squared(first, last);
    let mut maximum = 0.0;
    let mut split = 0;
    for (index, point) in points
        .iter()
        .copied()
        .enumerate()
        .take(end - 1)
        .skip(start + 1)
    {
        let distance = if length == 0.0 {
            distance_squared(point, first)
        } else {
            let projection = (((point[0] - first[0]) * (last[0] - first[0])
                + (point[1] - first[1]) * (last[1] - first[1]))
                / length)
                .clamp(0.0, 1.0);
            distance_squared(point, interpolate(first, last, projection))
        };
        if distance > maximum {
            maximum = distance;
            split = index;
        }
    }
    if maximum.sqrt() > 0.5 {
        simplify(points, start, split + 1, output);
        simplify(points, split, end, output);
    } else {
        if output.is_empty() {
            output.push(first);
        }
        output.push(last);
    }
}

fn sample_curve(curve: [Point; 4], output: &mut Vec<Point>) {
    let [first, second, third, last] = curve;
    let mut flatness = 0.0;
    for axis in 0..2 {
        let near = (3.0 * second[axis] - 2.0 * first[axis] - last[axis]).powi(2);
        let far = (3.0 * third[axis] - 2.0 * last[axis] - first[axis]).powi(2);
        flatness += near.max(far);
    }
    if flatness < 1.0 {
        if output
            .last()
            .is_none_or(|point| distance_squared(*point, first).sqrt() > 1.0)
        {
            output.push(first);
        }
        output.push(last);
    } else {
        let near = interpolate(first, second, 0.5);
        let middle = interpolate(second, third, 0.5);
        let far = interpolate(third, last, 0.5);
        let left = interpolate(near, middle, 0.5);
        let right = interpolate(middle, far, 0.5);
        let split = interpolate(left, right, 0.5);
        sample_curve([first, near, left, split], output);
        sample_curve([split, right, far, last], output);
    }
}

fn flush_curve(points: &mut Vec<Point>, pending: &mut Vec<Point>) {
    let mut sampled = Vec::new();
    for index in 0..pending.len() / 3 {
        let offset = index * 3;
        sample_curve(
            [
                pending[offset],
                pending[offset + 1],
                pending[offset + 2],
                pending[offset + 3],
            ],
            &mut sampled,
        );
    }
    points.extend(sampled);
    pending.clear();
}

fn sampled_sets(segments: &[PathSegment]) -> Vec<Vec<Point>> {
    let mut sets = Vec::new();
    let mut points = Vec::new();
    let mut pending = Vec::new();
    let mut first = [0.0, 0.0];
    for segment in segments {
        match *segment {
            PathSegment::MoveTo { abs: true, x, y } => {
                flush_curve(&mut points, &mut pending);
                if !points.is_empty() {
                    sets.push(std::mem::take(&mut points));
                }
                first = [x, y];
                points.push(first);
            }
            PathSegment::LineTo { abs: true, x, y } => {
                flush_curve(&mut points, &mut pending);
                points.push([x, y]);
            }
            PathSegment::CurveTo {
                abs: true,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                if pending.is_empty() {
                    pending.push(points.last().copied().unwrap_or(first));
                }
                pending.extend([[x1, y1], [x2, y2], [x, y]]);
            }
            PathSegment::ClosePath { abs: true } => {
                flush_curve(&mut points, &mut pending);
                points.push(first);
            }
            _ => panic!("fixed HEAD fixture must use absolute M/L/C/Z commands"),
        }
    }
    flush_curve(&mut points, &mut pending);
    if !points.is_empty() {
        sets.push(points);
    }
    sets.iter()
        .map(|points| {
            let mut output = Vec::new();
            simplify(points, 0, points.len(), &mut output);
            output
        })
        .collect()
}

pub(in crate::svg::parity) fn outline_first_paths(
    path: &str,
    randomness: &RoughRandomness,
    force_single_set: bool,
) -> (OpSet<f64>, OpSet<f64>) {
    let segments = PathParser::from(path)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut options = OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .bowing(1.0)
        .stroke_width(1.3)
        .build()
        .unwrap();
    let stroke = svg_operations(&segments, &mut options);
    let sets = if force_single_set {
        Vec::new()
    } else {
        sampled_sets(&segments)
    };
    let mut fill_options = options.clone();
    let fill = if force_single_set || sets.len() == 1 {
        fill_options.disable_multi_stroke = Some(true);
        let mut fill = svg_operations(&segments, &mut fill_options);
        let mut index = 0;
        fill.ops.retain(|operation| {
            let keep = index == 0 || operation.op != OpType::Move;
            index += 1;
            keep
        });
        fill
    } else {
        let mut operations = Vec::new();
        for set in sets.iter().filter(|set| set.len() > 2) {
            for (index, point) in set.iter().enumerate() {
                fill_options.random();
                fill_options.random();
                operations.push(Op {
                    op: if index == 0 {
                        OpType::Move
                    } else {
                        OpType::LineTo
                    },
                    data: point.to_vec(),
                });
            }
        }
        opset(OpSetType::FillPath, operations)
    };
    (fill, stroke)
}

#[test]
fn fixed_head_solid_protocol_matches_operations_and_following_prng() {
    use super::{RoughGeometryTarget, prepare_solid_svg_path};
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    for seed in [0.0, 1.0, -1.0] {
        for path in [
            "M0 0 L40 0 L40 20 L0 20 Z",
            "M0 0 C0 40 60 40 60 0 Z",
            "M0 0 C0 40 60 40 60 0 Z M80 0 C80 20 100 20 100 0 L80 0 Z",
            "M0 0 C0 20 20 20 20 0 C20 -20 40 -20 40 0 Z",
        ] {
            for force_single_set in [false, true] {
                for target in [RoughGeometryTarget::Svg, RoughGeometryTarget::Query] {
                    let randomness =
                        || RoughRandomness::new(RoughJsSeed::new(seed), RoughMathRandom::new(7));
                    let expected_randomness = randomness();
                    let actual_randomness = randomness();
                    let expected =
                        outline_first_paths(path, &expected_randomness, force_single_set);
                    let meter = OperationWorkMeter::new(
                        RenderResourcePolicy::unbounded_for_trusted_input(),
                    );
                    let actual = prepare_solid_svg_path(
                        target,
                        path,
                        1.3,
                        "0 0",
                        &meter,
                        &actual_randomness,
                        force_single_set,
                    )
                    .expect("ordinary fixed HEAD path must remain renderable");
                    assert!(
                        actual.fill == expected.0,
                        "HEAD={SOURCE_COMMIT}, seed={seed}, fill, path={path}"
                    );
                    assert!(
                        actual.stroke == expected.1,
                        "HEAD={SOURCE_COMMIT}, seed={seed}, stroke, path={path}"
                    );
                    assert!(meter.used() > 0);
                    let mut expected_options = OptionsBuilder::default()
                        .randomness(expected_randomness)
                        .roughness(0.0)
                        .build()
                        .unwrap();
                    let mut actual_options = OptionsBuilder::default()
                        .randomness(actual_randomness)
                        .roughness(0.0)
                        .build()
                        .unwrap();
                    let expected_next =
                        zero_roughness_line([1.0, 2.0], [30.0, 40.0], &mut expected_options);
                    let actual_next =
                        roughr::renderer::line::<f64>(1.0, 2.0, 30.0, 40.0, &mut actual_options);
                    assert!(
                        actual_next == expected_next,
                        "HEAD={SOURCE_COMMIT}, seed={seed}, following path={path}"
                    );
                }
            }
        }
    }
}
