use merman_render_flow::class_style;
use merman_render_flow::layout_config as config;
#[cfg(any(feature = "layout-grid", feature = "layout-layered"))]
use merman_render_flow::style::FlowchartEdgeStylePlan;
pub(crate) use merman_render_flow::theme as native_theme;
#[cfg(test)]
mod bpmn_layout_tests;
#[cfg(feature = "layout-dagre")]
pub(crate) mod dagre;
#[cfg(feature = "layout-elk")]
pub mod elk;
#[cfg(feature = "layout-shoal")]
use merman_render_flow::measured;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;

#[cfg(any(
    test,
    feature = "layout-dagre",
    feature = "layout-grid",
    feature = "layout-layered"
))]
pub(crate) use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};

pub(crate) use class_style::FlowchartClassDefinitions;
pub(crate) use merman_render_flow::projection::{
    FlowchartEdgeOwners, FlowchartEdgeTransportPlan, FlowchartRenderModelRef,
    project_flowchart_edge,
};
pub(crate) use native_theme::FlowchartPreparedTheme;

#[cfg(feature = "layout-shoal")]
pub(crate) use crate::graph_label::flowchart_label_is_empty_for_render;
#[cfg(feature = "layout-shoal")]
pub(crate) use config::FlowchartLayoutSettings;
pub(crate) use config::{FlowchartBaseTypographyPlan, FlowchartConfigView};
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use merman_render_core::text::literal_label::FlowchartSvgWidthMode;
#[cfg(test)]
pub(crate) use merman_render_core::text::{
    flowchart_wrap_svg_source_word_lines,
    literal_label::flowchart_non_markdown_svg_source_word_lines,
};
#[cfg(all(test, feature = "layout-elk"))]
pub(crate) use merman_render_flow::node_geometry::flowchart_node_label_min_width;
#[cfg(any(feature = "layout-shoal", all(test, feature = "diagram-flowchart")))]
pub(crate) use merman_render_flow::node_geometry::flowchart_node_render_dimensions;
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_flow::node_geometry::{
    resolve_flowchart_shape, validate_flowchart_node_asset_dimensions,
};
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_flow::shapes::FlowchartShape;
#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "layout-shoal"
))]
pub(crate) use merman_render_flow::style::flowchart_effective_text_style_for_node_classes;
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use merman_render_flow::style::flowchart_swimlane_label_rect_text_style;
#[cfg(test)]
pub(crate) use merman_render_flow::svg_label::FlowchartPreparedLabelResolution;
#[cfg(any(
    feature = "layout-shoal",
    all(test, any(feature = "all-diagrams", feature = "layout-elk"))
))]
pub(crate) use merman_render_flow::svg_label::FlowchartSvgLabelOwner;
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use merman_render_flow::svg_label::FlowchartSvgLabelRenderPlan;
pub(crate) use merman_render_flow::svg_label::{
    FlowchartSvgLabelSidecar, FlowchartSvgLabelSidecarBuilder,
};

#[cfg(feature = "layout-grid")]
pub(crate) fn grid_layout(
    model: &FlowchartModel,
    context: &FlowchartRenderContext,
    classes: &FlowchartClassDefinitions<'_>,
    config: &merman_model::presentation::PresentationConfig,
    execution: &crate::LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style: &FlowchartEdgeStylePlan,
) -> crate::Result<crate::model::FlowchartLayout> {
    let grid = merman_render_flow::grid::layout(
        FlowchartRenderModelRef::new(model, context),
        classes,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        labels,
        edge_style,
        execution.work_meter_ref(),
    )?;
    Ok(crate::model::FlowchartLayout {
        nodes: grid.nodes,
        edges: grid.edges,
        edge_owners: FlowchartEdgeOwners::new(grid.edge_keys),
        clusters: grid.clusters,
        bounds: grid.bounds,
        dom_node_order_by_root: grid.dom_node_order_by_root,
        route_contract: crate::graph_layout::RouteContract::Grid(grid.paint),
        placement: Default::default(),
        uses_elk_adapter_dom: false,
    })
}

#[cfg(feature = "layout-layered")]
pub(crate) fn layered_layout(
    model: &FlowchartModel,
    context: &FlowchartRenderContext,
    classes: &FlowchartClassDefinitions<'_>,
    config: &merman_model::presentation::PresentationConfig,
    execution: &crate::LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    styles: &FlowchartEdgeStylePlan,
) -> std::result::Result<crate::model::FlowchartLayout, merman_render_flow::layered::FlowLayeredError>
{
    let layout = merman_render_flow::layered::layout(
        FlowchartRenderModelRef::new(model, context),
        classes,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        labels,
        styles,
        execution.work_meter_ref(),
    )?;
    Ok(crate::model::FlowchartLayout {
        nodes: layout.nodes,
        edges: layout.edges,
        edge_owners: layout.edge_owners,
        clusters: layout.clusters,
        bounds: layout.bounds,
        dom_node_order_by_root: layout.dom_node_order_by_root,
        route_contract: crate::graph_layout::RouteContract::Layered(std::sync::Arc::new(
            layout.routes,
        )),
        placement: Default::default(),
        uses_elk_adapter_dom: false,
    })
}
