pub(crate) use merman_model::families::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};

#[cfg(feature = "compat-json")]
use serde_json::Value;

use super::CSS_EDGE_NOTE_EDGE;
use super::model::{StateDiagramModel, StateModelConstruction, StateNode};
#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
use crate::ManagedSemanticJson;
use crate::{OperationControl, OperationControlResult};
#[cfg(feature = "compat-json")]
use crate::{Result, SourcePresentationMetadata};

#[cfg(all(test, feature = "compat-json"))]
pub(super) use merman_model::families::state::compat::test_support::{
    projected_document_count, reset_projected_document_count,
};

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
    config: &crate::MermaidConfig,
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

pub(crate) fn sanitize_common_db_fields(
    model: &mut StateDiagramRenderModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
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

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &StateDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::state::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &StateDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedSemanticJson>> {
    Ok(merman_model::families::state::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}
