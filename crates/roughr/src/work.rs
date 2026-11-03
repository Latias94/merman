use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkError {
    Interrupted,
    InvalidGeometry,
    ArithmeticOverflow,
    UnsupportedFill,
}

impl std::fmt::Display for WorkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Interrupted => "rough generation interrupted",
            Self::InvalidGeometry => "nonfinite or nonadvancing rough geometry",
            Self::ArithmeticOverflow => "rough generation work overflow",
            Self::UnsupportedFill => "unsupported controlled rough fill style",
        })
    }
}

impl std::error::Error for WorkError {}

pub trait WorkControl {
    fn charge(&mut self, units: usize) -> Result<(), WorkError>;
}

pub(crate) struct UnlimitedWork;

impl WorkControl for UnlimitedWork {
    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }
}

pub(crate) fn finite<F: num_traits::Float>(values: &[F]) -> Result<(), WorkError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(WorkError::InvalidGeometry)
    }
}

pub(crate) fn advance<F: num_traits::Float>(value: F, increment: F) -> Result<F, WorkError> {
    let next = value + increment;
    if next.is_finite() && next > value {
        Ok(next)
    } else {
        Err(WorkError::InvalidGeometry)
    }
}

pub(crate) fn sort_by<T: Clone>(
    values: &mut [T],
    mut compare: impl FnMut(&T, &T) -> Ordering,
    control: &mut impl WorkControl,
) -> Result<(), WorkError> {
    for chunk in values.chunks_mut(32) {
        control.charge(
            chunk
                .len()
                .checked_mul(32)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )?;
        chunk.sort_by(&mut compare);
    }
    if values.len() <= 32 {
        return Ok(());
    }
    let mut scratch = Vec::new();
    for value in values.iter() {
        control.charge(1)?;
        scratch.push(value.clone());
    }
    let mut width = 32usize;
    while width < values.len() {
        let stride = width.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?;
        for start in (0..values.len()).step_by(stride) {
            let middle = start.saturating_add(width).min(values.len());
            let end = start.saturating_add(stride).min(values.len());
            let (mut left, mut right) = (start, middle);
            for output in &mut scratch[start..end] {
                control.charge(1)?;
                let index = if left < middle
                    && (right == end || compare(&values[left], &values[right]) != Ordering::Greater)
                {
                    let index = left;
                    left += 1;
                    index
                } else {
                    let index = right;
                    right += 1;
                    index
                };
                *output = values[index].clone();
            }
            for (output, value) in values[start..end].iter_mut().zip(&scratch[start..end]) {
                control.charge(1)?;
                *output = value.clone();
            }
        }
        width = stride;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        FillStyle, Options, OptionsBuilder, RoughJsSeed, RoughMathRandom, RoughRandomness,
    };
    use crate::generator::Generator;
    use crate::Point2D;

    struct Counter {
        maximum: usize,
        used: usize,
        rejected: usize,
    }

    impl Counter {
        fn new(maximum: usize) -> Self {
            Self {
                maximum,
                used: 0,
                rejected: 0,
            }
        }
    }

    impl WorkControl for Counter {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            let next = self
                .used
                .checked_add(units)
                .ok_or(WorkError::ArithmeticOverflow)?;
            if next > self.maximum {
                self.rejected += 1;
                return Err(WorkError::Interrupted);
            }
            self.used = next;
            Ok(())
        }
    }

    fn options(seed: f64, style: FillStyle) -> Options {
        OptionsBuilder::default()
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(seed),
                RoughMathRandom::new(7),
            ))
            .fill(crate::Srgba::new(0.2, 0.3, 0.4, 1.0))
            .stroke(crate::Srgba::new(0.0, 0.0, 0.0, 1.0))
            .fill_style(style)
            .hachure_gap(6.0)
            .roughness(0.7)
            .build()
            .unwrap()
    }

    #[test]
    fn legacy_svg_path_keeps_fixed_head_finite_line_operations_and_randomness() {
        for seed in [0.0, 1.0, -1.0] {
            let mut expected_options = options(seed, FillStyle::Solid);
            expected_options.roughness = Some(0.0);
            let mut actual_options = options(seed, FillStyle::Solid);
            actual_options.roughness = Some(0.0);
            let mut expected = Vec::new();
            for _ in 0..2 {
                let diverge = f64::from(0.2_f32) + expected_options.random() * f64::from(0.2_f32);
                for _ in 0..10 {
                    expected_options.random();
                }
                expected.push(crate::core::Op {
                    op: crate::core::OpType::Move,
                    data: vec![0.0, 0.0],
                });
                expected.push(crate::core::Op {
                    op: crate::core::OpType::BCurveTo,
                    data: vec![1e155 * diverge, 0.0, 2.0 * 1e155 * diverge, 0.0, 1e155, 0.0],
                });
            }
            let actual =
                crate::renderer::svg_path::<f64>("M0 0 L1e155 0".into(), &mut actual_options);
            assert!(
                actual.ops == expected,
                "fixed HEAD line differs for seed={seed}"
            );
            assert!(actual
                .ops
                .iter()
                .flat_map(|operation| &operation.data)
                .all(|value| value.is_finite()));
            assert_eq!(
                actual_options.random(),
                expected_options.random(),
                "following HEAD PRNG differs for seed={seed}"
            );
        }
    }

    #[test]
    fn controlled_generation_preserves_operations_and_following_random_streams() {
        let generator = Generator::default();
        for seed in [0.0, 1.0, -1.0] {
            for style in [FillStyle::Hachure, FillStyle::CrossHatch, FillStyle::Solid] {
                for primitive in 0..4 {
                    let original = Some(options(seed, style));
                    let controlled = Some(options(seed, style));
                    let mut counter = Counter::new(usize::MAX);
                    let expected = match primitive {
                        0 => generator.line(0.0, 0.0, 40.0, 20.0, &original),
                        1 => generator.rectangle(0.0, 0.0, 40.0, 20.0, &original),
                        2 => generator.circle(20.0, 20.0, 40.0, &original),
                        _ => generator.path::<f64>("M0 0 C0 20 40 20 40 0 Z".into(), &original),
                    };
                    let actual = match primitive {
                        0 => generator.line_with_control(
                            0.0,
                            0.0,
                            40.0,
                            20.0,
                            &controlled,
                            &mut counter,
                        ),
                        1 => generator.rectangle_with_control(
                            0.0,
                            0.0,
                            40.0,
                            20.0,
                            &controlled,
                            &mut counter,
                        ),
                        2 => generator.circle_with_control(
                            20.0,
                            20.0,
                            40.0,
                            &controlled,
                            &mut counter,
                        ),
                        _ => generator.path_with_control::<f64>(
                            "M0 0 C0 20 40 20 40 0 Z",
                            &controlled,
                            &mut counter,
                        ),
                    }
                    .unwrap();
                    assert!(
                        actual.sets == expected.sets,
                        "seed={seed}, style={style}, primitive={primitive}"
                    );
                    assert!(counter.used > 0);
                    let expected_next = generator.line::<f64>(1.0, 2.0, 30.0, 40.0, &original);
                    let actual_next = generator.line::<f64>(1.0, 2.0, 30.0, 40.0, &controlled);
                    assert!(
                        actual_next.sets == expected_next.sets,
                        "following PRNG: seed={seed}, style={style}, primitive={primitive}"
                    );
                }
            }
        }
    }

    #[test]
    fn controlled_generation_bounds_sampling_subdivision_scanlines_and_line_ops() {
        for stage in 0..4 {
            let mut counter = Counter::new(512);
            let mut options = options(1.0, FillStyle::Hachure);
            let result = match stage {
                0 => Generator::default()
                    .circle_with_control(0.0, 0.0, 10_000.0, &Some(options), &mut counter)
                    .map(|_| ()),
                1 => {
                    let segments = crate::points_on_path::normalized_path_with_control(
                        "M0 0 C0 1000000 1000000 1000000 1000000 0 Z",
                        &mut counter,
                    )
                    .unwrap();
                    crate::points_on_path::points_on_normalized_segments_with_control::<f64>(
                        &segments,
                        Some(1.0),
                        Some(0.85),
                        &mut counter,
                    )
                    .map(|_| ())
                }
                2 => Generator::default()
                    .rectangle_with_control(0.0, 0.0, 1000.0, 1000.0, &Some(options), &mut counter)
                    .map(|_| ()),
                _ => {
                    let points: Vec<_> = (0..256)
                        .map(|index| Point2D::new(index as f64, 0.0))
                        .collect();
                    crate::renderer::linear_path_with_control(
                        &points,
                        false,
                        &mut options,
                        &mut counter,
                    )
                    .map(|_| ())
                }
            };
            assert_eq!(result, Err(WorkError::Interrupted), "stage={stage}");
            assert!(counter.used > 0 && counter.used <= 512);
            assert_eq!(counter.rejected, 1);
        }
    }

    #[test]
    fn controlled_generation_rejects_nonfinite_and_nonadvancing_geometry() {
        let mut counter = Counter::new(10_000);
        let generator = Generator::default();
        assert!(matches!(
            generator.circle_with_control(
                0.0,
                0.0,
                f64::MAX,
                &Some(options(1.0, FillStyle::Hachure)),
                &mut counter
            ),
            Err(WorkError::InvalidGeometry)
        ));
        let mut options = options(1.0, FillStyle::Hachure);
        assert!(matches!(
            crate::renderer::line_with_control(0.0, 0.0, 1e155, 0.0, &mut options, &mut counter),
            Err(WorkError::InvalidGeometry)
        ));
        options.hachure_angle = Some(-90.0);
        options.hachure_gap = Some(0.1);
        let mut polygons = vec![vec![
            Point2D::new(0.0, 1e16),
            Point2D::new(10.0, 1e16),
            Point2D::new(10.0, 1e16 + 10.0),
            Point2D::new(0.0, 1e16 + 10.0),
        ]];
        assert!(matches!(
            crate::renderer::hachure_fill_polygons_with_control(
                &mut polygons,
                &mut options,
                &mut counter
            ),
            Err(WorkError::InvalidGeometry)
        ));
        assert!(matches!(
            crate::renderer::line_with_control(0.0, 0.0, f64::NAN, 0.0, &mut options, &mut counter),
            Err(WorkError::InvalidGeometry)
        ));
    }

    #[test]
    fn controlled_sort_preserves_stable_ties_and_stops_between_chunks() {
        let input: Vec<_> = (0..96).map(|index| (index % 5, index)).collect();
        let mut expected = input.clone();
        expected.sort_by_key(|value| value.0);
        let mut actual = input.clone();
        sort_by(
            &mut actual,
            |left, right| left.0.cmp(&right.0),
            &mut Counter::new(usize::MAX),
        )
        .unwrap();
        assert_eq!(actual, expected);
        let mut counter = Counter::new(1024);
        assert_eq!(
            sort_by(
                &mut input.clone(),
                |left, right| left.0.cmp(&right.0),
                &mut counter
            ),
            Err(WorkError::Interrupted)
        );
        assert_eq!(counter.used, 1024);
        assert_eq!(counter.rejected, 1);
    }
}
