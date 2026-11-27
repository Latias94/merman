use merman_render_core::{Bounds, LayoutEdge, LayoutNode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    /// Local Cytoscape child contribution phases for Architecture services.
    ///
    /// These bounds are exposed for source-backed group/content audits. SVG renderers should use
    /// their own phase-specific logic instead of treating this as a generic root-bounds source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cytoscape_service_bounds: Vec<ArchitectureCytoscapeServiceBounds>,
    /// Final FCoSE compound rectangles exposed for source-backed Architecture audits.
    ///
    /// Renderers should not treat these as Mermaid SVG group rects without a phase-specific
    /// source audit; they are layout-engine rects, not browser `node.boundingBox()` values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fcose_compound_bounds: Vec<ArchitectureCompoundBounds>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureCompoundBounds {
    pub id: String,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureCytoscapeServiceBounds {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_group: Option<String>,
    pub body_bounds: Bounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_bounds: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_metrics: Option<ArchitectureCytoscapeServiceLabelMetrics>,
    pub union_bounds: Bounds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureCytoscapeServiceLabelMetrics {
    pub text_width: f64,
    pub half_width: f64,
}
