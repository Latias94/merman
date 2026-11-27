use super::db::StateModelConstruction;
use merman_model::families::state::{
    CSS_EDGE_NOTE_EDGE, StateDiagramModel, StateDiagramRenderEdge, StateDiagramRenderModel,
    StateDiagramRenderNode, StateNode,
};
use merman_model::{MermaidConfig, OperationControl, OperationControlResult};

pub(super) struct StateNodePresentation {
    pub dom_id: String,
    pub padding: Option<f64>,
    pub rx: Option<f64>,
    pub ry: Option<f64>,
    pub color_index: Option<usize>,
    pub wrapping_width: Option<f64>,
    pub min_width: Option<f64>,
}

pub(super) fn into_render_model(
    construction: StateModelConstruction,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<StateDiagramRenderModel> {
    let StateDiagramModel {
        document,
        direction,
        acc_title,
        acc_descr,
        nodes,
        edges,
        relations,
        links,
        states,
        style_classes,
    } = construction.model;
    debug_assert_eq!(nodes.len(), construction.presentation.len());
    let mut render_nodes = Vec::with_capacity(nodes.len());
    for (index, (node, presentation)) in
        nodes.into_iter().zip(construction.presentation).enumerate()
    {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        render_nodes.push(render_node_from_semantic(node, presentation));
    }
    let mut render_edges = Vec::with_capacity(edges.len());
    for (index, edge) in edges.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let arrow_type_end = if edge.classes == CSS_EDGE_NOTE_EDGE {
            ""
        } else if config.get_str("look") == Some("neo") {
            "arrow_barb_neo"
        } else {
            "arrow_barb"
        };
        render_edges.push(StateDiagramRenderEdge {
            id: edge.id,
            start: edge.start,
            end: edge.end,
            classes: edge.classes,
            label: edge.label,
            arrow_type_end: arrow_type_end.to_string(),
        });
    }
    control.checkpoint()?;
    Ok(StateDiagramRenderModel {
        document,
        direction,
        acc_title,
        acc_descr,
        nodes: render_nodes,
        edges: render_edges,
        relations,
        links,
        states,
        style_classes,
    })
}

fn render_node_from_semantic(
    node: StateNode,
    presentation: StateNodePresentation,
) -> StateDiagramRenderNode {
    StateDiagramRenderNode {
        id: node.id,
        label_style: String::new(),
        label: node.label,
        description: node.description,
        dom_id: presentation.dom_id,
        is_group: node.is_group,
        node_type: node.node_type,
        parent_id: node.parent_id,
        css_classes: node.css_classes,
        css_compiled_styles: node.css_compiled_styles,
        css_styles: node.css_styles,
        dir: node.dir,
        explicit_dir: node.explicit_dir,
        padding: presentation.padding,
        rx: presentation.rx,
        ry: presentation.ry,
        shape: node.shape,
        position: node.position,
        color_index: presentation.color_index,
        wrapping_width: presentation.wrapping_width,
        min_width: presentation.min_width,
    }
}
