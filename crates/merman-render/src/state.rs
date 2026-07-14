//! State diagram (stateDiagram-v2) layout.
//!
//! Baseline: Mermaid@11.12.2.

const STATE_END_NODE_DAGRE_WIDTH_PX_11_12_2: f64 = 14.013_293_266_296_387;

type StateDiagramModel = merman_core::diagrams::state::StateDiagramRenderModel;
type StateNode = merman_core::diagrams::state::StateDiagramRenderNode;

mod config;
mod layout;

pub(crate) use config::{StateConfigView, state_text_style};

pub use layout::{
    debug_build_state_diagram_v2_dagre_graph, debug_extract_state_diagram_v2_cluster_graph,
    layout_state_diagram_v2_typed,
};

/// Renders a typed State model and layout without compatibility JSON.
pub fn render_state_diagram_v2_typed_with_debug(
    layout: &crate::model::StateDiagramV2Layout,
    model: &merman_core::diagrams::state::StateDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    session: &crate::environment::RenderSession,
    options: &crate::svg::SvgRenderOptions,
    debug: &crate::svg::SvgDebugOptions,
) -> crate::Result<String> {
    crate::svg::render_state_diagram_v2_svg_model_with_debug(
        layout,
        model,
        effective_config,
        diagram_title,
        session,
        options,
        debug,
    )
}
