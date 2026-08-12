use std::collections::BTreeMap;

use crate::diagram_theme::{
    EffectGraph, EffectInput, EffectPrimitive, FilterRegion, ThemeColorValue,
};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct StateEffectOutsets {
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
    pub(crate) left: f64,
}

impl StateEffectOutsets {
    fn include(&mut self, other: Self) {
        self.top = self.top.max(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
        self.left = self.left.max(other.left);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StateSvgEffect {
    id: String,
    region: FilterRegion,
    offset_x: f32,
    offset_y: f32,
    color: ThemeColorValue,
    outsets: StateEffectOutsets,
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
            region: graph.region(),
            offset_x: *offset_x,
            offset_y: *offset_y,
            color: color.clone(),
            outsets: StateEffectOutsets {
                top: f64::from((-offset_y).max(0.0)),
                right: f64::from(offset_x.max(0.0)),
                bottom: f64::from(offset_y.max(0.0)),
                left: f64::from((-offset_x).max(0.0)),
            },
        })
    }

    #[cfg(test)]
    pub(crate) fn from_graph_for_test(graph: &EffectGraph) -> Option<Self> {
        Self::from_graph(graph)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) const fn region(&self) -> FilterRegion {
        self.region
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

#[derive(Debug, Clone)]
pub(crate) struct StateNodeEffectPlan {
    effect_id: String,
}

impl StateNodeEffectPlan {
    pub(crate) fn effect_id(&self) -> &str {
        &self.effect_id
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StateEffectPlan {
    effects: BTreeMap<String, StateSvgEffect>,
    outsets: StateEffectOutsets,
}

impl StateEffectPlan {
    pub(crate) fn admit(&mut self, graph: &EffectGraph) -> Option<StateNodeEffectPlan> {
        let effect = StateSvgEffect::from_graph(graph)?;
        let effect_id = effect.id.clone();
        self.outsets.include(effect.outsets);
        self.effects.entry(effect_id.clone()).or_insert(effect);
        Some(StateNodeEffectPlan { effect_id })
    }

    pub(crate) fn effects(&self) -> impl ExactSizeIterator<Item = &StateSvgEffect> {
        self.effects.values()
    }

    pub(crate) fn effect(&self, id: &str) -> Option<&StateSvgEffect> {
        self.effects.get(id)
    }

    pub(crate) const fn outsets(&self) -> StateEffectOutsets {
        self.outsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(primitives: impl IntoIterator<Item = EffectPrimitive>) -> EffectGraph {
        EffectGraph::new(
            "shadow",
            FilterRegion::bounded(-0.2, -0.2, 1.4, 1.4),
            primitives,
        )
        .expect("valid effect graph")
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
        assert_eq!(
            plan.outsets(),
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
}
