//! Facade loader selection and shared backend hooks for Mindmap.

use merman_model::{
    families::mindmap::MindmapDiagramRenderModel, presentation::PresentationConfig,
};
use merman_render_graph::mindmap::{
    MindmapLayoutBackend, MindmapLayoutContext, MindmapSwimlaneGeometry, prepare_mindmap_diagram,
};
pub(crate) use merman_render_graph::mindmap::{MindmapPreparedArtifact, native_html_label_source};
use std::sync::Arc;

use crate::{
    LayoutExecution, Result,
    layout_backend::GraphLayoutBackend,
    model::{LayoutEdge, LayoutNode},
    resources::OperationWorkMeter,
};

#[cfg(test)]
mod tests;

pub(crate) fn prepare(
    model: &MindmapDiagramRenderModel,
    config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
) -> Result<MindmapPreparedArtifact> {
    let selected = execution.layout_backend();
    let unavailable = || selected.unavailable_for("mindmap");
    let backend = match selected {
        GraphLayoutBackend::TidyTree => MindmapLayoutBackend::TidyTree,
        GraphLayoutBackend::Swimlane => MindmapLayoutBackend::Swimlane(solve_swimlane),
        #[cfg(feature = "layout-dagre")]
        GraphLayoutBackend::Dagre => MindmapLayoutBackend::Dagre,
        #[cfg(feature = "layout-cytoscape")]
        GraphLayoutBackend::CoseBilkent => MindmapLayoutBackend::CoseBilkent,
        #[cfg(feature = "layout-elk")]
        GraphLayoutBackend::Elk(algorithm) => MindmapLayoutBackend::Elk {
            algorithm: algorithm.algorithm(),
            operation_seed: execution.elk_operation_seed(),
        },
        _ => MindmapLayoutBackend::Unavailable(&unavailable),
    };

    prepare_mindmap_diagram(
        model,
        config,
        execution.native_family_theme(),
        MindmapLayoutContext {
            backend,
            text_measurer: execution.text_measurer(),
            math_renderer: execution.math_renderer(),
            measure_math_label: crate::graph_label::flowchart_label_metrics_for_layout,
            work_meter: execution.family.session().work_meter(),
        },
    )
}

fn solve_swimlane(
    measured: &[(&LayoutNode, &str)],
    edges: &[LayoutEdge],
    config: &PresentationConfig,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<MindmapSwimlaneGeometry> {
    let placed = crate::swimlane::layout_flat(measured, edges, config, work_meter)?;
    let nodes = placed
        .nodes
        .into_iter()
        .map(|node| LayoutNode {
            id: node.id,
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            is_cluster: false,
            label_width: Some(node.label_width),
            label_height: Some(node.label_height),
        })
        .collect();
    let edge_points = placed
        .edges
        .into_iter()
        .map(|edge| (edge.id, edge.points))
        .collect();
    Ok(MindmapSwimlaneGeometry {
        nodes,
        edge_points,
        lanes: placed.lanes,
        bounds: placed.bounds,
    })
}
