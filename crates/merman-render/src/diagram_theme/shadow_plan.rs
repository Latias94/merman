//! Bounded ordered shadow lowering shared by family-owned paint terminals.

use super::{
    EffectColorSpace, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue,
    ThemeResourceLimitExceeded, ThemeResourcePolicy,
};

const DROP_SHADOW_PAINT_BOUNDS_SIGMAS: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SvgFilterRegion {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    units: SvgFilterUnits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SvgFilterUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

impl SvgFilterRegion {
    pub(crate) fn try_bounded(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        let region = Self {
            x,
            y,
            width,
            height,
            units: SvgFilterUnits::ObjectBoundingBox,
        };
        ([region.x, region.y, region.width, region.height]
            .into_iter()
            .all(f32::is_finite)
            && region.width > 0.0
            && region.height > 0.0)
            .then_some(region)
    }

    pub(crate) const fn as_array(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    pub(crate) const fn units(self) -> SvgFilterUnits {
        self.units
    }

    pub(crate) fn try_bounded_user_space(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        let mut region = Self::try_bounded(x, y, width, height)?;
        region.units = SvgFilterUnits::UserSpaceOnUse;
        Some(region)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct EffectOutsets {
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
    pub(crate) left: f64,
}

impl EffectOutsets {
    pub(crate) fn include(&mut self, other: Self) {
        self.top = self.top.max(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
        self.left = self.left.max(other.left);
    }

    fn for_drop_shadow(
        stroke_width: f64,
        offset_x: f32,
        offset_y: f32,
        std_deviation: f32,
    ) -> Option<Self> {
        if !stroke_width.is_finite()
            || stroke_width < 0.0
            || !offset_x.is_finite()
            || !offset_y.is_finite()
            || !std_deviation.is_finite()
            || std_deviation < 0.0
        {
            return None;
        }

        let half_stroke = stroke_width / 2.0;
        let offset_x = f64::from(offset_x);
        let offset_y = f64::from(offset_y);
        // Gaussian support is mathematically unbounded. Merman deliberately materializes a finite
        // four-sigma paint envelope so every target receives the same bounded clipping contract.
        let blur_support = f64::from(std_deviation) * DROP_SHADOW_PAINT_BOUNDS_SIGMAS;
        let outsets = Self {
            top: half_stroke + blur_support + (-offset_y).max(0.0),
            right: half_stroke + blur_support + offset_x.max(0.0),
            bottom: half_stroke + blur_support + offset_y.max(0.0),
            left: half_stroke + blur_support + (-offset_x).max(0.0),
        };
        [outsets.top, outsets.right, outsets.bottom, outsets.left]
            .into_iter()
            .all(f64::is_finite)
            .then_some(outsets)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SvgShadowStage {
    pub(crate) input: EffectInput,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) std_deviation: f32,
    pub(crate) color: ThemeColorValue,
}

#[derive(Debug, Clone)]
pub(crate) struct SvgShadowEffect {
    id: String,
    color_space: EffectColorSpace,
    stages: Vec<SvgShadowStage>,
}

impl SvgShadowEffect {
    pub(crate) fn from_graph(graph: &EffectGraph) -> Option<Self> {
        let stages = graph
            .primitives()
            .iter()
            .map(|primitive| {
                let EffectPrimitive::DropShadow {
                    input,
                    offset_x,
                    offset_y,
                    blur_radius,
                    spread,
                    color,
                } = primitive
                else {
                    return None;
                };
                if *spread != 0.0 {
                    return None;
                }
                Some(SvgShadowStage {
                    input: *input,
                    offset_x: *offset_x,
                    offset_y: *offset_y,
                    std_deviation: *blur_radius,
                    color: color.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            id: graph.id().to_owned(),
            color_space: graph.color_space(),
            stages,
        })
    }

    pub(crate) fn stages(&self) -> &[SvgShadowStage] {
        &self.stages
    }

    pub(crate) const fn color_space(&self) -> EffectColorSpace {
        self.color_space
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn paint_outsets(&self, source: EffectOutsets) -> Option<EffectOutsets> {
        let mut outsets = source;
        for stage in &self.stages {
            let input = match stage.input {
                EffectInput::SourceGraphic => source,
                EffectInput::Previous => outsets,
            };
            let Some(extension) = EffectOutsets::for_drop_shadow(
                0.0,
                stage.offset_x,
                stage.offset_y,
                stage.std_deviation,
            ) else {
                return None;
            };
            outsets = EffectOutsets {
                top: input.top + extension.top,
                right: input.right + extension.right,
                bottom: input.bottom + extension.bottom,
                left: input.left + extension.left,
            };
        }
        Some(outsets)
    }

    pub(crate) fn materialize_rect(
        &self,
        resources: &ThemeResourcePolicy,
        width: f64,
        height: f64,
        stroke_width: f64,
    ) -> Result<Option<MaterializedShadowEffect>, ThemeResourceLimitExceeded> {
        if !stroke_width.is_finite() || stroke_width < 0.0 {
            return Ok(None);
        }
        let half_stroke = stroke_width / 2.0;
        self.materialize_with_source_outsets(
            resources,
            width,
            height,
            EffectOutsets {
                top: half_stroke,
                right: half_stroke,
                bottom: half_stroke,
                left: half_stroke,
            },
        )
    }

    /// The family supplies the actual shape's stroke envelope, including any miter joins.
    pub(crate) fn materialize_with_source_outsets(
        &self,
        resources: &ThemeResourcePolicy,
        width: f64,
        height: f64,
        source: EffectOutsets,
    ) -> Result<Option<MaterializedShadowEffect>, ThemeResourceLimitExceeded> {
        if !width.is_finite()
            || width <= 0.0
            || !height.is_finite()
            || height <= 0.0
            || ![source.top, source.right, source.bottom, source.left]
                .into_iter()
                .all(|v| v.is_finite() && v >= 0.0)
        {
            return Ok(None);
        }
        let Some(outsets) = self.paint_outsets(source) else {
            return Ok(None);
        };
        let min_x = -outsets.left / width;
        let min_y = -outsets.top / height;
        let max_x = 1.0 + outsets.right / width;
        let max_y = 1.0 + outsets.bottom / height;
        let Some(x) = round_down_f32(min_x) else {
            return Ok(None);
        };
        let Some(y) = round_down_f32(min_y) else {
            return Ok(None);
        };
        let Some(max_x) = round_up_f32(max_x) else {
            return Ok(None);
        };
        let Some(max_y) = round_up_f32(max_y) else {
            return Ok(None);
        };
        let Some(region_width) = round_up_f32(f64::from(max_x) - f64::from(x)) else {
            return Ok(None);
        };
        let Some(region_height) = round_up_f32(f64::from(max_y) - f64::from(y)) else {
            return Ok(None);
        };
        if region_width <= 0.0 || region_height <= 0.0 {
            return Ok(None);
        }

        let Some(region) = SvgFilterRegion::try_bounded(x, y, region_width, region_height) else {
            return Ok(None);
        };
        let magnitude = region
            .as_array()
            .into_iter()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        resources.check_materialized_filter_region_magnitude(magnitude)?;

        Ok(Some(MaterializedShadowEffect { region, outsets }))
    }

    /// Materializes a filter in the path's local user-space coordinates. This is required for
    /// line-like terminals whose object bounding box has a zero width or height.
    pub(crate) fn materialize_user_space(
        &self,
        resources: &ThemeResourcePolicy,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        source: EffectOutsets,
    ) -> Result<Option<MaterializedShadowEffect>, ThemeResourceLimitExceeded> {
        if ![min_x, min_y, max_x, max_y].into_iter().all(f64::is_finite)
            || min_x > max_x
            || min_y > max_y
            || ![source.top, source.right, source.bottom, source.left]
                .into_iter()
                .all(|v| v.is_finite() && v >= 0.0)
        {
            return Ok(None);
        }
        let Some(outsets) = self.paint_outsets(source) else {
            return Ok(None);
        };
        let Some(x) = round_down_f32(min_x - outsets.left) else {
            return Ok(None);
        };
        let Some(y) = round_down_f32(min_y - outsets.top) else {
            return Ok(None);
        };
        let Some(max_x) = round_up_f32(max_x + outsets.right) else {
            return Ok(None);
        };
        let Some(max_y) = round_up_f32(max_y + outsets.bottom) else {
            return Ok(None);
        };
        let Some(width) = round_up_f32(f64::from(max_x) - f64::from(x)) else {
            return Ok(None);
        };
        let Some(height) = round_up_f32(f64::from(max_y) - f64::from(y)) else {
            return Ok(None);
        };
        let Some(region) = SvgFilterRegion::try_bounded_user_space(x, y, width, height) else {
            return Ok(None);
        };
        let magnitude = region
            .as_array()
            .into_iter()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        resources.check_materialized_filter_region_magnitude(magnitude)?;
        Ok(Some(MaterializedShadowEffect { region, outsets }))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MaterializedShadowEffect {
    region: SvgFilterRegion,
    outsets: EffectOutsets,
}

impl MaterializedShadowEffect {
    pub(crate) const fn region(self) -> SvgFilterRegion {
        self.region
    }

    pub(crate) const fn outsets(self) -> EffectOutsets {
        self.outsets
    }
}

fn round_down_f32(value: f64) -> Option<f32> {
    let rounded = value as f32;
    if !value.is_finite() || !rounded.is_finite() {
        return None;
    }
    if f64::from(rounded) <= value {
        return Some(rounded);
    }
    let outward = rounded.next_down();
    outward.is_finite().then_some(outward)
}

fn round_up_f32(value: f64) -> Option<f32> {
    let rounded = value as f32;
    if !value.is_finite() || !rounded.is_finite() {
        return None;
    }
    if f64::from(rounded) >= value {
        return Some(rounded);
    }
    let outward = rounded.next_up();
    outward.is_finite().then_some(outward)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glow(blur_radius: f32) -> SvgShadowEffect {
        SvgShadowEffect::from_graph(
            &EffectGraph::new(
                "glow",
                [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 3.0,
                    offset_y: -2.0,
                    blur_radius,
                    spread: 0.0,
                    color: ThemeColorValue::parse("#00f2ff").unwrap(),
                }],
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn user_space_filter_bounds_enclose_horizontal_vertical_and_point_sources() {
        let effect = glow(6.0);
        for [min_x, min_y, max_x, max_y] in [
            [10.125, 20.125, 110.625, 20.125],
            [20.125, 10.125, 20.125, 110.625],
            [20.125, 20.125, 20.125, 20.125],
        ] {
            let materialized = effect
                .materialize_user_space(
                    &ThemeResourcePolicy::default(),
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    EffectOutsets {
                        top: 2.0,
                        right: 2.0,
                        bottom: 2.0,
                        left: 2.0,
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                materialized.region().units(),
                SvgFilterUnits::UserSpaceOnUse
            );
            let [x, y, width, height] = materialized.region().as_array().map(f64::from);
            assert!(width > 0.0 && height > 0.0);
            assert!(x <= min_x - 26.0 && y <= min_y - 28.0);
            assert!(x + width >= max_x + 29.0 && y + height >= max_y + 26.0);
        }
    }

    #[test]
    fn user_space_filter_rejects_invalid_geometry_without_fabricating_an_extent() {
        let effect = glow(6.0);
        for bounds in [
            [f64::NAN, 0.0, 10.0, 10.0],
            [0.0, 0.0, f64::INFINITY, 10.0],
            [10.0, 0.0, 0.0, 10.0],
            [0.0, 10.0, 10.0, 0.0],
        ] {
            assert!(
                effect
                    .materialize_user_space(
                        &ThemeResourcePolicy::default(),
                        bounds[0],
                        bounds[1],
                        bounds[2],
                        bounds[3],
                        EffectOutsets::default(),
                    )
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            effect
                .materialize_user_space(
                    &ThemeResourcePolicy::default(),
                    0.0,
                    0.0,
                    10.0,
                    0.0,
                    EffectOutsets {
                        left: -1.0,
                        ..Default::default()
                    },
                )
                .unwrap()
                .is_none()
        );
        let mut zero_effect = glow(0.0);
        zero_effect.stages[0].offset_x = 0.0;
        zero_effect.stages[0].offset_y = 0.0;
        assert!(
            zero_effect
                .materialize_user_space(
                    &ThemeResourcePolicy::default(),
                    1.0,
                    1.0,
                    1.0,
                    1.0,
                    EffectOutsets::default(),
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn source_graphic_resets_accumulated_shadow_outsets() {
        let stages = [
            (EffectInput::SourceGraphic, 50.0, -50.0, 8.0),
            (EffectInput::Previous, 20.0, 10.0, 10.0),
            (EffectInput::SourceGraphic, 3.0, -2.0, 2.0),
            (EffectInput::Previous, -4.0, 5.0, 1.0),
        ];
        let graph = EffectGraph::new(
            "reset",
            stages
                .into_iter()
                .map(
                    |(input, offset_x, offset_y, blur_radius)| EffectPrimitive::DropShadow {
                        input,
                        offset_x,
                        offset_y,
                        blur_radius,
                        spread: 0.0,
                        color: ThemeColorValue::parse("#204060").unwrap(),
                    },
                ),
        )
        .unwrap();
        let effect = SvgShadowEffect::from_graph(&graph).unwrap();
        let result = effect
            .materialize_rect(&ThemeResourcePolicy::default(), 50.0, 30.0, 4.0)
            .unwrap()
            .unwrap();
        assert_eq!(
            result.outsets(),
            EffectOutsets {
                top: 16.0,
                right: 17.0,
                bottom: 19.0,
                left: 18.0
            }
        );
    }
}
