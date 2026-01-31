use crate::model::{ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutNode, LayoutPoint};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64().or_else(|| cur.as_i64().map(|v| v as f64))
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureNodeModel {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureEdgeModel {
    #[serde(rename = "lhsId", alias = "lhs")]
    lhs_id: String,
    #[serde(rename = "rhsId", alias = "rhs")]
    rhs_id: String,
    #[serde(default, rename = "lhsDir")]
    lhs_dir: Option<String>,
    #[serde(default, rename = "rhsDir")]
    rhs_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureModel {
    #[serde(default)]
    nodes: Vec<ArchitectureNodeModel>,
    #[serde(default)]
    edges: Vec<ArchitectureEdgeModel>,
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        // Architecture renderer uses top-left anchored `translate(x, y)` for nodes.
        pts.push((n.x, n.y));
        pts.push((n.x + n.width, n.y + n.height));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
    }
    Bounds::from_points(pts)
}

pub fn layout_architecture_diagram(
    model: &Value,
    effective_config: &Value,
    _text_measurer: &dyn TextMeasurer,
) -> Result<ArchitectureDiagramLayout> {
    let model: ArchitectureModel = serde_json::from_value(model.clone())?;

    let icon_size = config_f64(effective_config, &["architecture", "iconSize"]).unwrap_or(80.0);
    let icon_size = icon_size.max(1.0);
    let half_icon = icon_size / 2.0;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    for (idx, n) in model.nodes.iter().enumerate() {
        let (width, height) = match n.node_type.as_str() {
            "service" => (icon_size, icon_size),
            "junction" => (icon_size, icon_size),
            other => {
                return Err(Error::InvalidModel {
                    message: format!("unsupported architecture node type: {other}"),
                });
            }
        };

        nodes.push(LayoutNode {
            id: n.id.clone(),
            x: 0.0,
            y: idx as f64 * (height + 40.0),
            width,
            height,
            is_cluster: false,
        });
    }

    let mut node_by_id: std::collections::BTreeMap<String, LayoutNode> =
        std::collections::BTreeMap::new();
    for n in &nodes {
        node_by_id.insert(n.id.clone(), n.clone());
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for (idx, e) in model.edges.iter().enumerate() {
        let Some(a) = node_by_id.get(&e.lhs_id) else {
            return Err(Error::InvalidModel {
                message: format!("edge lhs node not found: {}", e.lhs_id),
            });
        };
        let Some(b) = node_by_id.get(&e.rhs_id) else {
            return Err(Error::InvalidModel {
                message: format!("edge rhs node not found: {}", e.rhs_id),
            });
        };

        fn endpoint(
            x: f64,
            y: f64,
            dir: Option<&str>,
            icon_size: f64,
            half_icon: f64,
        ) -> (f64, f64) {
            match dir.unwrap_or("") {
                "L" => (x, y + half_icon),
                "R" => (x + icon_size, y + half_icon),
                "T" => (x + half_icon, y),
                "B" => (x + half_icon, y + icon_size),
                _ => (x + half_icon, y + half_icon),
            }
        }

        let (sx, sy) = endpoint(a.x, a.y, e.lhs_dir.as_deref(), icon_size, half_icon);
        let (tx, ty) = endpoint(b.x, b.y, e.rhs_dir.as_deref(), icon_size, half_icon);
        let mid = LayoutPoint {
            x: (sx + tx) / 2.0,
            y: (sy + ty) / 2.0,
        };
        edges.push(LayoutEdge {
            id: format!("edge-{idx}"),
            from: e.lhs_id.clone(),
            to: e.rhs_id.clone(),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint { x: sx, y: sy },
                mid,
                LayoutPoint { x: tx, y: ty },
            ],
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }

    Ok(ArchitectureDiagramLayout {
        bounds: compute_bounds(&nodes, &edges),
        nodes,
        edges,
    })
}
