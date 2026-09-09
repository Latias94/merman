//! Fallible hachure/crosshatch operation production.

use crate::core::{FillStyle, Options};
use crate::geometry::Line;
use euclid::{default::Point2D, Angle, Translation2D, Trig, Vector2D};
use num_traits::{Float, FromPrimitive};
use std::cmp::Ordering;

/// A polygon in Rough.js's unitless coordinate system.
pub type HachurePolygon<F> = Vec<Point2D<F>>;

pub use crate::generation::{GenerationError as HachureError, GenerationEvent as HachureEvent};

/// Produces hachure operations without owning the output collection.
///
/// The caller must discard partial output on error. Polygons and RNG state may have advanced;
/// retry with fresh input/state. Work callbacks are separate from output-operation admission.
pub fn try_hachure_fill<F, E>(
    polygons: &mut [HachurePolygon<F>],
    options: &mut Options,
    mut consume: impl FnMut(HachureEvent<F>) -> Result<(), E>,
) -> Result<(), HachureError<E>>
where
    F: Float + Trig + FromPrimitive,
{
    if !matches!(
        options.fill_style,
        Some(FillStyle::Hachure | FillStyle::CrossHatch)
    ) {
        return Err(HachureError::UnsupportedFillStyle);
    }
    fill_pass(polygons, options, &mut consume)?;
    if options.fill_style == Some(FillStyle::CrossHatch) {
        // Match HatchFiller's mutation and reuse both the consumer budget and RNG state.
        options.set_hachure_angle(options.hachure_angle.map(|angle| angle + 90.0));
        fill_pass(polygons, options, &mut consume)?;
    }
    Ok(())
}

struct Edge<F> {
    ymin: F,
    ymax: F,
    x: F,
    slope: F,
    order: usize,
}

fn work<F: Float + Trig, E>(
    consume: &mut impl FnMut(HachureEvent<F>) -> Result<(), E>,
    units: usize,
) -> Result<(), HachureError<E>> {
    consume(HachureEvent::Work(units)).map_err(HachureError::Consumer)
}

fn rotate_polygons<F, E>(
    polygons: &mut [Vec<Point2D<F>>],
    degrees: F,
    consume: &mut impl FnMut(HachureEvent<F>) -> Result<(), E>,
) -> Result<(), HachureError<E>>
where
    F: Float + Trig + FromPrimitive,
{
    // Use the same euclid transform as rotate_points; mutate without allocating a copy.
    let transform = Translation2D::new(-F::zero(), -F::zero())
        .to_transform()
        .then_rotate(Angle::radians(degrees.to_radians()))
        .then_translate(Vector2D::new(F::zero(), F::zero()));
    for polygon in polygons {
        work(consume, 1)?;
        for point in polygon {
            work(consume, 1)?;
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(HachureError::InvalidGeometry);
            }
            if degrees != F::zero() {
                *point = transform.transform_point(*point);
            }
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(HachureError::InvalidGeometry);
            }
        }
    }
    Ok(())
}

fn compare<F: Float>(lhs: F, rhs: F) -> Ordering {
    // Coordinates and derived slopes are checked before entering either edge table.
    lhs.partial_cmp(&rhs).unwrap_or(Ordering::Equal)
}

