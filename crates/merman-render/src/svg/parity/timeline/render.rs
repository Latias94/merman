use super::super::*;
use crate::model::{TimelineLineLayout, TimelineNodeLayout, TimelineTaskLayout};
use crate::timeline::TimelineCssBinding;

fn write_timeline_css(
    out: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
    theme: &TimelineCssBinding,
    typography: &crate::timeline::TimelineTypographyThemePlan,
    event_emission: &mut TimelineEventEmissionState<'_>,
) -> Result<()> {
    // Keep `:root` last (matches upstream Mermaid timeline SVG baselines).
    let common = typography.common_css();
    common.write_prefix_with_font_emission(out, diagram_id)?;
    let _ = write!(out, r#"#{} .edge{{stroke-width:3;}}"#, diagram_id);
    for (i, section_theme) in theme.sections.iter().enumerate() {
        let section = i as i64 - 1;
        let sw = 17 - 3 * (i as i64);

        if theme.is_redux_theme {
            let redux_stroke = if theme.is_color_theme {
                theme.border_colors.get(i).map(String::as_str)
            } else {
                Some(theme.node_border.as_str())
            };
            let _ = write!(
                out,
                r#"#{diagram_id} .section-{section} rect,#{diagram_id} .section-{section} path,#{diagram_id} .section-{section} circle{{"#,
            );
            if let Some(fill) = timeline_section_fill(theme, i, false) {
                let _ = write!(out, "fill:{fill};");
            }
            if let Some(stroke) = event_emission
                .theme
                .shape_stroke_for_redux()
                .or(redux_stroke)
            {
                let _ = write!(out, "stroke:{stroke};");
            }
            let _ = write!(
                out,
                r#"stroke-width:{};filter:{};}}#{diagram_id} .section-{section} text{{"#,
                theme.stroke_width,
                scoped_svg_url(diagram_id, "drop-shadow"),
            );
            let typed_stroke = event_emission.theme.stroke_for_redux();
            let node_border = typed_stroke.unwrap_or(&theme.node_border);
            let _ = write!(out, "fill:{node_border};");
            let _ = write!(
                out,
                r#"font-weight:{};}}#{diagram_id} .node-icon-{section}{{font-size:40px;color:{};}}#{diagram_id} .section-edge-{section}{{stroke:{};}}#{diagram_id} .edge-depth-{section}{{stroke-width:{sw};}}#{diagram_id} .section-{section} line{{stroke:{};stroke-width:3;}}#{diagram_id} .lineWrapper line{{"#,
                theme.font_weight,
                section_theme.c_scale_label,
                section_theme.c_scale,
                section_theme.c_scale_inv,
            );
            let _ = write!(out, "stroke:{node_border};");
            out.checkpoint()?;
            if typed_stroke.is_some()
                && let Some(outcome) = &mut event_emission.outcome
            {
                outcome.record_stroke_css();
            }
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
                timeline_section_fill(theme, i, false).expect("existing section"),
                diagram_id,
                section,
                timeline_section_label_fill(theme, i, None).expect("existing section"),
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
    common.write_root_with_font_emission(out, diagram_id, diagram_id)?;
    Ok(())
}

fn timeline_section_fill(
    theme: &TimelineCssBinding,
    slot: usize,
    neo_gradient: bool,
) -> Option<&str> {
    let section = theme.sections.get(slot)?;
    Some(if neo_gradient {
        &theme.main_bkg
    } else if theme.is_redux_theme {
        if theme.is_color_theme && !theme.is_dark_theme {
            return theme.border_colors.get(slot).map(String::as_str);
        } else {
            &theme.main_bkg
        }
    } else {
        &section.c_scale
    })
}

fn timeline_section_label_fill<'a>(
    theme: &'a TimelineCssBinding,
    slot: usize,
    event_stroke: Option<&'a str>,
) -> Option<&'a str> {
    let section = theme.sections.get(slot)?;
    Some(if theme.is_redux_theme {
        event_stroke.unwrap_or(&theme.node_border)
    } else {
        &section.c_scale_label
    })
}

struct TimelineTextPaintEmissionState<'a> {
    plan: &'a crate::timeline::TimelineTextPaintPlan,
    theme: &'a TimelineCssBinding,
    neo_gradient: bool,
    event_stroke: Option<&'a str>,
    outcome: Option<crate::timeline::TimelineTextPaintOutcome>,
}

