//! Shared graph layout projections for renderer families.

use crate::{Bounds, LayoutPoint};
use serde::{Deserialize, Serialize};

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
    #[serde(skip)]
    pub label_width: Option<f64>,
    #[serde(skip)]
    pub label_height: Option<f64>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwimlaneDirection {
    #[serde(rename = "TB")]
    Tb,
    #[serde(rename = "LR")]
    Lr,
    #[serde(rename = "BT")]
    Bt,
    #[serde(rename = "RL")]
    Rl,
}

impl SwimlaneDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tb => "TB",
            Self::Lr => "LR",
            Self::Bt => "BT",
            Self::Rl => "RL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwimlaneTitleRect {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwimlaneLaneLayout {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    /// Prepared terminal label width shared by Swimlane layout and SVG rendering.
    pub title_label_width: f64,
    /// Prepared terminal label height shared by Swimlane layout and SVG rendering.
    pub title_label_height: f64,
    #[serde(default)]
    pub content_top: Option<f64>,
    #[serde(default)]
    pub title_rect: Option<SwimlaneTitleRect>,
    #[serde(default)]
    pub requested_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwimlaneNodeLayout {
    pub id: String,
    pub label: String,
    pub label_type: String,
    pub shape: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub top_lane_id: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label_width: f64,
    pub label_height: f64,
    pub layer: usize,
    pub order: usize,
    pub is_edge_label: bool,
}

/// Bound measured nodes and edge paths in traversal order, charging each retained part.
pub fn measure_layout_bounds<E>(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    mut charge: impl FnMut(usize) -> std::result::Result<(), E>,
) -> std::result::Result<Option<Bounds>, E> {
    fn include(bounds: &mut Option<Bounds>, x: f64, y: f64) {
        let Some(bounds) = bounds.as_mut() else {
            *bounds = Some(Bounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            });
            return;
        };
        bounds.min_x = bounds.min_x.min(x);
        bounds.min_y = bounds.min_y.min(y);
        bounds.max_x = bounds.max_x.max(x);
        bounds.max_y = bounds.max_y.max(y);
    }

    let mut bounds = None;
    for n in nodes {
        charge(2)?;
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        include(&mut bounds, n.x - hw, n.y - hh);
        include(&mut bounds, n.x + hw, n.y + hh);
    }
    for e in edges {
        charge(1)?;
        charge(e.points.len())?;
        for p in &e.points {
            include(&mut bounds, p.x, p.y);
        }
        if let Some(l) = &e.label {
            charge(2)?;
            let hw = l.width / 2.0;
            let hh = l.height / 2.0;
            include(&mut bounds, l.x - hw, l.y - hh);
            include(&mut bounds, l.x + hw, l.y + hh);
        }
    }
    Ok(bounds)
}
