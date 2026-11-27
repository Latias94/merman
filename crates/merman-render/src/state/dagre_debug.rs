//! Public debug facade for the State Dagre input graph.

use super::StateDiagramModel;
use crate::Result;
use crate::text::TextMeasurer;
use dugong::graphlib::Graph;
use dugong::{EdgeLabel, GraphLabel, NodeLabel};
use serde_json::Value;

fn resolve_debug_style(model: &StateDiagramModel, config: &Value) -> super::StateStylePlan {
    use crate::layout_backend::{self, GraphLayoutBackend};

    let backend = layout_backend::resolve(crate::DiagramFamilyId::STATE, "stateDiagram", config)
        .unwrap_or(GraphLayoutBackend::Dagre);
    super::StateStylePlan::resolve_classic(model, config, backend == GraphLayoutBackend::Dagre)
}

/// Debug-only helper: builds the Dagre input graph for stateDiagram-v2 *before* layout runs.
///
/// This shares the same graph construction path as `layout`.
/// It is used by `xtask` to compare `dugong` against Mermaid's JS Dagre implementation
/// (`dagre-d3-es`) at the layout output layer (nodes/edges/points) rather than at the SVG layer.
#[doc(hidden)]
pub fn debug_build_state_diagram_dagre_graph(
    model: &StateDiagramModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<Graph<NodeLabel, EdgeLabel, GraphLabel>> {
    let style_plan = resolve_debug_style(model, effective_config);
    merman_render_graph::state::debug_build_state_diagram_dagre_graph(
        model,
        effective_config,
        &style_plan,
        measurer,
    )
}
