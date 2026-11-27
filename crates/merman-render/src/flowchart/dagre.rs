use std::sync::Arc;

use merman_model::presentation::PresentationConfig;

use super::{FlowchartModel, FlowchartRenderContext, FlowchartSvgLabelSidecarBuilder};
use crate::{
    Result, math::MathRenderer, model::FlowchartLayout, resources::OperationWorkMeter,
    text::TextMeasurer,
};

#[cfg(test)]
pub(crate) fn layout_flowchart_typed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<FlowchartLayout> {
    layout_flowchart_typed_with_work_meter(
        model,
        effective_config,
        measurer,
        math_renderer,
        Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )),
    )
}

#[cfg(test)]
pub(crate) fn layout_flowchart_typed_with_work_meter(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
        model,
        effective_config,
        false,
        work_meter.as_ref(),
    )?;
    layout(
        model,
        &FlowchartRenderContext::default(),
        &super::FlowchartClassDefinitions::new(&model.class_defs),
        effective_config,
        measurer,
        math_renderer,
        None,
        &edge_style_plan,
        work_meter,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout receives independent label artifacts and cumulative work controls"
)]
pub(crate) fn layout(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &super::FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let result = merman_render_flow::dagre::layout(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        measurer,
        math_renderer,
        svg_label_sidecar,
        edge_style_plan,
        work_meter,
    )?;
    Ok(FlowchartLayout {
        nodes: result.nodes,
        edges: result.edges,
        edge_owners: result.edge_owners,
        clusters: result.clusters,
        bounds: result.bounds,
        dom_node_order_by_root: result.dom_node_order_by_root,
        route_contract: crate::graph_layout::RouteContract::DagreParity,
        placement: Default::default(),
        uses_elk_adapter_dom: false,
    })
}

#[cfg(test)]
mod tests;
