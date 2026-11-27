//! Post-solve geometry projection for the Flow families' ELK adapter.

mod projection;

pub use projection::{FlowchartElkProjection, canonical_subgraphs, project_layout};

#[cfg(any(test, feature = "test-support"))]
pub use projection::{align_degenerate_node_to_anchor, mermaid_elk_paint_order};
