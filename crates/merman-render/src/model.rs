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
    #[serde(default)]
    pub start_label_left: Option<LayoutLabel>,
    #[serde(default)]
    pub start_label_right: Option<LayoutLabel>,
    #[serde(default)]
    pub end_label_left: Option<LayoutLabel>,
    #[serde(default)]
    pub end_label_right: Option<LayoutLabel>,
    /// Optional SVG marker id for marker-start (e.g. ER cardinality markers).
    #[serde(default)]
    pub start_marker: Option<String>,
    /// Optional SVG marker id for marker-end (e.g. ER cardinality markers).
    #[serde(default)]
    pub end_marker: Option<String>,
    /// Optional SVG dash pattern (e.g. identifying vs non-identifying ER relationships).
    #[serde(default)]
    pub stroke_dasharray: Option<String>,
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
pub struct ClassDiagramV2Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoDiagramLayout {
    pub bounds: Option<Bounds>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketBlockLayout {
    pub start: i64,
    pub end: i64,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketWordLayout {
    pub blocks: Vec<PacketBlockLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketDiagramLayout {
    pub bounds: Option<Bounds>,
    pub width: f64,
    pub height: f64,
    pub row_height: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub bit_width: f64,
    pub bits_per_row: i64,
    pub show_bits: bool,
    pub words: Vec<PacketWordLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieSliceLayout {
    pub label: String,
    pub value: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub is_full_circle: bool,
    pub percent: i64,
    pub text_x: f64,
    pub text_y: f64,
    pub fill: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieLegendItemLayout {
    pub label: String,
    pub value: f64,
    pub fill: String,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieDiagramLayout {
    pub bounds: Option<Bounds>,
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub outer_radius: f64,
    pub legend_x: f64,
    pub legend_start_y: f64,
    pub legend_step_y: f64,
    pub slices: Vec<PieSliceLayout>,
    pub legend_items: Vec<PieLegendItemLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutDiagram {
    FlowchartV2(FlowchartV2Layout),
    StateDiagramV2(StateDiagramV2Layout),
    ClassDiagramV2(ClassDiagramV2Layout),
    ErDiagram(ErDiagramLayout),
    SequenceDiagram(SequenceDiagramLayout),
    InfoDiagram(InfoDiagramLayout),
    PacketDiagram(PacketDiagramLayout),
    PieDiagram(PieDiagramLayout),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutedDiagram {
    pub meta: LayoutMeta,
    pub semantic: Value,
    pub layout: LayoutDiagram,
}
