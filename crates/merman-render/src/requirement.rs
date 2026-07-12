use crate::json::from_value_ref;
use crate::model::{
    Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, RequirementDiagramLayout,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;
use serde_json::Value;

mod config;

pub(crate) use config::RequirementConfigView;

fn normalize_dir(direction: &str) -> String {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => "TB".to_string(),
        "BT" => "BT".to_string(),
        "LR" => "LR".to_string(),
        "RL" => "RL".to_string(),
        other => other.to_string(),
    }
}

fn rank_dir_from(direction: &str) -> RankDir {
    match normalize_dir(direction).as_str() {
        "TB" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RequirementLabelMetrics {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) max_width_px: i64,
}

pub(crate) fn requirement_styles_force_bold(css_styles: &[String]) -> bool {
    css_styles.iter().any(|raw| {
        let s = raw.trim().trim_end_matches(';');
        let Some((key, value)) = s.split_once(':') else {
            return false;
        };
        if !key.trim().eq_ignore_ascii_case("font-weight") {
            return false;
        }
        let value = value
            .split_once("!important")
            .map(|(v, _)| v)
            .unwrap_or(value)
            .trim()
            .to_ascii_lowercase();
        value == "bold"
            || value == "bolder"
            || value
                .parse::<u16>()
                .map(|weight| weight >= 600)
                .unwrap_or(false)
    })
}

fn calculate_text_dimensions_like_mermaid(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
) -> (i64, i64, i64) {
    let mut width = 0_i64;
    let mut height = 0_i64;
    let mut line_height = 0_i64;

    for line in crate::text::split_html_br_lines(text) {
        let measured_line = if line.is_empty() { "\u{200b}" } else { line };
        let line_width = measurer
            .measure_svg_simple_text_bbox_width_px(measured_line, style)
            .max(0.0)
            .round() as i64;
        let measured_height = measurer
            .measure_svg_simple_text_bbox_height_px(measured_line, style)
            .max(0.0)
            .round() as i64;
        width = width.max(line_width);
        height += measured_height;
        line_height = line_height.max(measured_height);
    }

    (width, height, line_height)
}

pub(crate) fn calculate_text_width_like_mermaid_px(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
) -> i64 {
    let mut sans = style.clone();
    sans.font_family = Some("sans-serif".to_string());
    sans.font_weight = None;

    let mut configured = style.clone();
    configured.font_weight = None;

    let sans_dimensions = calculate_text_dimensions_like_mermaid(measurer, &sans, text);
    let configured_dimensions = calculate_text_dimensions_like_mermaid(measurer, &configured, text);

    // Mermaid measures both families but selects sans-serif only when every dimension is larger.
    // In particular, it does not take the maximum width of the two probes.
    if sans_dimensions.0 > configured_dimensions.0
        && sans_dimensions.1 > configured_dimensions.1
        && sans_dimensions.2 > configured_dimensions.2
    {
        sans_dimensions.0
    } else {
        configured_dimensions.0
    }
}

pub(crate) fn measure_requirement_label_metrics(
    measurer: &dyn TextMeasurer,
    html_style_regular: &TextStyle,
    html_style_bold: &TextStyle,
    calculation_style: &TextStyle,
    display_text: &str,
    calculation_text: &str,
    bold: bool,
) -> Option<RequirementLabelMetrics> {
    if display_text.trim().is_empty() {
        return None;
    }

    let html_style = if bold {
        html_style_bold
    } else {
        html_style_regular
    };
    let max_width_px =
        (calculate_text_width_like_mermaid_px(measurer, calculation_style, calculation_text) + 50)
            .max(0);
    let max_width = (max_width_px > 0).then_some(max_width_px as f64);
    let measured = crate::text::measure_markdown_with_flowchart_bold_deltas(
        measurer,
        calculation_text,
        html_style,
        max_width,
        WrapMode::HtmlLike,
    );
    let height = measured.height.max(1.0);
    let width = measured.width.max(1.0);

    Some(RequirementLabelMetrics {
        width,
        height,
        max_width_px,
    })
}

#[derive(Debug, Clone)]
struct RequirementBoxLayout {
    width: f64,
    height: f64,
}

fn requirement_box_layout(
    measurer: &dyn TextMeasurer,
    html_style_regular: &TextStyle,
    html_style_bold: &TextStyle,
    calculation_style: &TextStyle,
    lines: &[(String, String, bool)],
    gap: f64,
    padding: f64,
) -> RequirementBoxLayout {
    // Mirrors Mermaid `requirementBox.ts` label stacking and bbox-based sizing.
    let mut html_metrics: Vec<Option<RequirementLabelMetrics>> = Vec::with_capacity(lines.len());
    let mut max_w: f64 = 0.0;

    // First pass: label bbox widths/heights (HTML).
    for (display, calculation, bold) in lines {
        let m = measure_requirement_label_metrics(
            measurer,
            html_style_regular,
            html_style_bold,
            calculation_style,
            display,
            calculation,
            *bold,
        );
        if let Some(m) = &m {
            max_w = max_w.max(m.width);
        }
        html_metrics.push(m);
    }

    let total_w = max_w + padding;

    // Second pass: vertical extents.
    let mut min_y = 0.0;
    let mut max_y = 0.0;
    let mut y_offset = 0.0;

    for (idx, m) in html_metrics.iter().enumerate() {
        let Some(m) = m else {
            continue;
        };

        if idx == 0 {
            min_y = -m.height / 2.0;
            max_y = m.height / 2.0;
            y_offset = m.height;
            continue;
        }
        if idx == 1 {
            let top = -m.height / 2.0 + y_offset;
            let bottom = m.height / 2.0 + y_offset;
            min_y = min_y.min(top);
            max_y = max_y.max(bottom);
            y_offset += m.height + gap;
            continue;
        }

        let top = -m.height / 2.0 + y_offset;
        let bottom = m.height / 2.0 + y_offset;
        min_y = min_y.min(top);
        max_y = max_y.max(bottom);
        y_offset += m.height;
    }

    let bbox_h = (max_y - min_y).max(1.0);
    let total_h = bbox_h + padding;

    RequirementBoxLayout {
        width: total_w.max(1.0),
        height: total_h.max(1.0),
    }
}

fn requirement_edge_id(src: &str, dst: &str, idx: usize) -> String {
    format!("{src}-{dst}-{idx}")
}

fn prefixed_nonempty_line(prefix: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("{prefix}{value}")
    }
}

