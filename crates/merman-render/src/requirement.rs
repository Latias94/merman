use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, RequirementDiagramLayout};
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequirementNodeModel {
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(default)]
    requirement_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    risk: Option<String>,
    #[serde(default)]
    verify_method: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ElementNodeModel {
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(default)]
    doc_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RequirementRelationshipModel {
    #[serde(rename = "type")]
    rel_type: String,
    src: String,
    dst: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequirementDiagramModel {
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    requirements: Vec<RequirementNodeModel>,
    #[serde(default)]
    elements: Vec<ElementNodeModel>,
    #[serde(default)]
    relationships: Vec<RequirementRelationshipModel>,
}

fn measure_line_width(text_measurer: &dyn TextMeasurer, text: &str, bold: bool) -> f64 {
    let style = TextStyle {
        font_family: None,
        font_size: 16.0,
        font_weight: if bold { Some("bold".to_string()) } else { None },
    };
    let m = text_measurer.measure(text, &style);
    // Mermaid's `calculateTextWidth` tends to over-estimate and then adds a +50px padding.
    // Keep this bias so node widths stay roughly comparable across fixtures.
    (m.width + 50.0).max(1.0)
}

fn compute_box_size(
    text_measurer: &dyn TextMeasurer,
    lines: &[(String, bool)],
    has_body: bool,
) -> (f64, f64) {
    let padding = 20.0;
    let gap = 20.0;
    let line_h = 24.0;

    let mut max_w: f64 = 0.0;
    for (t, bold) in lines {
        max_w = max_w.max(measure_line_width(text_measurer, t, *bold));
    }

    let mut content_h = line_h * lines.len() as f64;
    if has_body {
        content_h += gap;
    }
    let total_w = max_w + padding;
    let total_h = content_h + padding;

    let _ = gap;
    let _ = line_h;
    (total_w, total_h)
}

fn axis_dir(direction: &str) -> (&'static str, i32) {
    match direction {
        "BT" => ("y", -1),
        "LR" => ("x", 1),
        "RL" => ("x", -1),
        _ => ("y", 1),
    }
}

pub fn layout_requirement_diagram(
    model: &Value,
    _effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
) -> Result<RequirementDiagramLayout> {
    let model: RequirementDiagramModel = serde_json::from_value(model.clone())?;

    let direction = model.direction.unwrap_or_else(|| "TB".to_string());
    let (axis, dir_sign) = axis_dir(&direction);

    let mut nodes: Vec<LayoutNode> = Vec::new();

    let mut cursor = 0.0;
    let spacing = 50.0;

    for r in &model.requirements {
        let mut lines: Vec<(String, bool)> = Vec::new();
        lines.push((format!("<<{}>>", r.node_type), false));
        lines.push((r.name.clone(), true));
        let mut has_body = false;
        if r.requirement_id.as_deref().unwrap_or("").trim().len() > 0 {
            lines.push((
                format!("ID: {}", r.requirement_id.clone().unwrap_or_default()),
                false,
            ));
            has_body = true;
        }
        if r.text.as_deref().unwrap_or("").trim().len() > 0 {
            lines.push((
                format!("Text: {}", r.text.clone().unwrap_or_default()),
                false,
            ));
            has_body = true;
        }
        if r.risk.as_deref().unwrap_or("").trim().len() > 0 {
            lines.push((
                format!("Risk: {}", r.risk.clone().unwrap_or_default()),
                false,
            ));
            has_body = true;
        }
        if r.verify_method.as_deref().unwrap_or("").trim().len() > 0 {
            lines.push((
                format!(
                    "Verification: {}",
                    r.verify_method.clone().unwrap_or_default()
                ),
                false,
            ));
            has_body = true;
        }

        let (w, h) = compute_box_size(text_measurer, &lines, has_body);

        let (x, y) = if axis == "y" {
            (0.0, cursor)
        } else {
            (cursor, 0.0)
        };
        let y = y * dir_sign as f64;
        let x = x * dir_sign as f64;

        nodes.push(LayoutNode {
            id: r.name.clone(),
            x,
            y,
            width: w,
            height: h,
            is_cluster: false,
        });

        cursor += if axis == "y" {
            h + spacing
        } else {
            w + spacing
        };
    }

    for e in &model.elements {
        let mut lines: Vec<(String, bool)> = Vec::new();
        lines.push(("<<Element>>".to_string(), false));
        lines.push((e.name.clone(), true));
        let mut has_body = false;
        if e.node_type.trim().len() > 0 {
            lines.push((format!("Type: {}", e.node_type), false));
            has_body = true;
        }
        if e.doc_ref.as_deref().unwrap_or("").trim().len() > 0 {
            lines.push((
                format!("Doc Ref: {}", e.doc_ref.clone().unwrap_or_default()),
                false,
            ));
            has_body = true;
        }

        let (w, h) = compute_box_size(text_measurer, &lines, has_body);

        let (x, y) = if axis == "y" {
            (0.0, cursor)
        } else {
            (cursor, 0.0)
        };
        let y = y * dir_sign as f64;
        let x = x * dir_sign as f64;

        nodes.push(LayoutNode {
            id: e.name.clone(),
            x,
            y,
            width: w,
            height: h,
            is_cluster: false,
        });

        cursor += if axis == "y" {
            h + spacing
        } else {
            w + spacing
        };
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for rel in &model.relationships {
        let Some(src) = nodes.iter().find(|n| n.id == rel.src) else {
            return Err(Error::InvalidModel {
                message: format!("relationship src node not found: {}", rel.src),
            });
        };
        let Some(dst) = nodes.iter().find(|n| n.id == rel.dst) else {
            return Err(Error::InvalidModel {
                message: format!("relationship dst node not found: {}", rel.dst),
            });
        };

        let (sx, sy, mx, my, ex, ey) = if axis == "y" {
            let start_x = src.x + src.width / 2.0;
            let start_y = src.y + if dir_sign > 0 { src.height } else { 0.0 };
            let end_x = dst.x + dst.width / 2.0;
            let end_y = dst.y + if dir_sign > 0 { 0.0 } else { dst.height };
            let mid_x = start_x;
            let mid_y = (start_y + end_y) / 2.0;
            (start_x, start_y, mid_x, mid_y, end_x, end_y)
        } else {
            let start_x = src.x + if dir_sign > 0 { src.width } else { 0.0 };
            let start_y = src.y + src.height / 2.0;
            let end_x = dst.x + if dir_sign > 0 { 0.0 } else { dst.width };
            let end_y = dst.y + dst.height / 2.0;
            let mid_x = (start_x + end_x) / 2.0;
            let mid_y = start_y;
            (start_x, start_y, mid_x, mid_y, end_x, end_y)
        };

        let points = vec![
            LayoutPoint { x: sx, y: sy },
            LayoutPoint { x: mx, y: my },
            LayoutPoint { x: ex, y: ey },
        ];

        edges.push(LayoutEdge {
            id: format!("{}-{}-0", rel.src, rel.dst),
            from: rel.src.clone(),
            to: rel.dst.clone(),
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

    fn bounds_for_nodes_edges(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
        if nodes.is_empty() && edges.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for n in nodes {
            min_x = min_x.min(n.x);
            min_y = min_y.min(n.y);
            max_x = max_x.max(n.x + n.width);
            max_y = max_y.max(n.y + n.height);
        }
        for e in edges {
            for p in &e.points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
        }

        if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
            return None;
        }
        Some(Bounds {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    let bounds = bounds_for_nodes_edges(&nodes, &edges);

    Ok(RequirementDiagramLayout {
        nodes,
        edges,
        bounds,
    })
}
