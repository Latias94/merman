use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagram_theme::{
    EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue, ThemeResourceLimitExceeded,
    ThemeResourcePolicy,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StateSvgFilterRegion {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl StateSvgFilterRegion {
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
pub(crate) struct StateEffectOutsets {
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
    pub(crate) left: f64,
}

impl StateEffectOutsets {
    pub(crate) fn include(&mut self, other: Self) {
        self.top = self.top.max(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
        self.left = self.left.max(other.left);
    }

    fn for_hard_shadow(stroke_width: f64, offset_x: f32, offset_y: f32) -> Option<Self> {
        if !stroke_width.is_finite()
            || stroke_width < 0.0
            || !offset_x.is_finite()
            || !offset_y.is_finite()
        {
            return None;
        }

        let half_stroke = stroke_width / 2.0;
        let offset_x = f64::from(offset_x);
        let offset_y = f64::from(offset_y);
        let outsets = Self {
            top: half_stroke + (-offset_y).max(0.0),
            right: half_stroke + offset_x.max(0.0),
            bottom: half_stroke + offset_y.max(0.0),
            left: half_stroke + (-offset_x).max(0.0),
        };
        [outsets.top, outsets.right, outsets.bottom, outsets.left]
            .into_iter()
            .all(f64::is_finite)
            .then_some(outsets)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateSvgEffect {
    id: String,
    offset_x: f32,
    offset_y: f32,
    color: ThemeColorValue,
}

impl StateSvgEffect {
    fn from_graph(graph: &EffectGraph) -> Option<Self> {
        let [
            EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x,
                offset_y,
                blur_radius,
                spread,
                color,
            },
        ] = graph.primitives()
        else {
            return None;
        };
        if *blur_radius != 0.0 || *spread != 0.0 {
            return None;
        }

        Some(Self {
            id: graph.id().to_string(),
            offset_x: *offset_x,
            offset_y: *offset_y,
            color: color.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_graph_for_test(graph: &EffectGraph) -> Option<Self> {
        Self::from_graph(graph)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn materialize_classic_rect_region(
        &self,
        resources: &ThemeResourcePolicy,
        width: f64,
        height: f64,
        stroke_width: f64,
    ) -> Result<Option<StateMaterializedEffect>, ThemeResourceLimitExceeded> {
        // State derives the exact hard-shadow envelope from the final emitted paint geometry.
        if !width.is_finite() || width <= 0.0 || !height.is_finite() || height <= 0.0 {
            return Ok(None);
        }

        let Some(outsets) =
            StateEffectOutsets::for_hard_shadow(stroke_width, self.offset_x, self.offset_y)
        else {
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

        let Some(region) = StateSvgFilterRegion::try_bounded(x, y, region_width, region_height)
        else {
            return Ok(None);
        };
        let magnitude = region
            .as_array()
            .into_iter()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        resources.check_materialized_filter_region_magnitude(magnitude)?;

        Ok(Some(StateMaterializedEffect { region, outsets }))
    }

    pub(crate) const fn offset_x(&self) -> f32 {
        self.offset_x
    }

    pub(crate) const fn offset_y(&self) -> f32 {
        self.offset_y
    }

    pub(crate) const fn color(&self) -> &ThemeColorValue {
        &self.color
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StateMaterializedEffect {
    region: StateSvgFilterRegion,
    outsets: StateEffectOutsets,
}

impl StateMaterializedEffect {
    pub(crate) const fn region(self) -> StateSvgFilterRegion {
        self.region
    }

    pub(crate) const fn outsets(self) -> StateEffectOutsets {
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

#[derive(Debug, Clone)]
pub(crate) struct StateNodeEffectPlan {
    effect_id: String,
}

impl StateNodeEffectPlan {
    pub(crate) fn effect_id(&self) -> &str {
        &self.effect_id
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateEffectPlan {
    effects: BTreeMap<String, StateSvgEffect>,
    resources: Arc<ThemeResourcePolicy>,
}

impl Default for StateEffectPlan {
    fn default() -> Self {
        Self::with_resource_policy(Arc::new(ThemeResourcePolicy::default()))
    }
}

impl StateEffectPlan {
    pub(crate) fn with_resource_policy(resources: Arc<ThemeResourcePolicy>) -> Self {
        Self {
            effects: BTreeMap::new(),
            resources,
        }
    }

    pub(crate) fn admit(&mut self, graph: &EffectGraph) -> Option<StateNodeEffectPlan> {
        let effect = StateSvgEffect::from_graph(graph)?;
        let effect_id = effect.id.clone();
        self.effects.entry(effect_id.clone()).or_insert(effect);
        Some(StateNodeEffectPlan { effect_id })
    }

    pub(crate) fn effect(&self, id: &str) -> Option<&StateSvgEffect> {
        self.effects.get(id)
    }

    pub(crate) fn materialize_classic_rect(
        &self,
        id: &str,
        width: f64,
        height: f64,
        stroke_width: f64,
    ) -> Result<Option<(&StateSvgEffect, StateMaterializedEffect)>, ThemeResourceLimitExceeded>
    {
        let Some(effect) = self.effect(id) else {
            return Ok(None);
        };
        let Some(materialized) =
            effect.materialize_classic_rect_region(&self.resources, width, height, stroke_width)?
        else {
            return Ok(None);
        };
        Ok(Some((effect, materialized)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{ThemeResourceLimitId, ThemeResourceLimitPhase};

    fn graph(primitives: impl IntoIterator<Item = EffectPrimitive>) -> EffectGraph {
        EffectGraph::new("shadow", primitives).expect("valid effect graph")
    }

    fn hard_shadow(offset_x: f32, offset_y: f32) -> EffectGraph {
        graph([EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x,
            offset_y,
            blur_radius: 0.0,
            spread: 0.0,
            color: ThemeColorValue::parse("#111827").unwrap(),
        }])
    }

    fn plan_with_shadow(
        resources: ThemeResourcePolicy,
        offset_x: f32,
        offset_y: f32,
    ) -> StateEffectPlan {
        let mut plan = StateEffectPlan::with_resource_policy(Arc::new(resources));
        plan.admit(&hard_shadow(offset_x, offset_y))
            .expect("supported hard shadow");
        plan
    }

    fn assert_approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1.0e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn admits_only_single_source_graphic_hard_shadows() {
        let color = ThemeColorValue::parse("#111827").unwrap();
        let mut plan = StateEffectPlan::default();
        let admitted = plan
            .admit(&graph([EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 5.0,
                offset_y: -3.0,
                blur_radius: 0.0,
                spread: 0.0,
                color: color.clone(),
            }]))
            .expect("hard shadow should be admitted");
        assert_eq!(admitted.effect_id(), "shadow");
        let (_, materialized) = plan
            .materialize_classic_rect(admitted.effect_id(), 50.0, 20.0, 0.0)
            .expect("materialization resource admission")
            .expect("valid classic rect region");
        assert_eq!(
            materialized.outsets(),
            StateEffectOutsets {
                top: 3.0,
                right: 5.0,
                bottom: 0.0,
                left: 0.0,
            }
        );

        assert!(
            StateEffectPlan::default()
                .admit(&graph([
                    EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 1.0,
                    },
                    EffectPrimitive::DropShadow {
                        input: EffectInput::Previous,
                        offset_x: 1.0,
                        offset_y: 1.0,
                        blur_radius: 0.0,
                        spread: 0.0,
                        color: color.clone(),
                    },
                ]))
                .is_none()
        );
        assert!(
            StateEffectPlan::default()
                .admit(&graph([EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 1.0,
                    offset_y: 1.0,
                    blur_radius: 1.0,
                    spread: 0.0,
                    color,
                }]))
                .is_none()
        );
        for (blur_radius, spread) in [(f32::EPSILON, 0.0), (0.0, f32::EPSILON)] {
            assert!(
                StateEffectPlan::default()
                    .admit(&graph([EffectPrimitive::DropShadow {
                        input: EffectInput::SourceGraphic,
                        offset_x: 1.0,
                        offset_y: 1.0,
                        blur_radius,
                        spread,
                        color: ThemeColorValue::parse("#111827").unwrap(),
                    }]))
                    .is_none()
            );
        }
    }

    #[test]
    fn materializes_outward_regions_from_rect_paint_bounds() {
        let positive = plan_with_shadow(ThemeResourcePolicy::default(), 5.0, 6.0);
        let (_, region) = positive
            .materialize_classic_rect("shadow", 50.0, 20.0, 2.0)
            .expect("materialization resource admission")
            .expect("positive hard shadow region");
        assert_approx(region.region().x, -0.02);
        assert_approx(region.region().y, -0.05);
        assert_approx(region.region().width, 1.14);
        assert_approx(region.region().height, 1.4);

        let negative = plan_with_shadow(ThemeResourcePolicy::default(), -5.0, -6.0);
        let (_, region) = negative
            .materialize_classic_rect("shadow", 50.0, 20.0, 2.0)
            .expect("materialization resource admission")
            .expect("negative hard shadow region");
        assert_approx(region.region().x, -0.12);
        assert_approx(region.region().y, -0.35);
        assert_approx(region.region().width, 1.14);
        assert_approx(region.region().height, 1.4);

        let (_, larger) = positive
            .materialize_classic_rect("shadow", 100.0, 40.0, 2.0)
            .expect("materialization resource admission")
            .expect("larger classic rect region");
        assert_approx(larger.region().x, -0.01);
        assert_approx(larger.region().y, -0.025);
        assert_approx(larger.region().width, 1.07);
        assert_approx(larger.region().height, 1.2);
    }

    #[test]
    fn materialized_outsets_include_the_actual_stroke_width() {
        let mut plan = StateEffectPlan::default();
        plan.admit(&hard_shadow(-5.0, 6.0))
            .expect("hard shadow admitted");
        let (_, materialized) = plan
            .materialize_classic_rect("shadow", 50.0, 20.0, 4.0)
            .expect("materialization resource admission")
            .expect("classic rect region");

        assert_eq!(
            materialized.outsets(),
            StateEffectOutsets {
                top: 2.0,
                right: 2.0,
                bottom: 8.0,
                left: 7.0,
            }
        );
    }

    #[test]
    fn invalid_classic_rect_geometry_fails_closed() {
        let plan = plan_with_shadow(ThemeResourcePolicy::default(), 5.0, 6.0);
        for (width, height, stroke_width) in [
            (0.0, 20.0, 2.0),
            (-1.0, 20.0, 2.0),
            (50.0, 0.0, 2.0),
            (50.0, -1.0, 2.0),
            (f64::NAN, 20.0, 2.0),
            (50.0, f64::INFINITY, 2.0),
            (50.0, 20.0, -1.0),
            (50.0, 20.0, f64::NAN),
        ] {
            assert!(
                plan.materialize_classic_rect("shadow", width, height, stroke_width)
                    .expect("invalid geometry is not a resource rejection")
                    .is_none(),
                "geometry {width}x{height} with stroke {stroke_width} must be rejected"
            );
        }
    }

    #[test]
    fn terminal_region_obeys_the_compiled_soft_limit_at_the_exact_boundary() {
        let policy = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 2)
            .expect("valid terminal region limit");
        let plan = plan_with_shadow(policy, 50.0, 0.0);

        assert!(
            plan.materialize_classic_rect("shadow", 50.0, 20.0, 0.0)
                .expect("exact boundary is admitted")
                .is_some(),
            "a terminal width of exactly two object-bounding-box units must be admitted"
        );
        let error = plan
            .materialize_classic_rect("shadow", 49.0, 20.0, 0.0)
            .expect_err("one geometry unit shorter must cross the terminal policy");
        assert_eq!(
            error.limit,
            ThemeResourceLimitId::MaxEffectFilterRegionMagnitude.as_str()
        );
        assert_eq!(error.phase, ThemeResourceLimitPhase::EffectMaterialize);
        assert!(error.actual > error.max);
        assert_eq!(error.max, 2);
        assert_eq!(
            error.profile,
            Some(merman_core::resources::ResourceProfile::Interactive)
        );
    }

    #[test]
    fn terminal_region_obeys_the_non_overridable_hard_cap() {
        let policy = ThemeResourcePolicy::unbounded_for_trusted_input();
        let hard_cap = policy
            .value(ThemeResourceLimitId::EffectFilterRegionMagnitudeHardCap)
            .expect("effect filter-region hard cap");
        let exact = plan_with_shadow(policy.clone(), (hard_cap - 1) as f32, 0.0);
        assert!(
            exact
                .materialize_classic_rect("shadow", 1.0, 1.0, 0.0)
                .expect("exact hard-cap boundary is admitted")
                .is_some(),
            "the exact terminal hard-cap boundary must remain usable"
        );

        let exceeded = plan_with_shadow(policy, hard_cap as f32, 0.0);
        let error = exceeded
            .materialize_classic_rect("shadow", 1.0, 1.0, 0.0)
            .expect_err("terminal materialization must not bypass the hard cap");
        assert_eq!(
            error.limit,
            ThemeResourceLimitId::EffectFilterRegionMagnitudeHardCap.as_str()
        );
        assert_eq!(error.phase, ThemeResourceLimitPhase::EffectMaterialize);
        assert!(error.actual > error.max);
        assert_eq!(error.max, hard_cap);
        assert_eq!(
            error.profile,
            Some(merman_core::resources::ResourceProfile::UnboundedForTrustedInput)
        );
    }
}
