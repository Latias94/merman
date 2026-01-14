use merman_core::ParseMetadata;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutMeta {
    pub diagram_type: String,
    pub title: Option<String>,
    pub config: Value,
    pub effective_config: Value,
}

impl LayoutMeta {
    pub fn from_parse_metadata(meta: &ParseMetadata) -> Self {
        Self {
            diagram_type: meta.diagram_type.clone(),
            title: meta.title.clone(),
            config: meta.config.as_value().clone(),
            effective_config: meta.effective_config.as_value().clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Option<Self> {
        let mut it = points.into_iter();
        let (x0, y0) = it.next()?;
        let mut b = Self {
            min_x: x0,
            min_y: y0,
            max_x: x0,
            max_y: y0,
        };
        for (x, y) in it {
            b.min_x = b.min_x.min(x);
            b.min_y = b.min_y.min(y);
            b.max_x = b.max_x.max(x);
            b.max_y = b.max_y.max(y);
        }
        Some(b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutLabel {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub is_cluster: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCluster {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Mermaid cluster "diff" value used during cluster positioning.
    pub diff: f64,
    /// Mermaid cluster "offsetY" value: title bbox height minus half padding.
    pub offset_y: f64,
    pub title: String,
    pub title_label: LayoutLabel,
    pub requested_dir: Option<String>,
    pub effective_dir: String,
    pub padding: f64,
    pub title_margin_top: f64,
    pub title_margin_bottom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub from_cluster: Option<String>,
    #[serde(default)]
    pub to_cluster: Option<String>,
    pub points: Vec<LayoutPoint>,
    pub label: Option<LayoutLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowchartV2Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDiagramV2Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutDiagram {
    FlowchartV2(FlowchartV2Layout),
    StateDiagramV2(StateDiagramV2Layout),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutedDiagram {
    pub meta: LayoutMeta,
    pub semantic: Value,
    pub layout: LayoutDiagram,
}
