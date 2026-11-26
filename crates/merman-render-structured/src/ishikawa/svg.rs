use merman_model::OperationPhase;
use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    rough_svg::{controlled, ops_to_svg_path_d, parse_hex_color_to_srgba},
    svg_output::SvgOutput,
    xml::{escape_attr_display, escape_attr_into, escape_xml_into},
};

use super::{
    IshikawaBranchLayout, IshikawaCauseLabelGroupLayout, IshikawaDiagramLayout,
    IshikawaLabelBoxLayout, IshikawaLineLayout, IshikawaSubGroupLayout, IshikawaTextLayout,
};

pub struct IshikawaRoughPaint<'a> {
    pub line_color: &'a str,
    pub sub_branch_stroke: &'a str,
    pub head_fill: &'a str,
    pub label_fill: &'a str,
    pub node_stroke: &'a str,
    pub marker_fill: &'a str,
    pub solid_head: bool,
}

struct RoughContext<'a> {
    randomness: roughr::core::RoughRandomness,
    paint: IshikawaRoughPaint<'a>,
    work_meter: &'a OperationWorkMeter,
}

#[derive(Clone, Copy)]
struct RoughPaint<'a> {
    fill_color: &'a str,
    stroke_color: &'a str,
    stroke_width: f32,
    fill_weight: f32,
}