fn fill_pass<F, E>(
    polygons: &mut [Vec<Point2D<F>>],
    options: &mut Options,
    consume: &mut impl FnMut(HachureEvent<F>) -> Result<(), E>,
) -> Result<(), HachureError<E>>
where
    F: Float + Trig + FromPrimitive,
{
    let angle = options.hachure_angle.unwrap_or(0.0) + 90.0;
    let mut gap = options.hachure_gap.unwrap_or(0.0);
    if gap < 0.0 {
        gap = options.stroke_width.unwrap_or(0.0) * 4.0;
    }
    if !angle.is_finite() || !gap.is_finite() {
        return Err(HachureError::InvalidGeometry);
    }
    let angle = F::from(angle).ok_or(HachureError::InvalidGeometry)?;
    let gap = F::from(gap.max(0.1)).ok_or(HachureError::InvalidGeometry)?;
    rotate_polygons(polygons, angle, consume)?;

    let mut edges = Vec::new();
    for polygon in polygons.iter_mut() {
        work(consume, 1)?;
        if polygon.first() != polygon.last() {
            if let Some(first) = polygon.first().copied() {
                work(consume, 1)?;
                polygon.try_reserve(1).map_err(HachureError::Allocation)?;
                polygon.push(first);
            }
        }
        if polygon.len() <= 2 {
            continue;
        }
        for pair in polygon.windows(2) {
            work(consume, 1)?;
            let p1 = pair[0];
            let p2 = pair[1];
            if p1.y == p2.y {
                continue;
            }
            let slope = (p2.x - p1.x) / (p2.y - p1.y);
            if !slope.is_finite() {
                return Err(HachureError::InvalidGeometry);
            }
            let ymin = p1.y.min(p2.y);
            edges.try_reserve(1).map_err(HachureError::Allocation)?;
            edges.push(Edge {
                ymin,
                ymax: p1.y.max(p2.y),
                x: if ymin == p1.y { p1.x } else { p2.x },
                slope,
                order: edges.len(),
            });
        }
    }
    work(consume, edges.len())?;
    // Explicit original-order ties preserve stable-sort semantics without sort scratch storage.
    edges.sort_unstable_by(|lhs, rhs| {
        compare(lhs.ymin, rhs.ymin)
            .then_with(|| compare(lhs.x, rhs.x))
            .then_with(|| compare(lhs.ymax, rhs.ymax))
            .then_with(|| lhs.order.cmp(&rhs.order))
    });
    edges.reverse();
    let Some(first) = edges.last() else {
        return rotate_polygons(polygons, -angle, consume);
    };
    let mut y = first.ymin;
    let mut active: Vec<Edge<F>> = Vec::new();
    loop {
        // Empty scan rows consume work too; do not jump over them or hide them in next().
        work(consume, 1)?;
        while edges.last().is_some_and(|edge| edge.ymin <= y) {
            work(consume, 1)?;
            active.try_reserve(1).map_err(HachureError::Allocation)?;
            if let Some(edge) = edges.pop() {
                active.push(edge);
            }
        }
        work(consume, active.len())?;
        active.retain(|edge| edge.ymax > y);
        for (index, edge) in active.iter_mut().enumerate() {
            edge.order = index;
        }
        active.sort_unstable_by(|lhs, rhs| {
            compare(lhs.x, rhs.x).then_with(|| lhs.order.cmp(&rhs.order))
        });
        if !active.len().is_multiple_of(2) {
            return Err(HachureError::InvalidGeometry);
        }
        for pair in active.chunks_exact(2) {
            work(consume, 1)?;
            let mut line = Line {
                start_point: Point2D::new(pair[0].x, y),
                end_point: Point2D::new(pair[1].x, y),
            };
            if angle != F::zero() {
                line.rotate(&Point2D::new(F::zero(), F::zero()), -angle);
            }
            // One line has a fixed-size operation batch. Never accumulate the complete fill.
            for op in crate::renderer::_double_line(
                line.start_point.x,
                line.start_point.y,
                line.end_point.x,
                line.end_point.y,
                options,
                true,
            ) {
                consume(HachureEvent::Op(op)).map_err(HachureError::Consumer)?;
            }
        }
        if edges.is_empty() && active.is_empty() {
            break;
        }
        let next_y = y + gap;
        if !next_y.is_finite() || next_y <= y {
            return Err(HachureError::InvalidGeometry);
        }
        y = next_y;
        for edge in &mut active {
            work(consume, 1)?;
            edge.x = edge.x + gap * edge.slope;
            if !edge.x.is_finite() {
                return Err(HachureError::InvalidGeometry);
            }
        }
    }
    rotate_polygons(polygons, -angle, consume)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{OptionsBuilder, RoughJsSeed, RoughMathRandom, RoughRandomness};
    use euclid::point2;

    fn options(style: FillStyle, angle: f32, seed: f64) -> Options {
        OptionsBuilder::default()
            .fill_style(style)
            .hachure_angle(angle)
            .hachure_gap(1.0)
            .roughness(0.7)
            .randomness(RoughRandomness::new(
                RoughJsSeed::new(seed),
                RoughMathRandom::new(456),
            ))
            .build()
            .unwrap()
    }

    fn polygons() -> Vec<Vec<Point2D<f64>>> {
        vec![vec![
            point2(0.0, 0.0),
            point2(20.0, 0.0),
            point2(13.0, 15.0),
            point2(0.0, 9.0),
        ]]
    }

    #[test]
    fn stream_preserves_hachure_and_crosshatch_operations() {
        let duplicate = vec![polygons().remove(0); 2];
        let mut shared = polygons();
        shared.push(vec![
            point2(0.0, 0.0),
            point2(20.0, 0.0),
            point2(20.0, 20.0),
            point2(0.0, 20.0),
        ]);
        for source in [polygons(), duplicate, shared] {
            for style in [FillStyle::Hachure, FillStyle::CrossHatch] {
                for angle in [-90.0, -41.0, 60.0] {
                    for seed in [0.0, 1.0, 42.0] {
                        let mut expected_options = options(style, angle, seed);
                        let mut actual_options = options(style, angle, seed);
                        let expected = crate::renderer::pattern_fill_polygons(
                            source.clone(),
                            &mut expected_options,
                        );
                        let mut actual = Vec::new();
                        try_hachure_fill(&mut source.clone(), &mut actual_options, |event| {
                            if let HachureEvent::Op(op) = event {
                                actual.push(op);
                            }
                            Ok::<_, ()>(())
                        })
                        .unwrap();
                        assert_eq!(actual, expected.ops, "{style:?}, {angle}, {seed}");
                        assert_eq!(
                            actual_options.random(),
                            expected_options.random(),
                            "next RNG draw must also match"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cancellation_is_checked_inside_empty_scan_rows() {
        let mut polygons = vec![
            vec![
                point2(0.0, 0.0),
                point2(2.0, 0.0),
                point2(2.0, 2.0),
                point2(0.0, 2.0),
            ],
            vec![
                point2(0.0, 1000.0),
                point2(2.0, 1000.0),
                point2(2.0, 1002.0),
                point2(0.0, 1002.0),
            ],
        ];
        let mut work = 0;
        let mut ops = 0;
        let result = try_hachure_fill(
            &mut polygons,
            &mut options(FillStyle::Hachure, -90.0, 1.0),
            |event| {
                match event {
                    HachureEvent::Work(amount) => {
                        work += amount;
                        if work > 200 {
                            return Err("cancelled");
                        }
                    }
                    HachureEvent::Op(_) => ops += 1,
                }
                Ok(())
            },
        );
        assert!(matches!(result, Err(HachureError::Consumer("cancelled"))));
        assert!(
            ops > 0 && ops < 16,
            "must cancel between the separated polygons"
        );
    }

    #[test]
    fn consumer_rejection_stops_callbacks_immediately() {
        let mut callbacks_after_rejection = 0;
        let mut rejected = false;
        let result = try_hachure_fill(
            &mut polygons(),
            &mut options(FillStyle::CrossHatch, 60.0, 1.0),
            |event| {
                if rejected {
                    callbacks_after_rejection += 1;
                }
                if matches!(event, HachureEvent::Op(_)) {
                    rejected = true;
                    return Err("full");
                }
                Ok(())
            },
        );
        assert!(matches!(result, Err(HachureError::Consumer("full"))));
        assert_eq!(callbacks_after_rejection, 0);
    }

    #[test]
    fn invalid_or_non_advancing_geometry_returns_an_error() {
        for y in [f64::NAN, 1e30] {
            let mut input = vec![vec![
                point2(0.0, y),
                point2(2.0, y),
                point2(2.0, y + 1e20),
                point2(0.0, y + 1e20),
            ]];
            let result = try_hachure_fill(
                &mut input,
                &mut options(FillStyle::Hachure, -90.0, 1.0),
                |_| Ok::<_, ()>(()),
            );
            assert!(matches!(result, Err(HachureError::InvalidGeometry)));
        }
        let mut emitted = 0;
        try_hachure_fill(
            &mut [Vec::<Point2D<f64>>::new()],
            &mut options(FillStyle::Hachure, 60.0, 1.0),
            |event| {
                if matches!(event, HachureEvent::Op(_)) {
                    emitted += 1;
                }
                Ok::<_, ()>(())
            },
        )
        .unwrap();
        assert_eq!(emitted, 0);
    }
}
