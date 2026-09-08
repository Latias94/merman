//! Typed Venn Rough.js geometry, independent of SVG serialization.
//!
//! These collecting entry points still need fallible sampling and scanline production before
//! they can be used by the caller-bounded DrawingList builder.

use crate::model::VennCircleLayout;
use crate::{Error, Result};
use roughr::core::{FillStyle, OpSet, OpSetType, OptionsBuilder, RoughRandomness};

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
    let ellipse = roughr::renderer::ellipse_with_params(circle.x, circle.y, &mut options, &params);
    let fill =
        roughr::renderer::pattern_fill_polygons(vec![ellipse.estimated_points], &mut options);
    Ok(RoughCircleGeometry {
        fill,
        outline: ellipse.opset,
    })
}

pub(crate) fn intersection_fill_geometry(
    path: &str,
    fill: roughr::Srgba,
    randomness: &RoughRandomness,
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
    let polygons =
        roughr::points_on_path::points_on_path::<f64>(path.to_owned(), Some(1.0), Some(distance));
    // Rough.js advances the outline PRNG even with stroke:none, before generating the fill.
    let _discarded_outline = roughr::renderer::svg_path::<f64>(path.to_owned(), &mut options);
    let fill_path = roughr::renderer::pattern_fill_polygons(polygons, &mut options);
    if fill_path.op_set_type != OpSetType::FillSketch {
        return Err(Error::InvalidModel {
            message: "Venn RoughJS intersection did not produce a sketch fill path".to_owned(),
        });
    }
    Ok(fill_path)
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
                let actual = circle_geometry(&circle, fill, stroke, 7.0, angle, &random()).unwrap();
                assert_eq!(expected.sets, [actual.fill.clone(), actual.outline.clone()]);
                for set in [&actual.fill, &actual.outline] {
                    let segments = crate::rough_geometry::opset_to_path_segments(set).unwrap();
                    assert_eq!(segments.len(), set.ops.len());
                }
            }
        }
    }
}
