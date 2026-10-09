//! Edge-owned shadow geometry shared by viewport preparation and edge emission.

use rustc_hash::FxHashMap;

use super::*;
use crate::diagram_theme::{EffectOutsets, MaterializedShadowEffect, ThemeResourcePolicy};

#[derive(Debug, Default)]
pub(super) struct FlowchartEdgeEffects {
    edges: FxHashMap<crate::flowchart::FlowchartEdgeKey, MaterializedShadowEffect>,
}

impl FlowchartEdgeEffects {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        render_edges: &[super::render_input::FlowchartRenderEdge<'_>],
        edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        resources: &ThemeResourcePolicy,
        marker_plan: &super::defs::FlowchartMarkerEmissionPlan,
    ) -> crate::Result<Self> {
        let mut plan = Self::default();
        if !ctx.effect_eligibility.edge {
            return Ok(plan);
        }

        if flowchart_config_look(ctx.config) != "classic" {
            return Ok(plan);
        }
        let Some(effect) = ctx.edge_theme.effect() else {
            return Ok(plan);
        };
        let ancestor_classes: &[&str] =
            if ctx.uses_elk_adapter_dom || ctx.swimlane_direction.is_some() {
                &["root", "edges", "edgePaths"]
            } else {
                &["root", "edgePaths"]
            };
        if !super::style::edge_shadow_structural_styles_are_bounded(
            ctx.class_defs,
            ancestor_classes,
        ) {
            return Ok(plan);
        }
        for render_edge in render_edges {
            let edge = render_edge.as_ref();
            let Some(cache_entry) = edge_cache.get(&edge.key) else {
                continue;
            };
            ctx.work_meter.charge(1)?;
            let source_style = ctx.edge_style_plan.edge_for(edge.key)?;
            if !source_style.source_filter_status().is_absent()
                || !source_style.edge_shadow_source_is_bounded()
            {
                continue;
            }
            let Some(paint_outset) = ctx
                .edge_style_plan
                .stroke_width_for(edge.key)?
                .paint_outset()
            else {
                continue;
            };
            let Some(marker_outset) = marker_plan.edge_paint_outset_for(edge.key) else {
                continue;
            };
            // The SVG default miter limit is four; round or bevel joins only reduce this bound.
            let paint_outset = (4.0 * paint_outset).max(marker_outset);
            let source = EffectOutsets {
                top: paint_outset,
                right: paint_outset,
                bottom: paint_outset,
                left: paint_outset,
            };
            let Some(pb) = cache_entry.geom.pb else {
                continue;
            };
            let Some(shadow) = effect.materialize_user_space(
                resources, pb.min_x, pb.min_y, pb.max_x, pb.max_y, source,
            )?
            else {
                continue;
            };
            plan.edges.insert(edge.key, shadow);
        }
        Ok(plan)
    }

    pub(super) fn edge(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<&MaterializedShadowEffect> {
        self.edges.get(&key)
    }

    pub(super) fn expected_applications(&self) -> usize {
        self.edges.len()
    }

    pub(super) fn include_bounds(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
        origin_x: f64,
        abs_top_transform: f64,
        include: &mut impl FnMut(f64, f64, f64, f64),
    ) {
        let Some(shadow) = self.edges.get(&key) else {
            return;
        };
        let region = shadow.region().as_array();
        include(
            f64::from(region[0]) + origin_x,
            f64::from(region[1]) + abs_top_transform,
            f64::from(region[0]) + f64::from(region[2]) + origin_x,
            f64::from(region[1]) + f64::from(region[3]) + abs_top_transform,
        );
    }
}
