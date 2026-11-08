use super::*;
use crate::diagram_theme::{ResolvedStyleProperty, ThemeTarget};
use crate::family::{TerminalPaint, TerminalPaintPlan};
use crate::wardley::{
    WardleyAnnotationsBoxLayout, WardleyArrowLayout, WardleyCircleLayout, WardleyDiagramLayout,
    WardleyDominantBaseline, WardleyFontWeight, WardleyLineLayout, WardleyNodeShapeLayout,
    WardleySourceOverlayLayout, WardleyTextAnchor, WardleyTextLayout,
};
use merman_model::families::wardley::WardleyDiagramModel;

struct WardleyPaintContext<'a> {
    plan: TerminalPaintPlan<'a>,
    base: &'a crate::wardley::WardleyPaintBinding,
    text_ordinals: std::collections::BTreeMap<ThemeTarget, usize>,
}

impl WardleyPaintContext<'_> {
    fn text_fill(
        &mut self,
        role: crate::wardley::WardleyTextRole,
    ) -> Result<Option<TerminalPaint>> {
        use crate::wardley::WardleyTextRole as Role;
        let shared_paths: &[&str] = &[self.base.axis_text_color_source];
        let node_paths: &[&str] = &[self.base.component_label_color_source];
        let (target, paths, base): (_, &[&str], _) = match role {
            Role::Title => (
                ThemeTarget::Title,
                shared_paths,
                self.base.axis_text_color.as_str(),
            ),
            Role::AxisLabel | Role::StageLabel => (
                ThemeTarget::Axis,
                shared_paths,
                self.base.axis_text_color.as_str(),
            ),
            Role::LinkLabel => (
                ThemeTarget::EdgeLabel,
                shared_paths,
                self.base.axis_text_color.as_str(),
            ),
            Role::NodeLabel => (
                ThemeTarget::NodeLabel,
                node_paths,
                self.base.component_label_color.as_str(),
            ),
            Role::AnchorLabel => (
                ThemeTarget::NodeLabel,
                &[
                    "themeVariables.wardley.anchorLabelColor",
                    "themeVariables.primaryTextColor",
                ],
                self.base.anchor_label_color.as_str(),
            ),
            Role::EvolvedLabel => (
                ThemeTarget::NodeLabel,
                &["themeVariables.wardley.evolutionStroke"],
                self.base.evolution_stroke.as_str(),
            ),
            _ => return Ok(None),
        };
        let ordinal = self.text_ordinals.entry(target).or_default();
        *ordinal += 1;
        let ordinal = *ordinal;
        let style = self.plan.style(target, ordinal, true)?;
        let fill = self.plan.paint(
            target,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Fill,
            paths,
            base,
        );
        if target != ThemeTarget::Axis {
            self.plan
                .unsupported_property(target, style.as_ref(), ResolvedStyleProperty::Stroke);
        }
        Ok(fill)
    }

    fn line_stroke(
        &mut self,
        target: ThemeTarget,
        ordinal: usize,
        paths: &[&str],
        base: &str,
    ) -> Result<Option<TerminalPaint>> {
        let style = self.plan.style(target, ordinal, false)?;
        let stroke = self.plan.paint(
            target,
            ordinal,
            style.as_ref(),
            ResolvedStyleProperty::Stroke,
            paths,
            base,
        );
        if target == ThemeTarget::Edge {
            self.plan
                .unsupported_property(target, style.as_ref(), ResolvedStyleProperty::Fill);
        }
        Ok(stroke)
    }
}

struct WardleyLineStyle<'a> {
    class: Option<&'a str>,
    stroke: &'a str,
    stroke_width: Option<f64>,
    dash: Option<&'a str>,
}

