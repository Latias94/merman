use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, MindmapDiagramLayout};
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MindmapNodeModel {
    id: String,
    label: String,
    level: i64,
    padding: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MindmapEdgeModel {
    id: String,
    start: String,
    end: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MindmapModel {
    #[serde(default)]
    nodes: Vec<MindmapNodeModel>,
    #[serde(default)]
    edges: Vec<MindmapEdgeModel>,
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        let x0 = n.x - n.width / 2.0;
        let y0 = n.y - n.height / 2.0;
        let x1 = n.x + n.width / 2.0;
        let y1 = n.y + n.height / 2.0;
        pts.push((x0, y0));
        pts.push((x1, y1));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
    }
    Bounds::from_points(pts)
}

pub fn layout_mindmap_diagram(
    model: &Value,
    _effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
) -> Result<MindmapDiagramLayout> {
    let model: MindmapModel = serde_json::from_value(model.clone())?;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    let mut id_order: Vec<(i64, String)> = model
        .nodes
        .iter()
        .map(|n| (n.id.parse::<i64>().unwrap_or(i64::MAX), n.id.clone()))
        .collect();
    id_order.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let node_by_id: std::collections::BTreeMap<String, MindmapNodeModel> =
        model.nodes.into_iter().map(|n| (n.id.clone(), n)).collect();

    let style = TextStyle {
        font_family: None,
        font_size: 16.0,
        font_weight: None,
    };

    for (idx, (_num, id)) in id_order.iter().enumerate() {
        let Some(n) = node_by_id.get(id) else {
            continue;
        };
        let depth = (n.level / 2).max(0) as f64;
        let m = text_measurer.measure(&n.label, &style);

        let width = (m.width + 2.0 * n.padding).max(1.0);
        let height = (m.height + n.padding).max(1.0);

        nodes.push(LayoutNode {
            id: n.id.clone(),
            x: depth * 240.0,
            y: idx as f64 * 80.0,
            width,
            height,
            is_cluster: false,
        });
    }

    let mut node_pos: std::collections::BTreeMap<String, (f64, f64)> =
        std::collections::BTreeMap::new();
    for n in &nodes {
        node_pos.insert(n.id.clone(), (n.x, n.y));
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for e in model.edges {
        let Some((sx, sy)) = node_pos.get(&e.start).copied() else {
            return Err(Error::InvalidModel {
                message: format!("edge start node not found: {}", e.start),
            });
        };
        let Some((tx, ty)) = node_pos.get(&e.end).copied() else {
            return Err(Error::InvalidModel {
                message: format!("edge end node not found: {}", e.end),
            });
        };
        let points = vec![LayoutPoint { x: sx, y: sy }, LayoutPoint { x: tx, y: ty }];
        edges.push(LayoutEdge {
            id: e.id,
            from: e.start,
            to: e.end,
            from_cluster: None,
            to_cluster: None,
            points,
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

    let bounds = compute_bounds(&nodes, &edges);
    Ok(MindmapDiagramLayout {
        nodes,
        edges,
        bounds,
    })
}