impl TimelineTextPaintEmissionState<'_> {
    fn record_node(
        &mut self,
        node: &TimelineNodeLayout,
        slot: Option<usize>,
        inline_fill: Option<&str>,
    ) {
        let Some(outcome) = &mut self.outcome else {
            return;
        };
        let shape_fill = inline_fill.or_else(|| {
            slot.and_then(|slot| timeline_section_fill(self.theme, slot, self.neo_gradient))
        });
        let label_fill =
            slot.and_then(|slot| timeline_section_label_fill(self.theme, slot, self.event_stroke));
        outcome.record_node(
            shape_fill,
            label_fill,
            node.label_lines.iter().any(|line| !line.trim().is_empty()),
        );
    }

    fn finish(self) {
        if let Some(outcome) = self.outcome {
            self.plan.record_emission(outcome);
        }
    }
}

fn write_timeline_connector(
    out: &mut impl SvgOutput,
    connector: &TimelineLineLayout,
    diagram_id: Option<SvgDiagramId<'_>>,
) -> Result<()> {
    fn write_connector(
        out: &mut impl SvgOutput,
        connector: &TimelineLineLayout,
        marker_end: impl std::fmt::Display,
    ) {
        let _ = write!(
            out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="{marker_end}" stroke-dasharray="5,5"/></g>"#,
            x1 = fmt(connector.x1),
            y1 = fmt(connector.y1),
            x2 = fmt(connector.x2),
            y2 = fmt(connector.y2),
        );
    }

    match diagram_id {
        Some(diagram_id) => write_connector(
            out,
            connector,
            escape_attr_display(scoped_svg_url(diagram_id, "arrowhead")),
        ),
        None => write_connector(out, connector, "url(#arrowhead)"),
    }
    out.checkpoint()
}

fn write_timeline_event_wrapper_open(
    out: &mut impl SvgOutput,
    event: &TimelineNodeLayout,
    opacity_token: Option<&str>,
) -> Result<()> {
    match opacity_token {
        Some(opacity_token) => {
            let _ = write!(
                out,
                r#"<g class="eventWrapper" opacity="{opacity}" transform="translate({x}, {y})">"#,
                opacity = opacity_token,
                x = fmt(event.x),
                y = fmt(event.y)
            );
        }
        None => {
            let _ = write!(
                out,
                r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
                x = fmt(event.x),
                y = fmt(event.y)
            );
        }
    }
    out.checkpoint()
}

fn negative_radius_token(token: &str, radius_value: f64) -> String {
    if radius_value == 0.0 {
        "0".to_string()
    } else {
        format!("-{token}")
    }
}

struct TimelineEventEmissionState<'a> {
    theme: &'a crate::timeline::TimelineEventTheme,
    next_event_index: usize,
    outcome: Option<crate::timeline::TimelineEventThemeOutcome>,
}

struct TimelineTypographyEmissionState<'a> {
    plan: &'a crate::timeline::TimelineTypographyThemePlan,
    outcome: Option<crate::timeline::TimelineTypographyOutcome>,
}

impl<'a> TimelineTypographyEmissionState<'a> {
    fn new(plan: &'a crate::timeline::TimelineTypographyThemePlan) -> Self {
        Self {
            plan,
            outcome: plan.begin_emission(),
        }
    }

    fn record_base_text_run(&mut self, text: &str) {
        if let Some(outcome) = &mut self.outcome {
            outcome.record_base_text_run(text);
        }
    }

    fn finish(self) {
        if let Some(outcome) = self.outcome {
            self.plan.record_emission(outcome);
        }
    }
}

impl<'a> TimelineEventEmissionState<'a> {
    fn new(theme: &'a crate::timeline::TimelineEventTheme) -> Self {
        Self {
            theme,
            next_event_index: 0,
            outcome: theme.begin_emission(),
        }
    }

    fn record_stroke_line(&mut self) {
        if let Some(outcome) = &mut self.outcome {
            outcome.record_stroke_line();
        }
    }

    fn record_palette_node(&mut self, slot: Option<usize>, palette_fill_written: bool) {
        if let Some(outcome) = &mut self.outcome {
            outcome.record_palette_node(self.theme, slot, palette_fill_written);
        }
    }

    fn record_event(&mut self, event_index: usize) {
        if let Some(outcome) = &mut self.outcome {
            outcome.record_event(self.theme, event_index);
        }
        self.next_event_index += 1;
    }

    fn finish(self) {
        if let Some(outcome) = self.outcome {
            self.theme.record_emission(outcome);
        }
    }
}

