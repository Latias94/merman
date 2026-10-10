use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diagram_theme::{
    EffectGraph, MaterializedShadowEffect, SvgShadowEffect, ThemeResourceLimitExceeded,
    ThemeResourcePolicy,
};

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
    effects: BTreeMap<String, SvgShadowEffect>,
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
        let effect = SvgShadowEffect::from_graph(graph)?;
        let effect_id = effect.id().to_owned();
        self.effects.entry(effect_id.clone()).or_insert(effect);
        Some(StateNodeEffectPlan { effect_id })
    }

    pub(crate) fn effect(&self, id: &str) -> Option<&SvgShadowEffect> {
        self.effects.get(id)
    }

    pub(crate) fn materialize_classic_rect(
        &self,
        id: &str,
        width: f64,
        height: f64,
        stroke_width: f64,
    ) -> Result<Option<(&SvgShadowEffect, MaterializedShadowEffect)>, ThemeResourceLimitExceeded>
    {
        let Some(effect) = self.effect(id) else {
            return Ok(None);
        };
        let Some(materialized) =
            effect.materialize_rect(&self.resources, width, height, stroke_width)?
        else {
            return Ok(None);
        };
        Ok(Some((effect, materialized)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        EffectInput, EffectOutsets, EffectPrimitive, ThemeColorValue, ThemeResourceLimitId,
        ThemeResourceLimitPhase,
    };

    fn graph(primitives: impl IntoIterator<Item = EffectPrimitive>) -> EffectGraph {
        EffectGraph::new("shadow", primitives).expect("valid effect graph")
    }

    fn drop_shadow(offset_x: f32, offset_y: f32, blur_radius: f32) -> EffectGraph {
        graph([EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x,
            offset_y,
            blur_radius,
            spread: 0.0,
            color: ThemeColorValue::parse("#111827").unwrap(),
        }])
    }

    fn hard_shadow(offset_x: f32, offset_y: f32) -> EffectGraph {
        drop_shadow(offset_x, offset_y, 0.0)
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
    fn admits_shadow_sequences_without_spread_and_rejects_other_primitives() {
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
            EffectOutsets {
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
        let mut soft_plan = StateEffectPlan::default();
        let soft = soft_plan
            .admit(&graph([EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 1.0,
                offset_y: -2.0,
                blur_radius: 2.0,
                spread: 0.0,
                color,
            }]))
            .expect("a bounded single soft shadow should be admitted");
        let (_, materialized) = soft_plan
            .materialize_classic_rect(soft.effect_id(), 50.0, 20.0, 0.0)
            .expect("materialization resource admission")
            .expect("valid soft-shadow region");
        assert_eq!(
            materialized.outsets(),
            EffectOutsets {
                top: 10.0,
                right: 9.0,
                bottom: 8.0,
                left: 8.0,
            }
        );

        assert!(
            StateEffectPlan::default()
                .admit(&graph([EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 1.0,
                    offset_y: 1.0,
                    blur_radius: 0.0,
                    spread: f32::EPSILON,
                    color: ThemeColorValue::parse("#111827").unwrap(),
                }]))
                .is_none()
        );
    }

    #[test]
    fn materializes_four_sigma_soft_shadow_paint_bounds() {
        let mut plan = StateEffectPlan::default();
        plan.admit(&drop_shadow(0.0, 0.0, 8.0))
            .expect("soft shadow admitted");
        let (_, materialized) = plan
            .materialize_classic_rect("shadow", 50.0, 20.0, 2.0)
            .expect("materialization resource admission")
            .expect("soft-shadow region");

        assert_eq!(
            materialized.outsets(),
            EffectOutsets {
                top: 33.0,
                right: 33.0,
                bottom: 33.0,
                left: 33.0,
            }
        );
        assert_approx(materialized.region().as_array()[0], -0.66);
        assert_approx(materialized.region().as_array()[1], -1.65);
        assert_approx(materialized.region().as_array()[2], 2.32);
        assert_approx(materialized.region().as_array()[3], 4.3);
    }

    #[test]
    fn materializes_outward_regions_from_rect_paint_bounds() {
        let positive = plan_with_shadow(ThemeResourcePolicy::default(), 5.0, 6.0);
        let (_, region) = positive
            .materialize_classic_rect("shadow", 50.0, 20.0, 2.0)
            .expect("materialization resource admission")
            .expect("positive hard shadow region");
        assert_approx(region.region().as_array()[0], -0.02);
        assert_approx(region.region().as_array()[1], -0.05);
        assert_approx(region.region().as_array()[2], 1.14);
        assert_approx(region.region().as_array()[3], 1.4);

        let negative = plan_with_shadow(ThemeResourcePolicy::default(), -5.0, -6.0);
        let (_, region) = negative
            .materialize_classic_rect("shadow", 50.0, 20.0, 2.0)
            .expect("materialization resource admission")
            .expect("negative hard shadow region");
        assert_approx(region.region().as_array()[0], -0.12);
        assert_approx(region.region().as_array()[1], -0.35);
        assert_approx(region.region().as_array()[2], 1.14);
        assert_approx(region.region().as_array()[3], 1.4);

        let (_, larger) = positive
            .materialize_classic_rect("shadow", 100.0, 40.0, 2.0)
            .expect("materialization resource admission")
            .expect("larger classic rect region");
        assert_approx(larger.region().as_array()[0], -0.01);
        assert_approx(larger.region().as_array()[1], -0.025);
        assert_approx(larger.region().as_array()[2], 1.07);
        assert_approx(larger.region().as_array()[3], 1.2);
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
            EffectOutsets {
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
    fn soft_shadow_blur_is_rejected_when_its_materialized_region_exceeds_the_ceiling() {
        let policy = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 4)
            .expect("valid terminal region limit");
        let mut plan = StateEffectPlan::with_resource_policy(Arc::new(policy));
        plan.admit(&drop_shadow(0.0, 0.0, 8.0))
            .expect("soft shadow admitted");

        let error = plan
            .materialize_classic_rect("shadow", 50.0, 20.0, 2.0)
            .expect_err("four-sigma region height must cross the terminal policy");

        assert_eq!(
            error.limit,
            ThemeResourceLimitId::MaxEffectFilterRegionMagnitude.as_str()
        );
        assert_eq!(error.phase, ThemeResourceLimitPhase::EffectMaterialize);
        assert!(error.actual > error.max);
        assert_eq!(error.max, 4);
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
