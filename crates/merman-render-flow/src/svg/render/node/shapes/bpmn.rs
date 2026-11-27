use crate::bpmn::{BpmnShape, svg::BpmnSvgRequest};

pub(in crate::svg::render::node) fn render_bpmn(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &crate::svg::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    shape: BpmnShape,
) -> merman_render_core::Result<()> {
    let metrics = common.label_metrics(ctx, label);
    BpmnSvgRequest {
        shape,
        label_size: kurbo::Size::new(metrics.width, metrics.height),
        has_label: !label.text.is_empty(),
        style: common.style,
        theme_style: common.theme_style,
        icon: common.node_icon,
        node_id: common.node_id,
        document_ids: ctx.document_ids(),
        icon_registry: ctx.icon_registry,
        config: ctx.config,
        work_meter: ctx.work_meter,
    }
    .write(out, &mut label.dx, &mut label.dy)
}
