use crate::model::{
    Bounds, FlowchartV2Layout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
struct FlowchartV2Model {
    pub direction: String,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default)]
    pub subgraphs: Vec<FlowSubgraph>,
}

#[derive(Debug, Clone, Deserialize)]
struct FlowNode {
    pub id: String,
    pub label: Option<String>,
    #[serde(rename = "layoutShape")]
    pub layout_shape: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct FlowSubgraph {
    pub id: String,
    pub title: String,
    pub nodes: Vec<String>,
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn rank_dir_from_flow(direction: &str) -> RankDir {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

pub fn layout_flowchart_v2(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<FlowchartV2Layout> {
    let model: FlowchartV2Model = serde_json::from_value(semantic.clone())?;

    let nodesep = config_f64(effective_config, &["flowchart", "nodeSpacing"]).unwrap_or(50.0);
    let ranksep = config_f64(effective_config, &["flowchart", "rankSpacing"]).unwrap_or(50.0);
    let node_padding = config_f64(effective_config, &["flowchart", "padding"]).unwrap_or(15.0);
    let cluster_padding = 8.0;
    let title_margin_top = config_f64(
        effective_config,
        &["flowchart", "subGraphTitleMargin", "top"],
    )
    .unwrap_or(0.0);
    let title_margin_bottom = config_f64(
        effective_config,
        &["flowchart", "subGraphTitleMargin", "bottom"],
    )
    .unwrap_or(0.0);
    let title_total_margin = title_margin_top + title_margin_bottom;

    let font_family = config_string(effective_config, &["fontFamily"]);
    let font_size = config_f64(effective_config, &["fontSize"]).unwrap_or(16.0);
    let font_weight = config_string(effective_config, &["fontWeight"]);
    let text_style = TextStyle {
        font_family,
        font_size,
        font_weight,
    };

    let compound = !model.subgraphs.is_empty();
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from_flow(&model.direction),
        nodesep,
        ranksep,
        edgesep: 10.0,
        ..Default::default()
    });

    for n in &model.nodes {
        let label = n.label.as_deref().unwrap_or(&n.id);
        let metrics = measurer.measure(label, &text_style);
        let (width, height) = node_dimensions(n.layout_shape.as_deref(), metrics, node_padding);
        g.set_node(
            n.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    for sg in &model.subgraphs {
        let metrics = measurer.measure(&sg.title, &text_style);
        let width = metrics.width + cluster_padding * 2.0;
        let height = metrics.height + cluster_padding * 2.0;
        g.set_node(
            sg.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    if compound {
        for sg in &model.subgraphs {
            for child in &sg.nodes {
                g.set_parent(child.clone(), sg.id.clone());
            }
        }
    }

    for e in &model.edges {
        let (lw, lh) = if let Some(label) = e.label.as_deref() {
            let metrics = measurer.measure(label, &text_style);
            (metrics.width + node_padding, metrics.height + node_padding)
        } else {
            (0.0, 0.0)
        };
        let el = EdgeLabel {
            width: lw,
            height: lh,
            labelpos: LabelPos::C,
            minlen: e.length.max(1),
            weight: 1.0,
            ..Default::default()
        };
        g.set_edge_named(e.from.clone(), e.to.clone(), Some(e.id.clone()), Some(el));
    }

    dugong::layout(&mut g);

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    for n in &model.nodes {
        let Some(label) = g.node(&n.id) else {
            return Err(Error::InvalidModel {
                message: format!("missing layout node {}", n.id),
            });
        };
        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x: label.x.unwrap_or(0.0),
            y: label.y.unwrap_or(0.0),
            width: label.width,
            height: label.height,
            is_cluster: false,
        });
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();
    for sg in &model.subgraphs {
        let Some(node) = g.node(&sg.id) else {
            return Err(Error::InvalidModel {
                message: format!("missing layout subgraph node {}", sg.id),
            });
        };

        let cx = node.x.unwrap_or(0.0);
        let cy = node.y.unwrap_or(0.0);

        let title_metrics = measurer.measure(&sg.title, &text_style);
        let base_width = node.width;
        let base_height = node.height;

        // Mermaid cluster rendering ensures the box is large enough to fit the title and then
        // applies subgraph title margins by extending the box height.
        let width = base_width.max(title_metrics.width + cluster_padding);
        let height = base_height + title_total_margin;

        let title_label = LayoutLabel {
            x: cx,
            y: cy - height / 2.0 + title_margin_top + title_metrics.height / 2.0,
            width: title_metrics.width,
            height: title_metrics.height,
        };

        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width,
            height,
            title: sg.title.clone(),
            title_label,
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });

        out_nodes.push(LayoutNode {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width,
            height,
            is_cluster: true,
        });
    }
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for e in &model.edges {
        let Some(label) = g.edge(&e.from, &e.to, Some(&e.id)) else {
            return Err(Error::InvalidModel {
                message: format!("missing layout edge {}", e.id),
            });
        };
        let points = label
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();
        let label_pos = match (label.x, label.y) {
            (Some(x), Some(y)) if label.width > 0.0 || label.height > 0.0 => Some(LayoutLabel {
                x,
                y,
                width: label.width,
                height: label.height,
            }),
            _ => None,
        };
        out_edges.push(LayoutEdge {
            id: e.id.clone(),
            from: e.from.clone(),
            to: e.to.clone(),
            points,
            label: label_pos,
        });
    }

    let bounds = compute_bounds(&out_nodes, &out_edges);

    Ok(FlowchartV2Layout {
        nodes: out_nodes,
        edges: out_edges,
        clusters,
        bounds,
    })
}

fn node_dimensions(
    layout_shape: Option<&str>,
    metrics: crate::text::TextMetrics,
    padding: f64,
) -> (f64, f64) {
    let mut width = metrics.width + padding * 2.0;
    let mut height = metrics.height + padding * 2.0;

    if let Some(shape) = layout_shape {
        if shape == "diamond" {
            width *= 1.2;
            height *= 1.2;
        }
    }

    (width.max(1.0), height.max(1.0))
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        let hw = n.width / 2.0;
        let hh = n.height / 2.0;
        pts.push((n.x - hw, n.y - hh));
        pts.push((n.x + hw, n.y + hh));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
        if let Some(l) = &e.label {
            let hw = l.width / 2.0;
            let hh = l.height / 2.0;
            pts.push((l.x - hw, l.y - hh));
            pts.push((l.x + hw, l.y + hh));
        }
    }
    Bounds::from_points(pts)
}