pub fn write_classic_svg_body(
    out: &mut impl SvgOutput,
    layout: &IshikawaDiagramLayout,
    marker_id: &str,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let _ = write!(out, r#"<defs><marker id=""#);
    escape_attr_into(out, marker_id);
    out.push_str(
        r#"" viewBox="0 0 10 10" refX="0" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M 10 0 L 0 5 L 10 10 Z" class="ishikawa-arrow"></path></marker></defs>"#,
    );
    out.checkpoint()?;

    if let Some(spine) = &layout.spine {
        push_line(out, spine, marker_id)?;
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
        push_head_text(out, &head.label, -head.x, -head.y)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    for pair in &layout.pairs {
        work_meter.checkpoint(OperationPhase::Emit)?;
        out.push_str(r#"<g class="ishikawa-pair">"#);
        push_branch(out, &pair.upper, marker_id)?;
        if let Some(lower) = &pair.lower {
            push_branch(out, lower, marker_id)?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    Ok(())
}

pub fn write_hand_drawn_svg_body(
    out: &mut impl SvgOutput,
    layout: &IshikawaDiagramLayout,
    randomness: roughr::core::RoughRandomness,
    paint: IshikawaRoughPaint<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let rough = RoughContext {
        randomness,
        paint,
        work_meter,
    };

    if let Some(head) = &layout.head {
        let _ = write!(
            out,
            r#"<g class="ishikawa-head-group" transform="translate({}, {})">"#,
            fmt(head.x),
            fmt(head.y)
        );
        push_rough_head(out, &head.path_d, &rough)?;
        push_head_text(out, &head.label, -head.x, -head.y)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    for pair in &layout.pairs {
        out.push_str(r#"<g class="ishikawa-pair">"#);
        push_hand_drawn_branch(out, &pair.upper, &rough)?;
        if let Some(lower) = &pair.lower {
            push_hand_drawn_branch(out, lower, &rough)?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    if let Some(spine) = &layout.spine {
        push_rough_line(out, spine, &rough)?;
    }
    Ok(())
}

fn push_branch(
    out: &mut impl SvgOutput,
    branch: &IshikawaBranchLayout,
    marker_id: &str,
) -> Result<()> {
    push_line(out, &branch.line, marker_id)?;
    push_cause_label_group(out, &branch.label_group)?;
    for sub_group in &branch.sub_groups {
        push_sub_group(out, sub_group, marker_id)?;
    }
    Ok(())
}

fn push_cause_label_group(
    out: &mut impl SvgOutput,
    group: &IshikawaCauseLabelGroupLayout,
) -> Result<()> {
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
    push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_sub_group(
    out: &mut impl SvgOutput,
    group: &IshikawaSubGroupLayout,
    marker_id: &str,
) -> Result<()> {
    out.push_str(r#"<g class="ishikawa-sub-group">"#);
    push_line(out, &group.line, marker_id)?;
    push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_hand_drawn_branch(
    out: &mut impl SvgOutput,
    branch: &IshikawaBranchLayout,
    rough: &RoughContext,
) -> Result<()> {
    push_rough_line(out, &branch.line, rough)?;
    push_rough_arrow_marker(out, &branch.line, rough)?;
    push_hand_drawn_cause_label_group(out, &branch.label_group, rough)?;
    for sub_group in &branch.sub_groups {
        push_hand_drawn_sub_group(out, sub_group, rough)?;
    }
    Ok(())
}

fn push_hand_drawn_cause_label_group(
    out: &mut impl SvgOutput,
    group: &IshikawaCauseLabelGroupLayout,
    rough: &RoughContext,
) -> Result<()> {
    out.push_str(r#"<g class="ishikawa-label-group">"#);
    let label_box = &group.label_box;
    push_rough_hachure_rect(out, "ishikawa-label-box", label_box, rough)?;
    push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_hand_drawn_sub_group(
    out: &mut impl SvgOutput,
    group: &IshikawaSubGroupLayout,
    rough: &RoughContext,
) -> Result<()> {
    out.push_str(r#"<g class="ishikawa-sub-group">"#);
    push_rough_line(out, &group.line, rough)?;
    push_rough_arrow_marker(out, &group.line, rough)?;
    push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_line(out: &mut impl SvgOutput, line: &IshikawaLineLayout, marker_id: &str) -> Result<()> {
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
    out.checkpoint()
}

fn push_rough_line(
    out: &mut impl SvgOutput,
    line: &IshikawaLineLayout,
    rough: &RoughContext,
) -> Result<()> {
    let stroke = if line.class_name == "ishikawa-sub-branch" {
        rough.paint.sub_branch_stroke
    } else {
        rough.paint.line_color
    };
    let options = roughr::core::OptionsBuilder::default()
        .randomness(rough.randomness.clone())
        .roughness(1.5)
        .stroke(rough_color(stroke))
        .stroke_width(2.0)
        .build()
        .expect("static Ishikawa rough line options must be valid");
    let drawable = controlled::generate(
        rough.work_meter,
        merman_model::OperationPhase::Emit,
        |control| {
            roughr::generator::Generator::default().line_with_control::<f64>(
                line.x1,
                line.y1,
                line.x2,
                line.y2,
                &Some(options),
                control,
            )
        },
    )?;
    push_rough_group(
        out,
        Some(&line.class_name),
        drawable.sets,
        RoughPaint {
            fill_color: stroke,
            stroke_color: stroke,
            stroke_width: 2.0,
            fill_weight: 0.0,
        },
    )
}

fn push_rough_head(out: &mut impl SvgOutput, path_d: &str, rough: &RoughContext) -> Result<()> {
    let fill = rough.paint.head_fill;
    let fill_style = if rough.paint.solid_head {
        roughr::core::FillStyle::Solid
    } else {
        roughr::core::FillStyle::Hachure
    };
    let drawable = controlled::generate(
        rough.work_meter,
        merman_model::OperationPhase::Emit,
        |control| {
            roughr::generator::Generator::default().path_with_control::<f64>(
                path_d,
                &Some(rough_shape_options(rough, fill, fill_style)),
                control,
            )
        },
    )?;
    push_rough_group(
        out,
        Some("ishikawa-head"),
        drawable.sets,
        RoughPaint {
            fill_color: fill,
            stroke_color: rough.paint.node_stroke,
            stroke_width: 2.0,
            fill_weight: 2.5,
        },
    )
}

fn push_rough_hachure_rect(
    out: &mut impl SvgOutput,
    class_name: &str,
    label_box: &IshikawaLabelBoxLayout,
    rough: &RoughContext,
) -> Result<()> {
    let fill = rough.paint.label_fill;
    let drawable = controlled::generate(
        rough.work_meter,
        merman_model::OperationPhase::Emit,
        |control| {
            roughr::generator::Generator::default().rectangle_with_control::<f64>(
                label_box.x,
                label_box.y,
                label_box.width,
                label_box.height,
                &Some(rough_shape_options(
                    rough,
                    fill,
                    roughr::core::FillStyle::Hachure,
                )),
                control,
            )
        },
    )?;
    push_rough_group(
        out,
        Some(class_name),
        drawable.sets,
        RoughPaint {
            fill_color: fill,
            stroke_color: rough.paint.node_stroke,
            stroke_width: 2.0,
            fill_weight: 2.5,
        },
    )
}

fn push_rough_arrow_marker(
    out: &mut impl SvgOutput,
    line: &IshikawaLineLayout,
    rough: &RoughContext,
) -> Result<()> {
    if !line.marker_start {
        return Ok(());
    }
    let dx = line.x1 - line.x2;
    let dy = line.y1 - line.y2;
    let len = dx.hypot(dy);
    if len == 0.0 {
        return Ok(());
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

    let color = rough_color(rough.paint.marker_fill);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(rough.randomness.clone())
        .roughness(1.0)
        .fill(color)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(color)
        .stroke_width(1.0)
        .build()
        .expect("static Ishikawa rough arrow options must be valid");
    let drawable = controlled::generate(
        rough.work_meter,
        merman_model::OperationPhase::Emit,
        |control| {
            roughr::generator::Generator::default().path_with_control::<f64>(
                &path_d,
                &Some(options),
                control,
            )
        },
    )?;
    push_rough_group(
        out,
        None,
        drawable.sets,
        RoughPaint {
            fill_color: rough.paint.marker_fill,
            stroke_color: rough.paint.marker_fill,
            stroke_width: 1.0,
            fill_weight: 0.0,
        },
    )
}

fn rough_shape_options(
    rough: &RoughContext,
    fill_color: &str,
    fill_style: roughr::core::FillStyle,
) -> roughr::core::Options {
    roughr::core::OptionsBuilder::default()
        .randomness(rough.randomness.clone())
        .roughness(1.5)
        .fill(rough_color(fill_color))
        .fill_style(fill_style)
        .fill_weight(2.5)
        .hachure_gap(5.0)
        .stroke(rough_color(rough.paint.node_stroke))
        .stroke_width(2.0)
        .build()
        .expect("static Ishikawa rough hachure options must be valid")
}

fn rough_color(css: &str) -> roughr::Srgba {
    parse_hex_color_to_srgba(css).unwrap_or_else(|| roughr::Srgba::new(0.0, 0.0, 0.0, 1.0))
}

fn push_rough_group(
    out: &mut impl SvgOutput,
    class_name: Option<&str>,
    sets: Vec<roughr::core::OpSet<f64>>,
    paint: RoughPaint<'_>,
) -> Result<()> {
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
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn push_head_text(
    out: &mut impl SvgOutput,
    text: &IshikawaTextLayout,
    dx: f64,
    dy: f64,
) -> Result<()> {
    let transform_x = text.x + dx;
    let transform_y = text.y + dy;
    let first_y = -((text.lines.len().saturating_sub(1)) as f64 * text.line_height) / 2.0;
    let _ = write!(
        out,
        r#"<text class="{}" text-anchor="{}" x="{}" y="{}" transform="translate({},{})""#,
        escape_attr_display(&text.class_name),
        escape_attr_display(&text.anchor),
        fmt(0.0),
        fmt(first_y),
        fmt(transform_x),
        fmt(transform_y)
    );
    out.push('>');
    for (idx, line) in text.lines.iter().enumerate() {
        let _ = write!(
            out,
            r#"<tspan x="{}" dy="{}">"#,
            fmt(0.0),
            if idx == 0 {
                "0".to_string()
            } else {
                fmt(text.line_height).to_string()
            }
        );
        escape_xml_into(out, line);
        out.push_str("</tspan>");
        out.checkpoint()?;
    }
    out.push_str("</text>");
    out.checkpoint()?;
    Ok(())
}

fn push_text_with_offset(
    out: &mut impl SvgOutput,
    text: &IshikawaTextLayout,
    dx: f64,
    dy: f64,
) -> Result<()> {
    let first_y =
        text.y + dy - ((text.lines.len().saturating_sub(1)) as f64 * text.line_height) / 2.0;
    let _ = write!(
        out,
        r#"<text class="{}" text-anchor="{}" x="{}" y="{}""#,
        escape_attr_display(&text.class_name),
        escape_attr_display(&text.anchor),
        fmt(text.x + dx),
        fmt(first_y)
    );
    out.push('>');
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
                    fmt(text.line_height).to_string()
                }
            );
            escape_xml_into(out, line);
            out.push_str("</tspan>");
            out.checkpoint()?;
        }
    }
    out.push_str("</text>");
    out.checkpoint()?;
    Ok(())
}
