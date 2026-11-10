use super::*;
use crate::diagram_theme::{MaterializedShadowEffect, SvgShadowEffect};

struct StateNodeShadow<'a> {
    node: &'a StateSvgNode,
    layout: &'a LayoutNode,
    body: StateRectBody<'a>,
    effect: &'a SvgShadowEffect,
    materialized: MaterializedShadowEffect,
}

fn prepare_state_node_shadows<'a>(
    ctx: &'a StateRenderCtx<'_>,
    root: Option<&str>,
) -> Result<Vec<StateNodeShadow<'a>>> {
    let mut shadows = Vec::new();
    if !ctx.has_node_shadows || state_effective_look(ctx) != "classic" {
        return Ok(shadows);
    }
    for &id in &ctx.node_order {
        ctx.work_meter.charge_emit_work(1)?;
        let Some(style) = ctx.style_plan.node(id) else {
            continue;
        };
        let Some(binding) = style.effect() else {
            continue;
        };
        let Some(node) = ctx.nodes_by_id.get(id).copied() else {
            continue;
        };
        let Some(layout) = ctx.layout_nodes_by_id.get(id).copied() else {
            continue;
        };
        if layout.is_cluster
            || node.shape != "rect"
            || state_is_hidden(ctx, id)
            || state_leaf_context(ctx, id) != root
        {
            continue;
        }
        let body = StateRectBody::new(layout, Some(style), ctx.style_plan.compatibility());
        let Some((effect, materialized)) = ctx.style_plan.effect_plan().materialize_classic_rect(
            binding.effect_id(),
            body.width,
            body.height,
            style.classic_stroke_paint_width().unwrap_or(0.0),
        )?
        else {
            continue;
        };
        ctx.work_meter.charge_emit_work(effect.stages().len())?;
        shadows.push(StateNodeShadow {
            node,
            layout,
            body,
            effect,
            materialized,
        });
    }
    Ok(shadows)
}

pub(super) fn render_state_node_shadows(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    root: Option<&str>,
    origin: (f64, f64),
    effect_outsets: &mut crate::diagram_theme::EffectOutsets,
) -> Result<()> {
    let shadows = prepare_state_node_shadows(ctx, root)?;
    for shadow in shadows {
        out.checkpoint()?;
        let node = shadow.node;
        let layout = shadow.layout;
        let node_dom_id = state_scoped_dom_id(ctx, &node.dom_id);
        let scoped_filter_id = format!("{}-theme-effect-{}", node_dom_id, shadow.effect.id());
        let region = shadow.materialized.region();
        let filter_url =
            write_theme_shadow_depth_application(out, &scoped_filter_id, shadow.effect, region);
        // Match body CSS and SourceAlpha without duplicating IDs, links, labels or hit targets.
        let _ = write!(
            out,
            r#"<g class="{}" data-look="{}" data-state-shadow-for="{}" aria-hidden="true" pointer-events="none" transform="translate({}, {})">"#,
            escape_attr(&state_node_class(node)),
            escape_attr(state_data_look(ctx)),
            node_dom_id.attr(),
            fmt_display(layout.x - origin.0),
            fmt_display(layout.y - origin.1),
        );
        shadow.body.write(out, Some(&filter_url));
        out.push_str("</g>");
        out.checkpoint()?;
        effect_outsets.include(shadow.materialized.outsets());
        ctx.effect_evidence
            .record_depth_application(shadow.effect, &scoped_filter_id, region);
    }
    out.checkpoint()
}
