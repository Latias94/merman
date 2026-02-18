//! Debug/analysis helpers for SVG output and layout parity.

mod architecture;
mod dagre;
mod flowchart;
mod mindmap;
mod svg;

pub(crate) use architecture::{debug_architecture_delta, summarize_architecture_deltas};
pub(crate) use dagre::compare_dagre_layout;
pub(crate) use flowchart::{
    debug_flowchart_data_points, debug_flowchart_edge_trace, debug_flowchart_layout,
    debug_flowchart_svg_diff, debug_flowchart_svg_positions, debug_flowchart_svg_roots,
};
pub(crate) use mindmap::debug_mindmap_svg_positions;
pub(crate) use svg::{debug_svg_bbox, debug_svg_data_points};
