//! Typed Venn Rough.js geometry, independent of SVG serialization.
//!
//! Ellipse, curve sampling, simplification, and scanline work are charged during production.
//! Path normalization and direct DrawingList builder admission remain separate concerns.

use crate::model::VennCircleLayout;
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use merman_core::OperationPhase;
use roughr::core::{FillStyle, OpSet, OpSetType, OptionsBuilder, RoughRandomness};
use roughr::filler::hachure_stream::{HachurePolygon, try_hachure_fill};
use roughr::generation::{GenerationError, GenerationEvent};

pub(crate) struct RoughCircleGeometry {
    pub(crate) fill: OpSet<f64>,
    pub(crate) outline: OpSet<f64>,
}

pub(crate) fn circle_geometry(
    circle: &VennCircleLayout,
    fill: roughr::Srgba,
    stroke: roughr::Srgba,
    stroke_width: f32,
    hachure_angle: f32,
    randomness: &RoughRandomness,
    work_meter: &OperationWorkMeter,
) -> Result<RoughCircleGeometry> {
    let mut options = OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.7)
        .bowing(1.0)
        .fill(fill)
        .fill_style(FillStyle::Hachure)
        .fill_weight(2.0)
        .hachure_gap(8.0)
        .hachure_angle(hachure_angle)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .map_err(|error| invalid_options("circle", error))?;
    let params = roughr::renderer::generate_ellipse_params(
        circle.radius * 2.0,
        circle.radius * 2.0,
        &mut options,
    );
    // Match Generator::ellipse's outline-before-fill order, without Drawable's complete
    // operation-set clone. Hachure fill owns the estimated points after outline generation.
    let mut outline = Vec::new();
    let estimated = roughr::renderer::try_ellipse_with_params(
        circle.x,
        circle.y,
        &mut options,
        &params,
        |event| collect_operation(event, &mut outline, work_meter),
    )
    .map_err(generation_error)?;
    let fill = collect_hachure(&mut [estimated], &mut options, work_meter)?;
    Ok(RoughCircleGeometry {
        fill,
        outline: OpSet {
            op_set_type: OpSetType::Path,
            ops: outline,
            path: None,
            size: None,
        },
    })
}

pub(crate) fn intersection_fill_geometry(
    path: &str,
    fill: roughr::Srgba,
    randomness: &RoughRandomness,
    work_meter: &OperationWorkMeter,
) -> Result<OpSet<f64>> {
    let mut options = OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.7)
        .bowing(1.0)
        .fill(fill)
        .fill_style(FillStyle::CrossHatch)
        .fill_weight(2.0)
        .hachure_gap(6.0)
        .hachure_angle(60.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .map_err(|error| invalid_options("intersection", error))?;
    options.stroke = None;

    let distance = (1.0 + options.roughness.unwrap_or(1.0) as f64) / 2.0;
    let mut segments = Vec::new();
    for segment in svgtypes::PathParser::from(path) {
        work_meter.charge_at(1, OperationPhase::Emit)?;
        let segment = segment.map_err(|error| Error::InvalidModel {
            message: format!("invalid Venn intersection path: {error}"),
        })?;
        segments
            .try_reserve(1)
            .map_err(|_| Error::DrawingListAllocationFailed {
                collection: "Venn intersection source segments",
            })?;
        segments.push(segment);
    }
    // The legacy normalizer still owns its intermediate arrays. Reuse this normalized
    // sequence for sampling and outline RNG consumption instead of parsing the path twice.
    let normalized = roughr::points_on_path::normalized_segments(&segments);
    let mut polygons = roughr::points_on_path::try_points_on_normalized_segments::<f64, _>(
        &normalized,
        1.0,
        Some(distance),
        |units| {
            work_meter
                .charge_at(units, OperationPhase::Emit)
                .map_err(Error::from)
        },
    )
    .map_err(generation_error)?;
    // Rough.js advances the outline PRNG even with stroke:none, before generating the fill.
    roughr::renderer::try_svg_normalized_segments::<f64, _>(&normalized, &mut options, |event| {
        if let GenerationEvent::Work(units) = event {
            work_meter.charge_at(units, OperationPhase::Emit)?;
        }
        Ok(())
    })
    .map_err(generation_error)?;
    collect_hachure(&mut polygons, &mut options, work_meter)
}