fn write_themed_line(
    out: &mut impl SvgOutput,
    line: WardleyLineLayout,
    style: WardleyLineStyle<'_>,
    context: &mut WardleyPaintContext<'_>,
    target: ThemeTarget,
    ordinal: usize,
    paths: &[&str],
) -> Result<()> {
    let paint = context.line_stroke(target, ordinal, paths, style.stroke)?;
    let actual = paint.as_ref().map_or(style.stroke, TerminalPaint::css);
    write_line(
        out,
        line,
        style.class,
        actual,
        style.stroke_width,
        style.dash,
    )?;
    context.plan.record(paint.as_ref(), actual);
    Ok(())
}

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
    role: crate::wardley::WardleyTextRole,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    let typed_fill = if text.text.trim().is_empty() {
        None
    } else {
        paint_context.text_fill(role)?
    };
    if typed_fill.is_some() {
        surface_receipt.paint_overridden_text_roles.insert(role);
    }
    let fill = typed_fill.as_ref().map_or(style.fill, TerminalPaint::css);
    out.push_str("<text");
    if let Some(class) = style.class {
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
    if style.include_font_weight {
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
    surface_receipt.record_text(role, style.class, &text.text, fill);
    paint_context.plan.record(typed_fill.as_ref(), fill);
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
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-axes">"#);
    out.checkpoint()?;
    write_themed_line(
        out,
        layout.axes.x_axis,
        WardleyLineStyle {
            class: None,
            stroke: &theme.axis_color,
            stroke_width: Some(1.0),
            dash: None,
        },
        paint_context,
        ThemeTarget::Axis,
        1,
        &["themeVariables.wardley.axisColor"],
    )?;
    write_themed_line(
        out,
        layout.axes.y_axis,
        WardleyLineStyle {
            class: None,
            stroke: &theme.axis_color,
            stroke_width: Some(1.0),
            dash: None,
        },
        paint_context,
        ThemeTarget::Axis,
        2,
        &["themeVariables.wardley.axisColor"],
    )?;
    write_text(
        out,
        &layout.axes.x_label,
        WardleyTextStyle {
            class: Some("wardley-axis-label wardley-axis-label-x"),
            fill: &theme.axis_text_color,
            include_font_weight: true,
        },
        crate::wardley::WardleyTextRole::AxisLabel,
        surface_receipt,
        paint_context,
    )?;
    write_text(
        out,
        &layout.axes.y_label,
        WardleyTextStyle {
            class: Some("wardley-axis-label wardley-axis-label-y"),
            fill: &theme.axis_text_color,
            include_font_weight: true,
        },
        crate::wardley::WardleyTextRole::AxisLabel,
        surface_receipt,
        paint_context,
    )?;
    out.push_str("</g>");
    out.checkpoint()
}

fn write_stages(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    if layout.stages.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-stages">"#);
    out.checkpoint()?;
    for (index, stage) in layout.stages.iter().enumerate() {
        if let Some(divider) = stage.divider {
            write_themed_line(
                out,
                divider,
                WardleyLineStyle {
                    class: None,
                    stroke: &theme.stage_divider_color,
                    stroke_width: Some(1.0),
                    dash: Some("5 5"),
                },
                paint_context,
                ThemeTarget::Axis,
                index + 3,
                &["themeVariables.wardley.stageDividerColor"],
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
            crate::wardley::WardleyTextRole::StageLabel,
            surface_receipt,
            paint_context,
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_grid(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    if layout.grid.is_empty() {
        return Ok(());
    }
    out.push_str(r#"<g class="wardley-grid">"#);
    out.checkpoint()?;
    for (index, grid) in layout.grid.iter().enumerate() {
        write_themed_line(
            out,
            grid.vertical,
            WardleyLineStyle {
                class: None,
                stroke: &theme.grid_color,
                stroke_width: None,
                dash: Some("2 6"),
            },
            paint_context,
            ThemeTarget::Axis,
            index * 2 + layout.stages.len() + 3,
            &["themeVariables.wardley.gridColor"],
        )?;
        write_themed_line(
            out,
            grid.horizontal,
            WardleyLineStyle {
                class: None,
                stroke: &theme.grid_color,
                stroke_width: None,
                dash: Some("2 6"),
            },
            paint_context,
            ThemeTarget::Axis,
            index * 2 + layout.stages.len() + 4,
            &["themeVariables.wardley.gridColor"],
        )?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_pipelines(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    model: &WardleyDiagramModel,
    theme: &crate::wardley::WardleyPaintBinding,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    if model.pipelines.is_empty() {
        return Ok(());
    }

    out.push_str(r#"<g class="wardley-pipelines">"#);
    out.checkpoint()?;
    for (index, pipeline) in layout.pipeline_boxes.iter().enumerate() {
        let rect = pipeline.rect;
        let paint = paint_context.line_stroke(
            ThemeTarget::Node,
            index + 1,
            &["themeVariables.wardley.axisColor"],
            &theme.axis_color,
        )?;
        let stroke = paint
            .as_ref()
            .map_or(theme.axis_color.as_str(), TerminalPaint::css);
        let _ = write!(
            out,
            r#"<rect class="wardley-pipeline-box" x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="1.5" rx="{}" ry="{}"/>"#,
            fmt(rect.x),
            fmt(rect.y),
            fmt(rect.width),
            fmt(rect.height),
            escape_attr_display(stroke),
            fmt(rect.corner_radius),
            fmt(rect.corner_radius)
        );
        out.checkpoint()?;
        paint_context.plan.record(paint.as_ref(), stroke);
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str(r#"<g class="wardley-pipeline-links">"#);
    out.checkpoint()?;
    for (index, link) in layout.pipeline_links.iter().enumerate() {
        write_themed_line(
            out,
            link.line,
            WardleyLineStyle {
                class: Some("wardley-pipeline-evolution-link"),
                stroke: &theme.link_stroke,
                stroke_width: Some(1.0),
                dash: Some("4 4"),
            },
            paint_context,
            ThemeTarget::Edge,
            index + 1,
            &["themeVariables.wardley.linkStroke"],
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
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-links">"#);
    out.checkpoint()?;
    for (index, link) in layout.links.iter().enumerate() {
        options.checkpoint_emit()?;
        let class = if link.dashed {
            "wardley-link wardley-link--dashed"
        } else {
            "wardley-link"
        };
        write_themed_line(
            out,
            link.line,
            WardleyLineStyle {
                class: Some(class),
                stroke: &theme.link_stroke,
                stroke_width: Some(1.0),
                dash: link.dashed.then_some("6 6"),
            },
            paint_context,
            ThemeTarget::Edge,
            layout.pipeline_links.len() + index + 1,
            &["themeVariables.wardley.linkStroke"],
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
                crate::wardley::WardleyTextRole::LinkLabel,
                surface_receipt,
                paint_context,
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
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-trends">"#);
    out.checkpoint()?;
    for (index, trend) in layout.trends.iter().enumerate() {
        options.checkpoint_emit()?;
        write_themed_line(
            out,
            trend.line,
            WardleyLineStyle {
                class: Some("wardley-trend"),
                stroke: &theme.evolution_stroke,
                stroke_width: Some(1.0),
                dash: Some("4 4"),
            },
            paint_context,
            ThemeTarget::Edge,
            layout.pipeline_links.len() + layout.links.len() + index + 1,
            &["themeVariables.wardley.evolutionStroke"],
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
    ordinal: usize,
    context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    let (base_fill, base_stroke, fill_path, stroke_path) = match overlay {
        WardleySourceOverlayLayout::Build { .. } => (
            &theme.build_fill,
            &theme.build_stroke,
            "themeVariables.wardley.buildFill",
            "themeVariables.wardley.buildStroke",
        ),
        WardleySourceOverlayLayout::Buy { .. } => (
            &theme.buy_fill,
            &theme.component_stroke,
            "themeVariables.wardley.buyFill",
            "themeVariables.wardley.componentStroke",
        ),
        WardleySourceOverlayLayout::Outsource { .. } => (
            &theme.outsource_fill,
            &theme.component_stroke,
            "themeVariables.wardley.outsourceFill",
            "themeVariables.wardley.componentStroke",
        ),
        WardleySourceOverlayLayout::Market { .. } => (
            &theme.market_fill,
            &theme.component_stroke,
            "themeVariables.wardley.marketFill",
            "themeVariables.wardley.componentStroke",
        ),
    };
    let style = context.plan.style(ThemeTarget::Node, ordinal, false)?;
    let fill = context.plan.paint(
        ThemeTarget::Node,
        ordinal,
        style.as_ref(),
        ResolvedStyleProperty::Fill,
        &[fill_path],
        base_fill,
    );
    let stroke = context.plan.paint(
        ThemeTarget::Node,
        ordinal,
        style.as_ref(),
        ResolvedStyleProperty::Stroke,
        &[stroke_path],
        base_stroke,
    );
    let actual_fill = fill.as_ref().map_or(base_fill.as_str(), TerminalPaint::css);
    let actual_stroke = stroke
        .as_ref()
        .map_or(base_stroke.as_str(), TerminalPaint::css);
    write_source_overlay_binding(out, overlay, actual_fill, actual_stroke)?;
    context.plan.record(fill.as_ref(), actual_fill);
    context.plan.record(stroke.as_ref(), actual_stroke);
    Ok(())
}

fn write_source_overlay_binding(
    out: &mut impl SvgOutput,
    overlay: &WardleySourceOverlayLayout,
    fill: &str,
    stroke: &str,
) -> Result<()> {
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
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    out.push_str(r#"<g class="wardley-nodes">"#);
    out.checkpoint()?;
    for (index, node) in layout.nodes.iter().enumerate() {
        out.push_str(r#"<g class="wardley-node"#);
        if let Some(class_name) = node.class_name.as_deref().filter(|class| !class.is_empty()) {
            let _ = write!(out, " wardley-node--{}", escape_attr_display(class_name));
        }
        out.push_str(r#"">"#);
        out.checkpoint()?;

        if let Some(overlay) = &node.source_overlay {
            write_source_overlay(out, overlay, theme, index + 1, paint_context)?;
        }
        let (typed_fill, typed_stroke) = if matches!(
            &node.shape,
            WardleyNodeShapeLayout::Circle { .. } | WardleyNodeShapeLayout::PipelineSquare { .. }
        ) {
            let style = paint_context
                .plan
                .style(ThemeTarget::Node, index + 1, false)?;
            (
                paint_context.plan.paint(
                    ThemeTarget::Node,
                    index + 1,
                    style.as_ref(),
                    ResolvedStyleProperty::Fill,
                    &["themeVariables.wardley.componentFill"],
                    &theme.component_fill,
                ),
                paint_context.plan.paint(
                    ThemeTarget::Node,
                    index + 1,
                    style.as_ref(),
                    ResolvedStyleProperty::Stroke,
                    &["themeVariables.wardley.componentStroke"],
                    &theme.component_stroke,
                ),
            )
        } else {
            (None, None)
        };
        let fill = typed_fill
            .as_ref()
            .map_or(theme.component_fill.as_str(), TerminalPaint::css);
        let stroke = typed_stroke
            .as_ref()
            .map_or(theme.component_stroke.as_str(), TerminalPaint::css);
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
        paint_context.plan.record(typed_fill.as_ref(), fill);
        paint_context.plan.record(typed_stroke.as_ref(), stroke);
        if let Some(inertia) = node.inertia {
            write_themed_line(
                out,
                inertia,
                WardleyLineStyle {
                    class: Some("wardley-inertia"),
                    stroke: &theme.component_stroke,
                    stroke_width: Some(6.0),
                    dash: None,
                },
                paint_context,
                ThemeTarget::Node,
                index + 1,
                &["themeVariables.wardley.componentStroke"],
            )?;
        }
        let (label_fill, label_role) = match node.class_name.as_deref() {
            Some("evolved") => (
                &theme.evolution_stroke,
                crate::wardley::WardleyTextRole::EvolvedLabel,
            ),
            Some("anchor") => (
                &theme.anchor_label_color,
                crate::wardley::WardleyTextRole::AnchorLabel,
            ),
            _ => (
                &theme.component_label_color,
                crate::wardley::WardleyTextRole::NodeLabel,
            ),
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
            surface_receipt,
            paint_context,
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
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
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
            crate::wardley::WardleyTextRole::AnnotationBoxLine,
            surface_receipt,
            paint_context,
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
    paint_context: &mut WardleyPaintContext<'_>,
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
                crate::wardley::WardleyTextRole::AnnotationPoint,
                surface_receipt,
                paint_context,
            )?;
            out.push_str("</g>");
            out.checkpoint()?;
        }
    }
    if let Some(annotations_box) = &layout.annotations_box {
        write_annotations_box(out, annotations_box, theme, surface_receipt, paint_context)?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_notes(
    out: &mut impl SvgOutput,
    layout: &WardleyDiagramLayout,
    theme: &crate::wardley::WardleyPaintBinding,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
    paint_context: &mut WardleyPaintContext<'_>,
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
            WardleyTextStyle {
                class: None,
                fill: &theme.axis_text_color,
                include_font_weight: true,
            },
            crate::wardley::WardleyTextRole::Note,
            surface_receipt,
            paint_context,
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
    paint_context: &mut WardleyPaintContext<'_>,
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
        surface_receipt,
        paint_context,
    )
}

fn write_arrows(
    out: &mut impl SvgOutput,
    class: &str,
    arrows: &[WardleyArrowLayout],
    theme: &crate::wardley::WardleyPaintBinding,
    role: crate::wardley::WardleyTextRole,
    surface_receipt: &mut crate::wardley::WardleySurfaceReceipt,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    if arrows.is_empty() {
        return Ok(());
    }
    let _ = write!(out, r#"<g class="{}">"#, escape_attr_display(class));
    out.checkpoint()?;
    for arrow in arrows {
        write_arrow(out, arrow, theme, role, surface_receipt, paint_context)?;
    }
    out.push_str("</g>");
    out.checkpoint()
}

fn write_marker_path(
    out: &mut impl SvgOutput,
    path: &str,
    fill: &str,
    edge_ordinal: Option<usize>,
    ordinal: usize,
    source_path: &str,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    let style = if edge_ordinal.is_some() {
        paint_context
            .plan
            .style(ThemeTarget::Marker, ordinal, false)?
    } else {
        None
    };
    let mut typed_fill = paint_context.plan.paint(
        ThemeTarget::Marker,
        ordinal,
        style.as_ref(),
        ResolvedStyleProperty::Fill,
        &[source_path],
        fill,
    );
    if let Some(edge_ordinal) = edge_ordinal
        && style
            .as_ref()
            .is_some_and(|style| style.fill_resolution().specified().is_unspecified())
    {
        // Marker slots have their own ordinals; fallback belongs to a real referencing edge.
        let edge_style = paint_context
            .plan
            .style(ThemeTarget::Edge, edge_ordinal, false)?;
        typed_fill = paint_context.plan.paint(
            ThemeTarget::Edge,
            edge_ordinal,
            edge_style.as_ref(),
            ResolvedStyleProperty::Stroke,
            &[source_path],
            fill,
        );
    }
    let typed_stroke = paint_context.plan.paint(
        ThemeTarget::Marker,
        ordinal,
        style.as_ref(),
        ResolvedStyleProperty::Stroke,
        &[source_path],
        "none",
    );
    let actual_fill = typed_fill.as_ref().map_or(fill, TerminalPaint::css);
    let actual_stroke = typed_stroke.as_ref().map_or("none", TerminalPaint::css);
    let _ = write!(
        out,
        r#"<path d="{}" fill="{}" stroke="{}"/>"#,
        path,
        escape_attr_display(actual_fill),
        escape_attr_display(actual_stroke)
    );
    out.checkpoint()?;
    paint_context.plan.record(typed_fill.as_ref(), actual_fill);
    paint_context
        .plan
        .record(typed_stroke.as_ref(), actual_stroke);
    Ok(())
}

fn write_defs(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    theme: &crate::wardley::WardleyPaintBinding,
    layout: &WardleyDiagramLayout,
    paint_context: &mut WardleyPaintContext<'_>,
) -> Result<()> {
    paint_context
        .plan
        .charge(layout.links.len().saturating_mul(2))?;
    let diagram_id = escape_attr_display(diagram_id);
    let _ = write!(
        out,
        r#"<defs><marker id="arrow-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">"#
    );
    write_marker_path(
        out,
        "M 0 0 L 10 5 L 0 10 z",
        &theme.evolution_stroke,
        (!layout.trends.is_empty()).then_some(layout.pipeline_links.len() + layout.links.len() + 1),
        1,
        "themeVariables.wardley.evolutionStroke",
        paint_context,
    )?;
    let _ = write!(
        out,
        r#"</marker><marker id="link-arrow-end-{diagram_id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto">"#
    );
    write_marker_path(
        out,
        "M 0 0 L 10 5 L 0 10 z",
        &theme.link_stroke,
        layout
            .links
            .iter()
            .position(|link| link.markers.end)
            .map(|index| layout.pipeline_links.len() + index + 1),
        2,
        "themeVariables.wardley.linkStroke",
        paint_context,
    )?;
    let _ = write!(
        out,
        r#"</marker><marker id="link-arrow-start-{diagram_id}" viewBox="0 0 10 10" refX="1" refY="5" markerWidth="5" markerHeight="5" orient="auto">"#
    );
    write_marker_path(
        out,
        "M 10 0 L 0 5 L 10 10 z",
        &theme.link_stroke,
        layout
            .links
            .iter()
            .position(|link| link.markers.start)
            .map(|index| layout.pipeline_links.len() + index + 1),
        3,
        "themeVariables.wardley.linkStroke",
        paint_context,
    )?;
    out.push_str("</marker></defs>");
    out.checkpoint()
}

pub(crate) fn render_wardley_diagram_svg_model(
    layout: &WardleyDiagramLayout,
    model: &WardleyDiagramModel,
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
    let mut surface_receipt = typography_theme.begin_terminal_receipt();
    let mut paint_context = WardleyPaintContext {
        plan: TerminalPaintPlan::new(
            options.resolved_theme(),
            typography_theme.config(),
            crate::wardley::PAINT_TARGETS,
            options.work_meter(),
        )?,
        base: typography_theme.base_paint_binding(),
        text_ordinals: std::collections::BTreeMap::new(),
    };

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
            WardleyTextStyle {
                class: Some("wardley-title"),
                fill: &theme.axis_text_color,
                include_font_weight: true,
            },
            crate::wardley::WardleyTextRole::Title,
            &mut surface_receipt,
            &mut paint_context,
        )?;
    }
    write_axes(
        &mut out,
        layout,
        theme,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    write_stages(
        &mut out,
        layout,
        theme,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    write_grid(&mut out, layout, theme, &mut paint_context)?;
    write_pipelines(&mut out, layout, model, theme, &mut paint_context)?;
    write_links(
        &mut out,
        layout,
        theme,
        diagram_id,
        &mut surface_receipt,
        options,
        &mut paint_context,
    )?;
    write_trends(
        &mut out,
        layout,
        theme,
        diagram_id,
        options,
        &mut paint_context,
    )?;
    write_nodes(
        &mut out,
        layout,
        theme,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    write_annotations(
        &mut out,
        layout,
        theme,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    write_notes(
        &mut out,
        layout,
        theme,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    write_arrows(
        &mut out,
        "wardley-accelerators",
        &layout.accelerators,
        theme,
        crate::wardley::WardleyTextRole::Accelerator,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    write_arrows(
        &mut out,
        "wardley-deaccelerators",
        &layout.deaccelerators,
        theme,
        crate::wardley::WardleyTextRole::Deaccelerator,
        &mut surface_receipt,
        &mut paint_context,
    )?;
    out.push_str("</g>");
    out.checkpoint()?;
    write_defs(&mut out, diagram_id, theme, layout, &mut paint_context)?;
    surface_receipt.paint_evidence = paint_context.plan.finish();
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

    #[test]
    fn wardley_evolved_model_label_keeps_its_evolution_paint_under_host_text() {
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
            .unwrap()
            .resolve(crate::DiagramFamilyId::WARDLEY);
        let work_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": { "wardley": { "evolutionStroke": "#ab1234" } }
        }));
        let typography =
            crate::wardley::WardleyTypographyThemePlan::resolve(Some(&theme), &config, &work_meter)
                .unwrap();
        let model: WardleyDiagramModel = serde_json::from_value(serde_json::json!({
            "nodes": [{
                "id": "evolved", "label": "Evolved API", "x": 0.8, "y": 0.6,
                "className": "evolved"
            }]
        }))
        .unwrap();
        let layout = crate::wardley::layout_wardley_diagram_typed_with_theme(
            &model,
            None,
            config.as_value(),
            &typography,
            &DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let mut svg = String::new();
        let mut receipt = typography.begin_terminal_receipt();
        let mut paint_context = WardleyPaintContext {
            plan: TerminalPaintPlan::new(
                Some(&theme),
                &config,
                crate::wardley::PAINT_TARGETS,
                &work_meter,
            )
            .unwrap(),
            base: typography.base_paint_binding(),
            text_ordinals: std::collections::BTreeMap::new(),
        };
        write_nodes(
            &mut svg,
            &layout,
            typography.paint_binding(),
            &mut receipt,
            &mut paint_context,
        )
        .unwrap();
        receipt.paint_evidence = paint_context.plan.finish();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.attribute("fill"), Some("#ab1234"));
        assert!(typography.record_terminal(receipt));
    }

    fn render_wardley_with_policy(
        policy: RenderResourcePolicy,
    ) -> crate::Result<root_svg::RootedSvg> {
        let model = WardleyDiagramModel::default();
        let effective_config = serde_json::json!({});
        let effective_mermaid_config =
            crate::test_support::presentation_config(effective_config.clone());
        let typography_theme = crate::wardley::WardleyTypographyThemePlan::resolve(
            None,
            &effective_mermaid_config,
            &crate::resources::OperationWorkMeter::new(policy),
        )?;
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
