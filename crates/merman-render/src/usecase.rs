//! Mermaid 12 Usecase measurements and shared graph-layout projection.

use crate::Result;
use crate::model::{Bounds, LayoutEdge, LayoutNode};
use crate::resources::OperationWorkMeter;
use crate::text::{TextMeasurer, TextMetrics, TextStyle};
use merman_core::diagrams::usecase::{UsecaseDiagramRenderModel, UsecaseLabelType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

mod elk_edge_geometry;
mod layout;
mod measure;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsecaseDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseLabelPlan {
    pub text: String,
    pub label_type: UsecaseLabelType,
    pub metrics: TextMetrics,
    pub style: TextStyle,
    pub max_width: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseJsonRowPlan {
    pub key: UsecaseLabelPlan,
    pub value: UsecaseLabelPlan,
    pub accessible_key: String,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseJsonTablePlan {
    pub rows: Vec<UsecaseJsonRowPlan>,
    pub key_width: f64,
    pub value_width: f64,
    pub title_height: f64,
    pub border_width: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseNodePlan {
    pub id: String,
    pub parent: Option<String>,
    pub source_label: String,
    pub label: UsecaseLabelPlan,
    pub stereotype: Option<UsecaseLabelPlan>,
    pub folded_stereotype: bool,
    pub width: f64,
    pub height: f64,
    pub is_boundary: bool,
    pub package: bool,
    pub ellipse: bool,
    pub table: Option<UsecaseJsonTablePlan>,
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseEdgePlan {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<UsecaseLabelPlan>,
    pub minlen: usize,
    pub start_marker: Option<String>,
    pub end_marker: Option<String>,
    pub dotted: bool,
    pub internal: bool,
}

#[derive(Debug)]
pub(crate) struct UsecasePreparedArtifact {
    pub(crate) layout: UsecaseDiagramLayout,
    pub(crate) nodes: Vec<UsecaseNodePlan>,
    pub(crate) edges: Vec<UsecaseEdgePlan>,
    edge_paths: std::collections::HashMap<String, Vec<crate::model::LayoutPoint>>,
}

impl UsecasePreparedArtifact {
    pub(crate) fn layout(&self) -> &UsecaseDiagramLayout {
        &self.layout
    }

    pub(crate) fn edge_points(&self, edge: &LayoutEdge) -> Vec<crate::model::LayoutPoint> {
        self.edge_paths
            .get(&edge.id)
            .cloned()
            .unwrap_or_else(|| edge.points.clone())
    }
}

pub(crate) fn prepare_usecase_diagram(
    model: &UsecaseDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    work_meter: Arc<OperationWorkMeter>,
    #[cfg(feature = "layout-elk")] operation_seed: merman_layout_elk::ElkOperationSeed,
) -> Result<UsecasePreparedArtifact> {
    let mut work = crate::layout_work::OperationLayoutWorkControl::new(work_meter);
    let (nodes, edges) = measure::measure(model, effective_config, measurer, &mut work)?;
    let mut layout = layout::layout(
        model,
        effective_config,
        &nodes,
        &edges,
        &mut work,
        #[cfg(feature = "layout-elk")]
        operation_seed,
    )?;
    let edge_paths =
        layout::prepare_edge_paths(&mut layout, &nodes, &edges, effective_config, &mut work)?;
    Ok(UsecasePreparedArtifact {
        layout,
        nodes,
        edges,
        edge_paths,
    })
}
