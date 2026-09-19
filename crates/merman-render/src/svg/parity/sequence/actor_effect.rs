//! Participant rectangle and lifeline effects retain their own terminal geometry.

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

/// Lifelines obtain geometry from the actual writer rather than retaining a second line plan.
#[derive(Default)]
pub(super) struct SequenceLifelinePaint {
    effect: Option<SvgShadowEffect>,
    emitted: usize,
    pub(super) bounds: Option<Bounds>,
}

impl SequenceLifelinePaint {
    pub(super) fn new(effect: Option<SvgShadowEffect>) -> Self {
        Self {
            effect,
            ..Self::default()
        }
    }

    pub(super) fn has_effect(&self) -> bool {
        self.effect.is_some()
    }
    pub(super) fn emitted_count(&self) -> usize {
        self.emitted
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_definition(
        &mut self,
        out: &mut impl SvgOutput,
        index: usize,
        x: f64,
        y1: f64,
        y2: f64,
        width: f64,
        options: &SvgExecution<'_>,
        receipt: &mut crate::sequence::SequenceLifelineThemeReceipt,
    ) -> Result<Option<(String, SvgFilterRegion)>> {
        if self.effect.is_none() && width == super::actor_shapes::LIFELINE_STROKE_WIDTH_PX {
            return Ok(None);
        }
        // Use the serialized coordinates, including descending or zero-length lines.
        let [x, y1, y2] = [x, y1, y2].map(crate::number_format::canonicalize_number);
        let mut bounds = Bounds {
            min_x: x - width / 2.0,
            max_x: x + width / 2.0,
            min_y: y1.min(y2) - width / 2.0,
            max_y: y1.max(y2) + width / 2.0,
        };
        let mut application = None;
        if let Some(effect) = &self.effect {
            options
                .work_meter()
                .charge(effect.stages().len().saturating_mul(3))?;
            if let Some(shadow) = effect.materialize_user_space(
                &options.theme_resource_policy(),
                bounds.min_x,
                bounds.min_y,
                bounds.max_x,
                bounds.max_y,
                crate::diagram_theme::EffectOutsets::default(),
            )? {
                let region = shadow.region();
                let [x, y, w, h] = region.as_array().map(f64::from);
                bounds = Bounds {
                    min_x: x,
                    min_y: y,
                    max_x: x + w,
                    max_y: y + h,
                };
                let id = format!(
                    "{}-lifeline-{index}-theme-effect-{}",
                    options.diagram_id_or("merman"),
                    effect.id()
                );
                super::super::shadow::write_theme_shadow_application(out, &id, effect, region);
                application = Some((id, region));
            } else {
                receipt.effect_unhandled = true;
            }
        }
        if let Some(total) = &mut self.bounds {
            total.min_x = total.min_x.min(bounds.min_x);
            total.min_y = total.min_y.min(bounds.min_y);
            total.max_x = total.max_x.max(bounds.max_x);
            total.max_y = total.max_y.max(bounds.max_y);
        } else {
            self.bounds = Some(bounds);
        }
        out.checkpoint()?;
        Ok(application)
    }

    pub(super) fn record_emission(
        &mut self,
        application: Option<&(String, SvgFilterRegion)>,
        evidence: &SvgShadowEvidenceRecorder,
        receipt: &mut crate::sequence::SequenceLifelineThemeReceipt,
    ) {
        if let Some((id, region)) = application {
            evidence.record_application(
                self.effect.as_ref().expect("materialized lifeline effect"),
                id,
                *region,
            );
            self.emitted = self.emitted.saturating_add(1);
            receipt.record_effect_emission();
        } else if receipt.effect_cleared {
            receipt.record_effect_emission();
        }
    }
}
