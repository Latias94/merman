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
}

impl SvgFilterRegion {
    pub(crate) fn try_bounded(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        let region = Self {
            x,
            y,
            width,
            height,
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
                return Ok(None);
            };
            outsets = EffectOutsets {
                top: input.top + extension.top,
                right: input.right + extension.right,
                bottom: input.bottom + extension.bottom,
                left: input.left + extension.left,
            };
        }
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