pub(crate) fn render_timeline_diagram_svg_model(
    layout: &TimelineDiagramLayout,
    event_theme: &crate::timeline::TimelineEventTheme,
    typography_theme: &crate::timeline::TimelineTypographyThemePlan,
    text_paint: &crate::timeline::TimelineTextPaintPlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let theme = typography_theme.css_binding();
    let is_redux_theme = theme.is_redux_theme;
    let is_neo = theme.is_neo;
    let use_neo_gradient = theme.use_neo_gradient;

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
        reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
    )]
    fn render_node(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        n: &crate::model::TimelineNodeLayout,
        is_redux_theme: bool,
        is_neo: bool,
        is_event: bool,
        event_index: Option<usize>,
        event_radius: Option<(&str, f64)>,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        text_emission: &mut TimelineTextPaintEmissionState<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        let node_local_id = format!("node-{node_count}");
        let node_id = scoped_svg_id(diagram_id, &node_local_id);
        *node_count += 1;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let rd = event_radius.map_or(crate::timeline::MERMAID_EVENT_RADIUS_PX, |(_, value)| value);
        let d = if let Some((radius_token, radius_value)) = event_radius {
            let negative_radius = negative_radius_token(radius_token, radius_value);
            format!(
                "M0 {y0} v{v1} q0,{negative_radius} {rd},{negative_radius} h{hw} q{rd},0 {rd},{rd} v{v2} H0 Z",
                y0 = fmt(h - radius_value),
                v1 = fmt(-h + 2.0 * radius_value),
                rd = radius_token,
                hw = fmt(w - 2.0 * radius_value),
                v2 = fmt(h - radius_value),
            )
        } else if is_redux_theme {
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
                radius = crate::timeline::MERMAID_EVENT_RADIUS_TOKEN,
                hw = fmt(w - 2.0 * rd),
                v2 = fmt(h - rd),
            )
        };

        let event_theme = event_emission.theme;
        let node_palette_slot = event_theme.palette_slot_for_section(&n.section_class);
        let palette_fill =
            node_palette_slot.and_then(|slot| event_theme.palette_fill_for_slot(slot));
        let palette_line_stroke =
            node_palette_slot.and_then(|slot| event_theme.palette_line_stroke_for_slot(slot));
        let direct_fill =
            event_index.and_then(|event_index| event_theme.fill_for_event(event_index));
        let effective_fill = direct_fill.or(palette_fill);
        let fill_style = effective_fill
            .map(|fill| format!(r#" style="fill:{};""#, escape_attr(fill)))
            .unwrap_or_default();
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
        let _ = write!(
            out,
            r#"<path id="{node_id}" class="node-bkg node-undefined" d="{d}"{fill_style}/>"#,
            node_id = escape_attr_display(node_id),
            d = escape_attr(&d),
            fill_style = fill_style,
        );
        out.checkpoint()?;
        if !is_redux_theme {
            let line_style = palette_line_stroke
                .map(|stroke| format!(r#" style="stroke:{};""#, escape_attr(stroke)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"<line class="{line_class}"{line_style} x1="0" y1="{y}" x2="{x2}" y2="{y}"/>"#,
                line_class = escape_attr(&node_line_class(&n.section_class)),
                line_style = line_style,
                y = fmt(h),
                x2 = fmt(w)
            );
            out.checkpoint()?;
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
        out.push_str(r#"<text dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle">"#);
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
            typography_emission.record_base_text_run(line);
        }
        out.push_str("</text></g></g>");
        out.checkpoint()?;
        event_emission.record_palette_node(
            node_palette_slot,
            direct_fill.is_none() && palette_fill.is_some(),
        );
        text_emission.record_node(n, node_palette_slot, effective_fill);
        options.checkpoint_emit()?;
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
    )]
    fn render_event(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        event: &TimelineNodeLayout,
        is_redux_theme: bool,
        is_neo: bool,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        text_emission: &mut TimelineTextPaintEmissionState<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        let event_index = event_emission.next_event_index;
        let event_theme = event_emission.theme;
        write_timeline_event_wrapper_open(
            out,
            event,
            event_theme.opacity_token_for_event(event_index),
        )?;
        render_node(
            out,
            diagram_id,
            node_count,
            event,
            is_redux_theme,
            is_neo,
            true,
            Some(event_index),
            event_theme.radius_for_event(event_index),
            event_emission,
            typography_emission,
            text_emission,
            options,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;
        event_emission.record_event(event_index);
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
    )]
    fn render_task(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        task: &TimelineTaskLayout,
        direction: merman_core::diagrams::timeline::TimelineDirection,
        is_redux_theme: bool,
        is_neo: bool,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        text_emission: &mut TimelineTextPaintEmissionState<'_>,
        options: &SvgExecution<'_>,
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
            is_redux_theme,
            is_neo,
            false,
            None,
            None,
            event_emission,
            typography_emission,
            text_emission,
            options,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        match direction {
            merman_core::diagrams::timeline::TimelineDirection::LeftToRight => {
                for connector in &task.connectors {
                    options.checkpoint_emit()?;
                    write_timeline_connector(out, connector, Some(diagram_id))?;
                    event_emission.record_stroke_line();
                }
                for event in &task.events {
                    render_event(
                        out,
                        diagram_id,
                        node_count,
                        event,
                        is_redux_theme,
                        is_neo,
                        event_emission,
                        typography_emission,
                        text_emission,
                        options,
                    )?;
                }
            }
            merman_core::diagrams::timeline::TimelineDirection::TopDown => {
                for (index, event) in task.events.iter().enumerate() {
                    render_event(
                        out,
                        diagram_id,
                        node_count,
                        event,
                        is_redux_theme,
                        is_neo,
                        event_emission,
                        typography_emission,
                        text_emission,
                        options,
                    )?;
                    if let Some(connector) = task.connectors.get(index) {
                        options.checkpoint_emit()?;
                        write_timeline_connector(out, connector, None)?;
                        event_emission.record_stroke_line();
                    }
                }
                for connector in task.connectors.iter().skip(task.events.len()) {
                    options.checkpoint_emit()?;
                    write_timeline_connector(out, connector, None)?;
                    event_emission.record_stroke_line();
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

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::TIMELINE, diagram_id)
            .write_open(
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
    let arrowhead_id =
        if layout.direction == merman_core::diagrams::timeline::TimelineDirection::TopDown {
            "undefined-arrowhead".to_string()
        } else {
            scoped_svg_id(diagram_id, "arrowhead").to_string()
        };
    options.checkpoint_emit()?;

    let mut event_emission = TimelineEventEmissionState::new(event_theme);

    // Mermaid's vertical renderer lowers the activity axis to the first root child and invokes
    // marker initialization without a diagram id. Preserve both observable source behaviors.
    if layout.direction == merman_core::diagrams::timeline::TimelineDirection::TopDown {
        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="url(#arrowhead)"/></g>"#,
            x1 = fmt(layout.activity_line.x1),
            y1 = fmt(layout.activity_line.y1),
            x2 = fmt(layout.activity_line.x2),
            y2 = fmt(layout.activity_line.y2),
        );
        out.checkpoint()?;
        event_emission.record_stroke_line();
    }

    let mut typography_emission = TimelineTypographyEmissionState::new(typography_theme);
    let mut text_emission = TimelineTextPaintEmissionState {
        plan: text_paint,
        theme,
        neo_gradient: use_neo_gradient,
        event_stroke: event_theme.stroke_for_redux(),
        outcome: text_paint.begin_emission(),
    };
    options.checkpoint_emit()?;
    out.push_str("<style>");
    out.checkpoint()?;
    {
        let mut css = String::new();
        write_timeline_css(
            &mut css,
            diagram_id,
            theme,
            typography_theme,
            &mut event_emission,
        )?;
        super::super::output::XmlTextOutput::new(&mut out).push_str(&css);
    }
    out.checkpoint()?;
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    let mut node_count = 0usize;
    let _ = write!(
        &mut out,
        r#"<defs><marker id="{}" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
        escape_attr(&arrowhead_id)
    );
    out.checkpoint()?;
    if is_redux_theme {
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
    if use_neo_gradient {
        let _ = write!(
            &mut out,
            r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#,
            escape_attr_display(diagram_id),
        );
        // Mermaid sets each stop independently. A missing/null variable removes the
        // attribute; neither stop inherits nodeBorder or the other gradient stop.
        for (offset, color) in ["0%", "100%"].into_iter().zip(&theme.gradient_stops) {
            let _ = write!(&mut out, r#"<stop offset="{offset}""#);
            if let Some(color) = color {
                let _ = write!(&mut out, r#" stop-color="{}""#, escape_attr(color));
            }
            out.push_str(r#" stop-opacity="1"/>"#);
        }
        out.push_str("</linearGradient></defs>");
        out.checkpoint()?;
    }

    for section in &layout.sections {
        options.checkpoint_emit()?;
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
            is_redux_theme,
            is_neo,
            false,
            None,
            None,
            &mut event_emission,
            &mut typography_emission,
            &mut text_emission,
            options,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        for task in &section.tasks {
            options.checkpoint_emit()?;
            render_task(
                &mut out,
                diagram_id,
                &mut node_count,
                task,
                layout.direction,
                is_redux_theme,
                is_neo,
                &mut event_emission,
                &mut typography_emission,
                &mut text_emission,
                options,
            )?;
        }
    }

    for task in &layout.orphan_tasks {
        options.checkpoint_emit()?;
        render_task(
            &mut out,
            diagram_id,
            &mut node_count,
            task,
            layout.direction,
            is_redux_theme,
            is_neo,
            &mut event_emission,
            &mut typography_emission,
            &mut text_emission,
            options,
        )?;
    }

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="4ex" font-weight="bold" y="{y}">{text}</text>"#,
            x = fmt(layout.title_x),
            y = fmt(layout.title_y),
            text = escape_xml(title)
        );
        out.checkpoint()?;
        if let Some(outcome) = &mut text_emission.outcome {
            outcome.record_title();
        }
    }

    if layout.direction == merman_core::diagrams::timeline::TimelineDirection::LeftToRight {
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="{marker_end}"/></g>"#,
            x1 = fmt(layout.activity_line.x1),
            y1 = fmt(layout.activity_line.y1),
            x2 = fmt(layout.activity_line.x2),
            y2 = fmt(layout.activity_line.y2),
            marker_end = scoped_svg_url(diagram_id, "arrowhead"),
        );
        out.checkpoint()?;
        event_emission.record_stroke_line();
        if let Some(outcome) = &mut text_emission.outcome {
            outcome.record_marker_reference();
        }
    }

    out.push_str("</svg>\n");
    options.checkpoint_emit()?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    event_emission.finish();
    typography_emission.finish();
    text_emission.finish();
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::model::{Bounds, TimelineDiagramLayout, TimelineLineLayout};
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

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
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

        let error = write_timeline_connector(&mut out, &connector, None)
            .expect_err("the rejecting sink must stop Timeline connector emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Timeline connector emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn timeline_css_writer_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();
        let typography = crate::timeline::TimelineTypographyThemePlan::resolve(
            None,
            &merman_core::MermaidConfig::default(),
        );
        let event_theme = crate::timeline::TimelineEventTheme::baseline();
        let mut event_emission = TimelineEventEmissionState::new(&event_theme);

        let error = write_timeline_css(
            &mut super::super::super::output::XmlTextOutput::new(&mut out),
            "timelineCssFailure",
            typography.css_binding(),
            &typography,
            &mut event_emission,
        )
        .expect_err("the rejecting sink must stop Timeline stylesheet emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(out.write_attempts, 1);
    }

    fn test_axis_layout() -> TimelineDiagramLayout {
        TimelineDiagramLayout {
            direction: merman_core::diagrams::timeline::TimelineDirection::LeftToRight,
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

    #[test]
    fn timeline_root_honors_disabled_max_width() {
        let layout = test_axis_layout();
        let options = SvgRenderOptions {
            diagram_id: Some("timelineFixed".to_string()),
            ..Default::default()
        };
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session");
        let debug = SvgDebugOptions::default();
        let execution =
            SvgExecution::unthemed_for_test(&options, &debug, &session, DiagramFamilyId::TIMELINE)
                .expect("SVG execution");

        let svg = render_timeline_diagram_svg_model(
            &layout,
            &crate::timeline::TimelineEventTheme::baseline(),
            &crate::timeline::TimelineTypographyThemePlan::resolve(
                None,
                &merman_core::MermaidConfig::default(),
            ),
            &crate::timeline::TimelineTextPaintPlan::resolve(
                None,
                &merman_core::MermaidConfig::default(),
                execution.work_meter(),
            )
            .expect("text paint"),
            &execution,
        )
        .unwrap();
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
            let config = merman_core::MermaidConfig::from_value(effective_config.clone());
            let options = SvgRenderOptions::default();
            let debug = SvgDebugOptions::default();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .expect("render session");
            let execution = SvgExecution::unthemed_for_test(
                &options,
                &debug,
                &session,
                DiagramFamilyId::TIMELINE,
            )
            .expect("SVG execution");
            let svg = render_timeline_diagram_svg_model(
                &test_axis_layout(),
                &crate::timeline::TimelineEventTheme::baseline(),
                &crate::timeline::TimelineTypographyThemePlan::resolve(None, &config),
                &crate::timeline::TimelineTextPaintPlan::resolve(
                    None,
                    &config,
                    execution.work_meter(),
                )
                .expect("text paint"),
                &execution,
            )
            .expect("raw effective-config Timeline SVG");
            let document = crate::xml_dom::Document::parse(&svg).expect("valid Timeline SVG");
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
