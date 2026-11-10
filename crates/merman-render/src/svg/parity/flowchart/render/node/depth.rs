//! Compiled body depth reuses the admitted shape terminals, without labels or interactive IDs.

use super::*;
use crate::flowchart::FlowchartShape;

pub(in crate::svg::parity::flowchart) fn render_flowchart_node_depth(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing: RenderTiming,
    details: &mut FlowchartRenderDetails,
) -> crate::Result<()> {
    let Some(materialized) = ctx
        .node_effects
        .get()
        .and_then(|plan| plan.node(node_id))
        .and_then(|prepared| prepared.shadow())
    else {
        return Ok(());
    };
    ctx.work_meter.charge(1)?;
    let prepared = ctx
        .prepared_nodes
        .node(node_id)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!("missing prepared Flowchart depth style for `{node_id}`"),
        })?;
    let effect = prepared
        .style
        .effect()
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!("missing prepared Flowchart depth effect for `{node_id}`"),
        })?;
    let resolved = helpers::resolve_node_render_info(ctx, node_id).ok_or_else(|| {
        crate::Error::InvalidModel {
            message: format!("missing prepared Flowchart depth node `{node_id}`"),
        }
    })?;
    let geometry = prepare_node_geometry(ctx, node_id, &resolved)?;
    let dom_id = ctx
        .document_ids()
        .node(node_id)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!("missing Flowchart depth owner ID for `{node_id}`"),
        })?;
    let id = format!("{dom_id}-theme-effect-{}", effect.id());
    let reference = crate::svg::parity::shadow::write_theme_shadow_depth_application(
        out,
        &id,
        effect,
        materialized.region(),
    );
    let filter_attr = format!(r#" filter="{}""#, escape_attr(&reference));
    let filter_style = format!(";filter:{} !important", escape_attr(&reference));
    let selection = &prepared.selection;
    let style = selection
        .inline_style
        .as_deref()
        .unwrap_or(&prepared.source.node_style);
    let label_emission = label::FlowchartNodeLabelEmissionPlan::new(
        resolved.node_classes,
        prepared.source_text_style.as_ref(),
        &prepared.source,
        &prepared.terminal_sources,
        selection.typed_label_fill.as_deref(),
    );
    let common = FlowchartNodeRenderCommon {
        geometry,
        label_emission: &label_emission,
        style,
        junction_style: selection.junction_style.as_deref().unwrap_or(style),
        effect_filter_attr: &filter_attr,
        effect_filter_style: &filter_style,
        effect_selected: true,
        suppress_child_filters: true,
        theme_style: &selection.theme_style,
        rough_group_style: &prepared.rough_group_style,
        fill_color: &selection.fill,
        stroke_color: &selection.stroke,
        wrapped_in_a: false,
        timing,
    };
    out.push_str(r#"<g class=""#);
    helpers::write_class_attr(
        out,
        helpers::NodeWrapperClasses::new(resolved.class_attr_base, resolved.node_classes),
    );
    let _ = write!(
        out,
        r#"" data-merman-theme-depth="node" aria-hidden="true" pointer-events="none" data-look="{}" transform="translate({},{})">"#,
        escape_attr(common.look),
        fmt(common.layout_node.x + ctx.tx - origin_x),
        fmt(common.layout_node.y + ctx.ty - origin_y),
    );
    let receipt = match FlowchartShape::resolve(common.shape)? {
        FlowchartShape::Process => shapes::render_process_rectangle(out, &common, details),
        FlowchartShape::RoundedRectangle => shapes::render_rounded_rect(out, ctx, &common, details),
        FlowchartShape::Diamond => shapes::render_diamond(out, &common, details),
        FlowchartShape::Circle => shapes::render_circle(out, ctx, &common, details)?,
        FlowchartShape::DoubleCircle => shapes::render_double_circle(out, ctx, &common, details)?,
        _ => {
            return Err(crate::Error::InvalidModel {
                message: format!("unsupported prepared Flowchart depth silhouette for `{node_id}`"),
            });
        }
    };
    out.push_str("</g>");
    out.checkpoint()?;
    if !receipt.typed_effect_verified() {
        return Err(crate::Error::InvalidModel {
            message: format!("Flowchart depth did not reach its silhouette for `{node_id}`"),
        });
    }
    ctx.effect_evidence
        .record_depth_application(effect, &id, materialized.region());
    Ok(())
}