fn collect_hachure(
    polygons: &mut [HachurePolygon<f64>],
    options: &mut roughr::core::Options,
    work_meter: &OperationWorkMeter,
) -> Result<OpSet<f64>> {
    let mut ops = Vec::new();
    try_hachure_fill(polygons, options, |event| {
        collect_operation(event, &mut ops, work_meter)
    })
    .map_err(generation_error)?;
    Ok(OpSet {
        op_set_type: OpSetType::FillSketch,
        ops,
        size: None,
        path: None,
    })
}

fn collect_operation(
    event: GenerationEvent<f64>,
    ops: &mut Vec<roughr::core::Op<f64>>,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    match event {
        GenerationEvent::Work(units) => work_meter.charge_at(units, OperationPhase::Emit)?,
        GenerationEvent::Op(op) => {
            ops.try_reserve(1)
                .map_err(|_| Error::DrawingListAllocationFailed {
                    collection: "Venn rough operations",
                })?;
            ops.push(op);
        }
    }
    Ok(())
}

fn generation_error(error: GenerationError<Error>) -> Error {
    match error {
        GenerationError::Consumer(error) => error,
        GenerationError::Allocation(_) => Error::DrawingListAllocationFailed {
            collection: "Venn rough geometry scratch",
        },
        GenerationError::InvalidGeometry | GenerationError::UnsupportedFillStyle => {
            Error::InvalidModel {
                message: error.to_string(),
            }
        }
    }
}

