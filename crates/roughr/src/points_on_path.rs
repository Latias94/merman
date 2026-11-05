use std::fmt::Display;
use std::ops::MulAssign;

use euclid::default::Point2D;
use euclid::Trig;
use num_traits::{Float, FromPrimitive};
use svgtypes::{PathParser, PathSegment};

use crate::core::{_c, _cc};
use crate::curve_points::{points_on_bezier_curves_with_control, simplify_with_control};
use crate::svg_path::{absolutize, normalize};
use crate::work::{finite, UnlimitedWork, WorkControl, WorkError};
use crate::SvgPathSegment;

pub fn normalized_path_with_control(
    path: &str,
    control: &mut impl WorkControl,
) -> Result<Vec<PathSegment>, WorkError> {
    control.charge(path.len())?;
    let mut segments = Vec::new();
    for segment in PathParser::from(path) {
        control.charge(1)?;
        segments.push(segment.map_err(|_| WorkError::InvalidGeometry)?);
    }
    let absolute = crate::svg_path::absolutize_with_control(segments.iter(), control)?;
    Ok(crate::svg_path::normalize_with_control(absolute, control)?.collect())
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

pub fn points_on_normalized_segments_with_control<F>(
    normalized_segments: &[impl SvgPathSegment],
    tolerance: Option<F>,
    distance: Option<F>,
    control: &mut impl WorkControl,
) -> Result<Vec<Vec<Point2D<F>>>, WorkError>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
{
    generate_points_with_control(
        tolerance,
        distance,
        normalized_segments
            .iter()
            .copied()
            .map(SvgPathSegment::into_current),
        control,
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
    generate_points_with_control(tolerance, distance, normalized_segments, &mut UnlimitedWork)
        .expect("invalid SVG path sampling")
}

fn generate_points_with_control<F, Control>(
    tolerance: Option<F>,
    distance: Option<F>,
    normalized_segments: impl Iterator<Item = PathSegment>,
    control: &mut Control,
) -> Result<Vec<Vec<Point2D<F>>>, WorkError>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
    Control: WorkControl,
{
    let mut sets: Vec<Vec<Point2D<F>>> = vec![];
    let mut current_points: Vec<Point2D<F>> = vec![];
    let mut start = Point2D::new(_c::<F>(0.0), _c::<F>(0.0));
    let mut pending_curve: Vec<Point2D<F>> = vec![];

    let append_pending_curve = |current_points: &mut Vec<Point2D<F>>,
                                pending_curve: &mut Vec<Point2D<F>>,
                                control: &mut Control|
     -> Result<(), WorkError> {
        if pending_curve.len() >= 4 {
            current_points.append(&mut points_on_bezier_curves_with_control(
                &pending_curve[..],
                tolerance.unwrap_or(_c(0.0)),
                None,
                control,
            )?);
        }
        pending_curve.clear();
        Ok(())
    };

    let mut append_pending_points = |current_points: &mut Vec<Point2D<F>>,
                                     pending_curve: &mut Vec<Point2D<F>>,
                                     control: &mut Control|
     -> Result<(), WorkError> {
        {
            append_pending_curve(current_points, pending_curve, control)?;
        }
        if !current_points.is_empty() {
            control.charge(1)?;
            sets.push(std::mem::take(current_points));
        }
        Ok(())
    };

    for segment in normalized_segments {
        control.charge(4)?;
        crate::svg_path::finite_segment(segment)?;
        match segment {
            PathSegment::MoveTo { abs: true, x, y } => {
                append_pending_points(&mut current_points, &mut pending_curve, control)?;
                start = Point2D::new(_cc::<F>(x), _cc::<F>(y));
                current_points.push(start);
            }
            PathSegment::LineTo { abs: true, x, y } => {
                append_pending_curve(&mut current_points, &mut pending_curve, control)?;
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
                append_pending_curve(&mut current_points, &mut pending_curve, control)?;
                current_points.push(start);
            }
            _ => return Err(WorkError::InvalidGeometry),
        }
    }

    append_pending_points(&mut current_points, &mut pending_curve, control)?;

    if let Some(dst) = distance {
        finite(&[dst])?;
        let mut out = vec![];
        for set in sets.iter() {
            control.charge(1)?;
            let simplified_set = simplify_with_control(set, dst, control)?;
            if !simplified_set.is_empty() {
                out.push(simplified_set);
            }
        }
        Ok(out)
    } else {
        Ok(sets)
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
