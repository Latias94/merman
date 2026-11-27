#[cfg(test)]
use crate::{
    Result,
    flowchart::{FlowchartRenderModelRef, config::FlowchartConfigView},
    layout_work::OperationLayoutWorkControl,
};
use crate::{flowchart::FlowchartEdgeOwners, model::FlowchartLayout};
#[cfg(test)]
use merman_layout_elk as elk;
#[cfg(test)]
use merman_model::{
    families::flowchart::{FlowchartModel, FlowchartRenderContext},
    presentation::PresentationConfig,
};
use merman_render_flow::elk::FlowchartElkProjection;
#[cfg(test)]
use merman_render_flow::elk::project_layout;

#[cfg(test)]
pub(super) use merman_render_flow::elk::{
    align_degenerate_node_to_anchor, mermaid_elk_paint_order,
};

#[cfg(test)]
pub(super) fn flowchart_layout_from_elk_with_render_labels_and_work_control(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &PresentationConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
    work_control: Option<&mut OperationLayoutWorkControl>,
) -> Result<FlowchartLayout> {
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let settings = FlowchartConfigView::new(effective_config).layout_settings();
    project_layout(render_model, settings, graph, layout, work_control).map(assemble_layout)
}

pub(super) fn assemble_layout(projection: FlowchartElkProjection) -> FlowchartLayout {
    let FlowchartElkProjection {
        nodes,
        edges,
        edge_keys,
        clusters,
        bounds,
        dom_node_order_by_root,
    } = projection;

    FlowchartLayout {
        nodes,
        edges,
        edge_owners: FlowchartEdgeOwners::new(edge_keys),
        clusters,
        bounds,
        dom_node_order_by_root,
        route_contract: crate::graph_layout::RouteContract::ElkParity,
        placement: Default::default(),
        uses_elk_adapter_dom: true,
    }
}
