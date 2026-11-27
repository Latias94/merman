use std::fmt::Write as _;

use merman_model::{OperationPhase, families::timeline::TimelineDirection};
use merman_render_core::{
    Bounds, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg,
    svg_css::{CssSelectorDiagramId, PreparedCommonCss, SvgId, scoped_svg_id, scoped_svg_url},
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    xml::{escape_attr, escape_attr_display, escape_xml},
};

use super::{
    MERMAID_EVENT_RADIUS_PX, MERMAID_EVENT_RADIUS_TOKEN, TimelineCssBinding, TimelineDiagramLayout,
    TimelineLineLayout, TimelineNativeStyle, TimelineNodeLayout, TimelineStyle, TimelineTaskLayout,
};

fn write_timeline_css(
    out: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
    theme: &TimelineCssBinding,
    common: &PreparedCommonCss,
) -> Result<()> {
    // Keep `:root` last (matches upstream Mermaid timeline SVG baselines).
    common.write_prefix(out, diagram_id)?;
    let _ = write!(out, r#"#{} .edge{{stroke-width:3;}}"#, diagram_id);
    for (i, section_theme) in theme.sections.iter().enumerate() {
        let section = i as i64 - 1;
        let sw = 17 - 3 * (i as i64);

        if theme.is_redux_theme {
            let redux_stroke = theme.section_palette_color(i).unwrap_or(&theme.node_border);
            let fill = if theme.is_color_theme && !theme.is_dark_theme {
                theme.section_palette_color(i).unwrap_or(&theme.main_bkg)
            } else {
                &theme.main_bkg
            };
            let _ = write!(
                out,
                r#"#{diagram_id} .section-{section} rect,#{diagram_id} .section-{section} path,#{diagram_id} .section-{section} circle{{fill:{fill};stroke:{redux_stroke};"#,
            );
            let node_border = &theme.node_border;
            let _ = write!(
                out,
                r#"stroke-width:{};filter:{};}}#{diagram_id} .section-{section} text{{fill:{node_border};"#,
                theme.stroke_width,
                scoped_svg_url(diagram_id, "drop-shadow"),
            );
            let _ = write!(
                out,
                r#"font-weight:{};}}#{diagram_id} .node-icon-{section}{{font-size:40px;color:{};}}#{diagram_id} .section-edge-{section}{{stroke:{};}}#{diagram_id} .edge-depth-{section}{{stroke-width:{sw};}}#{diagram_id} .section-{section} line{{stroke:{};stroke-width:3;}}#{diagram_id} .lineWrapper line{{"#,
                theme.section_font_weight().expect("Redux section rule"),
                section_theme.c_scale_label,
                section_theme.c_scale,
                section_theme.c_scale_inv,
            );
            let _ = write!(out, "stroke:{node_border};");
            out.checkpoint()?;
            let _ = write!(
                out,
                r#"stroke-width:{};}}#{diagram_id} .disabled,#{diagram_id} .disabled circle,#{diagram_id} .disabled text{{fill:{};}}#{diagram_id} .disabled text{{fill:{};}}"#,
                theme.stroke_width, theme.disabled_fill, theme.disabled_text_fill,
            );
        } else {
            let _ = write!(
                out,
                r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle,#{} .section-{} path{{fill:{};}}#{} .section-{} text{{fill:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .lineWrapper line{{stroke:{};}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:{};}}#{} .disabled text{{fill:{};}}"#,
                diagram_id,
                section,
                diagram_id,
                section,
                diagram_id,
                section,
                diagram_id,
                section,
                section_theme.c_scale,
                diagram_id,
                section,
                section_theme.c_scale_label,
                diagram_id,
                section,
                section_theme.c_scale_label,
                diagram_id,
                section,
                section_theme.c_scale,
                diagram_id,
                section,
                sw,
                diagram_id,
                section,
                section_theme.c_scale_inv,
                diagram_id,
                section_theme.c_scale_label,
                diagram_id,
                diagram_id,
                diagram_id,
                theme.disabled_fill,
                diagram_id,
                theme.disabled_text_fill,
            );
        }
        out.checkpoint()?;
    }

    if theme.use_neo_gradient {
        let gradient = scoped_svg_url(diagram_id, "gradient");
        for (i, _) in theme.sections.iter().enumerate() {
            let section = i as i64 - 1;
            let _ = write!(
                out,
                r#"#{} .section-{}[data-look="neo"] rect,#{} .section-{}[data-look="neo"] path,#{} .section-{}[data-look="neo"] circle{{fill:{};stroke:{};stroke-width:2;}}#{} .section-{}[data-look="neo"] line{{stroke:{};stroke-width:2;}}"#,
                diagram_id,
                section,
                diagram_id,
                section,
                diagram_id,
                section,
                theme.main_bkg,
                gradient,
                diagram_id,
                section,
                gradient,
            );
            out.checkpoint()?;
        }
    }

    let _ = write!(
        out,
        r#"#{} .section-root rect,#{} .section-root path,#{} .section-root circle{{fill:{};}}#{} .section-root text{{fill:{};}}#{} .icon-container{{height:100%;display:flex;justify-content:center;align-items:center;}}#{} .edge{{fill:none;}}#{} .eventWrapper{{filter:brightness(120%);}}"#,
        diagram_id,
        diagram_id,
        diagram_id,
        theme.root_fill,
        diagram_id,
        theme.root_label,
        diagram_id,
        diagram_id,
        diagram_id
    );

    out.checkpoint()?;
    common.write_root(out, diagram_id, diagram_id)
}

fn write_timeline_connector(
    out: &mut impl SvgOutput,
    connector: &TimelineLineLayout,
    diagram_id: Option<SvgId<'_>>,
    native: Option<&TimelineNativeStyle>,
) -> Result<()> {
    fn write_connector(
        out: &mut impl SvgOutput,
        connector: &TimelineLineLayout,
        marker_end: impl std::fmt::Display,
        native: Option<&TimelineNativeStyle>,
    ) {
        let (width, stroke) = native.map_or((2.0, "black"), |style| {
            (style.connector_width, style.connector.as_str())
        });
        let _ = write!(
            out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="{width}" stroke="{stroke}" marker-end="{marker_end}" stroke-dasharray="5,5"/></g>"#,
            x1 = fmt(connector.x1),
            y1 = fmt(connector.y1),
            x2 = fmt(connector.x2),
            y2 = fmt(connector.y2),
            width = fmt(width),
            stroke = escape_attr(stroke),
        );
    }

    match diagram_id {
        Some(diagram_id) => write_connector(
            out,
            connector,
            escape_attr_display(scoped_svg_url(diagram_id, "arrowhead")),
            native,
        ),
        None => write_connector(out, connector, "url(#arrowhead)", native),
    }
    out.checkpoint()
}

fn write_timeline_activity_line(
    out: &mut impl SvgOutput,
    line: &TimelineLineLayout,
    marker_end: impl std::fmt::Display,
    native: Option<&TimelineNativeStyle>,
) -> Result<()> {
    let (width, stroke) = native.map_or((4.0, "black"), |style| {
        (style.connector_width, style.connector.as_str())
    });
    let _ = write!(
        out,
        r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="{width}" stroke="{stroke}" marker-end="{marker_end}"/></g>"#,
        x1 = fmt(line.x1),
        y1 = fmt(line.y1),
        x2 = fmt(line.x2),
        y2 = fmt(line.y2),
        width = fmt(width),
        stroke = escape_attr(stroke),
    );
    out.checkpoint()
}

/// Emits a balanced Timeline document for the host SVG pipeline.
pub fn render_timeline_svg(
    layout: &TimelineDiagramLayout,
    style: &TimelineStyle,
    diagram_id: SvgId<'_>,
    viewport: &root_svg::RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<root_svg::RootedSvg> {
    let mermaid = style.css_binding();
    let native = style.native();
    let is_redux_theme = mermaid.is_some_and(|theme| theme.is_redux_theme);
    let is_neo = style.layout_settings().is_neo;

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });

    fn node_line_class(section_class: &str) -> String {
        let rest = section_class
            .strip_prefix("section-")
            .unwrap_or(section_class);
        format!("node-line-{rest}")
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Timeline node emission uses both layout geometry and the resolved diagram style."
    )]
    fn render_node(
        out: &mut impl SvgOutput,
        diagram_id: SvgId<'_>,
        node_count: &mut usize,
        n: &TimelineNodeLayout,
        ordinal: usize,
        direction: TimelineDirection,
        is_redux_theme: bool,
        is_neo: bool,
        is_event: bool,
        native: Option<&TimelineNativeStyle>,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let node_local_id = format!("node-{node_count}");
        let node_id = scoped_svg_id(diagram_id, &node_local_id);
        *node_count += 1;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let rd = MERMAID_EVENT_RADIUS_PX;
        let mermaid_path = || {
            if is_redux_theme {
                format!(
                    "M0 {y0} v{v1} h{w} v{h} H0 Z",
                    y0 = fmt(h - rd),
                    v1 = fmt(-(h - rd)),
                    w = fmt(w),
                    h = fmt(h),
                )
            } else {
                format!(
                    "M0 {y0} v{v1} q0,-{radius} {radius},-{radius} h{hw} q{radius},0 {radius},{radius} v{v2} H0 Z",
                    y0 = fmt(h - rd),
                    v1 = fmt(-h + 2.0 * rd),
                    radius = MERMAID_EVENT_RADIUS_TOKEN,
                    hw = fmt(w - 2.0 * rd),
                    v2 = fmt(h - rd),
                )
            }
        };

        let native_paint =
            native.map(|style| style.card_paint(&n.section_class, ordinal, is_event));
        let look_attr = if is_neo { r#" data-look="neo""# } else { "" };

        let _ = write!(
            out,
            r#"<g class="timeline-node {section_class}"{look_attr}>"#,
            section_class = escape_attr(&n.section_class),
            look_attr = look_attr,
        );
        out.checkpoint()?;
        out.push_str("<g>");
        out.checkpoint()?;
        if let Some((style, (fill, _, accent))) = native.zip(native_paint.as_ref()) {
            let radius = if n.kind == "section" {
                style.container_radius
            } else {
                style.radius
            };
            let radius = radius.min(w / 2.0).min(h / 2.0);
            let gradient;
            let stroke = if style.gradient_stops.is_some() {
                gradient = scoped_svg_url(diagram_id, "gradient").to_string();
                gradient.as_str()
            } else {
                &style.border
            };
            let _ = write!(
                out,
                r#"<rect id="{node_id}" class="node-bkg node-undefined" width="{w}" height="{h}" rx="{radius}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#,
                node_id = escape_attr_display(node_id),
                w = fmt(w),
                h = fmt(h),
                radius = fmt(radius),
                fill = escape_attr(fill),
                stroke = escape_attr(stroke),
                stroke_width = escape_attr(&style.border_width),
            );
            out.checkpoint()?;
            if let Some(accent) = accent {
                let [x1, y1, x2, y2] = match (is_event, direction) {
                    (true, TimelineDirection::LeftToRight) => [radius, 0.0, w - radius, 0.0],
                    (true, TimelineDirection::TopDown) => [0.0, radius, 0.0, h - radius],
                    (false, _) => [radius, h, w - radius, h],
                };
                let _ = write!(
                    out,
                    r#"<line class="timeline-event-accent" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{width}"/>"#,
                    x1 = fmt(x1),
                    y1 = fmt(y1),
                    x2 = fmt(x2),
                    y2 = fmt(y2),
                    stroke = escape_attr(accent),
                    width = fmt(style.accent_width),
                );
                out.checkpoint()?;
            }
        } else {
            let _ = write!(
                out,
                r#"<path id="{node_id}" class="node-bkg node-undefined" d="{d}"/>"#,
                node_id = escape_attr_display(node_id),
                d = escape_attr(&mermaid_path()),
            );
            out.checkpoint()?;
            if !is_redux_theme {
                let _ = write!(
                    out,
                    r#"<line class="{line_class}" x1="0" y1="{y}" x2="{x2}" y2="{y}"/>"#,
                    line_class = escape_attr(&node_line_class(&n.section_class)),
                    y = fmt(h),
                    x2 = fmt(w)
                );
                out.checkpoint()?;
            }
        }
        out.push_str("</g>");
        out.checkpoint()?;

        let tx = w / 2.0;
        let ty = if is_redux_theme {
            if is_event {
                n.padding / 2.0 + 3.0
            } else {
                n.padding
            }
        } else {
            n.padding / 2.0
        };
        let _ = write!(
            out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(tx),
            y = fmt(ty)
        );
        out.checkpoint()?;
        out.push_str(r#"<text dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle""#);
        if let Some((_, fill, _)) = native_paint.as_ref() {
            let _ = write!(out, r#" fill="{}""#, escape_attr(fill));
        }
        out.push('>');
        out.checkpoint()?;
        for (idx, line) in n.label_lines.iter().enumerate() {
            let dy = if idx == 0 { "1em" } else { "1.1em" };
            let _ = write!(
                out,
                r#"<tspan x="0" dy="{dy}">{text}</tspan>"#,
                dy = dy,
                text = escape_xml(line)
            );
            out.checkpoint()?;
        }
        out.push_str("</text></g></g>");
        out.checkpoint()?;
        work_meter.checkpoint(OperationPhase::Emit)?;
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Timeline node emission uses both layout geometry and the resolved diagram style."
    )]
    fn render_event(
        out: &mut impl SvgOutput,
        diagram_id: SvgId<'_>,
        node_count: &mut usize,
        event: &TimelineNodeLayout,
        ordinal: usize,
        direction: TimelineDirection,
        is_redux_theme: bool,
        is_neo: bool,
        native: Option<&TimelineNativeStyle>,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let _ = write!(
            out,
            r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
            x = fmt(event.x),
            y = fmt(event.y),
        );
        out.checkpoint()?;
        render_node(
            out,
            diagram_id,
            node_count,
            event,
            ordinal,
            direction,
            is_redux_theme,
            is_neo,
            true,
            native,
            work_meter,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Timeline node emission uses both layout geometry and the resolved diagram style."
    )]
    fn render_task(
        out: &mut impl SvgOutput,
        diagram_id: SvgId<'_>,
        node_count: &mut usize,
        task: &TimelineTaskLayout,
        ordinal: usize,
        direction: TimelineDirection,
        is_redux_theme: bool,
        is_neo: bool,
        native: Option<&TimelineNativeStyle>,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let node = &task.node;
        let _ = write!(
            out,
            r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        out.checkpoint()?;
        render_node(
            out,
            diagram_id,
            node_count,
            node,
            ordinal,
            direction,
            is_redux_theme,
            is_neo,
            false,
            native,
            work_meter,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        match direction {
            TimelineDirection::LeftToRight => {
                for connector in &task.connectors {
                    work_meter.checkpoint(OperationPhase::Emit)?;
                    write_timeline_connector(out, connector, Some(diagram_id), native)?;
                }
                for event in &task.events {
                    render_event(
                        out,
                        diagram_id,
                        node_count,
                        event,
                        ordinal,
                        direction,
                        is_redux_theme,
                        is_neo,
                        native,
                        work_meter,
                    )?;
                }
            }
            TimelineDirection::TopDown => {
                let marker_id = native.map(|_| diagram_id);
                for (index, event) in task.events.iter().enumerate() {
                    render_event(
                        out,
                        diagram_id,
                        node_count,
                        event,
                        ordinal,
                        direction,
                        is_redux_theme,
                        is_neo,
                        native,
                        work_meter,
                    )?;
                    if let Some(connector) = task.connectors.get(index) {
                        work_meter.checkpoint(OperationPhase::Emit)?;
                        write_timeline_connector(out, connector, marker_id, native)?;
                    }
                }
                for connector in task.connectors.iter().skip(task.events.len()) {
                    work_meter.checkpoint(OperationPhase::Emit)?;
                    write_timeline_connector(out, connector, marker_id, native)?;
                }
            }
        }
        Ok(())
    }

    let root_bounds = root_svg::DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        0.0,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width));

    let mut out = BoundedSvgOutput::new(work_meter);
    let root_document = viewport.write_open(
        &mut out,
        root_spec,
        root_svg::RootChrome {
            dom: root_svg::RootDomProfile {
                fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
                fixed_style_placement: root_svg::RootStylePlacement::Tail,
                trailing_newline: false,
                ..Default::default()
            },
            ..root_svg::RootChrome::new(diagram_id, "timeline")
        },
    )?;
    out.checkpoint()?;
    let arrowhead_id = if layout.direction == TimelineDirection::TopDown && native.is_none() {
        "undefined-arrowhead".to_string()
    } else {
        scoped_svg_id(diagram_id, "arrowhead").to_string()
    };
    work_meter.checkpoint(OperationPhase::Emit)?;

    // Mermaid's vertical renderer lowers the activity axis to the first root child and invokes
    // marker initialization without a diagram id. Preserve both observable source behaviors.
    if layout.direction == TimelineDirection::TopDown {
        let marker_end = native.map_or_else(
            || "url(#arrowhead)".to_string(),
            |_| scoped_svg_url(diagram_id, "arrowhead").to_string(),
        );
        write_timeline_activity_line(&mut out, &layout.activity_line, marker_end, native)?;
    }

    work_meter.checkpoint(OperationPhase::Emit)?;
    out.push_str("<style>");
    out.checkpoint()?;
    {
        let mut css = String::new();
        if let Some(native) = native {
            let id = CssSelectorDiagramId(diagram_id);
            style.common_css().write_prefix(&mut css, id)?;
            let _ = write!(
                &mut css,
                "#{id} .timeline-node text{{font-weight:{};}}",
                native.node_weight
            );
        } else if let Some(mermaid) = mermaid {
            write_timeline_css(&mut css, diagram_id, mermaid, style.common_css())?;
        }
        XmlTextOutput::new(&mut out).push_str(&css);
    }
    out.checkpoint()?;
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    let mut node_count = 0usize;
    let marker_fill = native
        .map(|style| format!(r#" fill="{}""#, escape_attr(&style.connector)))
        .unwrap_or_default();
    let _ = write!(
        &mut out,
        r#"<defs><marker id="{}" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"{marker_fill}/></marker></defs>"#,
        escape_attr(&arrowhead_id)
    );
    out.checkpoint()?;
    if let Some(theme) = mermaid.filter(|theme| theme.is_redux_theme) {
        let flood_opacity = if is_neo && theme.is_dark_theme {
            "0.2"
        } else {
            "0.06"
        };
        let flood_color = if theme.is_dark_theme {
            "#FFFFFF"
        } else {
            "#000000"
        };
        let _ = write!(
            &mut out,
            r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="{}" flood-color="{}"/></filter></defs>"#,
            escape_attr_display(diagram_id),
            flood_opacity,
            flood_color,
        );
        out.checkpoint()?;
    }
    if let Some(gradient_stops) = style.gradient_stops() {
        let _ = write!(
            &mut out,
            r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#,
            escape_attr_display(diagram_id),
        );
        // Mermaid sets each stop independently. A missing/null variable removes the
        // attribute; neither stop inherits nodeBorder or the other gradient stop.
        for (offset, color) in ["0%", "100%"].into_iter().zip(gradient_stops) {
            let _ = write!(&mut out, r#"<stop offset="{offset}""#);
            if let Some(color) = color {
                let _ = write!(&mut out, r#" stop-color="{}""#, escape_attr(color));
            }
            out.push_str(r#" stop-opacity="1"/>"#);
        }
        out.push_str("</linearGradient></defs>");
        out.checkpoint()?;
    }

    for (ordinal, section) in layout.sections.iter().enumerate() {
        work_meter.checkpoint(OperationPhase::Emit)?;
        let node = &section.node;
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        out.checkpoint()?;
        render_node(
            &mut out,
            diagram_id,
            &mut node_count,
            node,
            ordinal,
            layout.direction,
            is_redux_theme,
            is_neo,
            false,
            native,
            work_meter,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        for task in &section.tasks {
            work_meter.checkpoint(OperationPhase::Emit)?;
            render_task(
                &mut out,
                diagram_id,
                &mut node_count,
                task,
                ordinal,
                layout.direction,
                is_redux_theme,
                is_neo,
                native,
                work_meter,
            )?;
        }
    }

    for (index, task) in layout.orphan_tasks.iter().enumerate() {
        work_meter.checkpoint(OperationPhase::Emit)?;
        let ordinal = if style.layout_settings().disable_multicolor {
            0
        } else {
            index
        };
        render_task(
            &mut out,
            diagram_id,
            &mut node_count,
            task,
            ordinal,
            layout.direction,
            is_redux_theme,
            is_neo,
            native,
            work_meter,
        )?;
    }

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        work_meter.checkpoint(OperationPhase::Emit)?;
        let title_style = style.layout_settings().title_style.as_ref();
        let title_size = title_style
            .filter(|_| style.used_font_size_px().is_some())
            .map_or_else(
                || "4ex".to_string(),
                |style| format!("{}px", fmt(style.font_size)),
            );
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="{title_size}" font-weight="{weight}" y="{y}""#,
            x = fmt(layout.title_x),
            weight = title_style
                .and_then(|style| style.font_weight.as_deref())
                .unwrap_or("bold"),
            y = fmt(layout.title_y),
        );
        if let Some(family) = title_style.and_then(|style| style.font_family.as_deref()) {
            let _ = write!(&mut out, r#" font-family="{}""#, escape_attr(family));
        }
        if let Some(native) = native {
            let _ = write!(&mut out, r#" fill="{}""#, escape_attr(&native.title));
        }
        let _ = write!(&mut out, ">{}</text>", escape_xml(title));
        out.checkpoint()?;
    }

    if layout.direction == TimelineDirection::LeftToRight {
        work_meter.checkpoint(OperationPhase::Emit)?;
        write_timeline_activity_line(
            &mut out,
            &layout.activity_line,
            scoped_svg_url(diagram_id, "arrowhead"),
            native,
        )?;
    }

    out.push_str("</svg>\n");
    work_meter.checkpoint(OperationPhase::Emit)?;
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::tests::presentation_config;
    use merman_model::{family::DiagramFamilyId, presentation::PresentationConfig};
    use merman_render_core::{
        Error, resources::RenderResourcePolicy, svg_css::SvgDiagramIdProjection,
    };
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected {
                Err(Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn timeline_connector_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();
        let connector = TimelineLineLayout {
            kind: "connector".to_string(),
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };

        let error = write_timeline_connector(&mut out, &connector, None, None)
            .expect_err("the rejecting sink must stop Timeline connector emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Timeline connector emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn timeline_css_writer_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();
        let style = TimelineStyle::resolve(
            &merman_model::presentation::PresentationConfig::default(),
            None,
        );
        let error = write_timeline_css(
            &mut XmlTextOutput::new(&mut out),
            "timelineCssFailure",
            style.css_binding().unwrap(),
            style.common_css(),
        )
        .expect_err("the rejecting sink must stop Timeline stylesheet emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(out.write_attempts, 1);
    }

    fn test_axis_layout() -> TimelineDiagramLayout {
        TimelineDiagramLayout {
            direction: merman_model::families::timeline::TimelineDirection::LeftToRight,
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 320.0,
                max_y: 180.0,
            }),
            left_margin: 150.0,
            base_x: 50.0,
            base_y: 50.0,
            pre_title_box_width: 0.0,
            sections: Vec::new(),
            orphan_tasks: Vec::new(),
            activity_line: TimelineLineLayout {
                kind: "activity".to_string(),
                x1: 50.0,
                y1: 0.0,
                x2: 320.0,
                y2: 0.0,
            },
            title: None,
            title_x: 0.0,
            title_y: 20.0,
            use_max_width: false,
        }
    }

    fn render_test_svg(
        layout: &TimelineDiagramLayout,
        style: &TimelineStyle,
        id: &str,
    ) -> root_svg::RootedSvg {
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let projection = SvgDiagramIdProjection::new(&work_meter);
        let id = SvgId::new(id)
            .expect("valid fixture diagram ID")
            .with_projection(&projection);
        let viewport = root_svg::RootViewportContext::new(DiagramFamilyId::TIMELINE, id);
        let svg = render_timeline_svg(layout, style, id, &viewport, &work_meter);
        projection.finish().expect("projected IDs admitted");
        svg.expect("render Timeline SVG")
    }

    #[test]
    fn timeline_root_honors_disabled_max_width() {
        let layout = test_axis_layout();
        let svg = render_test_svg(
            &layout,
            &TimelineStyle::resolve(&PresentationConfig::default(), None),
            "timelineFixed",
        );
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="320""#), "{root_open}");
        assert!(root_open.contains(r#"height="180""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="0 0 320 180""#),
            "{root_open}"
        );
        assert!(!root_open.contains("style="), "{root_open}");
        assert!(!root_open.contains("max-width"), "{root_open}");
    }

    #[test]
    fn timeline_raw_gradient_stops_have_no_node_border_or_other_stop_fallback() {
        let cases = [
            (serde_json::json!({}), None, None),
            (
                serde_json::json!({"gradientStart": null, "gradientStop": null}),
                None,
                None,
            ),
            (
                serde_json::json!({"gradientStart": "#112233"}),
                Some("#112233"),
                None,
            ),
            (
                serde_json::json!({"gradientStop": "#445566"}),
                None,
                Some("#445566"),
            ),
        ];
        for (mut variables, expected_start, expected_stop) in cases {
            variables["nodeBorder"] = serde_json::json!("#abcdef");
            variables["useGradient"] = serde_json::json!(true);
            // This renderer boundary accepts effective config directly. Public config merge
            // materializes null overrides; testing that path cannot exercise absent stop attrs.
            let effective_config = serde_json::json!({
                "theme": "redux", "look": "neo", "themeVariables": variables,
            });
            let config = presentation_config(effective_config);
            let svg = render_test_svg(
                &test_axis_layout(),
                &TimelineStyle::resolve(&config, None),
                "merman",
            );
            let document = roxmltree::Document::parse(&svg).expect("valid Timeline SVG");
            let stops = document
                .descendants()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            assert_eq!(stops.len(), 2);
            assert_eq!(stops[0].attribute("stop-color"), expected_start);
            assert_eq!(stops[1].attribute("stop-color"), expected_stop);
        }
    }
}
