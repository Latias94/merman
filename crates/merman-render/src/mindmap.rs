use crate::json::from_value_ref;
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
    #[serde(default, rename = "labelType")]
    label_type: String,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    #[allow(dead_code)]
    level: i64,
    #[serde(default)]
    padding: f64,
    #[serde(default)]
    #[allow(dead_code)]
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

fn mindmap_label_bbox_px(
    text: &str,
    label_type: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64) {
    // Mermaid mindmap labels are rendered via HTML `<foreignObject>` and respect
    // `mindmap.maxNodeWidth` (default 200px). When the raw label is wider than that, Mermaid
    // switches the label container to a fixed 200px width and allows HTML-like wrapping (e.g.
    // `white-space: break-spaces` in upstream SVG baselines).
    //
    // Mirror that by measuring with an explicit max width in HTML-like mode.
    let max_node_width_px = max_node_width_px.max(1.0);

    let wrapped = if label_type == "markdown" {
        crate::text::measure_markdown_with_flowchart_bold_deltas(
            measurer,
            text,
            style,
            Some(max_node_width_px),
            WrapMode::HtmlLike,
        )
    } else {
        measurer.measure_wrapped(text, style, Some(max_node_width_px), WrapMode::HtmlLike)
    };

    // Mermaid mindmap labels can overflow the configured `maxNodeWidth` when they contain long
    // unbreakable tokens. Upstream measures these via DOM in a way that resembles `scrollWidth`,
    // so keep the larger of:
    // - the wrapped layout width (clamped by `max-width`), and
    // - the unwrapped overflow width (ignores `max-width`).
    let unwrapped = if label_type == "markdown" {
        crate::text::measure_markdown_with_flowchart_bold_deltas(
            measurer,
            text,
            style,
            None,
            WrapMode::HtmlLike,
        )
    } else {
        measurer.measure_wrapped(text, style, None, WrapMode::HtmlLike)
    };

    (
        wrapped.width.max(unwrapped.width).max(0.0),
        wrapped.height.max(0.0),
    )
}

fn mindmap_node_dimensions_px(
    node: &MindmapNodeModel,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64) {
    let (bbox_w, bbox_h) = mindmap_label_bbox_px(
        &node.label,
        &node.label_type,
        measurer,
        style,
        max_node_width_px,
    );
    let padding = node.padding.max(0.0);
    let half_padding = padding / 2.0;

    // Align with Mermaid shape sizing rules for mindmap nodes (via `labelHelper(...)` + shape
    // handlers in `rendering-elements/shapes/*`).
    match node.shape.as_str() {
        // `defaultMindmapNode.ts`: w = bbox.width + 8 * halfPadding; h = bbox.height + 2 * halfPadding
        "" | "defaultMindmapNode" => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
        // Mindmap node shapes use the standard `labelHelper(...)` label bbox, but mindmap DB
        // adjusts `node.padding` depending on the delimiter type (e.g. `[` / `(` / `{{`).
        //
        // Upstream Mermaid@11.12.2 mindmap SVG baselines show:
        // - rect (`[text]`): w = bbox.width + 2*padding, h = bbox.height + padding
        // - rounded (`(text)`): w = bbox.width + 1.5*padding, h = bbox.height + 1.5*padding
        "rect" => (bbox_w + 2.0 * padding, bbox_h + padding),
        "rounded" => (bbox_w + 1.5 * padding, bbox_h + 1.5 * padding),
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
    let mut model: MindmapModel = from_value_ref(model)?;

    let text_style = mindmap_text_style(effective_config);
    let max_node_width_px = config_f64(effective_config, &["mindmap", "maxNodeWidth"])
        .unwrap_or(200.0)
        .max(1.0);

    model.nodes.sort_by_cached_key(|n| {
        let num = n.id.parse::<i64>().unwrap_or(i64::MAX);
        (num, n.id.clone())
    });

    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(model.nodes.len());
    for n in &model.nodes {
        let (width, height) =
            mindmap_node_dimensions_px(n, text_measurer, &text_style, max_node_width_px);

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
                    parent: None,
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
                    source_anchor: None,
                    target_anchor: None,
                    ideal_length: 0.0,
                    elasticity: 0.0,
                })
                .collect(),
            compounds: Vec::new(),
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

    let mut node_pos: std::collections::HashMap<&str, (f64, f64)> =
        std::collections::HashMap::with_capacity(nodes.len());
    for n in &nodes {
        node_pos.insert(n.id.as_str(), (n.x, n.y));
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for e in model.edges {
        let Some((sx, sy)) = node_pos.get(e.start.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("edge start node not found: {}", e.start),
            });
        };
        let Some((tx, ty)) = node_pos.get(e.end.as_str()).copied() else {
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
