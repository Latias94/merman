use super::super::roughjs_common::{ops_to_svg_path_d, parse_hex_color_to_srgba};
use super::super::*;
use crate::ishikawa::{
    IshikawaTextThemePlan, IshikawaTextThemeReceipt, IshikawaTypographyTerminal,
};
use crate::model::{
    IshikawaBranchLayout, IshikawaCauseLabelGroupLayout, IshikawaLabelBoxLayout,
    IshikawaLineLayout, IshikawaSubGroupLayout, IshikawaTextLayout,
};

struct RoughContext {
    randomness: roughr::core::RoughRandomness,
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

pub(crate) fn render_ishikawa_diagram_svg_with_theme(
    layout: &IshikawaDiagramLayout,
    text_theme: &IshikawaTextThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("ishikawa");
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut text_terminals = IshikawaTextTerminalWriter::new(text_theme);
    let theme = text_theme.css_binding();
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        layout.viewbox_x,
        layout.viewbox_y,
        layout.total_width,
        layout.total_height,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "ishikawa");
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::ISHIKAWA, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

    out.push_str("<style>");
    write_ishikawa_css(
        &mut out,
        super::super::util::css_selector_diagram_id(diagram_id),
        theme,
        &mut text_terminals.receipt,
    );
    out.push_str(r#"</style><g/><g class="ishikawa">"#);
    out.checkpoint()?;
    if theme.look == "handDrawn" {
        let rough = RoughContext {
            randomness: options.rough_randomness(
                theme.hand_drawn_seed.unwrap_or(options.seed() as f64),
                "render.ishikawa.roughjs",
            ),
            line_color: theme.line_color.clone(),
            fill_color: theme.main_bkg.clone(),
        };
        push_hand_drawn_diagram(&mut out, layout, &rough, &mut text_terminals)?;
    } else {
        let marker_id = format!("ishikawa-arrow-{diagram_id}");
        push_classic_diagram(&mut out, layout, &marker_id, options, &mut text_terminals)?;
    }

    out.push_str("</g></svg>\n");
    out.checkpoint()?;
    let rooted = root_document.complete(out.finish()?)?;
    if !text_theme.record_terminal(text_terminals.into_receipt()) {
        return Err(Error::InvalidModel {
            message: "Ishikawa text styling receipt did not match the terminal SVG".to_string(),
        });
    }
    Ok(rooted)
}

fn push_classic_diagram(
    out: &mut impl SvgOutput,
    layout: &IshikawaDiagramLayout,
    marker_id: &str,
    options: &SvgExecution<'_>,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
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
        text_terminals.push_head_text(out, &head.label, -head.x, -head.y)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    for pair in &layout.pairs {
        options.checkpoint_emit()?;
        out.push_str(r#"<g class="ishikawa-pair">"#);
        push_branch(out, &pair.upper, marker_id, text_terminals)?;
        if let Some(lower) = &pair.lower {
            push_branch(out, lower, marker_id, text_terminals)?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    Ok(())
}

fn push_hand_drawn_diagram(
    out: &mut impl SvgOutput,
    layout: &IshikawaDiagramLayout,
    rough: &RoughContext,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
) -> Result<()> {
    if let Some(head) = &layout.head {
        let _ = write!(
            out,
            r#"<g class="ishikawa-head-group" transform="translate({}, {})">"#,
            fmt(head.x),
            fmt(head.y)
        );
        push_rough_hachure_path(out, "ishikawa-head", &head.path_d, rough)?;
        text_terminals.push_head_text(out, &head.label, -head.x, -head.y)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    for pair in &layout.pairs {
        out.push_str(r#"<g class="ishikawa-pair">"#);
        push_hand_drawn_branch(out, &pair.upper, rough, text_terminals)?;
        if let Some(lower) = &pair.lower {
            push_hand_drawn_branch(out, lower, rough, text_terminals)?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    if let Some(spine) = &layout.spine {
        push_rough_line(out, spine, rough)?;
    }
    Ok(())
}

fn push_branch(
    out: &mut impl SvgOutput,
    branch: &IshikawaBranchLayout,
    marker_id: &str,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
) -> Result<()> {
    push_line(out, &branch.line, marker_id)?;
    push_cause_label_group(out, &branch.label_group, text_terminals)?;
    for sub_group in &branch.sub_groups {
        push_sub_group(out, sub_group, marker_id, text_terminals)?;
    }
    Ok(())
}

fn push_cause_label_group(
    out: &mut impl SvgOutput,
    group: &IshikawaCauseLabelGroupLayout,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
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
    text_terminals.push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_sub_group(
    out: &mut impl SvgOutput,
    group: &IshikawaSubGroupLayout,
    marker_id: &str,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="ishikawa-sub-group">"#);
    push_line(out, &group.line, marker_id)?;
    text_terminals.push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_hand_drawn_branch(
    out: &mut impl SvgOutput,
    branch: &IshikawaBranchLayout,
    rough: &RoughContext,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
) -> Result<()> {
    push_rough_line(out, &branch.line, rough)?;
    push_rough_arrow_marker(out, &branch.line, rough)?;
    push_hand_drawn_cause_label_group(out, &branch.label_group, rough, text_terminals)?;
    for sub_group in &branch.sub_groups {
        push_hand_drawn_sub_group(out, sub_group, rough, text_terminals)?;
    }
    Ok(())
}

fn push_hand_drawn_cause_label_group(
    out: &mut impl SvgOutput,
    group: &IshikawaCauseLabelGroupLayout,
    rough: &RoughContext,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="ishikawa-label-group">"#);
    let label_box = &group.label_box;
    push_rough_hachure_rect(out, "ishikawa-label-box", label_box, rough)?;
    text_terminals.push_text_with_offset(out, &group.label, 0.0, 0.0)?;
    out.push_str("</g>");
    out.checkpoint()
}

fn push_hand_drawn_sub_group(
    out: &mut impl SvgOutput,
    group: &IshikawaSubGroupLayout,
    rough: &RoughContext,
    text_terminals: &mut IshikawaTextTerminalWriter<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="ishikawa-sub-group">"#);
    push_rough_line(out, &group.line, rough)?;
    push_rough_arrow_marker(out, &group.line, rough)?;
    text_terminals.push_text_with_offset(out, &group.label, 0.0, 0.0)?;
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
    let options = roughr::core::OptionsBuilder::default()
        .randomness(rough.randomness.clone())
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
    )
}

fn push_rough_hachure_path(
    out: &mut impl SvgOutput,
    class_name: &str,
    path_d: &str,
    rough: &RoughContext,
) -> Result<()> {
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
    )
}

fn push_rough_hachure_rect(
    out: &mut impl SvgOutput,
    class_name: &str,
    label_box: &IshikawaLabelBoxLayout,
    rough: &RoughContext,
) -> Result<()> {
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

    let color = rough_color(&rough.line_color);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(rough.randomness.clone())
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
    )
}

fn rough_hachure_options(rough: &RoughContext) -> roughr::core::Options {
    roughr::core::OptionsBuilder::default()
        .randomness(rough.randomness.clone())
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

struct IshikawaTextTerminalWriter<'a> {
    plan: &'a IshikawaTextThemePlan,
    receipt: IshikawaTextThemeReceipt,
    next_visible_index: usize,
}

impl<'a> IshikawaTextTerminalWriter<'a> {
    fn new(plan: &'a IshikawaTextThemePlan) -> Self {
        Self {
            plan,
            receipt: plan.begin_terminal_receipt(),
            next_visible_index: 0,
        }
    }

    fn into_receipt(self) -> IshikawaTextThemeReceipt {
        self.receipt
    }

    fn push_head_text(
        &mut self,
        out: &mut impl SvgOutput,
        text: &IshikawaTextLayout,
        dx: f64,
        dy: f64,
    ) -> Result<()> {
        let emitted_index = self.visible_index(text);
        let transform_x = text.x + dx;
        let transform_y = text.y + dy;
        let first_y = -((text.lines.len().saturating_sub(1)) as f64 * text.line_height) / 2.0;
        let _ = write!(
            out,
            r#"<text class="{}" text-anchor="{}" x="{}" y="{}" transform="translate({},{})" data-merman-text-bbox="{},{},{},{}""#,
            escape_attr_display(&text.class_name),
            escape_attr_display(&text.anchor),
            fmt(0.0),
            fmt(first_y),
            fmt(transform_x),
            fmt(transform_y),
            fmt(text.bbox.min_x),
            fmt(text.bbox.min_y),
            fmt(text.bbox.max_x),
            fmt(text.bbox.max_y)
        );
        self.push_fill_style(out, emitted_index);
        out.push('>');
        for (idx, line) in text.lines.iter().enumerate() {
            let _ = write!(
                out,
                r#"<tspan x="{}" dy="{}">"#,
                fmt(0.0),
                if idx == 0 {
                    "0".to_string()
                } else {
                    fmt_string(text.line_height)
                }
            );
            escape_xml_into(out, line);
            out.push_str("</tspan>");
            out.checkpoint()?;
        }
        out.push_str("</text>");
        out.checkpoint()?;
        self.record_checkpointed_text(emitted_index, text, IshikawaTypographyTerminal::Head);
        Ok(())
    }

    fn push_text_with_offset(
        &mut self,
        out: &mut impl SvgOutput,
        text: &IshikawaTextLayout,
        dx: f64,
        dy: f64,
    ) -> Result<()> {
        let emitted_index = self.visible_index(text);
        let first_y =
            text.y + dy - ((text.lines.len().saturating_sub(1)) as f64 * text.line_height) / 2.0;
        let _ = write!(
            out,
            r#"<text class="{}" text-anchor="{}" x="{}" y="{}" data-merman-text-bbox="{},{},{},{}""#,
            escape_attr_display(&text.class_name),
            escape_attr_display(&text.anchor),
            fmt(text.x + dx),
            fmt(first_y),
            fmt(text.bbox.min_x),
            fmt(text.bbox.min_y),
            fmt(text.bbox.max_x),
            fmt(text.bbox.max_y)
        );
        self.push_fill_style(out, emitted_index);
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
                        fmt_string(text.line_height)
                    }
                );
                escape_xml_into(out, line);
                out.push_str("</tspan>");
                out.checkpoint()?;
            }
        }
        out.push_str("</text>");
        out.checkpoint()?;
        self.record_checkpointed_text(
            emitted_index,
            text,
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        Ok(())
    }

    fn visible_index(&self, text: &IshikawaTextLayout) -> Option<usize> {
        (!text.text.trim().is_empty()).then_some(self.next_visible_index)
    }

    fn push_fill_style(&self, out: &mut impl SvgOutput, emitted_index: Option<usize>) {
        let Some((_, fill)) = emitted_index.and_then(|index| self.plan.typed_fill(index)) else {
            return;
        };
        out.push_str(r#" style="fill:"#);
        escape_attr_into(out, fill);
        out.push_str(r#" !important;""#);
    }

    fn record_checkpointed_text(
        &mut self,
        emitted_index: Option<usize>,
        text: &IshikawaTextLayout,
        typography_terminal: IshikawaTypographyTerminal,
    ) {
        let Some(emitted_index) = emitted_index else {
            return;
        };
        let emitted_fill = self.plan.typed_fill(emitted_index);
        self.receipt.record_checkpointed_text(
            emitted_index,
            &text.class_name,
            emitted_fill,
            typography_terminal,
        );
        self.next_visible_index = self.next_visible_index.saturating_add(1);
    }
}

fn write_ishikawa_css(
    css: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
    theme: &crate::ishikawa::IshikawaCssBinding,
    text_receipt: &mut IshikawaTextThemeReceipt,
) {
    let _ = write!(
        css,
        "#{diagram_id} .ishikawa .ishikawa-spine,#{diagram_id} .ishikawa .ishikawa-branch,#{diagram_id} .ishikawa .ishikawa-sub-branch {{ stroke: {line_color}; stroke-width: 2; fill: none; }}\
#{diagram_id} .ishikawa .ishikawa-sub-branch {{ stroke-width: 1; }}\
#{diagram_id} .ishikawa .ishikawa-arrow {{ fill: {line_color}; }}\
#{diagram_id} .ishikawa .ishikawa-head {{ fill: {main_bkg}; stroke: {line_color}; stroke-width: 2; }}\
#{diagram_id} .ishikawa .ishikawa-label-box {{ fill: {main_bkg}; stroke: {line_color}; stroke-width: 2; }}",
        line_color = theme.line_color,
        main_bkg = theme.main_bkg,
    );
    text_receipt.write_text_rules(css, diagram_id, &theme.text_color);
    let _ = write!(
        css,
        "#{diagram_id} .ishikawa .ishikawa-label {{ text-anchor: end; }}\
#{diagram_id} .ishikawa .ishikawa-label.cause {{ text-anchor: middle; dominant-baseline: middle; }}\
#{diagram_id} .ishikawa .ishikawa-label.align {{ text-anchor: end; dominant-baseline: middle; }}\
#{diagram_id} .ishikawa .ishikawa-label.up {{ dominant-baseline: baseline; }}\
#{diagram_id} .ishikawa .ishikawa-label.down {{ dominant-baseline: hanging; }}"
    );
}
