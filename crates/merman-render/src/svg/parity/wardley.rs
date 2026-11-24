use super::*;
use crate::wardley::{
    WardleyAnnotationsBoxLayout, WardleyArrowLayout, WardleyCircleLayout, WardleyDiagramLayout,
    WardleyDominantBaseline, WardleyFontWeight, WardleyLineLayout, WardleyNodeShapeLayout,
    WardleyPaintBinding, WardleySourceOverlayLayout, WardleyStyle, WardleyTextAnchor,
    WardleyTextLayout, WardleyTextRole,
};
use merman_model::families::wardley::WardleyDiagramModel;

fn text_anchor(anchor: WardleyTextAnchor) -> &'static str {
    match anchor {
        WardleyTextAnchor::Start => "start",
        WardleyTextAnchor::Middle => "middle",
    }
}

fn font_weight(weight: WardleyFontWeight) -> &'static str {
    match weight {
        WardleyFontWeight::Normal => "normal",
        WardleyFontWeight::Bold => "bold",
    }
}

fn dominant_baseline(baseline: WardleyDominantBaseline) -> &'static str {
    match baseline {
        WardleyDominantBaseline::Auto => "auto",
        WardleyDominantBaseline::Middle => "middle",
        WardleyDominantBaseline::Central => "central",
    }
}

struct WardleyTextStyle<'a> {
    class: Option<&'a str>,
    fill: &'a str,
    include_font_weight: bool,
}

