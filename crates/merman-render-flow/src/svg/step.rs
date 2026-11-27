//! Prepare Step badge inputs from the resolved layout and hierarchy.

use super::*;
use crate::edge_svg::FlowchartSteps;
use crate::graph::{StepBadgePlacement, StepBadges};

pub(super) fn prepare_step_badges<'a>(
    ctx: &FlowchartRenderCtx<'a>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    cache: &FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    labels: &super::paint::FlowchartEdgeLabels,
) -> merman_render_core::Result<StepBadges<'a>> {
    let mut placement = StepBadgePlacement::new(ctx.config, ctx.work_meter);
    if !ctx.steps.badges_enabled() {
        return Ok(placement.finish());
    }
    for id in hierarchy.rendered_node_ids() {
        ctx.work_meter.charge(1)?;
        if let Some(node) = ctx.layout_nodes_by_id.get(id) {
            let offsets = hierarchy
                .effective_parent(id)
                .and_then(|root| hierarchy.root_offsets(root));
            let shift_y = offsets.map_or(0.0, |offset| offset.abs_top_transform - offset.origin_y);
            placement.push_box_obstacle(
                kurbo::Point::new(node.x, node.y + shift_y),
                kurbo::Size::new(node.width, node.height),
            );
        }
    }
    for edge in hierarchy.ordered_edges() {
        ctx.work_meter.charge(1)?;
        let Some(entry) = cache.get(&edge.key) else {
            continue;
        };
        if let Some(label) = ctx
            .layout_edges_by_key
            .get(&edge.key)
            .and_then(|edge| edge.label())
            && let Some(point) = labels.position(edge.key)
        {
            placement.push_box_obstacle(
                kurbo::Point::new(point.x, point.y + entry.abs_top_transform - entry.origin_y),
                kurbo::Size::new(label.width, label.height),
            );
        }
        placement.push_arrow_obstacles(
            &entry.geom.data_points,
            edge.edge.edge_type.as_deref(),
            kurbo::Vec2::new(entry.origin_x, entry.abs_top_transform),
        );
    }
    for edge in hierarchy.ordered_edges() {
        ctx.work_meter.charge(1)?;
        let Some(step) = edge
            .edge
            .step
            .filter(|step| merman_model::families::flowchart::FlowEdge::is_valid_step(*step))
        else {
            continue;
        };
        let Some(entry) = cache.get(&edge.key) else {
            continue;
        };
        let geom = &entry.geom;
        let origin = kurbo::Vec2::new(entry.origin_x, entry.abs_top_transform);
        let Some(point) = placement.place(
            &geom.d,
            geom.emitted_d_for_label.as_deref(),
            &geom.data_points,
            origin,
        )?
        else {
            continue;
        };
        let color =
            FlowchartSteps::color_for(ctx.edge_style_plan, ctx.compatibility, edge.key)?.to_owned();
        let root = hierarchy.edge_root(edge.key)?;
        placement.push_badge(point, origin, step, color, root);
    }
    Ok(placement.finish())
}
