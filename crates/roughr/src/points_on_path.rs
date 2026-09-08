use std::fmt::Display;
use std::ops::MulAssign;

use euclid::default::Point2D;
use euclid::Trig;
use num_traits::{Float, FromPrimitive};
use svgtypes::{PathParser, PathSegment};

use crate::core::{_c, _cc};
use crate::curve_points::{points_on_bezier_curves, simplify};
use crate::svg_path::{absolutize, normalize};
use crate::SvgPathSegment;

mod bounded;
pub use bounded::try_points_on_normalized_segments;

/// Normalizes SVG segments with work admission before input processing and output growth.
///
/// Returns current-version M/L/C/Z segments without complete absolute/intermediate arrays.
/// Each arc uses the existing constant-size cubic conversion batch. A rejection returns no
/// partial path; invalid emitted coordinates return `InvalidGeometry`.
pub fn try_normalized_segments<E>(
    segments: &[impl SvgPathSegment],
    mut work: impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<PathSegment>, crate::generation::GenerationError<E>> {
    use crate::generation::GenerationError;
    use crate::svg_path::{normalize_into, NormalizationEvent};
    let mut output = Vec::new();
    let absolute = absolutize(segments.iter().copied().map(SvgPathSegment::into_current));
    normalize_into(absolute, |event| {
        work(1).map_err(GenerationError::Consumer)?;
        if let NormalizationEvent::Segment(segment) = event {
            let finite = match segment {
                PathSegment::MoveTo { x, y, .. } | PathSegment::LineTo { x, y, .. } => {
                    x.is_finite() && y.is_finite()
                }
                PathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                    ..
                } => [x1, y1, x2, y2, x, y].iter().all(|value| value.is_finite()),
                PathSegment::ClosePath { .. } => true,
                _ => false,
            };
            if !finite {
                return Err(GenerationError::InvalidGeometry);
            }
            output.try_reserve(1).map_err(GenerationError::Allocation)?;
            output.push(segment);
        }
        Ok(())
    })?;
    Ok(output)
}

#[cfg(test)]
mod normalization_tests {
    use super::*;
    use crate::generation::GenerationError;

    #[test]
    fn normalization_rejects_work_before_processing_the_whole_path() {
        let input = [PathSegment::LineTo {
            abs: false,
            x: 1.0,
            y: 2.0,
        }; 100];
        let mut used = 0;
        let result = try_normalized_segments(&input, |amount| {
            used += amount;
            if used > 4 {
                Err("cancelled")
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(GenerationError::Consumer("cancelled"))
        ));
        assert_eq!(used, 5);
    }

    #[test]
    fn normalization_keeps_relative_reflection_and_subpath_state() {
        let input = PathParser::from("m1 2 l3 4 h5 v6 c1 2 3 4 5 6 s3 4 5 6 q3 6 6 0 t6 0 z l1 0")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let expected = PathParser::from("M1 2 L4 6 L9 6 L9 12 C10 14 12 16 14 18 C16 20 17 22 19 24 C21 28 23 28 25 24 C27 20 29 20 31 24 Z L2 2")
            .collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            try_normalized_segments::<()>(&input, |_| Ok(())).unwrap(),
            expected
        );
    }

