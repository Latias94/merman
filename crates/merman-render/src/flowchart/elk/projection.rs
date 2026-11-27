use crate::Result;
use crate::flowchart::config::FlowchartConfigView;
use crate::flowchart::{FlowchartEdgeOwners, FlowchartRenderModelRef};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::FlowchartLayout;
use merman_layout_elk as elk;
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
use merman_render_flow::elk::{FlowchartElkProjection, project_layout};

#[cfg(test)]
pub(super) use merman_render_flow::elk::{
    align_degenerate_node_to_anchor, mermaid_elk_paint_order,
};

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
    let FlowchartElkProjection {
        nodes,
        edges,
        edge_keys,
        clusters,
        bounds,
        dom_node_order_by_root,
    } = project_layout(render_model, settings, graph, layout, work_control)?;

    Ok(FlowchartLayout {
        nodes,
        edges,
        edge_owners: FlowchartEdgeOwners::new(edge_keys),
        clusters,
        bounds,
        dom_node_order_by_root,
        route_contract: crate::graph_layout::RouteContract::ElkParity,
        placement: Default::default(),
        uses_elk_adapter_dom: true,
    })
}
