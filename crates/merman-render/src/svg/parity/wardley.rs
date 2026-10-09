use super::*;
use crate::wardley::{
    WardleyAnnotationsBoxLayout, WardleyArrowLayout, WardleyCircleLayout, WardleyDiagramLayout,
    WardleyDominantBaseline, WardleyFontWeight, WardleyLineLayout, WardleyNodeShapeLayout,
    WardleySourceOverlayLayout, WardleyTextAnchor, WardleyTextLayout,
};
use merman_core::diagrams::wardley::WardleyDiagramRenderModel;

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

fn write_text(
    out: &mut impl SvgOutput,
    text: &WardleyTextLayout,
    class: Option<&str>,
    fill: &str,
    include_font_weight: bool,
    role: crate::wardley::WardleyTextRole,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
    out.push_str("<text");
    if let Some(class) = class {
        let _ = write!(out, r#" class="{}""#, escape_attr_display(class));
    }
    let _ = write!(
        out,
        r#" x="{}" y="{}" fill="{}" font-size="{}""#,
        fmt(text.x),
        fmt(text.y),
        escape_attr_display(fill),
        fmt(text.font_size)
    );
    if include_font_weight {
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
    out.checkpoint()?;
    surface_receipt.record_text(role, class, &text.text);
    Ok(())
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
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-axes">"#);
    out.checkpoint()?;
    write_line(
        out,
        layout.axes.x_axis,
        None,
        &theme.axis_color,
        Some(1.0),
        None,
    )?;
    write_line(
        out,
        layout.axes.y_axis,
        None,
        &theme.axis_color,
        Some(1.0),
        None,
    )?;
    write_text(
        out,
        &layout.axes.x_label,
        Some("wardley-axis-label wardley-axis-label-x"),
        &theme.axis_text_color,
        true,
        crate::wardley::WardleyTextRole::AxisLabel,
        surface_receipt,
    )?;
    write_text(
        out,
        &layout.axes.y_label,
        Some("wardley-axis-label wardley-axis-label-y"),
        &theme.axis_text_color,
        true,
        crate::wardley::WardleyTextRole::AxisLabel,
        surface_receipt,
    )?;
    out.push_str("</g>");
    out.checkpoint()
}

fn write_stages(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
    if layout.stages.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-stages">"#);
    out.checkpoint()?;
    for stage in &layout.stages {
        if let Some(divider) = stage.divider {
            write_line(out, divider, None, "#000", Some(1.0), Some("5 5"))?;
            let insert_at = out.len() - 2;
            out.replace_range(insert_at..insert_at, r#" opacity="0.8""#)?;
        }
        write_text(
            out,
            &stage.label,
            Some("wardley-stage-label"),
            &theme.axis_text_color,
            false,
            crate::wardley::WardleyTextRole::StageLabel,
            surface_receipt,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_grid(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
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
    model: &WardleyDiagramRenderModel,
    theme: &crate::wardley::WardleyPaintBinding,
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
            Some(1.0),
            Some("4 4"),
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_links(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    diagram_id: SvgDiagramId<'_>,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
    options: &SvgExecution<'_>,
) -> Result<()> {
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
            Some(1.0),
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
                Some("wardley-link-label"),
                &theme.axis_text_color,
                false,
                crate::wardley::WardleyTextRole::LinkLabel,
                surface_receipt,
            )?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_trends(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
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
    theme: &crate::wardley::WardleyPaintBinding,
) -> Result<()> {
    match overlay {
        WardleySourceOverlayLayout::Build { circle } => write_circle(
            out,
            Some("wardley-build-overlay"),
            *circle,
            "#eee",
            "#000",
            1.0,
        ),
        WardleySourceOverlayLayout::Buy { circle } => write_circle(
            out,
            Some("wardley-buy-overlay"),
            *circle,
            "#ccc",
            &theme.component_stroke,
            1.0,
        ),
        WardleySourceOverlayLayout::Outsource { circle } => write_circle(
            out,
            Some("wardley-outsource-overlay"),
            *circle,
            "#666",
            &theme.component_stroke,
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
                "white",
                &theme.component_stroke,
                1.0,
            )?;
            for connector in connectors {
                write_line(
                    out,
                    *connector,
                    Some("wardley-market-line"),
                    &theme.component_stroke,
                    Some(1.0),
                    None,
                )?;
            }
            for dot in dots {
                write_circle(
                    out,
                    Some("wardley-market-dot"),
                    *dot,
                    "white",
                    &theme.component_stroke,
                    2.0,
                )?;
            }
            Ok(())
        }
    }
}

fn write_nodes(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
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
        match &node.shape {
            WardleyNodeShapeLayout::Circle { circle } => write_circle(
                out,
                None,
                *circle,
                &theme.component_fill,
                &theme.component_stroke,
                1.0,
            )?,
            WardleyNodeShapeLayout::PipelineSquare { rect } => {
                let _ = write!(
                    out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
                    fmt(rect.x),
                    fmt(rect.y),
                    fmt(rect.width),
                    fmt(rect.height),
                    escape_attr_display(&theme.component_fill),
                    escape_attr_display(&theme.component_stroke)
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
        let label_fill = match node.class_name.as_deref() {
            Some("evolved") => &theme.evolution_stroke,
            Some("anchor") => "#000",
            _ => &theme.component_label_color,
        };
        write_text(
            out,
            &node.label_layout,
            Some("wardley-node-label"),
            label_fill,
            true,
            crate::wardley::WardleyTextRole::NodeLabel,
            surface_receipt,
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
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-annotations-box">"#);
    out.checkpoint()?;
    if let Some(rect) = annotations_box.rect {
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="white" stroke="{}" stroke-width="1.5" rx="{}" ry="{}"/>"#,
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
    for line in &annotations_box.lines {
        write_text(
            out,
            line,
            None,
            &theme.axis_text_color,
            false,
            crate::wardley::WardleyTextRole::AnnotationBoxLine,
            surface_receipt,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_annotations(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
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
                "white",
                &theme.axis_color,
                1.5,
            )?;
            write_text(
                out,
                &point.label,
                None,
                &theme.axis_text_color,
                true,
                crate::wardley::WardleyTextRole::AnnotationPoint,
                surface_receipt,
            )?;
            out.push_str("</g>");
            out.checkpoint()?;
        }
    }
    if let Some(annotations_box) = &layout.annotations_box {
        write_annotations_box(out, annotations_box, theme, surface_receipt)?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_notes(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
    if layout.notes.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-notes">"#);
    out.checkpoint()?;
    for note in &layout.notes {
        write_text(
            out,
            &note.text,
            None,
            &theme.axis_text_color,
            true,
            crate::wardley::WardleyTextRole::Note,
            surface_receipt,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_arrow(
    out: &mut impl SvgOutput,
    arrow: &WardleyArrowLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    role: crate::wardley::WardleyTextRole,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
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
        r#" Z" fill="white" stroke="{}" stroke-width="1"/>"#,
        escape_attr_display(&theme.component_stroke)
    );
    out.checkpoint()?;
    write_text(
        out,
        &arrow.label,
        None,
        &theme.axis_text_color,
        true,
        role,
        surface_receipt,
    )
}

fn write_arrows(
    out: &mut impl SvgOutput,
    class: &str,
    arrows: &[WardleyArrowLayout],
    theme: &crate::wardley::WardleyPaintBinding,
    role: crate::wardley::WardleyTextRole,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
) -> Result<()> {
    if arrows.is_empty() {
        return Ok(());
    }
    let _ = write!(out, r#"<g class="{}">"#, escape_attr_display(class));
    out.checkpoint()?;
    for arrow in arrows {
        write_arrow(out, arrow, theme, role, surface_receipt)?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_defs(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    theme: &crate::wardley::WardleyPaintBinding,
) -> Result<()> {
    let diagram_id = escape_attr_display(diagram_id);
    let _ = write!(
        out,
        r#"<defs><marker id="arrow-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="{}" stroke="none"/></marker><marker id="link-arrow-end-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" fill="{}" stroke="none"/></marker><marker id="link-arrow-start-{diagram_id}" viewBox="0 0 10 10" refX="1" refY="5" markerWidth="5" markerHeight="5" orient="auto"><path d="M 10 0 L 0 5 L 10 10 z" fill="{}" stroke="none"/></marker></defs>"#,
        escape_attr_display(&theme.evolution_stroke),
        escape_attr_display(&theme.link_stroke),
        escape_attr_display(&theme.link_stroke)
    );
    out.checkpoint()
}

pub(crate) fn render_wardley_diagram_svg_model(
    layout: &WardleyDiagramLayout,
    model: &WardleyDiagramRenderModel,
    _diagram_title: Option<&str>,
    typography_theme: &crate::wardley::WardleyTypographyThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("wardley");
    let acc_title = model.acc_title.as_deref().filter(|value| !value.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let theme = typography_theme.paint_binding();
    let background_color = theme.background_color.as_str();
    let mut surface_receipt = typography_theme.begin_terminal_receipt(layout);

    let root_bounds = root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "wardley");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::WARDLEY, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    write_accessibility(&mut out, diagram_id, acc_title, acc_descr)?;

    if typography_theme.should_write_inherited_font() {
        let _ = write!(
            &mut out,
            r#"<g class="wardley-map" font-family="{}">"#,
            escape_attr_display(typography_theme.font_family_css())
        );
        out.checkpoint()?;
        surface_receipt
            .record_inherited_font_writer("wardley-map", typography_theme.font_family_css());
    } else {
        out.push_str(r#"<g class="wardley-map">"#);
        out.checkpoint()?;
    }
    let _ = write!(
        out,
        r#"<rect class="wardley-background" width="{}" height="{}" fill="{}"/>"#,
        fmt(layout.width),
        fmt(layout.height),
        escape_attr_display(background_color)
    );
    out.checkpoint()?;
    if let Some(title) = &layout.title {
        write_text(
            &mut out,
            title,
            Some("wardley-title"),
            &theme.axis_text_color,
            true,
            crate::wardley::WardleyTextRole::Title,
            &mut surface_receipt,
        )?;
    }
    write_axes(&mut out, layout, theme, &mut surface_receipt)?;
    write_stages(&mut out, layout, theme, &mut surface_receipt)?;
    write_grid(&mut out, layout, theme)?;
    write_pipelines(&mut out, layout, model, theme)?;
    write_links(
        &mut out,
        layout,
        theme,
        diagram_id,
        &mut surface_receipt,
        options,
    )?;
    write_trends(&mut out, layout, theme, diagram_id, options)?;
    write_nodes(&mut out, layout, theme, &mut surface_receipt)?;
    write_annotations(&mut out, layout, theme, &mut surface_receipt)?;
    write_notes(&mut out, layout, theme, &mut surface_receipt)?;
    write_arrows(
        &mut out,
        "wardley-accelerators",
        &layout.accelerators,
        theme,
        crate::wardley::WardleyTextRole::Accelerator,
        &mut surface_receipt,
    )?;
    write_arrows(
        &mut out,
        "wardley-deaccelerators",
        &layout.deaccelerators,
        theme,
        crate::wardley::WardleyTextRole::Deaccelerator,
        &mut surface_receipt,
    )?;
    out.push_str("</g>");
    out.checkpoint()?;
    write_defs(&mut out, diagram_id, theme)?;
    out.push_str("</svg>");
    options.checkpoint_emit()?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    if !typography_theme.record_terminal(surface_receipt) {
        return Err(crate::Error::InvalidModel {
            message: "Wardley typography receipt was recorded more than once".to_string(),
        });
    }
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use crate::text::DeterministicTextMeasurer;

    fn render_wardley_with_policy(
        policy: RenderResourcePolicy,
    ) -> crate::Result<root_svg::RootedSvg> {
        let model = WardleyDiagramRenderModel::default();
        let effective_config = serde_json::json!({});
        let effective_mermaid_config =
            merman_core::MermaidConfig::from_value(effective_config.clone());
        let typography_theme =
            crate::wardley::WardleyTypographyThemePlan::resolve(None, &effective_mermaid_config);
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

        render_wardley_diagram_svg_model(&layout, &model, None, &typography_theme, &execution)
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