    #[test]
    fn normalization_admits_arc_expansion_and_rejects_nonfinite_output() {
        let input = PathParser::from("M0 0 A10 10 0 0 1 20 0")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let output = try_normalized_segments::<()>(&input, |_| Ok(())).unwrap();
        assert_eq!(
            output.len(),
            3,
            "one move and two source-compatible cubic segments"
        );
        let mut used = 0;
        let result = try_normalized_segments(&input, |units| {
            used += units;
            if used > 4 {
                Err("full")
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(GenerationError::Consumer("full"))));
        assert_eq!(used, 5, "reject the second cubic before retaining it");
        assert!(matches!(
            try_normalized_segments::<()>(
                &[PathSegment::LineTo {
                    abs: true,
                    x: f64::INFINITY,
                    y: 0.0
                }],
                |_| Ok(()),
            ),
            Err(GenerationError::InvalidGeometry)
        ));
    }
}

pub fn points_on_path<F>(
    path: String,
    tolerance: Option<F>,
    distance: Option<F>,
) -> Vec<Vec<Point2D<F>>>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
{
    let path_parser = PathParser::from(path.as_ref());
    let path_segments: Vec<PathSegment> = path_parser.flatten().collect();
    let normalized_segments = normalize(absolutize(path_segments.iter()));

    generate_points(tolerance, distance, normalized_segments)
}

pub fn points_on_segments<F>(
    path_segments: Vec<impl SvgPathSegment>,
    tolerance: Option<F>,
    distance: Option<F>,
) -> Vec<Vec<Point2D<F>>>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
{
    let path_segments: Vec<PathSegment> = path_segments
        .into_iter()
        .map(SvgPathSegment::into_current)
        .collect();
    let normalized_segments = normalize(absolutize(path_segments.iter()));
    generate_points(tolerance, distance, normalized_segments)
}

pub fn normalized_segments<S>(path_segments: &[S]) -> Vec<S>
where
    S: SvgPathSegment,
{
    let path_segments: Vec<PathSegment> = path_segments
        .iter()
        .copied()
        .map(SvgPathSegment::into_current)
        .collect();
    normalize(absolutize(path_segments.iter()))
        .map(S::from_current)
        .collect()
}

pub fn points_on_normalized_segments<F>(
    normalized_segments: &[impl SvgPathSegment],
    tolerance: Option<F>,
    distance: Option<F>,
) -> Vec<Vec<Point2D<F>>>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
{
    generate_points(
        tolerance,
        distance,
        normalized_segments
            .iter()
            .copied()
            .map(SvgPathSegment::into_current),
    )
}

fn generate_points<F>(
    tolerance: Option<F>,
    distance: Option<F>,
    normalized_segments: impl Iterator<Item = PathSegment>,
) -> Vec<Vec<euclid::Point2D<F, euclid::UnknownUnit>>>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
{
    let mut sets: Vec<Vec<Point2D<F>>> = vec![];
    let mut current_points: Vec<Point2D<F>> = vec![];
    let mut start = Point2D::new(_c::<F>(0.0), _c::<F>(0.0));
    let mut pending_curve: Vec<Point2D<F>> = vec![];

    let append_pending_curve =
        |current_points: &mut Vec<Point2D<F>>, pending_curve: &mut Vec<Point2D<F>>| {
            if pending_curve.len() >= 4 {
                current_points.append(&mut points_on_bezier_curves(
                    &pending_curve[..],
                    tolerance.unwrap_or(_c(0.0)),
                    None,
                ));
            }
            pending_curve.clear();
        };

    let mut append_pending_points =
        |current_points: &mut Vec<Point2D<F>>, pending_curve: &mut Vec<Point2D<F>>| {
            {
                append_pending_curve(current_points, pending_curve);
            }
            if !current_points.is_empty() {
                sets.push(current_points.clone());
                current_points.clear();
            }
        };

    for segment in normalized_segments {
        match segment {
            PathSegment::MoveTo { abs: true, x, y } => {
                append_pending_points(&mut current_points, &mut pending_curve);
                start = Point2D::new(_cc::<F>(x), _cc::<F>(y));
                current_points.push(start);
            }
            PathSegment::LineTo { abs: true, x, y } => {
                append_pending_curve(&mut current_points, &mut pending_curve);
                current_points.push(Point2D::new(_cc::<F>(x), _cc::<F>(y)));
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
                if pending_curve.is_empty() {
                    let last_point = if !current_points.is_empty() {
                        current_points.last().unwrap()
                    } else {
                        &start
                    };
                    pending_curve.push(*last_point);
                }
                pending_curve.push(Point2D::new(_cc::<F>(x1), _cc::<F>(y1)));
                pending_curve.push(Point2D::new(_cc::<F>(x2), _cc::<F>(y2)));
                pending_curve.push(Point2D::new(_cc::<F>(x), _cc::<F>(y)));
            }
            PathSegment::ClosePath { abs: true } => {
                append_pending_curve(&mut current_points, &mut pending_curve);
                current_points.push(start);
            }
            _ => panic!("unexpected  path segment"),
        }
    }

    append_pending_points(&mut current_points, &mut pending_curve);

    if let Some(dst) = distance {
        let mut out = vec![];
        for set in sets.iter() {
            let simplified_set = simplify(set, dst);
            if !simplified_set.is_empty() {
                out.push(simplified_set);
            }
        }
        out
    } else {
        sets
    }
}

#[cfg(all(test, feature = "legacy-compat"))]
mod tests {
    use svgtypes_0_11::{PathParser, PathSegment};

    use super::normalized_segments;
    use crate::core::OptionsBuilder;

    #[test]
    fn legacy_svgtypes_segments_remain_renderable() {
        let segments: Vec<PathSegment> = PathParser::from("M 1 2 l 3 4 z")
            .collect::<Result<_, _>>()
            .unwrap();
        let normalized = normalized_segments(&segments);
        let bounded = super::try_normalized_segments::<()>(&segments, |_| Ok(())).unwrap();
        assert_eq!(
            bounded,
            normalized
                .iter()
                .copied()
                .map(crate::SvgPathSegment::into_current)
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            normalized.as_slice(),
            [
                PathSegment::MoveTo {
                    abs: true,
                    x: 1.0,
                    y: 2.0
                },
                PathSegment::LineTo {
                    abs: true,
                    x: 4.0,
                    y: 6.0
                },
                PathSegment::ClosePath { abs: true }
            ]
        ));

        let points = super::points_on_normalized_segments::<f64>(&normalized, Some(1.0), None);
        assert_eq!(points.len(), 1);

        let mut options = OptionsBuilder::default().seed(1_u64).build().unwrap();
        let rendered = crate::renderer::svg_normalized_segments::<f64>(&normalized, &mut options);
        assert!(!rendered.ops.is_empty());
    }
}
