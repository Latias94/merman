//! Fallible, non-recursive sampling of normalized M/L/C/Z paths.

use super::*;
use crate::curve_points::flatness;
use crate::generation::GenerationError;

/// Samples already normalized paths, admitting work before scratch/output growth.
///
/// Curve subdivision and Douglas-Peucker scans are cancellable. On error no polygon is
/// returned. Parsing and normalization belong to the caller, not to this bounded phase.
pub fn try_points_on_normalized_segments<F, E>(
    segments: &[impl SvgPathSegment],
    tolerance: F,
    distance: Option<F>,
    mut work: impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<Vec<Point2D<F>>>, GenerationError<E>>
where
    F: FromPrimitive + Trig + Float + MulAssign + Display,
{
    admit(&mut work)?;
    if !tolerance.is_finite()
        || tolerance <= F::zero()
        || distance.is_some_and(|value| !value.is_finite() || value < F::zero())
    {
        return Err(GenerationError::InvalidGeometry);
    }
    let mut sets = Vec::new();
    let mut current = Vec::new();
    let mut start = Point2D::new(F::zero(), F::zero());
    let mut cursor = start;
    // The collecting algorithm suppresses duplicate points within each contiguous curve
    // run, but not between that run and preceding Move/Line points.
    let mut last_curve_point = None;
    for segment in segments {
        admit(&mut work)?;
        match segment.into_current() {
            PathSegment::MoveTo { abs: true, x, y } => {
                finish_polygon(&mut current, &mut sets, distance, &mut work)?;
                start = point(x, y)?;
                cursor = start;
                last_curve_point = None;
                push(&mut current, start, &mut work)?;
            }
            PathSegment::LineTo { abs: true, x, y } => {
                cursor = point(x, y)?;
                last_curve_point = None;
                push(&mut current, cursor, &mut work)?;
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
                let end = point(x, y)?;
                sample_curve(
                    [cursor, point(x1, y1)?, point(x2, y2)?, end],
                    tolerance,
                    &mut current,
                    &mut last_curve_point,
                    &mut work,
                )?;
                cursor = end;
            }
            PathSegment::ClosePath { abs: true } => {
                cursor = start;
                last_curve_point = None;
                push(&mut current, start, &mut work)?;
            }
            _ => return Err(GenerationError::InvalidGeometry),
        }
    }
    finish_polygon(&mut current, &mut sets, distance, &mut work)?;
    Ok(sets)
}

fn admit<E>(work: &mut impl FnMut(usize) -> Result<(), E>) -> Result<(), GenerationError<E>> {
    work(1).map_err(GenerationError::Consumer)
}

fn push<T, E>(
    output: &mut Vec<T>,
    value: T,
    work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), GenerationError<E>> {
    admit(work)?;
    output.try_reserve(1).map_err(GenerationError::Allocation)?;
    output.push(value);
    Ok(())
}

fn point<F: Float, E>(x: f64, y: f64) -> Result<Point2D<F>, GenerationError<E>> {
    let x = F::from(x).ok_or(GenerationError::InvalidGeometry)?;
    let y = F::from(y).ok_or(GenerationError::InvalidGeometry)?;
    if x.is_finite() && y.is_finite() {
        Ok(Point2D::new(x, y))
    } else {
        Err(GenerationError::InvalidGeometry)
    }
}

fn sample_curve<F, E>(
    curve: [Point2D<F>; 4],
    tolerance: F,
    output: &mut Vec<Point2D<F>>,
    last: &mut Option<Point2D<F>>,
    work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), GenerationError<E>>
where
    F: Float + MulAssign,
{
    let mut stack = Vec::new();
    push(&mut stack, curve, work)?;
    while let Some(curve) = stack.pop() {
        admit(work)?;
        let measure = flatness(&curve, 0);
        if !measure.is_finite() {
            return Err(GenerationError::InvalidGeometry);
        }
        if measure < tolerance {
            let insert_start = match *last {
                None => true,
                Some(previous) => {
                    let distance = previous.distance_to(curve[0]);
                    if !distance.is_finite() {
                        return Err(GenerationError::InvalidGeometry);
                    }
                    distance > F::one()
                }
            };
            if insert_start {
                push(output, curve[0], work)?;
            }
            push(output, curve[3], work)?;
            *last = Some(curve[3]);
        } else {
            let half = F::one() / (F::one() + F::one());
            let [a, b, c, d] = curve;
            let ab = a.lerp(b, half);
            let bc = b.lerp(c, half);
            let cd = c.lerp(d, half);
            let abc = ab.lerp(bc, half);
            let bcd = bc.lerp(cd, half);
            let mid = abc.lerp(bcd, half);
            let left = [a, ab, abc, mid];
            let right = [mid, bcd, cd, d];
            if left == curve || right == curve {
                return Err(GenerationError::InvalidGeometry);
            }
            // LIFO preserves the original recursive left-before-right sample order.
            push(&mut stack, right, work)?;
            push(&mut stack, left, work)?;
        }
    }
    Ok(())
}

fn finish_polygon<F, E>(
    current: &mut Vec<Point2D<F>>,
    sets: &mut Vec<Vec<Point2D<F>>>,
    distance: Option<F>,
    work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), GenerationError<E>>
where
    F: Float,
{
    if !current.is_empty() {
        let points = std::mem::take(current);
        let points = if let Some(distance) = distance {
            simplify(&points, distance, work)?
        } else {
            points
        };
        push(sets, points, work)?;
    }
    Ok(())
}

fn simplify<F: Float, E>(
    points: &[Point2D<F>],
    epsilon: F,
    work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Vec<Point2D<F>>, GenerationError<E>> {
    let mut output = Vec::new();
    let mut stack = Vec::new();
    push(&mut stack, (0, points.len()), work)?;
    while let Some((start, end)) = stack.pop() {
        admit(work)?;
        let first = points[start];
        let last = points[end - 1];
        let mut max_distance = F::zero();
        let mut split = start;
        for (index, point) in points.iter().enumerate().take(end - 1).skip(start + 1) {
            admit(work)?;
            let distance = segment_distance_squared(*point, first, last)?;
            if distance > max_distance {
                max_distance = distance;
                split = index;
            }
        }
        if max_distance.sqrt() > epsilon {
            push(&mut stack, (split, end), work)?;
            push(&mut stack, (start, split + 1), work)?;
        } else {
            if output.is_empty() {
                push(&mut output, first, work)?;
            }
            push(&mut output, last, work)?;
        }
    }
    Ok(output)
}

fn segment_distance_squared<F: Float, E>(
    point: Point2D<F>,
    start: Point2D<F>,
    end: Point2D<F>,
) -> Result<F, GenerationError<E>> {
    let length = start.distance_to(end).powi(2);
    let distance = if length == F::zero() {
        point.distance_to(start).powi(2)
    } else {
        let fraction = ((point.x - start.x) * (end.x - start.x)
            + (point.y - start.y) * (end.y - start.y))
            / length;
        if !length.is_finite() || !fraction.is_finite() {
            return Err(GenerationError::InvalidGeometry);
        }
        point
            .distance_to(start.lerp(end, fraction.max(F::zero()).min(F::one())))
            .powi(2)
    };
    if distance.is_finite() {
        Ok(distance)
    } else {
        Err(GenerationError::InvalidGeometry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(path: &str) -> Vec<PathSegment> {
        let parsed = PathParser::from(path)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        normalized_segments(&parsed)
    }

    #[test]
    fn sampling_budget_can_stop_inside_one_curve() {
        let input = segments("M0 0 C0 10000 10000 10000 10000 0");
        let mut used = 0;
        let result =
            try_points_on_normalized_segments::<f64, _>(&input, 1.0, Some(0.85), |units| {
                used += units;
                if used > 8 {
                    Err("cancelled")
                } else {
                    Ok(())
                }
            });
        assert!(matches!(
            result,
            Err(GenerationError::Consumer("cancelled"))
        ));
    }

    #[test]
    fn sampling_matches_collecting_curve_and_simplification_order() {
        for path in [
            "M0 0 C0 100 100 100 100 0 C100 -100 0 -100 0 0 Z",
            "M0 0 C10 0 20 0 30 0 L40 0 C40 20 60 20 60 0 M1 2 L3 4 Z",
            "M0 0 A80 80 0 1 0 160 0 A80 80 0 1 0 0 0",
            "M1 2 M3 4",
            "",
        ] {
            let input = segments(path);
            for distance in [None, Some(0.0), Some(0.85), Some(10.0)] {
                let expected = points_on_normalized_segments::<f64>(&input, Some(1.0), distance);
                let actual =
                    try_points_on_normalized_segments::<f64, ()>(&input, 1.0, distance, |_| Ok(()))
                        .unwrap();
                assert_eq!(actual, expected, "{path}, distance={distance:?}");
            }
        }
    }

    #[test]
    fn simplification_scans_are_cancellable_and_keep_the_original_order() {
        let points = (0..128)
            .map(|index| Point2D::new(f64::from(index), f64::from(index % 2)))
            .collect::<Vec<_>>();
        let expected = crate::curve_points::simplify(&points, 0.85);
        let actual = simplify::<_, ()>(&points, 0.85, &mut |_| Ok(())).unwrap();
        assert_eq!(actual, expected);
        let mut used = 0;
        let error = simplify(&points, 0.85, &mut |units| {
            used += units;
            if used > 8 {
                Err("cancelled")
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(matches!(error, GenerationError::Consumer("cancelled")));
        assert_eq!(
            used, 9,
            "stop within the first scan, not after a complete simplified set"
        );
    }

    #[test]
    fn invalid_sampling_inputs_fail_without_recursive_overflow() {
        let input = segments("M0 0 C0 100 100 100 100 0");
        for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                try_points_on_normalized_segments::<f64, ()>(&input, tolerance, None, |_| Ok(())),
                Err(GenerationError::InvalidGeometry)
            ));
        }
        let invalid = [PathSegment::MoveTo {
            abs: true,
            x: f64::NAN,
            y: 0.0,
        }];
        assert!(matches!(
            try_points_on_normalized_segments::<f64, ()>(&invalid, 1.0, None, |_| Ok(())),
            Err(GenerationError::InvalidGeometry)
        ));
    }
}
