//! Flowchart edge marker and class helpers.

#[cfg(any(
    feature = "layout-shoal",
    test,
    all(
        feature = "native-compact",
        feature = "diagram-flowchart",
        feature = "layout-dagre"
    )
))]
pub(in crate::svg::parity::flowchart) use merman_render_flow::markers::{
    FlowchartMarkerBase, flowchart_edge_marker_end_base, flowchart_edge_marker_start_base,
};

pub(super) use merman_render_flow::edge_svg::flowchart_neo_mask_uses_dash_pairs;
