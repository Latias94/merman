use crate::model::{
    Bounds, FlowchartV2Layout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowchartV2Model {
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "classDefs")]
    pub class_defs: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default, rename = "edgeDefaults")]
    pub edge_defaults: Option<FlowEdgeDefaults>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default)]
    pub subgraphs: Vec<FlowSubgraph>,
    #[serde(default)]
    pub tooltips: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowEdgeDefaults {
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowNode {
    pub id: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(rename = "layoutShape")]
    pub layout_shape: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default, rename = "linkTarget")]
    pub link_target: Option<String>,
    #[serde(default, rename = "haveCallback")]
    pub have_callback: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default, rename = "type")]
    pub edge_type: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub style: Vec<String>,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FlowSubgraph {
    pub id: String,
    pub title: String,
    pub dir: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
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

fn edge_label_is_non_empty(edge: &FlowEdge) -> bool {
    edge.label
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn edge_label_node_id(edge: &FlowEdge) -> String {
    format!("edge-label-{}-{}-{}", edge.from, edge.to, edge.id)
}

fn edge_to_label_name(edge: &FlowEdge) -> String {
    format!("{}-to-label", edge.id)
}

fn edge_from_label_name(edge: &FlowEdge) -> String {
    format!("{}-from-label", edge.id)
}

fn lowest_common_parent(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    a: &str,
    b: &str,
) -> Option<String> {
    if !g.options().compound {
        return None;
    }

    let mut ancestors: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut cur = g.parent(a);
    while let Some(p) = cur {
        ancestors.insert(p.to_string());
        cur = g.parent(p);
    }

    let mut cur = g.parent(b);
    while let Some(p) = cur {
        if ancestors.contains(p) {
            return Some(p.to_string());
        }
        cur = g.parent(p);
    }

    None
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
    let wrapping_width =
        config_f64(effective_config, &["flowchart", "wrappingWidth"]).unwrap_or(200.0);
    // Mermaid subgraph labels are rendered via `createText(...)` without an explicit `width`,
    // which defaults to 200.
    let cluster_title_wrapping_width = 200.0;
    let html_labels = effective_config
        .get("flowchart")
        .and_then(|v| v.get("htmlLabels"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let wrap_mode = if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };
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
    let y_shift = title_total_margin / 2.0;
    let inherit_dir = effective_config
        .get("flowchart")
        .and_then(|v| v.get("inheritDir"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let font_family = config_string(effective_config, &["fontFamily"]);
    let font_size = config_f64(effective_config, &["fontSize"]).unwrap_or(16.0);
    let font_weight = config_string(effective_config, &["fontWeight"]);
    let text_style = TextStyle {
        font_family,
        font_size,
        font_weight,
    };

    let diagram_direction = model.direction.as_deref().unwrap_or("TB");
    let compound = !model.subgraphs.is_empty();
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        compound,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from_flow(diagram_direction),
        nodesep,
        ranksep,
        // Dagre layout defaults `edgesep` to 20 when unspecified.
        edgesep: 20.0,
        ..Default::default()
    });

    for n in &model.nodes {
        let label = n.label.as_deref().unwrap_or(&n.id);
        let metrics = measurer.measure_wrapped(label, &text_style, Some(wrapping_width), wrap_mode);
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
        let metrics = measurer.measure_wrapped(
            &sg.title,
            &text_style,
            Some(cluster_title_wrapping_width),
            wrap_mode,
        );
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

    let mut edge_label_nodes: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut extra_children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for e in &model.edges {
        if edge_label_is_non_empty(e) {
            // Mermaid's modern Dagre pipeline converts labeled edges into a label node and splits
            // the original edge into two edges to ensure the label participates in layout.
            let label_node_id = edge_label_node_id(e);
            edge_label_nodes.insert(e.id.clone(), label_node_id.clone());

            let label_text = e.label.as_deref().unwrap_or_default();
            let metrics =
                measurer.measure_wrapped(label_text, &text_style, Some(wrapping_width), wrap_mode);
            // Mermaid renders edge labels using the `labelRect` shape and measures the overall
            // SVG group bounding box; the label node should match the text box size (no extra
            // node padding).
            let label_width = metrics.width.max(1.0);
            let label_height = metrics.height.max(1.0);
            g.set_node(
                label_node_id.clone(),
                NodeLabel {
                    width: label_width,
                    height: label_height,
                    ..Default::default()
                },
            );

            if let Some(parent) = lowest_common_parent(&g, &e.from, &e.to) {
                g.set_parent(label_node_id.clone(), parent.clone());
                extra_children
                    .entry(parent)
                    .or_default()
                    .push(label_node_id.clone());
            }

            let minlen = e.length.max(1);
            let el = EdgeLabel {
                // Edge label is represented by the label node, so edges themselves carry no label size.
                width: 0.0,
                height: 0.0,
                labelpos: LabelPos::C,
                labeloffset: 10.0,
                minlen,
                weight: 1.0,
                ..Default::default()
            };

            g.set_edge_named(
                e.from.clone(),
                label_node_id.clone(),
                Some(edge_to_label_name(e)),
                Some(el.clone()),
            );
            g.set_edge_named(
                label_node_id,
                e.to.clone(),
                Some(edge_from_label_name(e)),
                Some(el),
            );
        } else {
            let el = EdgeLabel {
                width: 0.0,
                height: 0.0,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen: e.length.max(1),
                weight: 1.0,
                ..Default::default()
            };
            g.set_edge_named(e.from.clone(), e.to.clone(), Some(e.id.clone()), Some(el));
        }
    }

    dugong::layout(&mut g);

    #[derive(Debug, Clone, Copy)]
    struct Rect {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl Rect {
        fn from_center(x: f64, y: f64, width: f64, height: f64) -> Self {
            let hw = width / 2.0;
            let hh = height / 2.0;
            Self {
                min_x: x - hw,
                min_y: y - hh,
                max_x: x + hw,
                max_y: y + hh,
            }
        }

        fn width(&self) -> f64 {
            self.max_x - self.min_x
        }

        fn height(&self) -> f64 {
            self.max_y - self.min_y
        }

        fn center(&self) -> (f64, f64) {
            (
                (self.min_x + self.max_x) / 2.0,
                (self.min_y + self.max_y) / 2.0,
            )
        }

        fn union(&mut self, other: Rect) {
            self.min_x = self.min_x.min(other.min_x);
            self.min_y = self.min_y.min(other.min_y);
            self.max_x = self.max_x.max(other.max_x);
            self.max_y = self.max_y.max(other.max_y);
        }

        fn translate(&mut self, dx: f64, dy: f64) {
            self.min_x += dx;
            self.max_x += dx;
            self.min_y += dy;
            self.max_y += dy;
        }
    }

    let mut leaf_rects: std::collections::HashMap<String, Rect> = std::collections::HashMap::new();
    let mut base_pos: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    let mut edge_override_points: std::collections::HashMap<String, Vec<LayoutPoint>> =
        std::collections::HashMap::new();
    let mut edge_override_label: std::collections::HashMap<String, Option<LayoutLabel>> =
        std::collections::HashMap::new();
    let mut edge_packed_shift: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();

    for n in &model.nodes {
        let Some(label) = g.node(&n.id) else {
            return Err(Error::InvalidModel {
                message: format!("missing layout node {}", n.id),
            });
        };
        let x = label.x.unwrap_or(0.0);
        let y = label.y.unwrap_or(0.0);
        base_pos.insert(n.id.clone(), (x, y));
        leaf_rects.insert(
            n.id.clone(),
            Rect::from_center(x, y, label.width, label.height),
        );
    }

    for label_node_id in edge_label_nodes.values() {
        let Some(n) = g.node(label_node_id) else {
            continue;
        };
        let x = n.x.unwrap_or(0.0);
        let y = n.y.unwrap_or(0.0);
        base_pos.insert(label_node_id.clone(), (x, y));
        leaf_rects.insert(
            label_node_id.clone(),
            Rect::from_center(x, y, n.width, n.height),
        );
    }

    let mut subgraphs_by_id: std::collections::HashMap<String, FlowSubgraph> =
        std::collections::HashMap::new();
    for sg in &model.subgraphs {
        subgraphs_by_id.insert(sg.id.clone(), sg.clone());
    }

    fn normalize_dir(s: &str) -> String {
        s.trim().to_uppercase()
    }

    fn toggled_dir(parent: &str) -> String {
        let parent = normalize_dir(parent);
        if parent == "TB" || parent == "TD" {
            "LR".to_string()
        } else {
            "TB".to_string()
        }
    }

    fn effective_cluster_dir(sg: &FlowSubgraph, diagram_dir: &str, inherit_dir: bool) -> String {
        if let Some(dir) = sg.dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            return normalize_dir(dir);
        }
        if inherit_dir {
            return normalize_dir(diagram_dir);
        }
        toggled_dir(diagram_dir)
    }

    fn dir_to_rankdir(dir: &str) -> RankDir {
        match normalize_dir(dir).as_str() {
            "TB" | "TD" => RankDir::TB,
            "BT" => RankDir::BT,
            "LR" => RankDir::LR,
            "RL" => RankDir::RL,
            _ => RankDir::TB,
        }
    }

    // Apply Mermaid-like "recursive cluster" layout for clusters with:
    // - leaf-only membership (no nested subgraph ids),
    // - no external connections (all edges are internal to the cluster membership).
    //
    // This captures the key behavior where a cluster's `dir` influences internal layout.
    {
        let subgraph_ids: std::collections::HashSet<String> =
            subgraphs_by_id.keys().cloned().collect();

        for sg in model.subgraphs.iter() {
            let has_nested = sg.nodes.iter().any(|m| subgraph_ids.contains(m));
            if has_nested {
                continue;
            }

            let members: std::collections::HashSet<&str> =
                sg.nodes.iter().map(|s| s.as_str()).collect();
            if members.is_empty() {
                continue;
            }

            let mut has_external = false;
            let mut internal_edges: Vec<&FlowEdge> = Vec::new();
            for e in &model.edges {
                let in_from = members.contains(e.from.as_str());
                let in_to = members.contains(e.to.as_str());
                match (in_from, in_to) {
                    (true, true) => internal_edges.push(e),
                    (true, false) | (false, true) => {
                        has_external = true;
                        break;
                    }
                    (false, false) => {}
                }
            }
            if has_external {
                continue;
            }

            let dir = effective_cluster_dir(sg, diagram_direction, inherit_dir);
            let label_nodes_for_edges = internal_edges
                .iter()
                .filter(|e| edge_label_is_non_empty(e))
                .map(|e| edge_label_node_id(e))
                .collect::<Vec<_>>();

            let mut g_inner: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
                multigraph: true,
                compound: false,
                directed: true,
            });
            g_inner.set_graph(GraphLabel {
                rankdir: dir_to_rankdir(&dir),
                nodesep: 50.0,
                ranksep: 50.0,
                edgesep: 20.0,
                ..Default::default()
            });

            for node_id in &sg.nodes {
                let Some(orig) = g.node(node_id) else {
                    continue;
                };
                g_inner.set_node(
                    node_id.clone(),
                    NodeLabel {
                        width: orig.width,
                        height: orig.height,
                        ..Default::default()
                    },
                );
            }

            for e in &internal_edges {
                if edge_label_is_non_empty(e) {
                    let label_node_id = edge_label_node_id(e);

                    let label_text = e.label.as_deref().unwrap_or_default();
                    let metrics = measurer.measure_wrapped(
                        label_text,
                        &text_style,
                        Some(wrapping_width),
                        wrap_mode,
                    );
                    let label_width = metrics.width.max(1.0);
                    let label_height = metrics.height.max(1.0);
                    g_inner.set_node(
                        label_node_id.clone(),
                        NodeLabel {
                            width: label_width,
                            height: label_height,
                            ..Default::default()
                        },
                    );

                    let minlen = e.length.max(1);
                    let el = EdgeLabel {
                        width: 0.0,
                        height: 0.0,
                        labelpos: LabelPos::C,
                        labeloffset: 10.0,
                        minlen,
                        weight: 1.0,
                        ..Default::default()
                    };

                    g_inner.set_edge_named(
                        e.from.clone(),
                        label_node_id.clone(),
                        Some(edge_to_label_name(e)),
                        Some(el.clone()),
                    );
                    g_inner.set_edge_named(
                        label_node_id,
                        e.to.clone(),
                        Some(edge_from_label_name(e)),
                        Some(el),
                    );
                } else {
                    let el = EdgeLabel {
                        width: 0.0,
                        height: 0.0,
                        labelpos: LabelPos::C,
                        labeloffset: 10.0,
                        minlen: e.length.max(1),
                        weight: 1.0,
                        ..Default::default()
                    };
                    g_inner.set_edge_named(
                        e.from.clone(),
                        e.to.clone(),
                        Some(e.id.clone()),
                        Some(el),
                    );
                }
            }

            dugong::layout(&mut g_inner);

            // Translate the inner layout back into the diagram coordinate space by matching the
            // original membership bounding box center.
            let mut old_rect: Option<Rect> = None;
            for node_id in sg.nodes.iter().chain(label_nodes_for_edges.iter()) {
                let Some((x, y)) = base_pos.get(node_id) else {
                    continue;
                };
                let Some(orig) = g.node(node_id) else {
                    continue;
                };
                let r = Rect::from_center(*x, *y, orig.width, orig.height);
                if let Some(ref mut cur) = old_rect {
                    cur.union(r);
                } else {
                    old_rect = Some(r);
                }
            }
            let Some(old_rect) = old_rect else { continue };

            let mut new_rect: Option<Rect> = None;
            for node_id in sg.nodes.iter().chain(label_nodes_for_edges.iter()) {
                let Some(inner) = g_inner.node(node_id) else {
                    continue;
                };
                let x = inner.x.unwrap_or(0.0);
                let y = inner.y.unwrap_or(0.0);
                let r = Rect::from_center(x, y, inner.width, inner.height);
                if let Some(ref mut cur) = new_rect {
                    cur.union(r);
                } else {
                    new_rect = Some(r);
                }
            }
            let Some(new_rect) = new_rect else { continue };

            let (ocx, ocy) = old_rect.center();
            let (ncx, ncy) = new_rect.center();
            let dx = ocx - ncx;
            let dy = ocy - ncy;

            // Apply translated positions to base_pos and leaf_rects.
            for node_id in sg.nodes.iter().chain(label_nodes_for_edges.iter()) {
                let Some(inner) = g_inner.node(node_id) else {
                    continue;
                };
                let x = inner.x.unwrap_or(0.0) + dx;
                let y = inner.y.unwrap_or(0.0) + dy;
                base_pos.insert(node_id.clone(), (x, y));
                leaf_rects.insert(
                    node_id.clone(),
                    Rect::from_center(x, y, inner.width, inner.height),
                );
            }

            // Capture edge point overrides for internal edges.
            for e in &internal_edges {
                let (points, label_pos) = if edge_label_is_non_empty(e) {
                    let label_node_id = edge_label_node_id(e);
                    let to_label_name = edge_to_label_name(e);
                    let from_label_name = edge_from_label_name(e);

                    let Some(to_label) =
                        g_inner.edge(&e.from, &label_node_id, Some(to_label_name.as_str()))
                    else {
                        continue;
                    };
                    let Some(from_label) =
                        g_inner.edge(&label_node_id, &e.to, Some(from_label_name.as_str()))
                    else {
                        continue;
                    };

                    let mut pts = Vec::new();
                    for p in &to_label.points {
                        pts.push(LayoutPoint {
                            x: p.x + dx,
                            y: p.y + dy + y_shift,
                        });
                    }

                    let mut skip_first = false;
                    if let (Some(last), Some(first)) = (pts.last(), from_label.points.first()) {
                        let fx = first.x + dx;
                        let fy = first.y + dy + y_shift;
                        if (last.x - fx).abs() <= 1e-6 && (last.y - fy).abs() <= 1e-6 {
                            skip_first = true;
                        }
                    }

                    for (idx, p) in from_label.points.iter().enumerate() {
                        if skip_first && idx == 0 {
                            continue;
                        }
                        pts.push(LayoutPoint {
                            x: p.x + dx,
                            y: p.y + dy + y_shift,
                        });
                    }

                    let label_pos = g_inner.node(&label_node_id).and_then(|n| {
                        Some(LayoutLabel {
                            x: n.x.unwrap_or(0.0) + dx,
                            y: n.y.unwrap_or(0.0) + dy + y_shift,
                            width: n.width,
                            height: n.height,
                        })
                    });

                    (pts, label_pos)
                } else {
                    let Some(lbl) = g_inner.edge(&e.from, &e.to, Some(&e.id)) else {
                        continue;
                    };
                    let points = lbl
                        .points
                        .iter()
                        .map(|p| LayoutPoint {
                            x: p.x + dx,
                            y: p.y + dy + y_shift,
                        })
                        .collect::<Vec<_>>();
                    (points, None)
                };

                edge_override_points.insert(e.id.clone(), points);
                edge_override_label.insert(e.id.clone(), label_pos);
            }
        }
    }

    // The Mermaid-like recursive cluster behavior above can increase the effective size of a root
    // cluster (e.g. toggling TB->LR). Mermaid accounts for that via clusterNodes and then lays out
    // the top-level graph using those expanded dimensions.
    //
    // Our headless output keeps all members in one graph, so apply a deterministic packing step
    // for root, isolated clusters to avoid overlaps after the recursive step.
    //
    // This is especially visible when multiple disconnected subgraphs exist: their member nodes
    // do not overlap, but the post-layout cluster padding/title extents can.
    {
        let subgraph_ids: std::collections::HashSet<&str> =
            model.subgraphs.iter().map(|s| s.id.as_str()).collect();

        let mut subgraph_has_parent: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for sg in &model.subgraphs {
            for child in &sg.nodes {
                if subgraph_ids.contains(child.as_str()) {
                    subgraph_has_parent.insert(child.as_str());
                }
            }
        }

        fn collect_descendant_leaf_nodes<'a>(
            id: &'a str,
            subgraphs_by_id: &'a std::collections::HashMap<String, FlowSubgraph>,
            subgraph_ids: &std::collections::HashSet<&'a str>,
            out: &mut std::collections::HashSet<String>,
            visiting: &mut std::collections::HashSet<&'a str>,
        ) {
            if !visiting.insert(id) {
                return;
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                visiting.remove(id);
                return;
            };
            for member in &sg.nodes {
                if subgraph_ids.contains(member.as_str()) {
                    collect_descendant_leaf_nodes(
                        member,
                        subgraphs_by_id,
                        subgraph_ids,
                        out,
                        visiting,
                    );
                } else {
                    out.insert(member.clone());
                }
            }
            visiting.remove(id);
        }

        fn has_external_edges(
            leaves: &std::collections::HashSet<String>,
            edges: &[FlowEdge],
        ) -> bool {
            for e in edges {
                let in_from = leaves.contains(&e.from);
                let in_to = leaves.contains(&e.to);
                if in_from ^ in_to {
                    return true;
                }
            }
            false
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum PackAxis {
            X,
            Y,
        }

        let pack_axis = match normalize_dir(diagram_direction).as_str() {
            "LR" | "RL" => PackAxis::Y,
            _ => PackAxis::X,
        };

        let mut pack_rects: std::collections::HashMap<String, Rect> =
            std::collections::HashMap::new();
        let mut pack_visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

        fn compute_pack_rect(
            id: &str,
            subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
            leaf_rects: &std::collections::HashMap<String, Rect>,
            extra_children: &std::collections::HashMap<String, Vec<String>>,
            pack_rects: &mut std::collections::HashMap<String, Rect>,
            pack_visiting: &mut std::collections::HashSet<String>,
            measurer: &dyn TextMeasurer,
            text_style: &TextStyle,
            title_wrapping_width: f64,
            wrap_mode: WrapMode,
            cluster_padding: f64,
            title_total_margin: f64,
            node_padding: f64,
        ) -> Option<Rect> {
            if let Some(r) = pack_rects.get(id).copied() {
                return Some(r);
            }
            if !pack_visiting.insert(id.to_string()) {
                return None;
            }
            let Some(sg) = subgraphs_by_id.get(id) else {
                pack_visiting.remove(id);
                return None;
            };

            let mut content: Option<Rect> = None;
            for member in &sg.nodes {
                let member_rect = if let Some(r) = leaf_rects.get(member).copied() {
                    Some(r)
                } else if subgraphs_by_id.contains_key(member) {
                    compute_pack_rect(
                        member,
                        subgraphs_by_id,
                        leaf_rects,
                        extra_children,
                        pack_rects,
                        pack_visiting,
                        measurer,
                        text_style,
                        title_wrapping_width,
                        wrap_mode,
                        cluster_padding,
                        title_total_margin,
                        node_padding,
                    )
                } else {
                    None
                };

                if let Some(r) = member_rect {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }

            if let Some(extra) = extra_children.get(id) {
                for child in extra {
                    if let Some(r) = leaf_rects.get(child).copied() {
                        if let Some(ref mut cur) = content {
                            cur.union(r);
                        } else {
                            content = Some(r);
                        }
                    }
                }
            }

            let title_metrics = measurer.measure_wrapped(
                &sg.title,
                text_style,
                Some(title_wrapping_width),
                wrap_mode,
            );

            let mut rect = if let Some(r) = content {
                r
            } else {
                Rect::from_center(
                    0.0,
                    0.0,
                    title_metrics.width.max(1.0),
                    title_metrics.height.max(1.0),
                )
            };

            rect.min_x -= cluster_padding;
            rect.max_x += cluster_padding;
            rect.min_y -= cluster_padding;
            rect.max_y += cluster_padding;

            let min_width = title_metrics.width + node_padding;
            if rect.width() < min_width {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, min_width, rect.height());
            }

            if title_total_margin > 0.0 {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
            }

            let min_height = title_metrics.height + cluster_padding * 2.0 + title_total_margin;
            if rect.height() < min_height {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), min_height);
            }

            pack_visiting.remove(id);
            pack_rects.insert(id.to_string(), rect);
            Some(rect)
        }

        struct PackItem {
            rect: Rect,
            members: Vec<String>,
            internal_edge_ids: Vec<String>,
        }

        let mut items: Vec<PackItem> = Vec::new();
        for sg in &model.subgraphs {
            if subgraph_has_parent.contains(sg.id.as_str()) {
                continue;
            }

            let mut leaves: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut visiting: std::collections::HashSet<&str> = std::collections::HashSet::new();
            collect_descendant_leaf_nodes(
                &sg.id,
                &subgraphs_by_id,
                &subgraph_ids,
                &mut leaves,
                &mut visiting,
            );
            if leaves.is_empty() {
                continue;
            }
            if has_external_edges(&leaves, &model.edges) {
                continue;
            }

            let Some(rect) = compute_pack_rect(
                &sg.id,
                &subgraphs_by_id,
                &leaf_rects,
                &extra_children,
                &mut pack_rects,
                &mut pack_visiting,
                measurer,
                &text_style,
                cluster_title_wrapping_width,
                wrap_mode,
                cluster_padding,
                title_total_margin,
                node_padding,
            ) else {
                continue;
            };

            let mut members = leaves.iter().cloned().collect::<Vec<_>>();

            // Ensure internal labeled edge nodes participate in translation.
            let mut internal_edge_ids: Vec<String> = Vec::new();
            for e in &model.edges {
                if leaves.contains(&e.from) && leaves.contains(&e.to) {
                    internal_edge_ids.push(e.id.clone());
                    if edge_label_is_non_empty(e) {
                        members.push(edge_label_node_id(e));
                    }
                }
            }

            items.push(PackItem {
                rect,
                members,
                internal_edge_ids,
            });
        }

        if !items.is_empty() {
            items.sort_by(|a, b| match pack_axis {
                PackAxis::X => a.rect.min_x.total_cmp(&b.rect.min_x),
                PackAxis::Y => a.rect.min_y.total_cmp(&b.rect.min_y),
            });

            let mut cursor = match pack_axis {
                PackAxis::X => items.first().unwrap().rect.min_x,
                PackAxis::Y => items.first().unwrap().rect.min_y,
            };

            for item in items {
                let (cx, cy) = item.rect.center();
                let desired_center = match pack_axis {
                    PackAxis::X => cursor + item.rect.width() / 2.0,
                    PackAxis::Y => cursor + item.rect.height() / 2.0,
                };
                let (dx, dy) = match pack_axis {
                    PackAxis::X => (desired_center - cx, 0.0),
                    PackAxis::Y => (0.0, desired_center - cy),
                };

                if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
                    for id in &item.members {
                        if let Some((x, y)) = base_pos.get_mut(id) {
                            *x += dx;
                            *y += dy;
                        }
                        if let Some(r) = leaf_rects.get_mut(id) {
                            r.translate(dx, dy);
                        }
                    }
                    for edge_id in &item.internal_edge_ids {
                        edge_packed_shift.insert(edge_id.clone(), (dx, dy));
                    }
                }

                cursor += match pack_axis {
                    PackAxis::X => item.rect.width() + nodesep,
                    PackAxis::Y => item.rect.height() + nodesep,
                };
            }
        }
    }

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    for n in &model.nodes {
        let Some(label) = g.node(&n.id) else {
            return Err(Error::InvalidModel {
                message: format!("missing layout node {}", n.id),
            });
        };
        let (x, y) = base_pos.get(&n.id).copied().unwrap_or((0.0, 0.0));
        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x,
            // Mermaid shifts regular nodes by `subGraphTitleTotalMargin / 2` after Dagre layout.
            y: y + y_shift,
            width: label.width,
            height: label.height,
            is_cluster: false,
        });
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();

    let mut cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut cluster_base_widths: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn compute_cluster_rect(
        id: &str,
        subgraphs_by_id: &std::collections::HashMap<String, FlowSubgraph>,
        leaf_rects: &std::collections::HashMap<String, Rect>,
        extra_children: &std::collections::HashMap<String, Vec<String>>,
        cluster_rects: &mut std::collections::HashMap<String, Rect>,
        cluster_base_widths: &mut std::collections::HashMap<String, f64>,
        visiting: &mut std::collections::HashSet<String>,
        measurer: &dyn TextMeasurer,
        text_style: &TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        cluster_padding: f64,
        title_total_margin: f64,
        node_padding: f64,
    ) -> Result<Rect> {
        if let Some(r) = cluster_rects.get(id).copied() {
            return Ok(r);
        }
        if !visiting.insert(id.to_string()) {
            return Err(Error::InvalidModel {
                message: format!("cycle in subgraph membership involving {id}"),
            });
        }

        let Some(sg) = subgraphs_by_id.get(id) else {
            return Err(Error::InvalidModel {
                message: format!("missing subgraph definition for {id}"),
            });
        };

        let mut content: Option<Rect> = None;
        for member in &sg.nodes {
            let member_rect = if let Some(r) = leaf_rects.get(member).copied() {
                Some(r)
            } else if subgraphs_by_id.contains_key(member) {
                Some(compute_cluster_rect(
                    member,
                    subgraphs_by_id,
                    leaf_rects,
                    extra_children,
                    cluster_rects,
                    cluster_base_widths,
                    visiting,
                    measurer,
                    text_style,
                    title_wrapping_width,
                    wrap_mode,
                    cluster_padding,
                    title_total_margin,
                    node_padding,
                )?)
            } else {
                None
            };

            if let Some(r) = member_rect {
                if let Some(ref mut cur) = content {
                    cur.union(r);
                } else {
                    content = Some(r);
                }
            }
        }

        if let Some(extra) = extra_children.get(id) {
            for child in extra {
                if let Some(r) = leaf_rects.get(child).copied() {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }
        }

        let title_metrics =
            measurer.measure_wrapped(&sg.title, text_style, Some(title_wrapping_width), wrap_mode);
        let mut rect = if let Some(r) = content {
            r
        } else {
            Rect::from_center(
                0.0,
                0.0,
                title_metrics.width.max(1.0),
                title_metrics.height.max(1.0),
            )
        };

        // Expand to provide the cluster's internal padding.
        rect.min_x -= cluster_padding;
        rect.max_x += cluster_padding;
        rect.min_y -= cluster_padding;
        rect.max_y += cluster_padding;

        // Ensure the cluster is wide enough to fit the title.
        let base_width = rect.width();
        cluster_base_widths.insert(id.to_string(), base_width);

        // Mermaid uses `bbox.width + node.padding` (not `2x`) when determining if the cluster
        // needs to widen for the title label.
        let min_width = title_metrics.width + node_padding;
        if rect.width() < min_width {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, min_width, rect.height());
        }

        // Extend height to reserve space for subgraph title margins (Mermaid does this after layout).
        if title_total_margin > 0.0 {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), rect.height() + title_total_margin);
        }

        // Ensure the cluster is tall enough to fit the title placeholder.
        // When a cluster contains small nodes but a multi-line title, the member union can be
        // shorter than the title itself. Mermaid's rendering always accommodates the title bbox.
        let min_height = title_metrics.height + cluster_padding * 2.0 + title_total_margin;
        if rect.height() < min_height {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), min_height);
        }

        visiting.remove(id);
        cluster_rects.insert(id.to_string(), rect);
        Ok(rect)
    }

    for sg in &model.subgraphs {
        let rect = compute_cluster_rect(
            &sg.id,
            &subgraphs_by_id,
            &leaf_rects,
            &extra_children,
            &mut cluster_rects,
            &mut cluster_base_widths,
            &mut visiting,
            measurer,
            &text_style,
            cluster_title_wrapping_width,
            wrap_mode,
            cluster_padding,
            title_total_margin,
            node_padding,
        )?;
        let (cx, cy) = rect.center();

        let title_metrics = measurer.measure_wrapped(
            &sg.title,
            &text_style,
            Some(cluster_title_wrapping_width),
            wrap_mode,
        );
        let title_label = LayoutLabel {
            x: cx,
            y: cy - rect.height() / 2.0 + title_margin_top + title_metrics.height / 2.0,
            width: title_metrics.width,
            height: title_metrics.height,
        };

        let base_width = cluster_base_widths
            .get(&sg.id)
            .copied()
            .unwrap_or(rect.width());
        let padded_label_width = title_metrics.width + node_padding;
        let diff = if base_width <= padded_label_width {
            (padded_label_width - base_width) / 2.0 - node_padding
        } else {
            -node_padding
        };
        let offset_y = title_metrics.height - node_padding / 2.0;

        let effective_dir = effective_cluster_dir(sg, diagram_direction, inherit_dir);

        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            diff,
            offset_y,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|s| normalize_dir(s)),
            effective_dir,
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });

        out_nodes.push(LayoutNode {
            id: sg.id.clone(),
            x: cx,
            // Mermaid does not shift pure cluster nodes by `subGraphTitleTotalMargin / 2`.
            y: cy,
            width: rect.width(),
            height: rect.height(),
            is_cluster: true,
        });
    }
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for e in &model.edges {
        let (dx, dy) = edge_packed_shift.get(&e.id).copied().unwrap_or((0.0, 0.0));
        let (mut points, mut label_pos, label_pos_already_shifted) =
            if let Some(points) = edge_override_points.get(&e.id) {
                (
                    points.clone(),
                    edge_override_label.get(&e.id).cloned().unwrap_or(None),
                    false,
                )
            } else if let Some(label_node_id) = edge_label_nodes.get(&e.id) {
                let to_label_name = edge_to_label_name(e);
                let from_label_name = edge_from_label_name(e);

                let Some(to_label) = g.edge(&e.from, label_node_id, Some(to_label_name.as_str()))
                else {
                    return Err(Error::InvalidModel {
                        message: format!("missing layout edge {} (to-label)", e.id),
                    });
                };
                let Some(from_label) = g.edge(label_node_id, &e.to, Some(from_label_name.as_str()))
                else {
                    return Err(Error::InvalidModel {
                        message: format!("missing layout edge {} (from-label)", e.id),
                    });
                };

                let mut pts = Vec::new();
                for p in &to_label.points {
                    pts.push(LayoutPoint {
                        x: p.x,
                        y: p.y + y_shift,
                    });
                }
                let mut skip_first = false;
                if let (Some(last), Some(first)) = (pts.last(), from_label.points.first()) {
                    let fx = first.x;
                    let fy = first.y + y_shift;
                    if (last.x - fx).abs() <= 1e-6 && (last.y - fy).abs() <= 1e-6 {
                        skip_first = true;
                    }
                }
                for (idx, p) in from_label.points.iter().enumerate() {
                    if skip_first && idx == 0 {
                        continue;
                    }
                    pts.push(LayoutPoint {
                        x: p.x,
                        y: p.y + y_shift,
                    });
                }

                let label_pos = g.node(label_node_id).and_then(|n| {
                    let (x, y) = base_pos
                        .get(label_node_id)
                        .copied()
                        .unwrap_or((n.x.unwrap_or(0.0), n.y.unwrap_or(0.0)));
                    Some(LayoutLabel {
                        x,
                        y: y + y_shift,
                        width: n.width,
                        height: n.height,
                    })
                });

                (pts, label_pos, true)
            } else {
                let Some(label) = g.edge(&e.from, &e.to, Some(&e.id)) else {
                    return Err(Error::InvalidModel {
                        message: format!("missing layout edge {}", e.id),
                    });
                };
                let points = label
                    .points
                    .iter()
                    .map(|p| LayoutPoint {
                        x: p.x,
                        // Mermaid shifts all edge points by `subGraphTitleTotalMargin / 2` after Dagre layout.
                        y: p.y + y_shift,
                    })
                    .collect::<Vec<_>>();
                let label_pos = match (label.x, label.y) {
                    (Some(x), Some(y)) if label.width > 0.0 || label.height > 0.0 => {
                        Some(LayoutLabel {
                            x,
                            // Mermaid shifts edge label y by `subGraphTitleTotalMargin / 2` when positioning.
                            y: y + y_shift,
                            width: label.width,
                            height: label.height,
                        })
                    }
                    _ => None,
                };
                (points, label_pos, false)
            };

        if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
            for p in &mut points {
                p.x += dx;
                p.y += dy;
            }
            if !label_pos_already_shifted {
                if let Some(l) = label_pos.as_mut() {
                    l.x += dx;
                    l.y += dy;
                }
            }
        }
        out_edges.push(LayoutEdge {
            id: e.id.clone(),
            from: e.from.clone(),
            to: e.to.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label: label_pos,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
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
    // This function mirrors Mermaid `@11.12.2` node shape sizing rules at the "rendering-elements"
    // layer, but uses our headless `TextMeasurer` metrics instead of DOM `getBBox()`.
    //
    // References:
    // - `packages/mermaid/src/diagrams/flowchart/flowDb.ts` (shape assignment + padding)
    // - `packages/mermaid/src/rendering-util/rendering-elements/shapes/*.ts` (shape bounds)
    let text_w = metrics.width.max(1.0);
    let text_h = metrics.height.max(1.0);
    let p = padding.max(0.0);

    let shape = layout_shape.unwrap_or("squareRect");

    match shape {
        // Default flowchart process node.
        "squareRect" => (text_w + 4.0 * p, text_h + 2.0 * p),

        // Flowchart "round" node type maps to `roundedRect` in FlowDB.
        "roundedRect" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Diamond (decision/question).
        "diamond" | "question" | "diam" => {
            let w = text_w + p;
            let h = text_h + p;
            let s = w + h;
            (s, s)
        }

        // Hexagon.
        "hexagon" | "hex" => {
            let h = text_h + p;
            let w0 = text_w + 2.5 * p;
            // The current Mermaid implementation expands the half-width by `m = (w/2)/6`,
            // resulting in a total width of `7/6 * w`.
            (w0 * (7.0 / 6.0), h)
        }

        // Stadium/terminator.
        "stadium" => {
            let h = text_h + p;
            let w = text_w + h / 4.0 + p;
            (w, h)
        }

        // Subroutine (framed rectangle): adds an 8px "frame" on both sides.
        "subroutine" | "fr-rect" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + 16.0, h)
        }

        // Cylinder/database.
        "cylinder" | "cyl" => {
            let w = text_w + p;
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            // Mermaid's cylinder path height ends up including two extra `ry` from the ellipses.
            // See `createCylinderPathD` + `translate(..., -(h/2 + ry))`.
            let height = text_h + p + 3.0 * ry;
            (w, height)
        }

        // Circle.
        "circle" | "circ" => {
            // Mermaid uses half-padding for circles and bases radius on label width.
            let d = text_w + p;
            (d, d)
        }

        // Double circle.
        "doublecircle" | "dbl-circ" => {
            // `gap = 5` is hard-coded in Mermaid.
            let d = text_w + p + 10.0;
            (d, d)
        }

        // Lean and trapezoid variants (parallelograms/trapezoids).
        "lean_right" | "lean-r" | "lean-right" | "lean_left" | "lean-l" | "lean-left"
        | "trapezoid" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h, h)
        }

        // Inverted trapezoid uses `2 * padding` on both axes in Mermaid.
        "inv_trapezoid" | "inv-trapezoid" => {
            let w = text_w + 2.0 * p;
            let h = text_h + 2.0 * p;
            (w + h, h)
        }

        // Odd node (`>... ]`) is rendered using `rect_left_inv_arrow`.
        "odd" | "rect_left_inv_arrow" => {
            let w = text_w + p;
            let h = text_h + p;
            (w + h / 4.0, h)
        }

        // Ellipses are currently broken upstream but still emitted by FlowDB.
        // Keep a reasonable headless size for layout stability.
        "ellipse" => (text_w + 2.0 * p, text_h + 2.0 * p),

        // Fallback: treat unknown shapes as default rectangles.
        _ => (text_w + 4.0 * p, text_h + 2.0 * p),
    }
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
