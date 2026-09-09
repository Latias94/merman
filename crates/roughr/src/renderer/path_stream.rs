//! Operation production from normalized paths without retaining a complete outline.

use super::*;
use crate::generation::{GenerationError, GenerationEvent};

/// Streams normalized M/L/C/Z paths, admitting each source segment before generation.
///
/// Discard output and mutated random state on error. The existing per-segment Rough.js
/// operation batches have constant size; this function never collects the complete path.
pub fn try_svg_normalized_segments<F, E>(
    segments: &[impl SvgPathSegment],
    options: &mut Options,
    mut consume: impl FnMut(GenerationEvent<F>) -> Result<(), E>,
) -> Result<(), GenerationError<E>>
where
    F: Float + FromPrimitive + Trig,
{
    consume(GenerationEvent::Work(1)).map_err(GenerationError::Consumer)?;
    let mut current = Point2D::new(F::zero(), F::zero());
    let mut first = current;
    for segment in segments {
        consume(GenerationEvent::Work(1)).map_err(GenerationError::Consumer)?;
        let ops = match segment.into_current() {
            PathSegment::MoveTo { abs: true, x, y } => {
                current = point(x, y)?;
                first = current;
                continue;
            }
            PathSegment::LineTo { abs: true, x, y } => {
                let end = point(x, y)?;
                let ops = _double_line(current.x, current.y, end.x, end.y, options, false);
                current = end;
                ops
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
                let control1 = point(x1, y1)?;
                let control2 = point(x2, y2)?;
                let end = point(x, y)?;
                let ops = _bezier_to(
                    BezierToSpec {
                        x1: control1.x,
                        y1: control1.y,
                        x2: control2.x,
                        y2: control2.y,
                        x: end.x,
                        y: end.y,
                        current,
                    },
                    options,
                );
                current = end;
                ops
            }
            PathSegment::ClosePath { abs: true } => {
                let ops = _double_line(current.x, current.y, first.x, first.y, options, false);
                current = first;
                ops
            }
            _ => return Err(GenerationError::InvalidGeometry),
        };
        for op in ops {
            if !op.data.iter().all(|value| value.is_finite()) {
                return Err(GenerationError::InvalidGeometry);
            }
            consume(GenerationEvent::Op(op)).map_err(GenerationError::Consumer)?;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{OptionsBuilder, RoughJsSeed, RoughMathRandom, RoughRandomness};

    fn options(seed: f64) -> Options {
        OptionsBuilder::default()
            .roughness(0.7)
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(seed),
                RoughMathRandom::new(123),
            ))
            .build()
            .unwrap()
    }

    #[test]
    fn outline_generation_stops_at_the_next_source_segment() {
        let segments = [PathSegment::LineTo {
            abs: true,
            x: 10.0,
            y: 20.0,
        }; 100];
        let mut work = 0;
        let result = try_svg_normalized_segments::<f64, _>(&segments, &mut options(1.0), |event| {
            if let GenerationEvent::Work(units) = event {
                work += units;
                if work > 2 {
                    return Err("cancelled");
                }
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(GenerationError::Consumer("cancelled"))
        ));
    }

    #[test]
    fn streamed_outline_preserves_operations_and_next_random_draw() {
        let parsed = PathParser::from("M0 0 L10 20 C10 40 50 40 50 0 Z M5 5 L9 9")
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let segments = crate::points_on_path::normalized_segments(&parsed);
        for seed in [0.0, 1.0, 42.0] {
            let mut expected_options = options(seed);
            let expected = svg_normalized_segments::<f64>(&segments, &mut expected_options);
            let mut actual_options = options(seed);
            let mut ops = Vec::new();
            try_svg_normalized_segments::<f64, ()>(&segments, &mut actual_options, |event| {
                if let GenerationEvent::Op(op) = event {
                    ops.push(op);
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(ops, expected.ops);
            assert_eq!(actual_options.random(), expected_options.random());
        }
    }
}
