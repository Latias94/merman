use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, MindmapDiagramLayout};
use crate::text::WrapMode;
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut v = cfg;
    for p in path {
        v = v.get(*p)?;
    }
    v.as_f64()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut v = cfg;
    for p in path {
        v = v.get(*p)?;
    }
    v.as_str().map(|s| s.to_string())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MindmapNodeModel {
    id: String,
    label: String,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    level: i64,
    #[serde(default)]
    padding: f64,
    #[serde(default)]
    width: f64,
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

fn mindmap_text_style(effective_config: &Value) -> TextStyle {
    // Mermaid mindmap labels are rendered via HTML `<foreignObject>` and inherit the global font.
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()));
    let font_size = config_f64(effective_config, &["fontSize"])
        .unwrap_or(16.0)
        .max(1.0);
    TextStyle {
        font_family,
        font_size,
        font_weight: None,
    }
}

fn mindmap_label_bbox_px(text: &str, measurer: &dyn TextMeasurer, style: &TextStyle) -> (f64, f64) {
    // Mermaid mindmap uses HTML labels with `white-space: nowrap`, so we should not apply wrapping
    // even if `mindmap.maxNodeWidth` is set.
    let m = measurer.measure_wrapped(text, style, None, WrapMode::HtmlLike);
    (m.width.max(0.0), m.height.max(0.0))
}

fn mindmap_node_dimensions_px(
    node: &MindmapNodeModel,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> (f64, f64) {
    let (bbox_w, bbox_h) = mindmap_label_bbox_px(&node.label, measurer, style);
    let padding = node.padding.max(0.0);
    let half_padding = padding / 2.0;

    // Align with Mermaid shape sizing rules for mindmap nodes (via `labelHelper(...)` + shape
    // handlers in `rendering-elements/shapes/*`).
    match node.shape.as_str() {
        // `defaultMindmapNode.ts`: w = bbox.width + 8 * halfPadding; h = bbox.height + 2 * halfPadding
        "" | "defaultMindmapNode" => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
        // `squareRect.ts` -> `drawRect.ts`: labelPaddingX = padding*2, labelPaddingY = padding
        // totalW = bbox.width + 2*labelPaddingX = bbox.width + 4*padding
        // totalH = bbox.height + 2*labelPaddingY = bbox.height + 2*padding
        "rect" => (bbox_w + 4.0 * padding, bbox_h + 2.0 * padding),
        // `roundedRect.ts`: w = bbox.width + 2*padding; h = bbox.height + 2*padding
        "rounded" => (bbox_w + 2.0 * padding, bbox_h + 2.0 * padding),
        // `mindmapCircle.ts` -> `circle.ts`: radius = bbox.width/2 + padding (mindmap passes full padding)
        "mindmapCircle" => {
            let d = bbox_w + 2.0 * padding;
            (d, d)
        }
        // `cloud.ts`: w = bbox.width + 2*halfPadding; h = bbox.height + 2*halfPadding
        "cloud" => (bbox_w + 2.0 * half_padding, bbox_h + 2.0 * half_padding),
        // `bang.ts`: effectiveWidth = bbox.width + 10*halfPadding (min bbox+20 is always smaller here)
        //           effectiveHeight = bbox.height + 8*halfPadding (min bbox+20 is always smaller here)
        "bang" => (bbox_w + 10.0 * half_padding, bbox_h + 8.0 * half_padding),
        // `hexagon.ts`: h = bbox.height + padding; w = bbox.width + 2.5*padding; then expands by +w/6
        // due to `halfWidth = w/2 + m` where `m = (w/2)/6`.
        "hexagon" => {
            let w = bbox_w + 2.5 * padding;
            let h = bbox_h + padding;
            (w * (7.0 / 6.0), h)
        }
        _ => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
    }
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

fn shift_nodes_to_positive_bounds(nodes: &mut [LayoutNode], content_min: f64) {
    if nodes.is_empty() {
        return;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for n in nodes.iter() {
        min_x = min_x.min(n.x - n.width / 2.0);
        min_y = min_y.min(n.y - n.height / 2.0);
    }
    if !(min_x.is_finite() && min_y.is_finite()) {
        return;
    }
    let dx = content_min - min_x;
    let dy = content_min - min_y;
    for n in nodes.iter_mut() {
        n.x += dx;
        n.y += dy;
    }
}

pub fn layout_mindmap_diagram(
    model: &Value,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<MindmapDiagramLayout> {
    let model: MindmapModel = serde_json::from_value(model.clone())?;

    let text_style = mindmap_text_style(effective_config);

    let mut nodes: Vec<LayoutNode> = Vec::new();
    let mut id_order: Vec<(i64, String)> = model
        .nodes
        .iter()
        .map(|n| (n.id.parse::<i64>().unwrap_or(i64::MAX), n.id.clone()))
        .collect();
    id_order.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let node_by_id: std::collections::BTreeMap<String, MindmapNodeModel> =
        model.nodes.into_iter().map(|n| (n.id.clone(), n)).collect();

    for (_idx, (_num, id)) in id_order.iter().enumerate() {
        let Some(n) = node_by_id.get(id) else {
            continue;
        };
        let (width, height) = mindmap_node_dimensions_px(n, text_measurer, &text_style);

        nodes.push(LayoutNode {
            id: n.id.clone(),
            // Mermaid mindmap uses Cytoscape COSE-Bilkent and initializes node positions at (0,0).
            // We keep that behavior so `manatee` can reproduce upstream placements deterministically.
            x: 0.0,
            y: 0.0,
            width: width.max(1.0),
            height: height.max(1.0),
            is_cluster: false,
        });
    }

    if use_manatee_layout {
        let graph = manatee::Graph {
            nodes: nodes
                .iter()
                .map(|n| manatee::Node {
                    id: n.id.clone(),
                    width: n.width,
                    height: n.height,
                    x: n.x,
                    y: n.y,
                })
                .collect(),
            edges: model
                .edges
                .iter()
                .map(|e| manatee::Edge {
                    id: e.id.clone(),
                    source: e.start.clone(),
                    target: e.end.clone(),
                    ideal_length: 0.0,
                })
                .collect(),
        };
        let result = manatee::layout(&graph, manatee::Algorithm::CoseBilkent(Default::default()))
            .map_err(|e| Error::InvalidModel {
            message: format!("manatee layout failed: {e}"),
        })?;
        for n in &mut nodes {
            if let Some(p) = result.positions.get(n.id.as_str()) {
                n.x = p.x;
                n.y = p.y;
            }
        }
    }

    // Mermaid's COSE-Bilkent post-layout normalizes to a positive coordinate space via
    // `transform(0,0)` (layout-base), yielding a content bbox that starts around (15,15) before
    // the 10px viewport padding is applied (viewBox starts at 5,5).
    //
    // When we do NOT use the manatee COSE port, keep a compatibility translation so parity-root
    // viewport comparisons remain stable.
    if !use_manatee_layout {
        shift_nodes_to_positive_bounds(&mut nodes, 15.0);
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
