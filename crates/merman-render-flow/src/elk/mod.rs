//! ELK graph preparation, controlled layout, and geometry projection for the Flow families.

mod input;
mod operation;
mod projection;

pub use input::build_flowchart_elk_graph_with_render_labels_and_work_control;
#[cfg(any(test, feature = "test-support"))]
pub use input::parent_by_id;

pub use operation::{FlowchartElkLayoutExecution, layout};
#[cfg(any(test, feature = "test-support"))]
pub use operation::{layout_diagnostic, solve_diagnostic};
pub use projection::{FlowchartElkProjection, canonical_subgraphs, project_layout};

#[cfg(any(test, feature = "test-support"))]
pub use projection::{align_degenerate_node_to_anchor, mermaid_elk_paint_order};
