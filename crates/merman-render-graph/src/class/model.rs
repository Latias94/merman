//! Class layout DTOs and their retained label plans.

pub(super) use merman_render_core::{Bounds, LayoutCluster, LayoutEdge, LayoutNode, LayoutPoint};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug)]
pub struct ClassNodeRowMetrics {
    pub members: Vec<merman_render_core::text::TextMetrics>,
    pub methods: Vec<merman_render_core::text::TextMetrics>,
    pub(crate) _retained_reservation:
        Option<merman_render_core::resources::PreparedTextRetainedReservation>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct ClassPreparedHtmlLabel {
    pub metrics: merman_render_core::text::TextMetrics,
    pub max_width_px: i64,
    pub xhtml: String,
}

#[derive(Debug)]
pub struct ClassPreparedHtmlNodeLabels {
    pub title: ClassPreparedHtmlLabel,
    pub annotation: Option<ClassPreparedHtmlLabel>,
    pub members: Vec<ClassPreparedHtmlLabel>,
    pub methods: Vec<ClassPreparedHtmlLabel>,
    pub(crate) _retained_reservation:
        Option<merman_render_core::resources::PreparedTextRetainedReservation>,
}

/// Prepared labels share ownership through `Arc` without copying their admitted allocations.
#[derive(Debug)]
pub enum ClassNodeLabelPlan {
    RowMetrics(ClassNodeRowMetrics),
    PreparedHtml(ClassPreparedHtmlNodeLabels),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    /// Whether SVG emission follows Mermaid's registered ELK renderer DOM structure.
    #[serde(skip)]
    pub uses_elk_adapter_dom: bool,
    #[serde(skip)]
    pub class_label_plans_by_id: FxHashMap<String, Arc<ClassNodeLabelPlan>>,
    /// DOM ownership produced by the same recursive Dagre preparation that owns layout.
    ///
    /// Compatibility layout JSON remains flat, while SVG rendering consumes this typed tree so it
    /// never has to reconstruct extracted namespace roots from the semantic model.
    #[serde(skip)]
    pub render_tree: ClassRenderTree,
}

#[derive(Debug, Clone, Default)]
pub struct ClassRenderRoot {
    pub namespace_id: Option<String>,
    pub cluster_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub items: Vec<ClassRenderItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassRenderRootId(pub usize);

#[derive(Debug, Clone)]
pub struct ClassRenderTree {
    pub roots: Vec<ClassRenderRoot>,
    pub top: ClassRenderRootId,
}

impl Default for ClassRenderTree {
    fn default() -> Self {
        Self {
            roots: vec![ClassRenderRoot::default()],
            top: ClassRenderRootId(0),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClassRenderItem {
    Node(String),
    Subgraph(ClassRenderRootId),
}