pub fn layout_requirement_diagram(
    model: &Value,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
) -> Result<RequirementDiagramLayout> {
    let model: RequirementDiagramRenderModel = from_value_ref(model)?;
    layout_requirement_diagram_typed(&model, effective_config, text_measurer)
}

pub fn layout_requirement_diagram_typed(
    model: &RequirementDiagramRenderModel,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
) -> Result<RequirementDiagramLayout> {
    let direction = if model.direction.trim().is_empty() {
        normalize_dir("TB")
    } else {
        normalize_dir(&model.direction)
    };

    let cfg = RequirementConfigView::new(effective_config).layout_settings();
    let font_family = Some(cfg.font_family);
    let html_style_regular = TextStyle {
        font_family: font_family.clone(),
        font_size: cfg.font_size,
        font_weight: None,
    };
    let html_style_bold = TextStyle {
        font_family,
        font_size: cfg.font_size,
        font_weight: Some("bold".to_string()),
    };
    let calculation_style = TextStyle {
        font_family: Some(cfg.calculation_font_family),
        font_size: cfg.calculation_font_size,
        font_weight: None,
    };

    let padding = 20.0;
    let gap = 20.0;

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from(&direction),
        nodesep: cfg.nodesep,
        ranksep: cfg.ranksep,
        marginx: 8.0,
        marginy: 8.0,
        ..Default::default()
    });

    for r in &model.requirements {
        // Mermaid's underlying graph data structures historically used plain JS objects in a few
        // places. The `__proto__` id can still trigger prototype pollution safeguards, effectively
        // dropping the node from the rendered graph. Mirror the upstream SVG baselines.
        if r.name == "__proto__" {
            continue;
        }

        let type_disp = format!("&lt;&lt;{}&gt;&gt;", r.node_type);
        let type_calculation = type_disp.clone();
        let style_bold = requirement_styles_force_bold(&r.css_styles);
        let mut lines: Vec<(String, String, bool)> = Vec::new();
        lines.push((type_disp, type_calculation, style_bold));
        lines.push((r.name.clone(), r.name.clone(), true));

        let id_line = prefixed_nonempty_line("ID: ", &r.requirement_id);
        lines.push((id_line.clone(), id_line, style_bold));

        let text_line = prefixed_nonempty_line("Text: ", &r.text);
        lines.push((text_line.clone(), text_line, style_bold));

        let risk_line = prefixed_nonempty_line("Risk: ", &r.risk);
        lines.push((risk_line.clone(), risk_line, style_bold));

        let verify_line = prefixed_nonempty_line("Verification: ", &r.verify_method);
        lines.push((verify_line.clone(), verify_line, style_bold));

        let box_layout = requirement_box_layout(
            text_measurer,
            &html_style_regular,
            &html_style_bold,
            &calculation_style,
            &lines,
            gap,
            padding,
        );
        g.set_node(
            r.name.clone(),
            NodeLabel {
                width: box_layout.width,
                height: box_layout.height,
                ..Default::default()
            },
        );
    }

    for e in &model.elements {
        if e.name == "__proto__" {
            continue;
        }

        let type_disp = "&lt;&lt;Element&gt;&gt;".to_string();
        let type_calculation = type_disp.clone();
        let style_bold = requirement_styles_force_bold(&e.css_styles);
        let mut lines: Vec<(String, String, bool)> = Vec::new();
        lines.push((type_disp, type_calculation, style_bold));
        lines.push((e.name.clone(), e.name.clone(), true));

        let type_line = e.element_type.trim().to_string();
        let type_line = if type_line.is_empty() {
            String::new()
        } else {
            format!("Type: {type_line}")
        };
        lines.push((type_line.clone(), type_line, style_bold));

        let doc_line = prefixed_nonempty_line("Doc Ref: ", &e.doc_ref);
        lines.push((doc_line.clone(), doc_line, style_bold));

        let box_layout = requirement_box_layout(
            text_measurer,
            &html_style_regular,
            &html_style_bold,
            &calculation_style,
            &lines,
            gap,
            padding,
        );
        g.set_node(
            e.name.clone(),
            NodeLabel {
                width: box_layout.width,
                height: box_layout.height,
                ..Default::default()
            },
        );
    }

    for rel in &model.relationships {
        if !g.has_node(&rel.src) {
            return Err(Error::InvalidModel {
                message: format!("relationship src node not found: {}", rel.src),
            });
        }
        if !g.has_node(&rel.dst) {
            return Err(Error::InvalidModel {
                message: format!("relationship dst node not found: {}", rel.dst),
            });
        }

        // Mermaid's requirement diagram edge ids currently collide for multiple relationships
        // between the same nodes (the upstream DB resets the counter for every relation).
        // Downstream graphlib layout will overwrite earlier edges; only the last relation survives.
        // Mirror this behavior to match the upstream SVG baselines.
        let edge_id = requirement_edge_id(&rel.src, &rel.dst, 0);

        let label_display = format!("&lt;&lt;{}&gt;&gt;", rel.rel_type);
        let label_calculation = label_display.clone();
        let metrics = measure_requirement_label_metrics(
            text_measurer,
            &html_style_regular,
            &html_style_bold,
            &calculation_style,
            &label_display,
            &label_calculation,
            false,
        )
        .unwrap_or(RequirementLabelMetrics {
            width: 0.0,
            height: 0.0,
            max_width_px: 0,
        });

        let el = EdgeLabel {
            width: metrics.width.max(0.0),
            height: metrics.height.max(0.0),
            labelpos: LabelPos::C,
            // Dagre defaults to 10 when unspecified.
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        };
        g.set_edge_named(rel.src.clone(), rel.dst.clone(), Some(edge_id), Some(el));
    }

    dugong::layout_dagreish(&mut g);

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    for v in g.nodes() {
        let Some(n) = g.node(v) else {
            continue;
        };
        let (Some(cx), Some(cy)) = (n.x, n.y) else {
            continue;
        };
        out_nodes.push(LayoutNode {
            id: v.to_string(),
            x: cx - n.width / 2.0,
            y: cy - n.height / 2.0,
            width: n.width,
            height: n.height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
    }

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for ek in g.edge_keys() {
        let Some(e) = g.edge_by_key(&ek) else {
            continue;
        };

        let points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let label = match (e.x, e.y) {
            (Some(x), Some(y)) if e.width > 0.0 && e.height > 0.0 => Some(LayoutLabel {
                x,
                y,
                width: e.width,
                height: e.height,
            }),
            _ => None,
        };

        out_edges.push(LayoutEdge {
            id: ek
                .name
                .clone()
                .unwrap_or_else(|| format!("{}-{}", ek.v, ek.w)),
            from: ek.v.clone(),
            to: ek.w.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label,
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
            if let Some(l) = &e.label {
                min_x = min_x.min(l.x - l.width / 2.0);
                max_x = max_x.max(l.x + l.width / 2.0);
                min_y = min_y.min(l.y - l.height / 2.0);
                max_y = max_y.max(l.y + l.height / 2.0);
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

    let bounds = bounds_for_nodes_edges(&out_nodes, &out_edges);

    Ok(RequirementDiagramLayout {
        nodes: out_nodes,
        edges: out_edges,
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{TextMetrics, VendoredFontMetricsTextMeasurer};

    struct FamilySelectionMeasurer;

    impl TextMeasurer for FamilySelectionMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: 10.0,
                line_count: 1,
            }
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_family.as_deref() == Some("sans-serif") {
                200.0
            } else {
                100.0
            }
        }

        fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            10.0
        }
    }

    #[test]
    fn requirement_styles_detect_bold_font_weight() {
        assert!(super::requirement_styles_force_bold(&[
            "fill:#f9f".to_string(),
            " font-weight:bold".to_string(),
        ]));
        assert!(super::requirement_styles_force_bold(&[
            "font-weight: 700 !important".to_string(),
        ]));
        assert!(!super::requirement_styles_force_bold(&[
            "font-weight: normal".to_string(),
            "stroke:blue".to_string(),
        ]));
    }

    #[test]
    fn requirement_calculate_text_width_uses_mermaid_dimension_selection() {
        let style = TextStyle {
            font_family: Some("configured".to_string()),
            font_size: 16.0,
            font_weight: None,
        };

        assert_eq!(
            calculate_text_width_like_mermaid_px(&FamilySelectionMeasurer, &style, "label"),
            100
        );
    }

    #[test]
    fn requirement_box_wraps_with_root_font_probe_before_group_bbox_padding() {
        let measurer = VendoredFontMetricsTextMeasurer::default();
        let family = Some(crate::config::MERMAID_DEFAULT_FONT_FAMILY_CSS.to_string());
        let regular = TextStyle {
            font_family: family.clone(),
            font_size: 24.0,
            font_weight: None,
        };
        let bold = TextStyle {
            font_family: family.clone(),
            font_size: 24.0,
            font_weight: Some("bold".to_string()),
        };
        let calculation = TextStyle {
            font_family: family,
            font_size: 10.0,
            font_weight: None,
        };
        let lines = vec![
            (
                "&lt;&lt;Requirement&gt;&gt;".to_string(),
                "&lt;&lt;Requirement&gt;&gt;".to_string(),
                false,
            ),
            (
                "req_font_size".to_string(),
                "req_font_size".to_string(),
                true,
            ),
            (
                "ID: req_font_size".to_string(),
                "ID: req_font_size".to_string(),
                false,
            ),
            (
                "Text: font size precedence should be deterministic".to_string(),
                "Text: font size precedence should be deterministic".to_string(),
                false,
            ),
            ("Risk: Low".to_string(), "Risk: Low".to_string(), false),
            (
                "Verification: Test".to_string(),
                "Verification: Test".to_string(),
                false,
            ),
        ];

        let text = measure_requirement_label_metrics(
            &measurer,
            &regular,
            &bold,
            &calculation,
            &lines[3].0,
            &lines[3].1,
            false,
        )
        .expect("text line should be measured");
        let layout =
            requirement_box_layout(&measurer, &regular, &bold, &calculation, &lines, 20.0, 20.0);

        assert_eq!(text.max_width_px, 279);
        assert_eq!((text.width, text.height), (279.0, 108.0));
        assert_eq!((layout.width, layout.height), (299.0, 418.0));
    }
}
