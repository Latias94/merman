//! Actor rectangle filters share shadow lowering, but keep their own terminal ownership.

use super::super::*;
use super::actor_shapes::{SequenceActorRectStyle, actor_rect_geometry_supported};
use super::model::SequenceSvgModel;
use crate::diagram_theme::{SvgFilterRegion, SvgShadowEffect, SvgShadowEvidenceRecorder};
use crate::sequence::SequenceActorThemeReceipt;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct SequenceActorShadowPlan {
    effect: Option<SvgShadowEffect>,
    terminals: BTreeMap<String, (String, SvgFilterRegion)>,
    pub(super) bounds: Option<Bounds>,
}

impl SequenceActorShadowPlan {
    // Keep operation policy and the family-owned source/style facts explicit at this seam.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        effect: Option<SvgShadowEffect>,
        nodes: &rustc_hash::FxHashMap<&str, &LayoutNode>,
        model: &SequenceSvgModel,
        mirror: bool,
        style: SequenceActorRectStyle,
        config: &serde_json::Value,
        receipt: &mut SequenceActorThemeReceipt,
        options: &SvgExecution<'_>,
    ) -> Result<Self> {
        let mut plan = Self {
            effect,
            ..Self::default()
        };
        let Some(effect) = plan.effect.as_ref() else {
            return Ok(plan);
        };
        // Read the same generated CSS value as the actor writer. Relative lengths cannot supply
        // a reliable filter envelope; retain a residual instead of guessing a pixel width.
        let width = style.stroke_width.map(f64::from).or_else(|| {
            let value = MermaidThemeAdapter::new(config)
                .sequence_diagram()
                .stroke_width;
            let value = value.trim();
            value
                .strip_suffix("px")
                .unwrap_or(value)
                .parse::<f64>()
                .ok()
                .filter(|width| width.is_finite() && *width >= 0.0)
        });
        let Some(width) = width else {
            receipt.effect_unhandled = true;
            return Ok(plan);
        };
        for (index, actor_id) in model.actor_order.iter().enumerate() {
            options.work_meter().charge(1)?;
            let Some(actor) = model.actors.get(actor_id) else {
                continue;
            };
            if !actor_rect_geometry_supported(actor) {
                continue;
            }
            for placement in ["top", "bottom"] {
                if placement == "bottom" && !mirror {
                    continue;
                }
                let node_id = format!("actor-{placement}-{actor_id}");
                let Some(node) = nodes.get(node_id.as_str()) else {
                    continue;
                };
                options
                    .work_meter()
                    .charge(effect.stages().len().saturating_mul(3))?;
                let Some(materialized) = effect.materialize_rect(
                    &options.theme_resource_policy(),
                    node.width,
                    node.height,
                    width,
                )?
                else {
                    receipt.effect_unhandled = true;
                    continue;
                };
                let region = materialized.region();
                // Include the outward-rounded region actually written to SVG, not just the
                // unrounded sigma envelope, so native receipt containment uses the same bounds.
                let [x, y, w, h] = region.as_array().map(f64::from);
                let (left, top) = super::geometry::node_left_top(node);
                let bounds = Bounds {
                    min_x: left + x * node.width,
                    min_y: top + y * node.height,
                    max_x: left + (x + w) * node.width,
                    max_y: top + (y + h) * node.height,
                };
                if let Some(total) = &mut plan.bounds {
                    total.min_x = total.min_x.min(bounds.min_x);
                    total.min_y = total.min_y.min(bounds.min_y);
                    total.max_x = total.max_x.max(bounds.max_x);
                    total.max_y = total.max_y.max(bounds.max_y);
                } else {
                    plan.bounds = Some(bounds);
                }
                let id = format!(
                    "{}-actor-{index}-{placement}-theme-effect-{}",
                    options.diagram_id_or("merman"),
                    effect.id()
                );
                plan.terminals.insert(node_id, (id, region));
            }
        }
        Ok(plan)
    }

    pub(super) fn len(&self) -> usize {
        self.terminals.len()
    }

    pub(super) fn terminal<'a>(
        &'a self,
        node: &LayoutNode,
        recorder: &'a SvgShadowEvidenceRecorder,
    ) -> Option<SequenceActorShadow<'a>> {
        let (id, region) = self.terminals.get(&node.id)?;
        Some(SequenceActorShadow {
            effect: self.effect.as_ref()?,
            id,
            region: *region,
            recorder,
        })
    }
}

pub(super) struct SequenceActorShadow<'a> {
    effect: &'a SvgShadowEffect,
    id: &'a str,
    region: SvgFilterRegion,
    recorder: &'a SvgShadowEvidenceRecorder,
}

impl SequenceActorShadow<'_> {
    pub(super) fn write_definition(&self, out: &mut impl SvgOutput) -> String {
        super::super::shadow::write_theme_shadow_application(out, self.id, self.effect, self.region)
    }

    pub(super) fn record_emission(&self) {
        self.recorder
            .record_application(self.effect, self.id, self.region);
    }
}
