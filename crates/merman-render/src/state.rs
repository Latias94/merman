//! State diagram (stateDiagram-v2) layout.
//!
//! Source semantics: Mermaid 12.0.0.

type StateDiagramModel = merman_model::families::state::StateDiagramRenderModel;
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
type StateNode = merman_model::families::state::StateDiagramRenderNode;

#[cfg(feature = "layout-shoal")]
use merman_render_graph::state::StateCompatibilityPlan;
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
use merman_render_graph::state::StateLabelOwner;
pub(crate) use merman_render_graph::state::{
    StateLabelSidecar, StateLabelSidecarBuilder, StateStylePlan,
};
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
use merman_render_graph::state::{StateNodeStylePlan, state_value_to_label_text};

#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
mod config {
    #[cfg(feature = "layout-elk")]
    pub(super) use merman_render_graph::state::normalize_dir;
    pub(super) use merman_render_graph::state::{StateConfigView, state_node_is_effective_group};
}

#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
mod measure {
    pub(super) use merman_render_graph::state::{
        edge_label_metrics, state_fork_join_painted_dimensions, state_node_dimensions,
        title_label_metrics,
    };
}

#[cfg(feature = "layout-dagre")]
mod dagre_debug;
#[cfg(feature = "layout-elk")]
mod elk;
mod layout;
#[cfg(feature = "layout-shoal")]
mod shoal;

#[cfg(feature = "layout-dagre")]
pub use dagre_debug::debug_build_state_diagram_dagre_graph;
pub(crate) use layout::layout_state_diagram_typed_with_work_meter;
#[cfg(feature = "layout-dagre")]
pub use merman_render_graph::state::debug_extract_state_diagram_cluster_graph;
