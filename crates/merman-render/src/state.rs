//! State diagram (stateDiagram-v2) layout.
//!
//! Source semantics: Mermaid 12.0.0.

type StateDiagramModel = merman_model::families::state::StateDiagramRenderModel;
#[cfg(any(
    test,
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
type StateNode = merman_model::families::state::StateDiagramRenderNode;

#[cfg(feature = "layout-shoal")]
use merman_render_graph::state::StateCompatibilityPlan;
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
use merman_render_graph::state::StateLabelOwner;
#[cfg(feature = "layout-dagre")]
use merman_render_graph::state::{
    StateLabelMetricsRequest, StateLabelSourceKind, measure_state_markdown_label,
};
pub(crate) use merman_render_graph::state::{
    StateLabelSidecar, StateLabelSidecarBuilder, StateStylePlan,
};
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
use merman_render_graph::state::{StateNodeStylePlan, state_value_to_label_text};

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
mod config {
    #[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
    pub(super) use merman_render_graph::state::normalize_dir;
    pub(super) use merman_render_graph::state::{StateConfigView, state_node_is_effective_group};
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
mod measure {
    #[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
    pub(super) use merman_render_graph::state::state_fork_join_painted_dimensions;
    pub(super) use merman_render_graph::state::{
        edge_label_metrics, state_node_dimensions, title_label_metrics,
    };
}

#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk;
mod layout;
#[cfg(feature = "layout-shoal")]
mod shoal;

#[cfg(feature = "layout-dagre")]
pub use dagre::{debug_build_state_diagram_dagre_graph, debug_extract_state_diagram_cluster_graph};
pub(crate) use layout::layout_state_diagram_typed_with_work_meter;