fn write_text(
    out: &mut impl SvgOutput,
    text: &WardleyTextLayout,
    style: WardleyTextStyle<'_>,
    role: WardleyTextRole,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let fallback_size;
    let font_size = match wardley_style.font_size(role) {
        Some(size) => size,
        None => {
            fallback_size = crate::wardley::WardleyFontSize::numeric(text.font_size);
            &fallback_size
        }
    };
    out.push_str("<text");
    if let Some(class) = style.class {
        let _ = write!(out, r#" class="{}""#, escape_attr_display(class));
    }
    let _ = write!(
        out,
        r#" x="{}" y="{}" fill="{}" font-size="{}""#,
        ryu_js::Buffer::new().format(text.x),
        ryu_js::Buffer::new().format(text.y),
        escape_attr_display(style.fill),
        escape_attr_display(&font_size.css)
    );
    if role == WardleyTextRole::Title
        && let Some(family) = wardley_style.native_title_family()
    {
        let _ = write!(out, r#" font-family="{}""#, escape_attr_display(family));
    }
    if let Some(weight) = wardley_style.native_font_weight(role) {
        let _ = write!(out, r#" font-weight="{weight}""#);
    } else if style.include_font_weight {
        let _ = write!(out, r#" font-weight="{}""#, font_weight(text.font_weight));
    }
    let _ = write!(out, r#" text-anchor="{}""#, text_anchor(text.text_anchor));
    if let Some(baseline) = text.dominant_baseline {
        let _ = write!(
            out,
            r#" dominant-baseline="{}""#,
            dominant_baseline(baseline)
        );
    }
    if let Some(rotation) = text.rotation {
        let _ = write!(
            out,
            r#" transform="rotate({} {} {})""#,
            fmt(rotation.degrees),
            fmt(rotation.cx),
            fmt(rotation.cy)
        );
    }
    let _ = write!(out, ">{}</text>", escape_xml_display(&text.text));
    out.checkpoint()
}

fn write_line(
    out: &mut impl SvgOutput,
    line: WardleyLineLayout,
    class: Option<&str>,
    stroke: &str,
    stroke_width: Option<f64>,
    dash: Option<&str>,
) -> Result<()> {
    out.push_str("<line");
    if let Some(class) = class {
        let _ = write!(out, r#" class="{}""#, escape_attr_display(class));
    }
    let _ = write!(
        out,
        r#" x1="{}" x2="{}" y1="{}" y2="{}" stroke="{}""#,
        fmt(line.x1),
        fmt(line.x2),
        fmt(line.y1),
        fmt(line.y2),
        escape_attr_display(stroke)
    );
    if let Some(stroke_width) = stroke_width {
        let _ = write!(out, r#" stroke-width="{}""#, fmt(stroke_width));
    }
    if let Some(dash) = dash {
        let _ = write!(out, r#" stroke-dasharray="{}""#, escape_attr_display(dash));
    }
    out.push_str("/>");
    out.checkpoint()
}

fn write_circle(
    out: &mut impl SvgOutput,
    class: Option<&str>,
    circle: WardleyCircleLayout,
    fill: &str,
    stroke: &str,
    stroke_width: f64,
) -> Result<()> {
    out.push_str("<circle");
    if let Some(class) = class {
        let _ = write!(out, r#" class="{}""#, escape_attr_display(class));
    }
    let _ = write!(
        out,
        r#" cx="{}" cy="{}" r="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
        fmt(circle.center.x),
        fmt(circle.center.y),
        fmt(circle.radius),
        escape_attr_display(fill),
        escape_attr_display(stroke),
        fmt(stroke_width)
    );
    out.checkpoint()
}

fn write_accessibility(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    acc_title: Option<&str>,
    acc_descr: Option<&str>,
) -> Result<()> {
    if let Some(title) = acc_title {
        let _ = write!(
            out,
            r#"<title id="chart-title-{}">{}</title>"#,
            escape_attr_display(diagram_id),
            escape_xml_display(title)
        );
        out.checkpoint()?;
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            escape_attr_display(diagram_id),
            escape_xml_display(description)
        );
        out.checkpoint()?;
    }
    Ok(())
}

fn write_axes(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    out.push_str(r#"<g class="wardley-axes">"#);
    out.checkpoint()?;
    write_line(
        out,
        layout.axes.x_axis,
        None,
        &theme.axis_color,
        Some(theme.connector_width),
        None,
    )?;
    write_line(
        out,
        layout.axes.y_axis,
        None,
        &theme.axis_color,
        Some(theme.connector_width),
        None,
    )?;
    write_text(
        out,
        &layout.axes.x_label,
        WardleyTextStyle {
            class: Some("wardley-axis-label wardley-axis-label-x"),
            fill: &theme.axis_text_color,
            include_font_weight: true,
        },
        WardleyTextRole::AxisLabel,
        wardley_style,
    )?;
    write_text(
        out,
        &layout.axes.y_label,
        WardleyTextStyle {
            class: Some("wardley-axis-label wardley-axis-label-y"),
            fill: &theme.axis_text_color,
            include_font_weight: true,
        },
        WardleyTextRole::AxisLabel,
        wardley_style,
    )?;
    out.push_str("</g>");
    out.checkpoint()
}

fn write_stages(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    if layout.stages.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-stages">"#);
    out.checkpoint()?;
    for stage in &layout.stages {
        if let Some(divider) = stage.divider {
            write_line(
                out,
                divider,
                None,
                &theme.stage_divider_color,
                Some(1.0),
                Some("5 5"),
            )?;
            let insert_at = out.len() - 2;
            out.replace_range(insert_at..insert_at, r#" opacity="0.8""#)?;
        }
        write_text(
            out,
            &stage.label,
            WardleyTextStyle {
                class: Some("wardley-stage-label"),
                fill: &theme.axis_text_color,
                include_font_weight: false,
            },
            WardleyTextRole::StageLabel,
            wardley_style,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_grid(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &WardleyPaintBinding,
) -> Result<()> {
    if layout.grid.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-grid">"#);
    out.checkpoint()?;
    for grid in &layout.grid {
        write_line(
            out,
            grid.vertical,
            None,
            &theme.grid_color,
            None,
            Some("2 6"),
        )?;
        write_line(
            out,
            grid.horizontal,
            None,
            &theme.grid_color,
            None,
            Some("2 6"),
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_pipelines(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    model: &WardleyDiagramModel,
    theme: &WardleyPaintBinding,
) -> Result<()> {
    if model.pipelines.is_empty() {
        return Ok(());
    }

    out.push_str(r#"<g class="wardley-pipelines">"#);
    out.checkpoint()?;
    for pipeline in &layout.pipeline_boxes {
        let rect = pipeline.rect;
        let _ = write!(
            out,
            r#"<rect class="wardley-pipeline-box" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="1.5" rx="{}" ry="{}"/>"#,
            fmt(rect.x),
            fmt(rect.y),
            fmt(rect.width),
            fmt(rect.height),
            escape_attr_display(&theme.axis_color),
            fmt(rect.corner_radius),
            fmt(rect.corner_radius)
        );
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str(r#"<g class="wardley-pipeline-links">"#);
    out.checkpoint()?;
    for link in &layout.pipeline_links {
        write_line(
            out,
            link.line,
            Some("wardley-pipeline-evolution-link"),
            &theme.link_stroke,
            Some(theme.connector_width),
            Some("4 4"),
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_links(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    diagram_id: SvgDiagramId<'_>,
    options: &SvgExecution<'_>,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    out.push_str(r#"<g class="wardley-links">"#);
    out.checkpoint()?;
    for link in &layout.links {
        options.checkpoint_emit()?;
        let class = if link.dashed {
            "wardley-link wardley-link--dashed"
        } else {
            "wardley-link"
        };
        write_line(
            out,
            link.line,
            Some(class),
            &theme.link_stroke,
            Some(theme.connector_width),
            link.dashed.then_some("6 6"),
        )?;
        let insert_at = out.len() - 2;
        let mut marker_attrs = String::new();
        if link.markers.end {
            let _ = write!(
                marker_attrs,
                r#" marker-end="url(#link-arrow-end-{diagram_id})""#,
            );
        }
        if link.markers.start {
            let _ = write!(
                marker_attrs,
                r#" marker-start="url(#link-arrow-start-{diagram_id})""#,
            );
        }
        out.replace_range(insert_at..insert_at, &marker_attrs)?;
        options.checkpoint_emit()?;
    }
    for link in &layout.links {
        if let Some(label) = &link.label {
            write_text(
                out,
                label,
                WardleyTextStyle {
                    class: Some("wardley-link-label"),
                    fill: &theme.axis_text_color,
                    include_font_weight: false,
                },
                WardleyTextRole::LinkLabel,
                wardley_style,
            )?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_trends(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &WardleyPaintBinding,
    diagram_id: SvgDiagramId<'_>,
    options: &SvgExecution<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-trends">"#);
    out.checkpoint()?;
    for trend in &layout.trends {
        options.checkpoint_emit()?;
        write_line(
            out,
            trend.line,
            Some("wardley-trend"),
            &theme.evolution_stroke,
            Some(1.0),
            Some("4 4"),
        )?;
        let insert_at = out.len() - 2;
        let marker_attr = format!(r#" marker-end="url(#arrow-{diagram_id})""#);
        out.replace_range(insert_at..insert_at, &marker_attr)?;
        options.checkpoint_emit()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_source_overlay(
    out: &mut impl SvgOutput,
    overlay: &WardleySourceOverlayLayout,
    theme: &WardleyPaintBinding,
) -> Result<()> {
    let (fill, stroke) = match overlay {
        WardleySourceOverlayLayout::Build { .. } => (&theme.build_fill, &theme.build_stroke),
        WardleySourceOverlayLayout::Buy { .. } => (&theme.buy_fill, &theme.component_stroke),
        WardleySourceOverlayLayout::Outsource { .. } => {
            (&theme.outsource_fill, &theme.component_stroke)
        }
        WardleySourceOverlayLayout::Market { .. } => (&theme.market_fill, &theme.component_stroke),
    };
    match overlay {
        WardleySourceOverlayLayout::Build { circle } => write_circle(
            out,
            Some("wardley-build-overlay"),
            *circle,
            fill,
            stroke,
            1.0,
        ),
        WardleySourceOverlayLayout::Buy { circle } => {
            write_circle(out, Some("wardley-buy-overlay"), *circle, fill, stroke, 1.0)
        }
        WardleySourceOverlayLayout::Outsource { circle } => write_circle(
            out,
            Some("wardley-outsource-overlay"),
            *circle,
            fill,
            stroke,
            1.0,
        ),
        WardleySourceOverlayLayout::Market {
            outer_circle,
            connectors,
            dots,
        } => {
            write_circle(
                out,
                Some("wardley-market-overlay"),
                *outer_circle,
                fill,
                stroke,
                1.0,
            )?;
            for connector in connectors {
                write_line(
                    out,
                    *connector,
                    Some("wardley-market-line"),
                    stroke,
                    Some(1.0),
                    None,
                )?;
            }
            for dot in dots {
                write_circle(out, Some("wardley-market-dot"), *dot, fill, stroke, 2.0)?;
            }
            Ok(())
        }
    }
}

fn write_nodes(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    out.push_str(r#"<g class="wardley-nodes">"#);
    out.checkpoint()?;
    for node in &layout.nodes {
        out.push_str(r#"<g class="wardley-node"#);
        if let Some(class_name) = node.class_name.as_deref().filter(|class| !class.is_empty()) {
            let _ = write!(out, " wardley-node--{}", escape_attr_display(class_name));
        }
        out.push_str(r#"">"#);
        out.checkpoint()?;

        if let Some(overlay) = &node.source_overlay {
            write_source_overlay(out, overlay, theme)?;
        }
        let fill = &theme.component_fill;
        let stroke = &theme.component_stroke;
        match &node.shape {
            WardleyNodeShapeLayout::Circle { circle } => {
                write_circle(out, None, *circle, fill, stroke, 1.0)?
            }
            WardleyNodeShapeLayout::PipelineSquare { rect } => {
                let _ = write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
                    fmt(rect.x),
                    fmt(rect.y),
                    fmt(rect.width),
                    fmt(rect.height),
                    escape_attr_display(fill),
                    escape_attr_display(stroke)
                );
                out.checkpoint()?;
            }
            WardleyNodeShapeLayout::Anchor | WardleyNodeShapeLayout::None => {}
        }
        if let Some(inertia) = node.inertia {
            write_line(
                out,
                inertia,
                Some("wardley-inertia"),
                &theme.component_stroke,
                Some(6.0),
                None,
            )?;
        }
        let (label_fill, label_role) = match node.class_name.as_deref() {
            Some("evolved") => (&theme.evolution_stroke, WardleyTextRole::EvolvedLabel),
            Some("anchor") => (&theme.anchor_label_color, WardleyTextRole::AnchorLabel),
            _ => (&theme.component_label_color, WardleyTextRole::NodeLabel),
        };
        write_text(
            out,
            &node.label_layout,
            WardleyTextStyle {
                class: Some("wardley-node-label"),
                fill: label_fill,
                include_font_weight: true,
            },
            label_role,
            wardley_style,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_annotations_box(
    out: &mut impl SvgOutput,
    annotations_box: &WardleyAnnotationsBoxLayout,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    out.push_str(r#"<g class="wardley-annotations-box">"#);
    out.checkpoint()?;
    if let Some(rect) = annotations_box.rect {
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1.5" rx="{}" ry="{}"/>"#,
            fmt(rect.x),
            fmt(rect.y),
            fmt(rect.width),
            fmt(rect.height),
            escape_attr_display(&theme.annotation_fill),
            escape_attr_display(&theme.axis_color),
            fmt(rect.corner_radius),
            fmt(rect.corner_radius)
        );
        out.checkpoint()?;
    }
    for line in &annotations_box.lines {
        write_text(
            out,
            line,
            WardleyTextStyle {
                class: None,
                fill: &theme.annotation_text,
                include_font_weight: false,
            },
            WardleyTextRole::AnnotationBoxLine,
            wardley_style,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_annotations(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    if layout.annotations.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-annotations">"#);
    out.checkpoint()?;
    for annotation in &layout.annotations {
        for segment in &annotation.segments {
            write_line(
                out,
                *segment,
                Some("wardley-annotation-line"),
                &theme.axis_color,
                Some(1.5),
                Some("4 4"),
            )?;
        }
        for point in &annotation.points {
            out.push_str(r#"<g class="wardley-annotation">"#);
            out.checkpoint()?;
            write_circle(
                out,
                None,
                WardleyCircleLayout {
                    center: point.center,
                    radius: point.radius,
                },
                &theme.annotation_fill,
                &theme.axis_color,
                1.5,
            )?;
            write_text(
                out,
                &point.label,
                WardleyTextStyle {
                    class: None,
                    fill: &theme.annotation_text,
                    include_font_weight: true,
                },
                WardleyTextRole::AnnotationPoint,
                wardley_style,
            )?;
            out.push_str("</g>");
            out.checkpoint()?;
        }
    }
    if let Some(annotations_box) = &layout.annotations_box {
        write_annotations_box(out, annotations_box, wardley_style)?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_notes(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    if layout.notes.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-notes">"#);
    out.checkpoint()?;
    for note in &layout.notes {
        write_text(
            out,
            &note.text,
            WardleyTextStyle {
                class: None,
                fill: &theme.axis_text_color,
                include_font_weight: true,
            },
            WardleyTextRole::Note,
            wardley_style,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_arrow(
    out: &mut impl SvgOutput,
    arrow: &WardleyArrowLayout,
    role: WardleyTextRole,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    let theme = wardley_style.paint_binding();
    out.push_str(r#"<path d="M "#);
    out.checkpoint()?;
    for (index, point) in arrow.path.iter().enumerate() {
        if index > 0 {
            out.push_str(" L ");
        }
        let _ = write!(out, "{} {}", fmt(point.x), fmt(point.y));
        out.checkpoint()?;
    }
    let _ = write!(
        out,
        r#" Z" fill="{}" stroke="{}" stroke-width="1"/>"#,
        escape_attr_display(&theme.arrow_fill),
        escape_attr_display(&theme.component_stroke)
    );
    out.checkpoint()?;
    write_text(
        out,
        &arrow.label,
        WardleyTextStyle {
            class: None,
            fill: &theme.arrow_text,
            include_font_weight: true,
        },
        role,
        wardley_style,
    )
}

fn write_arrows(
    out: &mut impl SvgOutput,
    class: &str,
    arrows: &[WardleyArrowLayout],
    role: WardleyTextRole,
    wardley_style: &WardleyStyle,
) -> Result<()> {
    if arrows.is_empty() {
        return Ok(());
    }
    let _ = write!(out, r#"<g class="{}">"#, escape_attr_display(class));
    out.checkpoint()?;
    for arrow in arrows {
        write_arrow(out, arrow, role, wardley_style)?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_marker_path(out: &mut impl SvgOutput, path: &str, fill: &str) -> Result<()> {
    let _ = write!(
        out,
        r#"<path d="{}" fill="{}" stroke="none"/>"#,
        path,
        escape_attr_display(fill)
    );
    out.checkpoint()
}

fn write_defs(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    theme: &WardleyPaintBinding,
) -> Result<()> {
    let diagram_id = escape_attr_display(diagram_id);
    let _ = write!(
        out,
        r#"<defs><marker id="arrow-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">"#
    );
    write_marker_path(out, "M 0 0 L 10 5 L 0 10 z", &theme.evolution_stroke)?;
    let _ = write!(
        out,
        r#"</marker><marker id="link-arrow-end-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">"#
    );
    write_marker_path(out, "M 0 0 L 10 5 L 0 10 z", &theme.link_stroke)?;
    let _ = write!(
        out,
        r#"</marker><marker id="link-arrow-start-{diagram_id}" viewBox="0 0 10 10" refX="1" refY="5" markerWidth="5" markerHeight="5" orient="auto">"#
    );
    write_marker_path(out, "M 10 0 L 0 5 L 10 10 z", &theme.link_stroke)?;
    out.push_str("</marker></defs>");
    out.checkpoint()
}

pub(crate) fn render_wardley_diagram_svg_model(
    layout: &WardleyDiagramLayout,
    model: &WardleyDiagramModel,
    wardley_style: &WardleyStyle,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("wardley");
    let acc_title = model.acc_title.as_deref().filter(|value| !value.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let theme = wardley_style.paint_binding();

    let root_bounds = root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "wardley");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::WARDLEY, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;
    write_accessibility(&mut out, diagram_id, acc_title, acc_descr)?;

    if wardley_style.should_write_inherited_font() {
        let _ = write!(
            &mut out,
            r#"<g class="wardley-map" font-family="{}">"#,
            escape_attr_display(wardley_style.font_family_css())
        );
        out.checkpoint()?;
    } else {
        out.push_str(r#"<g class="wardley-map">"#);
        out.checkpoint()?;
    }
    if !wardley_style.is_native() && options.family.session().canvas_background().is_none() {
        let _ = write!(
            out,
            r#"<rect class="wardley-background" width="{}" height="{}" fill="{}"/>"#,
            fmt(layout.width),
            fmt(layout.height),
            escape_attr_display(&theme.background_color)
        );
        out.checkpoint()?;
    }
    if let Some(title) = &layout.title {
        write_text(
            &mut out,
            title,
            WardleyTextStyle {
                class: Some("wardley-title"),
                fill: wardley_style.title_color(),
                include_font_weight: true,
            },
            WardleyTextRole::Title,
            wardley_style,
        )?;
    }
    write_axes(&mut out, layout, wardley_style)?;
    write_stages(&mut out, layout, wardley_style)?;
    write_grid(&mut out, layout, theme)?;
    write_pipelines(&mut out, layout, model, theme)?;
    write_links(&mut out, layout, diagram_id, options, wardley_style)?;
    write_trends(&mut out, layout, theme, diagram_id, options)?;
    write_nodes(&mut out, layout, wardley_style)?;
    write_annotations(&mut out, layout, wardley_style)?;
    write_notes(&mut out, layout, wardley_style)?;
    write_arrows(
        &mut out,
        "wardley-accelerators",
        &layout.accelerators,
        WardleyTextRole::Accelerator,
        wardley_style,
    )?;
    write_arrows(
        &mut out,
        "wardley-deaccelerators",
        &layout.deaccelerators,
        WardleyTextRole::Deaccelerator,
        wardley_style,
    )?;
    out.push_str("</g>");
    out.checkpoint()?;
    write_defs(&mut out, diagram_id, theme)?;
    out.push_str("</svg>");
    options.checkpoint_emit()?;
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn wardley_evolved_model_label_keeps_its_source_evolution_paint() {
        use crate::theme::{FamilyThemeAdapter, ThemeCompiler};
        use crate::wardley::WardleyThemeAdapter;

        let native = ThemeCompiler::new()
            .compile_json(br#"{"schema_version":2,"extends":"blueprint"}"#, None)
            .unwrap();
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": { "wardley": { "evolutionStroke": "#ab1234" } }
        }));
        let style = WardleyStyle::resolve(&config, Some(WardleyThemeAdapter::resolve(&native)));
        let model: WardleyDiagramModel = serde_json::from_value(serde_json::json!({
            "nodes": [{
                "id": "evolved", "label": "Evolved API", "x": 0.8, "y": 0.6,
                "className": "evolved"
            }]
        }))
        .unwrap();
        let layout = crate::wardley::layout_wardley_diagram_typed_with_style(
            &model,
            None,
            config.as_value(),
            &style,
            &DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let mut svg = String::new();
        write_nodes(&mut svg, &layout, &style).unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.attribute("fill"), Some("#ab1234"));
    }

    fn render_wardley_with_policy(
        policy: RenderResourcePolicy,
    ) -> crate::Result<root_svg::RootedSvg> {
        let model = WardleyDiagramModel::default();
        let effective_config = serde_json::json!({});
        let effective_mermaid_config =
            crate::test_support::presentation_config(effective_config.clone());
        let style = WardleyStyle::resolve(&effective_mermaid_config, None);
        let layout = crate::wardley::layout_wardley_diagram_typed(
            &model,
            None,
            &effective_config,
            &DeterministicTextMeasurer::default(),
        )?;
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("render session");
        let request = SvgRenderOptions {
            diagram_id: Some("wardley-bounded".to_string()),
            ..Default::default()
        };
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::WARDLEY,
        )
        .expect("SVG execution");

        render_wardley_diagram_svg_model(&layout, &model, &style, &execution)
    }

    #[test]
    fn wardley_svg_sink_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
        let baseline =
            render_wardley_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
                .expect("render unbounded Wardley SVG");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1);

        let exact = render_wardley_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
                .expect("valid exact Wardley SVG ceiling"),
        )
        .expect("exact Wardley SVG sink ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let error = render_wardley_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
                .expect("valid below-exact Wardley SVG ceiling"),
        )
        .expect_err("one byte below the Wardley SVG sink size must fail");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Wardley MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, below_exact);
        assert!(limit.actual > limit.max);
        assert!(limit.explicit_overrides.iter().any(|resource_override| {
            resource_override.id == ResourceLimitId::MaxSvgBytes
                && resource_override.value == below_exact
        }));
    }
}
