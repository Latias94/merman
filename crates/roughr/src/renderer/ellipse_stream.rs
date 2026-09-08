//! Bounded ellipse sampling with a rolling curve window.

use super::*;
use crate::generation::{GenerationError, GenerationEvent};

/// Streams the outline and returns its estimated polygon for subsequent pattern filling.
///
/// Discard both output and mutated random state on error. Work events admit sampling before
/// allocation; rejecting an output operation stops subsequent sampling as well.
pub fn try_ellipse_with_params<F, E>(
    x: F,
    y: F,
    options: &mut Options,
    params: &EllipseParams<F>,
    mut consume: impl FnMut(GenerationEvent<F>) -> Result<(), E>,
) -> Result<Vec<Point2D<F>>, GenerationError<E>>
where
    F: Float + Trig + FromPrimitive,
{
    consume(GenerationEvent::Work(1)).map_err(GenerationError::Consumer)?;
    if ![x, y, params.rx, params.ry, params.increment]
        .iter()
        .all(|value| value.is_finite())
        || params.increment <= F::zero()
    {
        return Err(GenerationError::InvalidGeometry);
    }
    let overlap = params.increment
        * _offset(
            _c(0.1),
            _offset(_c::<F>(0.4), _c::<F>(1.0), options, None),
            options,
            None,
        );
    let spec = EllipsePointsSpec {
        increment: params.increment,
        cx: x,
        cy: y,
        rx: params.rx,
        ry: params.ry,
        offset: _c(1.0),
        overlap,
    };
    let estimated = sample_outline(spec, true, options, &mut consume)?;
    if !options.disable_multi_stroke.unwrap_or(false) && options.roughness.unwrap_or(0.0) != 0.0 {
        sample_outline(
            EllipsePointsSpec {
                offset: _c(1.5),
                overlap: F::zero(),
                ..spec
            },
            false,
            options,
            &mut consume,
        )?;
    }
    Ok(estimated)
}

struct CurveWindow<'a, F: Float + Trig, C> {
    consume: &'a mut C,
    points: [Point2D<F>; 4],
    filled: usize,
    started: bool,
    tension: F,
    retain_core: bool,
    estimated: Vec<Point2D<F>>,
}

impl<F: Float + Trig + FromPrimitive, C> CurveWindow<'_, F, C> {
    fn sample<E>(
        &mut self,
        core: bool,
        make_point: impl FnOnce() -> Point2D<F>,
    ) -> Result<(), GenerationError<E>>
    where
        C: FnMut(GenerationEvent<F>) -> Result<(), E>,
    {
        (self.consume)(GenerationEvent::Work(1)).map_err(GenerationError::Consumer)?;
        let point = make_point();
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(GenerationError::InvalidGeometry);
        }
        if core && self.retain_core {
            self.estimated
                .try_reserve(1)
                .map_err(GenerationError::Allocation)?;
            self.estimated.push(point);
        }
        if self.filled < 4 {
            self.points[self.filled] = point;
            self.filled += 1;
        } else {
            self.points.copy_within(1..4, 0);
            self.points[3] = point;
        }
        if self.filled == 4 {
            let [previous, start, end, next] = self.points;
            if !self.started {
                self.emit(OpType::Move, &[start.x, start.y])?;
                self.started = true;
            }
            // Keep _curve's exact evaluation order; algebraic rewrites change seeded floats.
            let s = self.tension;
            self.emit(
                OpType::BCurveTo,
                &[
                    start.x + (s * end.x - s * previous.x) / _c(6.0),
                    start.y + (s * end.y - s * previous.y) / _c(6.0),
                    end.x + (s * start.x - s * next.x) / _c(6.0),
                    end.y + (s * start.y - s * next.y) / _c(6.0),
                    end.x,
                    end.y,
                ],
            )?;
        }
        Ok(())
    }

    fn emit<E>(&mut self, op: OpType, coordinates: &[F]) -> Result<(), GenerationError<E>>
    where
        C: FnMut(GenerationEvent<F>) -> Result<(), E>,
    {
        if !coordinates.iter().all(|value| value.is_finite()) {
            return Err(GenerationError::InvalidGeometry);
        }
        let mut data = Vec::new();
        data.try_reserve_exact(coordinates.len())
            .map_err(GenerationError::Allocation)?;
        data.extend_from_slice(coordinates);
        (self.consume)(GenerationEvent::Op(Op { op, data })).map_err(GenerationError::Consumer)
    }
}

