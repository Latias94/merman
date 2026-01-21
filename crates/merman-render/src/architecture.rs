use crate::model::{ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutNode, LayoutPoint};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureNodeModel {
    id: String,
    #[serde(rename = "type")]
    node_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ArchitectureEdgeModel {
    lhs: String,
    rhs: String,
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
        pts.push((n.x - n.width / 2.0, n.y - n.height / 2.0));
        pts.push((n.x + n.width / 2.0, n.y + n.height / 2.0));
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
    _effective_config: &Value,
    _text_measurer: &dyn TextMeasurer,
) -> Result<ArchitectureDiagramLayout> {
    let model: ArchitectureModel = serde_json::from_value(model.clone())?;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    for (idx, n) in model.nodes.iter().enumerate() {
        let (width, height) = match n.node_type.as_str() {
            "service" => (85.0, 80.0),
            "junction" => (20.0, 20.0),
            "group" => (120.0, 80.0),
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
        let Some(a) = node_by_id.get(&e.lhs) else {
            return Err(Error::InvalidModel {
                message: format!("edge lhs node not found: {}", e.lhs),
            });
        };
        let Some(b) = node_by_id.get(&e.rhs) else {
            return Err(Error::InvalidModel {
                message: format!("edge rhs node not found: {}", e.rhs),
            });
        };
        edges.push(LayoutEdge {
            id: format!("edge-{idx}"),
            from: e.lhs.clone(),
            to: e.rhs.clone(),
            from_cluster: None,
            to_cluster: None,
            points: vec![
                LayoutPoint { x: a.x, y: a.y },
                LayoutPoint { x: b.x, y: b.y },
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