fn invalid_options(context: &str, error: impl std::fmt::Display) -> Error {
    Error::InvalidModel {
        message: format!("invalid Venn {context} RoughJS options: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roughr::core::{RoughJsSeed, RoughMathRandom};

    #[test]
    fn intersection_sampling_preserves_legacy_fill_and_stops_on_work_rejection() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        use merman_core::OperationControl;
        let paths = [
            "M0 0 C0 100 100 100 100 0 C100 -100 0 -100 0 0 Z",
            "M0 0 A80 80 0 1 0 160 0 A80 80 0 1 0 0 0",
            "M80 80 m-80 0 a80 80 0 1 0 160 0 a80 80 0 1 0 -160 0",
        ];
        let fill = roughr::Srgba::new(1.0, 0.3, 0.2, 1.0);
        for seed in [0.0, 1.0, 42.0] {
            for path in paths {
                let random =
                    || RoughRandomness::new(RoughJsSeed::new(seed), RoughMathRandom::new(123));
                let mut options = OptionsBuilder::default()
                    .randomness(random())
                    .roughness(0.7)
                    .bowing(1.0)
                    .fill(fill)
                    .fill_style(FillStyle::CrossHatch)
                    .fill_weight(2.0)
                    .hachure_gap(6.0)
                    .hachure_angle(60.0)
                    .disable_multi_stroke(false)
                    .disable_multi_stroke_fill(false)
                    .build()
                    .unwrap();
                options.stroke = None;
                let distance = (1.0 + f64::from(options.roughness.unwrap())) / 2.0;
                let polygons = roughr::points_on_path::points_on_path::<f64>(
                    path.to_owned(),
                    Some(1.0),
                    Some(distance),
                );
                let _outline = roughr::renderer::svg_path::<f64>(path.to_owned(), &mut options);
                let expected = roughr::renderer::pattern_fill_polygons(polygons, &mut options);
                let meter =
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
                let actual = intersection_fill_geometry(path, fill, &random(), &meter).unwrap();
                assert_eq!(actual, expected, "seed={seed}, path={path}");
            }
        }
        let random = RoughRandomness::new(RoughJsSeed::new(1.0), RoughMathRandom::new(123));
        let control = OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 20)
                .unwrap(),
            control.clone(),
        );
        assert!(matches!(
            intersection_fill_geometry(paths[0], fill, &random, &meter),
            Err(Error::ResourceLimitExceeded(_))
        ));
        let terminal = control
            .terminal_checkpoint_at(OperationPhase::Emit)
            .unwrap_err();
        control.cancel();
        assert_eq!(
            control.terminal_checkpoint_at(OperationPhase::Emit),
            Err(terminal)
        );
    }

    #[test]
    fn hachure_collection_shares_exact_work_and_keeps_errors_terminal() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        use merman_core::OperationControl;
        let polygons = || {
            vec![vec![
                roughr::Point2D::new(0.0, 0.0),
                roughr::Point2D::new(30.0, 0.0),
                roughr::Point2D::new(30.0, 30.0),
                roughr::Point2D::new(0.0, 30.0),
            ]]
        };
        let options = || {
            OptionsBuilder::default()
                .fill_style(FillStyle::CrossHatch)
                .hachure_angle(60.0)
                .hachure_gap(6.0)
                .randomness(RoughRandomness::new(
                    RoughJsSeed::new(1.0),
                    RoughMathRandom::new(123),
                ))
                .build()
                .unwrap()
        };
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let complete = collect_hachure(&mut polygons(), &mut options(), &meter).unwrap();
        let exact_work = meter.used();
        assert!(exact_work > 0 && !complete.ops.is_empty());
        for ceiling in [exact_work, exact_work - 1] {
            let control = OperationControl::new();
            let meter = OperationWorkMeter::new_with_control(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, ceiling)
                    .unwrap(),
                control.clone(),
            );
            let result = collect_hachure(&mut polygons(), &mut options(), &meter);
            if ceiling == exact_work {
                assert_eq!(result.unwrap(), complete);
            } else {
                let Error::ResourceLimitExceeded(error) = result.unwrap_err() else {
                    panic!("expected work limit");
                };
                assert_eq!(error.limit, "max_layout_work_units");
                assert_eq!(error.max, ceiling);
                assert!(error.actual > ceiling);
                let terminal = control
                    .terminal_checkpoint_at(OperationPhase::Emit)
                    .unwrap_err();
                control.cancel();
                assert_eq!(
                    control.terminal_checkpoint_at(OperationPhase::Emit),
                    Err(terminal)
                );
            }
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(20);
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        assert!(matches!(
            collect_hachure(&mut polygons(), &mut options(), &meter),
            Err(Error::Cancelled(_))
        ));
        assert!(meter.used() > 0 && meter.used() < exact_work);
    }

    #[test]
    fn typed_circle_preserves_generator_operations_and_seed_progression() {
        let circle = VennCircleLayout {
            set: "A".to_owned(),
            x: 110.0,
            y: 95.0,
            radius: 80.0,
        };
        let fill = roughr::Srgba::new(1.0, 0.3, 0.2, 1.0);
        let stroke = roughr::Srgba::new(0.1, 0.2, 0.3, 1.0);
        for seed in [0.0, 1.0, 42.0] {
            for angle in [-41.0, 19.0] {
                let random =
                    || RoughRandomness::new(RoughJsSeed::new(seed), RoughMathRandom::new(123));
                // The existing public generator is the characterization oracle, not the
                // document/SVG formatter. Compare every operation before formatting rounds it.
                let options = OptionsBuilder::default()
                    .randomness(random())
                    .roughness(0.7)
                    .bowing(1.0)
                    .fill(fill)
                    .fill_style(FillStyle::Hachure)
                    .fill_weight(2.0)
                    .hachure_gap(8.0)
                    .hachure_angle(angle)
                    .stroke(stroke)
                    .stroke_width(7.0)
                    .disable_multi_stroke(false)
                    .disable_multi_stroke_fill(false)
                    .build()
                    .unwrap();
                let expected = roughr::generator::Generator::default().circle::<f64>(
                    circle.x,
                    circle.y,
                    circle.radius * 2.0,
                    &Some(options),
                );
                let meter =
                    OperationWorkMeter::new(crate::resources::RenderResourcePolicy::default());
                let actual =
                    circle_geometry(&circle, fill, stroke, 7.0, angle, &random(), &meter).unwrap();
                assert_eq!(expected.sets, [actual.fill.clone(), actual.outline.clone()]);
                for set in [&actual.fill, &actual.outline] {
                    let segments = crate::rough_geometry::opset_to_path_segments(set).unwrap();
                    assert_eq!(segments.len(), set.ops.len());
                }
            }
        }
    }
}