fn advance<F: Float, E>(angle: F, increment: F) -> Result<F, GenerationError<E>> {
    let next = angle + increment;
    if !next.is_finite() || next <= angle {
        Err(GenerationError::InvalidGeometry)
    } else {
        Ok(next)
    }
}

fn sample_outline<F, E>(
    spec: EllipsePointsSpec<F>,
    retain_core: bool,
    options: &mut Options,
    consume: &mut impl FnMut(GenerationEvent<F>) -> Result<(), E>,
) -> Result<Vec<Point2D<F>>, GenerationError<E>>
where
    F: Float + Trig + FromPrimitive,
{
    let EllipsePointsSpec {
        increment,
        cx,
        cy,
        rx,
        ry,
        offset,
        overlap,
    } = spec;
    let mut window = CurveWindow {
        consume,
        points: [Point2D::new(F::zero(), F::zero()); 4],
        filled: 0,
        started: false,
        tension: _c::<F>(1.0) - _c(options.curve_tightness.unwrap_or(0.0)),
        retain_core,
        estimated: Vec::new(),
    };
    if options.roughness.unwrap_or(0.0) == 0.0 {
        let increment = increment / _c(4.0);
        if increment <= F::zero() {
            return Err(GenerationError::InvalidGeometry);
        }
        window.sample(false, || {
            Point2D::new(
                cx + rx * Float::cos(-increment),
                cy + ry * Float::sin(-increment),
            )
        })?;
        let mut angle = _c(0.0);
        while angle <= _cc::<F>(std::f64::consts::PI * 2.0) {
            window.sample(true, || {
                Point2D::new(cx + rx * Float::cos(angle), cy + ry * Float::sin(angle))
            })?;
            angle = advance(angle, increment)?;
        }
        window.sample(false, || {
            Point2D::new(cx + rx * Float::cos(_c(0.0)), cy + ry * Float::sin(_c(0.0)))
        })?;
        window.sample(false, || {
            Point2D::new(
                cx + rx * Float::cos(increment),
                cy + ry * Float::sin(increment),
            )
        })?;
    } else {
        let rad_offset: F =
            _offset_opt::<F>(_c(0.5), options, None) - (_c::<F>(f32::PI()) / _c(2.0));
        window.sample(false, || {
            Point2D::new(
                _offset_opt(offset, options, None)
                    + cx
                    + _c::<F>(0.9) * rx * Float::cos(rad_offset - increment),
                _offset_opt(offset, options, None)
                    + cy
                    + _c::<F>(0.9) * ry * Float::sin(rad_offset - increment),
            )
        })?;
        let end_angle = _cc::<F>(std::f64::consts::PI * 2.0) + rad_offset - _c(0.01);
        let mut angle = rad_offset;
        while angle < end_angle {
            window.sample(true, || {
                Point2D::new(
                    _offset_opt(offset, options, None) + cx + rx * Float::cos(angle),
                    _offset_opt(offset, options, None) + cy + ry * Float::sin(angle),
                )
            })?;
            angle = advance(angle, increment)?;
        }
        window.sample(false, || {
            Point2D::new(
                _offset_opt(offset, options, None)
                    + cx
                    + rx * Float::cos(
                        rad_offset + _cc::<F>(std::f64::consts::PI * 2.0) + overlap * _c(0.5),
                    ),
                _offset_opt(offset, options, None)
                    + cy
                    + ry * Float::sin(
                        rad_offset + _cc::<F>(std::f64::consts::PI * 2.0) + overlap * _c(0.5),
                    ),
            )
        })?;
        window.sample(false, || {
            Point2D::new(
                _offset_opt(offset, options, None)
                    + cx
                    + _c::<F>(0.98) * rx * Float::cos(rad_offset + overlap),
                _offset_opt(offset, options, None)
                    + cy
                    + _c::<F>(0.98) * ry * Float::sin(rad_offset + overlap),
            )
        })?;
        window.sample(false, || {
            Point2D::new(
                _offset_opt(offset, options, None)
                    + cx
                    + _c::<F>(0.9) * rx * Float::cos(rad_offset + overlap * _c(0.5)),
                _offset_opt(offset, options, None)
                    + cy
                    + _c::<F>(0.9) * ry * Float::sin(rad_offset + overlap * _c(0.5)),
            )
        })?;
    }
    Ok(window.estimated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{OptionsBuilder, RoughJsSeed, RoughMathRandom, RoughRandomness};

    fn options(seed: f64, roughness: f32) -> Options {
        OptionsBuilder::default()
            .roughness(roughness)
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(seed),
                RoughMathRandom::new(123),
            ))
            .build()
            .unwrap()
    }

    #[test]
    fn ellipse_sampling_can_cancel_before_the_complete_point_array() {
        let params = EllipseParams {
            rx: 20.0,
            ry: 30.0,
            increment: 0.001,
        };
        let mut work = 0;
        let result =
            try_ellipse_with_params(10.0, 20.0, &mut options(1.0, 0.7), &params, |event| {
                if let GenerationEvent::Work(amount) = event {
                    work += amount;
                    if work > 16 {
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
    fn ellipse_stream_matches_points_operations_and_next_random_draw() {
        for seed in [0.0, 1.0, 42.0] {
            for roughness in [0.0, 0.7] {
                for increment in [0.7, 7.0, 100.0] {
                    let params = EllipseParams {
                        rx: 80.0,
                        ry: 30.0,
                        increment,
                    };
                    let mut expected_options = options(seed, roughness);
                    let mut actual_options = options(seed, roughness);
                    let expected = ellipse_with_params(10.0, 20.0, &mut expected_options, &params);
                    let mut ops = Vec::new();
                    let points = try_ellipse_with_params(
                        10.0,
                        20.0,
                        &mut actual_options,
                        &params,
                        |event| {
                            if let GenerationEvent::Op(op) = event {
                                ops.push(op);
                            }
                            Ok::<_, ()>(())
                        },
                    )
                    .unwrap();
                    assert_eq!(points, expected.estimated_points);
                    assert_eq!(ops, expected.opset.ops);
                    assert_eq!(actual_options.random(), expected_options.random());
                }
            }
        }
    }

    #[test]
    fn output_rejection_stops_sampling_a_high_resolution_ellipse() {
        let params = EllipseParams {
            rx: 20.0,
            ry: 30.0,
            increment: 1e-10,
        };
        let mut work = 0;
        let mut ops = 0;
        let result =
            try_ellipse_with_params(10.0, 20.0, &mut options(1.0, 0.7), &params, |event| {
                match event {
                    GenerationEvent::Work(amount) => work += amount,
                    GenerationEvent::Op(_) => {
                        ops += 1;
                        if ops > 1 {
                            return Err("full");
                        }
                    }
                }
                Ok(())
            });
        assert!(matches!(result, Err(GenerationError::Consumer("full"))));
        assert_eq!(ops, 2);
        assert_eq!(
            work, 5,
            "one entry check and four points, not a completed ellipse"
        );
    }

    #[test]
    fn invalid_or_non_advancing_increments_fail_without_unbounded_sampling() {
        for increment in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-300] {
            let params = EllipseParams {
                rx: 20.0,
                ry: 30.0,
                increment,
            };
            let mut work = 0;
            let result =
                try_ellipse_with_params(10.0, 20.0, &mut options(1.0, 0.7), &params, |event| {
                    if let GenerationEvent::Work(amount) = event {
                        work += amount;
                    }
                    assert!(work < 10);
                    Ok::<_, ()>(())
                });
            assert!(matches!(result, Err(GenerationError::InvalidGeometry)));
        }
    }

    #[test]
    fn f32_sampling_matches_the_collector_and_rejects_step_underflow() {
        let params = EllipseParams {
            rx: 80.0_f32,
            ry: 30.0,
            increment: 100.0,
        };
        for roughness in [0.0, 0.7] {
            let mut expected_options = options(0.0, roughness);
            let mut actual_options = options(0.0, roughness);
            let expected = ellipse_with_params(10.0, 20.0, &mut expected_options, &params);
            let mut ops = Vec::new();
            let estimated =
                try_ellipse_with_params(10.0, 20.0, &mut actual_options, &params, |event| {
                    if let GenerationEvent::Op(op) = event {
                        ops.push(op);
                    }
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(estimated, expected.estimated_points);
            assert_eq!(ops, expected.opset.ops);
            assert_eq!(actual_options.random(), expected_options.random());
        }
        let params = EllipseParams {
            increment: f32::from_bits(1),
            ..params
        };
        assert!(matches!(
            try_ellipse_with_params(
                10.0,
                20.0,
                &mut options(1.0, 0.0),
                &params,
                |_| Ok::<_, ()>(())
            ),
            Err(GenerationError::InvalidGeometry)
        ));
    }
}
