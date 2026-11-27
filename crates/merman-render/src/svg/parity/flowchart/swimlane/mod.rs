use super::*;
use merman_render_flow::svg::FlowchartSvgModelRequest;

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(in crate::svg::parity) fn render_swimlane_svg_artifact(
    artifact: &crate::family::FlowchartFamilyArtifact<SwimlaneLayout>,
    metadata: &merman_model::presentation::PresentationMetadata,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let layout = artifact.pair().layout();
    let model = artifact.pair().semantic();
    let flowchart_layout = adapt_swimlane_layout(layout);
    merman_render_flow::svg::render_parity(
        FlowchartSvgModelRequest {
            layout: svg_layout(&flowchart_layout),
            swimlane_layout: Some(swimlane_svg_layout(layout)),
            model,
            render_context: artifact.render_context(),
            effective_config: &metadata.effective_config,
            diagram_type: metadata.diagram_type.as_str(),
            diagram_title: metadata.title.as_deref(),
            svg_label_sidecar: artifact.svg_label_sidecar(),
            edge_style_plan: artifact.edge_style_plan(),
            prepared_theme: artifact.prepared_theme(),
            prepared_nodes: artifact.prepared_nodes()?,
            render_config: artifact.render_config(),
        },
        &svg_services(options),
    )
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(in crate::svg::parity) fn swimlane_edge_geometry(
    artifact: &crate::family::FlowchartFamilyArtifact<SwimlaneLayout>,
    metadata: &merman_model::presentation::PresentationMetadata,
    options: &SvgExecution<'_>,
) -> Result<Vec<crate::model::EdgePaintGeometry>> {
    let layout = artifact.pair().layout();
    let model = artifact.pair().semantic();
    let flowchart_layout = adapt_swimlane_layout(layout);
    merman_render_flow::svg::edge_geometry(
        FlowchartSvgModelRequest {
            layout: svg_layout(&flowchart_layout),
            swimlane_layout: Some(swimlane_svg_layout(layout)),
            model,
            render_context: artifact.render_context(),
            effective_config: &metadata.effective_config,
            diagram_type: metadata.diagram_type.as_str(),
            diagram_title: metadata.title.as_deref(),
            svg_label_sidecar: artifact.svg_label_sidecar(),
            edge_style_plan: artifact.edge_style_plan(),
            prepared_theme: artifact.prepared_theme(),
            prepared_nodes: artifact.prepared_nodes()?,
            render_config: artifact.render_config(),
        },
        &svg_services(options),
    )
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn adapt_swimlane_layout(layout: &SwimlaneLayout) -> FlowchartLayout {
    let (nodes, edges, clusters) = merman_render_flow::svg::project_swimlane_layout(
        &layout.nodes,
        &layout.edges,
        &layout.lanes,
        layout.direction,
        layout.has_canonical_routes(),
    );

    let route_contract = crate::graph_layout::RouteContract::Swimlane;
    #[cfg(feature = "layout-shoal")]
    let route_contract = layout
        .shoal
        .as_ref()
        .map_or(route_contract, |shoal| shoal.routes.clone());
    FlowchartLayout {
        nodes,
        edges,
        edge_owners: layout.edge_owners.clone(),
        clusters,
        bounds: layout.bounds.clone(),
        // Swimlane supplies its node schedule through the prepared node inventory.
        dom_node_order_by_root: Default::default(),
        route_contract,
        placement: Default::default(),
        uses_elk_adapter_dom: false,
    }
}
