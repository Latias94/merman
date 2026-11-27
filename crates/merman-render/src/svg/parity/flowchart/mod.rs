//! Connect admitted Flow-family artifacts to the family SVG renderer.

use super::*;
#[cfg(any(test, feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_render_flow::svg::SwimlaneSvgLayout;
use merman_render_flow::svg::{FlowchartSvgLayout, FlowchartSvgRoute, FlowchartSvgServices};

mod node_inventory;
mod svg_emit;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
mod swimlane;

pub(crate) use merman_render_flow::layout_config::{
    FlowchartRenderConfig, prepare_flowchart_render_config,
};
pub(crate) use merman_render_flow::node_inventory::{
    FlowchartNodeLayoutView, FlowchartPreparedNodes,
};
pub(crate) use merman_render_flow::style::FlowchartEdgeStylePlan;
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_flow::svg::flowchart_marker_metrics;

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]
pub(super) use svg_emit::render_native_compact_flowchart_artifact;
pub(super) use svg_emit::{flowchart_edge_geometry, render_flowchart_svg_artifact};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) use swimlane::{render_swimlane_svg_artifact, swimlane_edge_geometry};

#[cfg(all(test, feature = "layout-elk"))]
mod browser_measurement_tests;
#[cfg(test)]
mod edge_label_tests;
#[cfg(test)]
mod label_tests;
#[cfg(all(test, feature = "diagram-flowchart", feature = "layout-dagre"))]
mod paint_tests;
#[cfg(all(test, feature = "diagram-flowchart"))]
mod rough_geometry_tests;
#[cfg(all(test, feature = "diagram-swimlane"))]
mod swimlane_points_tests;
#[cfg(all(test, feature = "diagram-flowchart"))]
mod viewbox_node_bounds_tests;
#[cfg(test)]
use merman_render_flow::layout_config as render_config;

fn svg_layout(layout: &FlowchartLayout) -> FlowchartSvgLayout<'_> {
    use crate::graph_layout::RouteContract;

    let route_contract = match &layout.route_contract {
        RouteContract::DagreParity => FlowchartSvgRoute::DagreParity,
        #[cfg(all(feature = "layout-layered", feature = "layout-dagre"))]
        RouteContract::LayeredDagreFallback => FlowchartSvgRoute::LayeredDagreFallback,
        #[cfg(feature = "layout-layered")]
        RouteContract::Layered(routes) => FlowchartSvgRoute::Layered(routes),
        #[cfg(feature = "layout-elk")]
        RouteContract::ElkParity => FlowchartSvgRoute::ElkParity,
        #[cfg(feature = "layout-grid")]
        RouteContract::Grid(settings) => FlowchartSvgRoute::Grid(*settings),
        RouteContract::Swimlane => FlowchartSvgRoute::Swimlane,
        #[cfg(any(feature = "layout-shoal", test))]
        RouteContract::CanonicalAxis(routes) => FlowchartSvgRoute::CanonicalAxis(routes),
        #[cfg(all(
            feature = "layout-shoal",
            any(
                feature = "diagram-class",
                feature = "diagram-er",
                feature = "diagram-requirement",
                feature = "diagram-state",
                feature = "diagram-usecase"
            )
        ))]
        RouteContract::CanonicalGraph(_) => FlowchartSvgRoute::Legacy,
    };
    FlowchartSvgLayout {
        nodes: &layout.nodes,
        edges: &layout.edges,
        edge_owners: &layout.edge_owners,
        clusters: &layout.clusters,
        dom_node_order_by_root: &layout.dom_node_order_by_root,
        route_contract,
    }
}

#[cfg(any(test, feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn swimlane_svg_layout(layout: &SwimlaneLayout) -> SwimlaneSvgLayout<'_> {
    SwimlaneSvgLayout {
        direction: layout.direction,
        nodes: &layout.nodes,
        lanes: &layout.lanes,
        edges: &layout.edges,
        edge_owners: &layout.edge_owners,
        has_canonical_routes: layout.has_canonical_routes(),
    }
}

fn svg_services<'a>(options: &'a SvgExecution<'_>) -> FlowchartSvgServices<'a> {
    let session = options.family.session();
    let diagram_id = options.diagram_id_or("merman");
    let icon_registry = options.icon_registry();
    FlowchartSvgServices {
        diagram_id,
        root_viewport: options.root_viewport(options.family_id(), diagram_id),
        operation: session.operation_context(),
        work_meter: options.work_meter(),
        render_seed: options.seed(),
        timing: options.timing(),
        measurer: options.text_measurer(),
        math_renderer: options.math_renderer(),
        icon_registry,
        defer_palette_stylesheet: options.generation == crate::svg::SvgGeneration::Static
            && icon_registry.is_none(),
        native_canvas: session
            .native_theme()
            .map(|theme| theme.resolved().canvas()),
        canvas_background: session.canvas_background(),
        native_family_theme: options.native_family_theme(),
        edge_trace: options.debug.flowchart_edge_trace(),
    }
}
