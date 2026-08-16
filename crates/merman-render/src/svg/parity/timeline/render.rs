use super::super::theme::TimelineTheme;
use super::super::*;
use crate::model::{TimelineLineLayout, TimelineNodeLayout, TimelineTaskLayout};
use merman_core::diagrams::timeline::TimelineDiagramRenderModel;

fn timeline_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
    theme: &TimelineTheme,
) -> String {
    let id = crate::svg::escape_css_identifier(diagram_id);
    let fragment_id = escape_xml(diagram_id);

    // Keep `:root` last (matches upstream Mermaid timeline SVG baselines).
    let parts = info_css_parts_with_config(diagram_id, effective_config);
    let root_rule = parts.root_rule;
    let mut out = parts.css_prefix;
    let scoped_drop_shadow = if diagram_id.is_empty() {
        theme.drop_shadow.clone()
    } else {
        format!("url(#{fragment_id}-drop-shadow)")
    };

    let _ = write!(&mut out, r#"#{} .edge{{stroke-width:3;}}"#, id);
    for (i, section_theme) in theme.sections.iter().enumerate() {
        let section = i as i64 - 1;
        let sw = 17 - 3 * (i as i64);

        if theme.is_redux_theme {
            let border_color = theme
                .border_colors
                .get(i)
                .cloned()
                .unwrap_or_else(|| theme.node_border.clone());
            let redux_fill = if theme.is_color_theme && !theme.is_dark_theme {
                border_color.clone()
            } else {
                theme.main_bkg.clone()
            };
            let redux_stroke = if theme.is_color_theme {
                border_color
            } else {
                theme.node_border.clone()
            };
            let _ = write!(
                &mut out,
                r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle{{fill:{};stroke:{};stroke-width:{};filter:{};}}#{} .section-{} text{{fill:{};font-weight:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .lineWrapper line{{stroke:{};stroke-width:{};}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:{};}}#{} .disabled text{{fill:{};}}"#,
                id,
                section,
                id,
                section,
                id,
                section,
                redux_fill,
                redux_stroke,
                theme.stroke_width,
                scoped_drop_shadow,
                id,
                section,
                theme.node_border,
                theme.font_weight,
                id,
                section,
                section_theme.c_scale_label,
                id,
                section,
                section_theme.c_scale,
                id,
                section,
                sw,
                id,
                section,
                section_theme.c_scale_inv,
                id,
                theme.node_border,
                theme.stroke_width,
                id,
                id,
                id,
                theme.disabled_fill,
                id,
                theme.disabled_text_fill,
            );
        } else {
            let _ = write!(
                &mut out,
                r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle,#{} .section-{} path{{fill:{};}}#{} .section-{} text{{fill:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .lineWrapper line{{stroke:{};}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:{};}}#{} .disabled text{{fill:{};}}"#,
                id,
                section,
                id,
                section,
                id,
                section,
                id,
                section,
                section_theme.c_scale,
                id,
                section,
                section_theme.c_scale_label,
                id,
                section,
                section_theme.c_scale_label,
                id,
                section,
                section_theme.c_scale,
                id,
                section,
                sw,
                id,
                section,
                section_theme.c_scale_inv,
                id,
                section_theme.c_scale_label,
                id,
                id,
                id,
                theme.disabled_fill,
                id,
                theme.disabled_text_fill,
            );
        }
    }

    let _ = write!(
        &mut out,
        r#"#{} .section-root rect,#{} .section-root path,#{} .section-root circle{{fill:{};}}#{} .section-root text{{fill:{};}}#{} .icon-container{{height:100%;display:flex;justify-content:center;align-items:center;}}#{} .edge{{fill:none;}}#{} .eventWrapper{{filter:brightness(120%);}}"#,
        id, id, id, theme.root_fill, id, theme.root_label, id, id, id
    );

    out.push_str(&root_rule);
    out
}

fn write_timeline_connector(
    out: &mut impl SvgOutput,
    connector: &TimelineLineLayout,
    marker_url: &str,
) -> Result<()> {
    let _ = write!(
        out,
        r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="{marker_end}" stroke-dasharray="5,5"/></g>"#,
        x1 = fmt(connector.x1),
        y1 = fmt(connector.y1),
        x2 = fmt(connector.x2),
        y2 = fmt(connector.y2),
        marker_end = escape_attr(marker_url),
    );
    out.checkpoint()
}

pub(crate) fn render_timeline_diagram_svg_model(
    layout: &TimelineDiagramLayout,
    _model: &TimelineDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_timeline_diagram_svg_inner(layout, effective_config, diagram_title, measurer, options)
}

fn render_timeline_diagram_svg_inner(
    layout: &TimelineDiagramLayout,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let theme = MermaidThemeAdapter::new(effective_config).timeline();
    let is_redux_theme = theme.is_redux_theme;

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

    fn render_node(
        out: &mut impl SvgOutput,
        diagram_id: &str,
        node_count: &mut usize,
        n: &crate::model::TimelineNodeLayout,
        is_redux_theme: bool,
        is_event: bool,
    ) -> Result<()> {
        let node_id = scoped_svg_id(diagram_id, &format!("node-{node_count}"));
        *node_count += 1;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let rd = 5.0;
        let d = if is_redux_theme {
            format!(
                "M0 {y0} v{v1} h{w} v{h} H0 Z",
                y0 = fmt(h - rd),
                v1 = fmt(-(h - rd)),
                w = fmt(w),
                h = fmt(h),
            )
        } else {
            format!(
                "M0 {y0} v{v1} q0,-5 5,-5 h{hw} q5,0 5,5 v{v2} H0 Z",
                y0 = fmt(h - rd),
                v1 = fmt(-h + 2.0 * rd),
                hw = fmt(w - 2.0 * rd),
                v2 = fmt(h - rd),
            )
        };

        let _ = write!(
            out,
            r#"<g class="timeline-node {section_class}">"#,
            section_class = escape_attr(&n.section_class)
        );
        out.checkpoint()?;
        out.push_str("<g>");
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"<path id="{node_id}" class="node-bkg node-undefined" d="{d}"/>"#,
            node_id = escape_attr(&node_id),
            d = escape_attr(&d)
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
        }
        out.push_str("</text></g></g>");
        out.checkpoint()
    }

    fn render_event(
        out: &mut impl SvgOutput,
        diagram_id: &str,
        node_count: &mut usize,
        event: &TimelineNodeLayout,
        is_redux_theme: bool,
    ) -> Result<()> {
        let _ = write!(
            out,
            r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
            x = fmt(event.x),
            y = fmt(event.y)
        );
        out.checkpoint()?;
        render_node(out, diagram_id, node_count, event, is_redux_theme, true)?;
        out.push_str("</g>");
        out.checkpoint()
    }

    fn render_task(
        out: &mut impl SvgOutput,
        diagram_id: &str,
        node_count: &mut usize,
        task: &TimelineTaskLayout,
        direction: merman_core::diagrams::timeline::TimelineDirection,
        is_redux_theme: bool,
        arrowhead_url: &str,
    ) -> Result<()> {
        let node = &task.node;
        let _ = write!(
            out,
            r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        out.checkpoint()?;
        render_node(out, diagram_id, node_count, node, is_redux_theme, false)?;
        out.push_str("</g>");
        out.checkpoint()?;

        match direction {
            merman_core::diagrams::timeline::TimelineDirection::LeftToRight => {
                for connector in &task.connectors {
                    write_timeline_connector(out, connector, arrowhead_url)?;
                }
                for event in &task.events {
                    render_event(out, diagram_id, node_count, event, is_redux_theme)?;
                }
            }
            merman_core::diagrams::timeline::TimelineDirection::TopDown => {
                for (index, event) in task.events.iter().enumerate() {
                    render_event(out, diagram_id, node_count, event, is_redux_theme)?;
                    if let Some(connector) = task.connectors.get(index) {
                        write_timeline_connector(out, connector, arrowhead_url)?;
                    }
                }
                for connector in task.connectors.iter().skip(task.events.len()) {
                    write_timeline_connector(out, connector, arrowhead_url)?;
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
    let (arrowhead_id, arrowhead_url) =
        if layout.direction == merman_core::diagrams::timeline::TimelineDirection::TopDown {
            (
                "undefined-arrowhead".to_string(),
                "url(#arrowhead)".to_string(),
            )
        } else {
            (
                scoped_svg_id(diagram_id, "arrowhead"),
                scoped_svg_url(diagram_id, "arrowhead"),
            )
        };

    // Mermaid's vertical renderer lowers the activity axis to the first root child and invokes
    // marker initialization without a diagram id. Preserve both observable source behaviors.
    if layout.direction == merman_core::diagrams::timeline::TimelineDirection::TopDown {
        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="{marker_end}"/></g>"#,
            x1 = fmt(layout.activity_line.x1),
            y1 = fmt(layout.activity_line.y1),
            x2 = fmt(layout.activity_line.x2),
            y2 = fmt(layout.activity_line.y2),
            marker_end = escape_attr(&arrowhead_url),
        );
        out.checkpoint()?;
    }

    let css = timeline_css(diagram_id, effective_config, &theme);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    drop(css);
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

    for section in &layout.sections {
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
            false,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        for task in &section.tasks {
            render_task(
                &mut out,
                diagram_id,
                &mut node_count,
                task,
                layout.direction,
                is_redux_theme,
                &arrowhead_url,
            )?;
        }
    }

    for task in &layout.orphan_tasks {
        render_task(
            &mut out,
            diagram_id,
            &mut node_count,
            task,
            layout.direction,
            is_redux_theme,
            &arrowhead_url,
        )?;
    }

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="4ex" font-weight="bold" y="{y}">{text}</text>"#,
            x = fmt(layout.title_x),
            y = fmt(layout.title_y),
            text = escape_xml(title)
        );
        out.checkpoint()?;
    }

    if layout.direction == merman_core::diagrams::timeline::TimelineDirection::LeftToRight {
        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="{marker_end}"/></g>"#,
            x1 = fmt(layout.activity_line.x1),
            y1 = fmt(layout.activity_line.y1),
            x2 = fmt(layout.activity_line.x2),
            y2 = fmt(layout.activity_line.y2),
            marker_end = escape_attr(&arrowhead_url),
        );
        out.checkpoint()?;
    }

    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
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

        let error = write_timeline_connector(&mut out, &connector, "url(#arrowhead)")
            .expect_err("the rejecting sink must stop Timeline connector emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Timeline connector emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn timeline_root_honors_disabled_max_width() {
        let layout = TimelineDiagramLayout {
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
        };
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

        let svg = render_timeline_diagram_svg_inner(
            &layout,
            &serde_json::json!({}),
            None,
            &crate::text::DeterministicTextMeasurer::default(),
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
        assert!(
            root_open.contains(r#"style="background-color: white;""#),
            "{root_open}"
        );
        assert!(!root_open.contains("max-width"), "{root_open}");
    }
}
