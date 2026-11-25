use super::*;
use crate::theme::elevation_svg;

pub(super) fn render_state_node_shadows(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    root: Option<&str>,
    origin: (f64, f64),
    effect_outsets: &mut EffectOutsets,
) -> Result<()> {
    if let Some(elevation) = ctx.native_elevation {
        use kurbo::{Rect, RoundedRect, Shape};

        for (index, &id) in ctx.node_order.iter().enumerate() {
            ctx.work_meter.charge_emit_work(1)?;
            let (Some(node), Some(layout)) =
                (ctx.nodes_by_id.get(id), ctx.layout_nodes_by_id.get(id))
            else {
                continue;
            };
            if layout.is_cluster
                || !matches!(node.shape.as_str(), "rect" | "rectWithTitle")
                || state_is_hidden(ctx, id)
                || state_leaf_context(ctx, id) != root
            {
                continue;
            }
            let bounds = Rect::from_center_size((0.0, 0.0), (layout.width, layout.height));
            let radius = ctx
                .style_plan
                .node(id)
                .and_then(crate::state::StateNodeStylePlan::radius_override)
                .unwrap_or(ctx.style_plan.compatibility().rect_radius);
            let outline = RoundedRect::from_rect(bounds, radius).to_path(0.1);
            let depth_bounds = elevation_svg::bounds(elevation, bounds);
            effect_outsets.include(EffectOutsets {
                top: bounds.y0 - depth_bounds.y0,
                right: depth_bounds.x1 - bounds.x1,
                bottom: depth_bounds.y1 - bounds.y1,
                left: bounds.x0 - depth_bounds.x0,
            });
            let shadow_id = format!("{}-state-{index}", ctx.diagram_id);
            let _ = elevation_svg::write_depth(
                out,
                elevation,
                &shadow_id,
                &outline,
                (layout.x - origin.0, layout.y - origin.1),
            );
            out.checkpoint()?;
        }
        return Ok(());
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct EffectOutsets {
    pub(super) top: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
    pub(super) left: f64,
}

impl EffectOutsets {
    fn include(&mut self, other: Self) {
        self.top = self.top.max(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
        self.left = self.left.max(other.left);
    }
}
