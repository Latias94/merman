use super::super::roughjs_common::{ops_to_svg_path_d, parse_hex_color_to_srgba};
use super::super::*;
use crate::model::{
    IshikawaBranchLayout, IshikawaCauseLabelGroupLayout, IshikawaLabelBoxLayout,
    IshikawaLineLayout, IshikawaSubGroupLayout, IshikawaTextLayout,
};

struct RoughContext {
    seed: u64,
    line_color: String,
    fill_color: String,
}

#[derive(Clone, Copy)]
struct RoughPaint<'a> {
    fill_color: &'a str,
    stroke_color: &'a str,
    stroke_width: f32,
    fill_weight: f32,
}

pub(crate) fn render_ishikawa_diagram_svg(
    layout: &IshikawaDiagramLayout,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("ishikawa");
    let mut out = String::new();
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        layout.viewbox_x,
        layout.viewbox_y,
        layout.total_width,
        layout.total_height,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "ishikawa");
    root_chrome.dom.trailing_newline = false;
    root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Ishikawa, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;

    let css = ishikawa_css(layout, effective_config);
    let _ = write!(&mut out, "<style>{css}</style>");
    out.push_str(r#"<g/><g class="ishikawa">"#);
    if crate::config::config_diagram_look(effective_config).as_str() == "handDrawn" {
        let theme = PresentationTheme::new(effective_config).ishikawa();
        let rough = RoughContext {
            seed: effective_config
                .get("handDrawnSeed")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            line_color: theme.line_color,
            fill_color: theme.main_bkg,
        };
        push_hand_drawn_diagram(&mut out, layout, &rough);
    } else {
        let marker_id = format!("ishikawa-arrow-{diagram_id}");
        push_classic_diagram(&mut out, layout, &marker_id);
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}

fn push_classic_diagram(out: &mut String, layout: &IshikawaDiagramLayout, marker_id: &str) {
    let _ = write!(out, r#"<defs><marker id=""#);
    escape_xml_into(out, marker_id);
    out.push_str(
        r#"" viewBox="0 0 10 10" refX="0" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M 10 0 L 0 5 L 10 10 Z" class="ishikawa-arrow"></path></marker></defs>"#,
    );

    if let Some(spine) = &layout.spine {
        push_line(out, spine, marker_id);
    }
    if let Some(head) = &layout.head {
        let _ = write!(
            out,
            r#"<g class="ishikawa-head-group" transform="translate({}, {})"><path class="ishikawa-head" d=""#,
            fmt(head.x),
            fmt(head.y)
        );
        escape_attr_into(out, &head.path_d);
        out.push_str(r#""></path>"#);
        push_ishikawa_head_text(out, &head.label, -head.x, -head.y);
        out.push_str("</g>");
    }
    for pair in &layout.pairs {
        out.push_str(r#"<g class="ishikawa-pair">"#);
        push_branch(out, &pair.upper, marker_id);
        if let Some(lower) = &pair.lower {
            push_branch(out, lower, marker_id);
        }
        out.push_str("</g>");
    }
}

fn push_hand_drawn_diagram(out: &mut String, layout: &IshikawaDiagramLayout, rough: &RoughContext) {
    if let Some(head) = &layout.head {
        let _ = write!(
            out,
            r#"<g class="ishikawa-head-group" transform="translate({}, {})">"#,
            fmt(head.x),
            fmt(head.y)
        );
        push_rough_hachure_path(out, "ishikawa-head", &head.path_d, rough);
        push_ishikawa_head_text(out, &head.label, -head.x, -head.y);
        out.push_str("</g>");
    }
    for pair in &layout.pairs {
        out.push_str(r#"<g class="ishikawa-pair">"#);
        push_hand_drawn_branch(out, &pair.upper, rough);
        if let Some(lower) = &pair.lower {
            push_hand_drawn_branch(out, lower, rough);
        }
        out.push_str("</g>");
    }
    if let Some(spine) = &layout.spine {
        push_rough_line(out, spine, rough);
    }
}

fn push_branch(out: &mut String, branch: &IshikawaBranchLayout, marker_id: &str) {
    push_line(out, &branch.line, marker_id);
    push_cause_label_group(out, &branch.label_group);
    for sub_group in &branch.sub_groups {
        push_sub_group(out, sub_group, marker_id);
    }
}

fn push_cause_label_group(out: &mut String, group: &IshikawaCauseLabelGroupLayout) {
    out.push_str(r#"<g class="ishikawa-label-group">"#);
    let label_box = &group.label_box;
    let _ = write!(
        out,
        r#"<rect class="ishikawa-label-box" x="{}" y="{}" width="{}" height="{}"></rect>"#,
        fmt(label_box.x),
        fmt(label_box.y),
        fmt(label_box.width),
        fmt(label_box.height)
    );
    push_text_with_offset(out, &group.label, 0.0, 0.0);
    out.push_str("</g>");
}

fn push_sub_group(out: &mut String, group: &IshikawaSubGroupLayout, marker_id: &str) {
    out.push_str(r#"<g class="ishikawa-sub-group">"#);
    push_line(out, &group.line, marker_id);
    push_text_with_offset(out, &group.label, 0.0, 0.0);
    out.push_str("</g>");
}

fn push_hand_drawn_branch(out: &mut String, branch: &IshikawaBranchLayout, rough: &RoughContext) {
    push_rough_line(out, &branch.line, rough);
    push_rough_arrow_marker(out, &branch.line, rough);
    push_hand_drawn_cause_label_group(out, &branch.label_group, rough);
    for sub_group in &branch.sub_groups {
        push_hand_drawn_sub_group(out, sub_group, rough);
    }
}

fn push_hand_drawn_cause_label_group(
    out: &mut String,
    group: &IshikawaCauseLabelGroupLayout,
    rough: &RoughContext,
) {
    out.push_str(r#"<g class="ishikawa-label-group">"#);
    let label_box = &group.label_box;
    push_rough_hachure_rect(out, "ishikawa-label-box", label_box, rough);
    push_text_with_offset(out, &group.label, 0.0, 0.0);
    out.push_str("</g>");
}

fn push_hand_drawn_sub_group(
    out: &mut String,
    group: &IshikawaSubGroupLayout,
    rough: &RoughContext,
) {
    out.push_str(r#"<g class="ishikawa-sub-group">"#);
    push_rough_line(out, &group.line, rough);
    push_rough_arrow_marker(out, &group.line, rough);
    push_text_with_offset(out, &group.label, 0.0, 0.0);
    out.push_str("</g>");
}

fn push_line(out: &mut String, line: &IshikawaLineLayout, marker_id: &str) {
    let _ = write!(
        out,
        r#"<line class="{}" x1="{}" y1="{}" x2="{}" y2="{}""#,
        escape_attr_display(&line.class_name),
        fmt(line.x1),
        fmt(line.y1),
        fmt(line.x2),
        fmt(line.y2)
    );
    if line.marker_start {
        let _ = write!(
            out,
            r#" marker-start="url(#{})""#,
            escape_attr_display(marker_id)
        );
    }
    out.push_str("></line>");
}

fn push_rough_line(out: &mut String, line: &IshikawaLineLayout, rough: &RoughContext) {
    let options = roughr::core::OptionsBuilder::default()
        .seed(rough.seed)
        .roughness(1.5)
        .stroke(rough_color(&rough.line_color))
        .stroke_width(2.0)
        .build()
        .expect("static Ishikawa rough line options must be valid");
    let drawable = roughr::generator::Generator::default().line::<f64>(
        line.x1,
        line.y1,
        line.x2,
        line.y2,
        &Some(options),
    );
    push_rough_group(
        out,
        Some(&line.class_name),
        drawable.sets,
        RoughPaint {
            fill_color: &rough.line_color,
            stroke_color: &rough.line_color,
            stroke_width: 2.0,
            fill_weight: 0.0,
        },
    );
}

fn push_rough_hachure_path(out: &mut String, class_name: &str, path_d: &str, rough: &RoughContext) {
    let drawable = roughr::generator::Generator::default()
        .path::<f64>(path_d.to_string(), &Some(rough_hachure_options(rough)));
    push_rough_group(
        out,
        Some(class_name),
        drawable.sets,
        RoughPaint {
            fill_color: &rough.fill_color,
            stroke_color: &rough.line_color,
            stroke_width: 2.0,
            fill_weight: 2.5,
        },
    );
}

fn push_rough_hachure_rect(
    out: &mut String,
    class_name: &str,
    label_box: &IshikawaLabelBoxLayout,
    rough: &RoughContext,
) {
    let drawable = roughr::generator::Generator::default().rectangle::<f64>(
        label_box.x,
        label_box.y,
        label_box.width,
        label_box.height,
        &Some(rough_hachure_options(rough)),
    );
    push_rough_group(
        out,
        Some(class_name),
        drawable.sets,
        RoughPaint {
            fill_color: &rough.fill_color,
            stroke_color: &rough.line_color,
            stroke_width: 2.0,
            fill_weight: 2.5,
        },
    );
}

fn push_rough_arrow_marker(out: &mut String, line: &IshikawaLineLayout, rough: &RoughContext) {
    if !line.marker_start {
        return;
    }
    let dx = line.x1 - line.x2;
    let dy = line.y1 - line.y2;
    let len = dx.hypot(dy);
    if len == 0.0 {
        return;
    }

    let ux = dx / len;
    let uy = dy / len;
    let size = 6.0;
    let px = -uy * size;
    let py = ux * size;
    let left_x = line.x1 - ux * size * 2.0 + px;
    let left_y = line.y1 - uy * size * 2.0 + py;
    let right_x = line.x1 - ux * size * 2.0 - px;
    let right_y = line.y1 - uy * size * 2.0 - py;
    let path_d = format!(
        "M {} {} L {} {} L {} {} Z",
        line.x1, line.y1, left_x, left_y, right_x, right_y
    );

    let color = rough_color(&rough.line_color);
    let options = roughr::core::OptionsBuilder::default()
        .seed(rough.seed)
        .roughness(1.0)
        .fill(color)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(color)
        .stroke_width(1.0)
        .build()
        .expect("static Ishikawa rough arrow options must be valid");
    let drawable = roughr::generator::Generator::default().path::<f64>(path_d, &Some(options));
    push_rough_group(
        out,
        None,
        drawable.sets,
        RoughPaint {
            fill_color: &rough.line_color,
            stroke_color: &rough.line_color,
            stroke_width: 1.0,
            fill_weight: 0.0,
        },
    );
}

fn rough_hachure_options(rough: &RoughContext) -> roughr::core::Options {
    roughr::core::OptionsBuilder::default()
        .seed(rough.seed)
        .roughness(1.5)
        .fill(rough_color(&rough.fill_color))
        .fill_style(roughr::core::FillStyle::Hachure)
        .fill_weight(2.5)
        .hachure_gap(5.0)
        .stroke(rough_color(&rough.line_color))
        .stroke_width(2.0)
        .build()
        .expect("static Ishikawa rough hachure options must be valid")
}

fn rough_color(css: &str) -> roughr::Srgba {
    parse_hex_color_to_srgba(css).unwrap_or_else(|| roughr::Srgba::new(0.0, 0.0, 0.0, 1.0))
}

fn push_rough_group(
    out: &mut String,
    class_name: Option<&str>,
    sets: Vec<roughr::core::OpSet<f64>>,
    paint: RoughPaint<'_>,
) {
    out.push_str("<g");
    if let Some(class_name) = class_name {
        out.push_str(r#" class=""#);
        escape_attr_into(out, class_name);
        out.push('"');
    }
    out.push('>');
    for set in sets {
        let d = ops_to_svg_path_d(&set);
        let (stroke, stroke_width, fill) = match set.op_set_type {
            roughr::core::OpSetType::FillSketch => (paint.fill_color, paint.fill_weight, "none"),
            roughr::core::OpSetType::FillPath => ("none", 0.0, paint.fill_color),
            roughr::core::OpSetType::Path => (paint.stroke_color, paint.stroke_width, "none"),
        };
        out.push_str(r#"<path d=""#);
        escape_attr_into(out, &d);
        out.push_str(r#"" stroke=""#);
        escape_attr_into(out, stroke);
        let _ = write!(out, r#"" stroke-width="{stroke_width}" fill=""#);
        escape_attr_into(out, fill);
        out.push_str(r#""></path>"#);
    }
    out.push_str("</g>");
}

fn push_ishikawa_head_text(out: &mut String, text: &IshikawaTextLayout, dx: f64, dy: f64) {
    let mut shifted = text.clone();
    shifted.anchor = "start".to_string();
    shifted.x = 0.0;
    shifted.y += dy;
    let transform_x = text.x + dx - (text.bbox.max_x - text.bbox.min_x) / 2.0;
    let transform_y = text.y + dy - shifted.y;
    let first_y =
        shifted.y - ((shifted.lines.len().saturating_sub(1)) as f64 * shifted.line_height) / 2.0;
    let _ = write!(
        out,
        r#"<text class="{}" text-anchor="{}" x="{}" y="{}" transform="translate({},{})">"#,
        escape_attr_display(&shifted.class_name),
        escape_attr_display(&shifted.anchor),
        fmt(shifted.x),
        fmt(first_y),
        fmt(transform_x),
        fmt(transform_y)
    );
    for (idx, line) in shifted.lines.iter().enumerate() {
        let _ = write!(
            out,
            r#"<tspan x="{}" dy="{}">"#,
            fmt(shifted.x),
            if idx == 0 {
                "0".to_string()
            } else {
                fmt_string(shifted.line_height)
            }
        );
        escape_xml_into(out, line);
        out.push_str("</tspan>");
    }
    out.push_str("</text>");
}

fn push_text_with_offset(out: &mut String, text: &IshikawaTextLayout, dx: f64, dy: f64) {
    let first_y =
        text.y + dy - ((text.lines.len().saturating_sub(1)) as f64 * text.line_height) / 2.0;
    let _ = write!(
        out,
        r#"<text class="{}" text-anchor="{}" x="{}" y="{}">"#,
        escape_attr_display(&text.class_name),
        escape_attr_display(&text.anchor),
        fmt(text.x + dx),
        fmt(first_y)
    );
    if text.lines.is_empty() {
        escape_xml_into(out, &text.text);
    } else {
        for (idx, line) in text.lines.iter().enumerate() {
            let _ = write!(
                out,
                r#"<tspan x="{}" dy="{}">"#,
                fmt(text.x + dx),
                if idx == 0 {
                    "0".to_string()
                } else {
                    fmt_string(text.line_height)
                }
            );
            escape_xml_into(out, line);
            out.push_str("</tspan>");
        }
    }
    out.push_str("</text>");
}

fn ishikawa_css(layout: &IshikawaDiagramLayout, effective_config: &serde_json::Value) -> String {
    let theme = PresentationTheme::new(effective_config).ishikawa();
    let font_size = crate::ishikawa::IshikawaConfigView::new(effective_config)
        .render_settings()
        .font_size_css
        .unwrap_or_else(|| format!("{}px", fmt_string(layout.font_size)));

    format!(
        ".ishikawa .ishikawa-spine,.ishikawa .ishikawa-branch,.ishikawa .ishikawa-sub-branch {{ stroke: {line_color}; stroke-width: 2; fill: none; }}\
.ishikawa .ishikawa-sub-branch {{ stroke-width: 1; }}\
.ishikawa .ishikawa-arrow {{ fill: {line_color}; }}\
.ishikawa .ishikawa-head {{ fill: {main_bkg}; stroke: {line_color}; stroke-width: 2; }}\
.ishikawa .ishikawa-label-box {{ fill: {main_bkg}; stroke: {line_color}; stroke-width: 2; }}\
.ishikawa text {{ font-family: {font_family}; font-size: {font_size}; fill: {text_color}; }}\
.ishikawa .ishikawa-head-label {{ font-weight: 600; text-anchor: middle; dominant-baseline: middle; font-size: 14px; }}\
.ishikawa .ishikawa-label {{ text-anchor: end; }}\
.ishikawa .ishikawa-label.cause {{ text-anchor: middle; dominant-baseline: middle; }}\
.ishikawa .ishikawa-label.align {{ text-anchor: end; dominant-baseline: middle; }}\
.ishikawa .ishikawa-label.up {{ dominant-baseline: baseline; }}\
.ishikawa .ishikawa-label.down {{ dominant-baseline: hanging; }}",
        line_color = theme.line_color,
        main_bkg = theme.main_bkg,
        font_family = theme.font_family,
        text_color = theme.text_color
    )
}
