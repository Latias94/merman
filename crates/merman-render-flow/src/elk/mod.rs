//! Graph preparation and solved geometry projection for the Flow families' ELK adapter.

mod input;
mod projection;

pub use input::build_flowchart_elk_graph_with_render_labels_and_work_control;
#[cfg(any(test, feature = "test-support"))]
pub use input::parent_by_id;

pub use projection::{FlowchartElkProjection, canonical_subgraphs, project_layout};

#[cfg(any(test, feature = "test-support"))]
pub use projection::{align_degenerate_node_to_anchor, mermaid_elk_paint_order};
