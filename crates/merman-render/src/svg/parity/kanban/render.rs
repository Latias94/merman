use super::super::*;
use crate::kanban::{KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX, KANBAN_SECTION_PADDING_PX};

fn kanban_css(diagram_id: &str, effective_config: &serde_json::Value) -> Result<String> {
    let id = escape_xml(diagram_id);
    let parts = info_css_parts_with_config(diagram_id, effective_config);
    let theme = PresentationTheme::new(effective_config).kanban()?;
    let mut out = parts.css_prefix;
    let root_rule = parts.root_rule;

    let _ = write!(&mut out, r#"#{} .edge{{stroke-width:3;}}"#, id);
    for (i, section_theme) in theme.sections.iter().enumerate() {
        let section = i as i64 - 1;
        let sw = 17_i64 - 3_i64 * (i as i64);
        let _ = write!(
            &mut out,
            r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle,#{} .section-{} polygon,#{} .section-{} path{{fill:{};stroke:{};}}#{} .section-{} text{{fill:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:lightgray;}}#{} .disabled text{{fill:#efefef;}}#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:1px;}}#{} .kanban-ticket-link{{fill:{};stroke:{};text-decoration:underline;}}"#,
            id,
            section,
            id,
            section,
            id,
            section,
            id,
            section,
            id,
            section,
            section_theme.section_fill,
            section_theme.section_fill,
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
            id,
            id,
            id,
            id,
            id,
            id,
            id,
            id,
            theme.background,
            theme.node_border,
            id,
            theme.background,
            theme.node_border
        );
    }

    let _ = write!(
        &mut out,
        r#"#{} .section-root rect,#{} .section-root path,#{} .section-root circle,#{} .section-root polygon{{fill:{};}}#{} .section-root text{{fill:{};}}#{} .icon-container{{height:100%;display:flex;justify-content:center;align-items:center;}}#{} .edge{{fill:none;}}#{} .cluster-label,#{} .label{{color:{};fill:{};}}#{} .kanban-label{{dy:1em;alignment-baseline:middle;text-anchor:middle;dominant-baseline:middle;text-align:center;}}#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}"#,
        id,
        id,
        id,
        id,
        theme.root_fill,
        id,
        theme.root_label,
        id,
        id,
        id,
        id,
        theme.text_color,
        theme.text_color,
        id,
        id,
        id
    );
    out.push_str(&root_rule);
    Ok(out)
}

fn kanban_dom_id(diagram_id: &str, raw_id: &str) -> String {
    if diagram_id.is_empty() {
        raw_id.to_string()
    } else {
        format!("{diagram_id}-{raw_id}")
    }
}

pub(crate) fn render_kanban_diagram_svg(
    layout: &crate::model::KanbanDiagramLayout,
    effective_config: &serde_json::Value,
    text_measurer: &dyn crate::text::TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let mut out = String::new();
    let root_bounds = root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "kanban");
    root_chrome.dom = root_svg::RootDomProfile {
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Kanban, diagram_id)
            .write_open(
                &mut out,
                root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width)
                    .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(vb_w)),
                root_chrome,
            )?;

    let css = kanban_css(diagram_id, effective_config)?;
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    let render_settings = crate::kanban::KanbanConfigView::new(effective_config).render_settings();
    let data_look = render_settings.look;
    let data_look_attr = escape_attr(data_look.as_str());

    // Mermaid emits a single empty <g/> before the diagram content for kanban.
    out.push_str(r#"<g/>"#);

    out.push_str(r#"<g class="sections">"#);
    for s in &layout.sections {
        let left = s.center_x - s.width / 2.0;
        let label_x = left + (s.width - s.label_width.max(0.0)) / 2.0;

        let _ = write!(
            &mut out,
            r##"<g class="cluster undefined section-{idx}" id="{id}" data-look="{look}"><rect style="" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}"/><g class="cluster-label" transform="translate({lx}, {ly})"><foreignObject width="{lw}" height="{fo_h}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {max_w}px; text-align: center;"><span class="nodeLabel"><p>{label}</p></span></div></foreignObject></g></g>"##,
            idx = s.index,
            id = escape_attr(&kanban_dom_id(diagram_id, &s.id)),
            look = data_look_attr,
            rx = fmt(s.rx),
            ry = fmt(s.ry),
            x = fmt(left),
            y = fmt(s.rect_y),
            w = fmt(s.width),
            h = fmt(s.rect_height),
            lx = fmt(label_x),
            ly = fmt(s.rect_y),
            lw = fmt(s.label_width.max(0.0)),
            fo_h = fmt(s.label_height.max(KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX)),
            max_w = fmt(s.width.max(1.0)),
            label = escape_xml(&s.label),
        );
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="items">"#);
    let item_label_inset_x = KANBAN_SECTION_PADDING_PX;

    fn measure_text_metrics_html(
        text_measurer: &dyn crate::text::TextMeasurer,
        text: &str,
        max_width: Option<f64>,
    ) -> crate::text::TextMetrics {
        let style = crate::text::TextStyle::default();
        text_measurer.measure_wrapped(text, &style, max_width, crate::text::WrapMode::HtmlLike)
    }

    fn kanban_ticket_url(ticket_base_url: Option<&str>, ticket: &str) -> Option<String> {
        Some(ticket_base_url?.replace("#TICKET#", ticket))
    }

    fn kanban_priority_stroke(priority: &str) -> Option<&'static str> {
        match priority.trim() {
            "Very High" => Some("red"),
            "High" => Some("orange"),
            "Medium" => None,
            "Low" => Some("blue"),
            "Very Low" => Some("lightblue"),
            _ => None,
        }
    }

    for n in &layout.items {
        let max_w = (n.width - item_label_inset_x).max(0.0);
        let rect_x = -n.width / 2.0;
        let rect_y = -n.height / 2.0;

        // The upstream kanban item shape (`kanbanItem.ts`) positions labels relative to the title
        // bbox and the "details row" bbox (ticket/assigned). Recompute the same anchor points here
        // so wrapped titles match exactly.
        let title_raw = measure_text_metrics_html(text_measurer, &n.label, None);
        let title_needs_wrap = max_w > 0.0 && title_raw.width > max_w;
        let title_metrics = if title_needs_wrap {
            measure_text_metrics_html(text_measurer, &n.label, Some(max_w))
        } else {
            title_raw
        };

        let ticket_h = n
            .ticket
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| measure_text_metrics_html(text_measurer, t, None).height)
            .unwrap_or(0.0);
        let assigned_metrics = n
            .assigned
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| measure_text_metrics_html(text_measurer, t, None))
            .unwrap_or(crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            });
        let height_adj = (ticket_h.max(assigned_metrics.height)) / 2.0;

        let left_x = rect_x + item_label_inset_x;
        let right_x = if assigned_metrics.width > 0.0 {
            n.width / 2.0 - item_label_inset_x - assigned_metrics.width
        } else {
            n.width / 2.0 - item_label_inset_x
        };

        let title_y = -height_adj - title_metrics.height / 2.0;
        let details_y = -height_adj + title_metrics.height / 2.0;

        let _ = write!(
            &mut out,
            r##"<g class="node undefined" id="{id}" transform="translate({x}, {y})">"##,
            id = escape_attr(&kanban_dom_id(diagram_id, &n.id)),
            x = fmt(n.center_x),
            y = fmt(n.center_y),
        );
        let _ = write!(
            &mut out,
            r##"<rect class="basic label-container __APA__" style="" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}"/>"##,
            rx = fmt(n.rx),
            ry = fmt(n.ry),
            x = fmt(rect_x),
            y = fmt(rect_y),
            w = fmt(n.width),
            h = fmt(n.height),
        );

        fn write_label_group(
            out: &mut String,
            position: (f64, f64),
            max_w: f64,
            text: Option<&str>,
            div_class: Option<&str>,
            wrap_title: bool,
            text_measurer: &dyn crate::text::TextMeasurer,
        ) {
            let (x, y) = position;
            let (fo_w, fo_h, div_style_overrides) = match text {
                Some(t) if !t.is_empty() => {
                    if wrap_title && max_w > 0.0 {
                        let raw = measure_text_metrics_html(text_measurer, t, None);
                        // Keep the title clip region at card content width. A narrow deterministic
                        // glyph estimate can otherwise crop the browser-rendered HTML label.
                        if raw.width > max_w {
                            let wrapped = measure_text_metrics_html(text_measurer, t, Some(max_w));
                            (
                                max_w,
                                wrapped.height,
                                Some(format!(
                                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; width: {mw}px;",
                                    mw = fmt(max_w),
                                )),
                            )
                        } else {
                            (
                                max_w,
                                KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX,
                                Some(format!(
                                    "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px;",
                                    mw = fmt(max_w),
                                )),
                            )
                        }
                    } else {
                        let raw = measure_text_metrics_html(text_measurer, t, None);
                        (
                            raw.width,
                            KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX,
                            Some(format!(
                                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px;",
                                mw = fmt(max_w),
                            )),
                        )
                    }
                }
                _ => (0.0, 0.0, None),
            };
            let class_attr = div_class
                .map(|c| format!(r#" class="{}""#, escape_attr(c)))
                .unwrap_or_default();
            let div_style = if let Some(s) = div_style_overrides {
                format!("text-align: center; {s}")
            } else {
                format!(
                    "text-align: center; display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px;",
                    mw = fmt(max_w),
                )
            };
            let span_class = if wrap_title {
                "nodeLabel markdown-node-label"
            } else {
                "nodeLabel"
            };
            let _ = write!(
                out,
                r##"<g class="label" style="text-align:left !important" transform="translate({x}, {y})"><rect/><foreignObject width="{w}" height="{h}"><div style="{div_style}" xmlns="http://www.w3.org/1999/xhtml"{class_attr}><span style="text-align:left !important" class="{span_class}">"##,
                x = fmt(x),
                y = fmt(y),
                w = fmt(fo_w),
                h = fmt(fo_h),
                div_style = escape_attr(&div_style),
                class_attr = class_attr,
                span_class = span_class,
            );
            if let Some(t) = text.filter(|t| !t.is_empty()) {
                let _ = write!(out, r#"<p>{}</p>"#, escape_xml(t));
            }
            out.push_str("</span></div></foreignObject></g>");
        }

        // Title label (may wrap).
        write_label_group(
            &mut out,
            (left_x, title_y),
            max_w,
            Some(n.label.as_str()),
            n.icon.as_deref().map(|_| "labelBkg"),
            true,
            text_measurer,
        );

        // Ticket label: wrap in <a> when ticketBaseUrl is configured (upstream behavior).
        let ticket_text = n.ticket.as_deref();
        if let Some(t) = ticket_text.filter(|t| !t.is_empty()) {
            if let Some(url) = kanban_ticket_url(render_settings.ticket_base_url.as_deref(), t) {
                let _ = write!(
                    &mut out,
                    r#"<a class="kanban-ticket-link" xlink:href="{}">"#,
                    escape_attr(&url)
                );
                write_label_group(
                    &mut out,
                    (left_x, details_y),
                    max_w,
                    Some(t),
                    None,
                    false,
                    text_measurer,
                );
                out.push_str("</a>");
            } else {
                write_label_group(
                    &mut out,
                    (left_x, details_y),
                    max_w,
                    Some(t),
                    None,
                    false,
                    text_measurer,
                );
            }
        } else {
            write_label_group(
                &mut out,
                (left_x, details_y),
                max_w,
                None,
                None,
                false,
                text_measurer,
            );
        }

        // Assigned label.
        write_label_group(
            &mut out,
            (right_x, details_y),
            max_w,
            n.assigned.as_deref(),
            None,
            false,
            text_measurer,
        );

        if let Some(p) = n.priority.as_deref() {
            let y1 = rect_y + (n.rx / 2.0).floor();
            let y2 = rect_y + n.height - (n.rx / 2.0).floor();
            let stroke_attr = kanban_priority_stroke(p)
                .map(|s| format!(r#" stroke="{}""#, escape_attr(s)))
                .unwrap_or_default();
            let _ = write!(
                &mut out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4"{stroke_attr}/>"#,
                x1 = fmt(rect_x + 2.0),
                y1 = fmt(y1),
                x2 = fmt(rect_x + 2.0),
                y2 = fmt(y2),
                stroke_attr = stroke_attr,
            );
        }

        out.push_str("</g>");
    }

    out.push_str("</g>");
    out.push_str("</svg>\n");
    root_document.complete(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity, TextMeasurementSource,
    };
    use crate::kanban::KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX;
    use crate::model::{Bounds, KanbanDiagramLayout, KanbanItemLayout, KanbanSectionLayout};
    use crate::text::TextMetrics;
    use std::sync::Arc;

    struct WideHost;

    impl HostTextMeasurer for WideHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            let width = match request.operation {
                TextMeasurementOperation::Wrapped => request.max_width.unwrap_or(400.0),
                TextMeasurementOperation::Measure => 400.0,
                _ => return Ok(None),
            };
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width,
                height: 40.0,
                line_count: 2,
            })))
        }
    }

    fn render_test_kanban(
        layout: &KanbanDiagramLayout,
        effective_config: &serde_json::Value,
        options: &SvgRenderOptions,
    ) -> Result<String> {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        with_test_svg_execution(options, |options| {
            render_kanban_diagram_svg(layout, effective_config, &measurer, options)
        })
        .and_then(|svg| svg.into_string_for(crate::family::RenderFamilyKind::Kanban))
    }

    fn attr_f64(tag: &str, name: &str) -> f64 {
        let prefix = format!(r#"{name}=""#);
        let start = tag.find(&prefix).expect("attribute") + prefix.len();
        let end = start + tag[start..].find('"').expect("attribute end");
        tag[start..end].parse().expect("numeric attribute")
    }

    fn foreign_object_before_text<'a>(svg: &'a str, text: &str) -> &'a str {
        let text_end = svg.find(text).expect("label text");
        let start = svg[..text_end]
            .rfind("<foreignObject ")
            .expect("foreignObject before label");
        let end = start + svg[start..].find('>').expect("foreignObject tag end") + 1;
        &svg[start..end]
    }

    #[test]
    fn kanban_css_includes_upstream_theme_rules() {
        let css = kanban_css("k", &serde_json::json!({})).unwrap();

        assert!(
            css.contains(
                "#k .section--1 rect,#k .section--1 path,#k .section--1 circle,#k .section--1 polygon,#k .section--1 path{fill:hsl(240, 100%, 86.2745098039%);stroke:hsl(240, 100%, 86.2745098039%);}"
            ),
            "expected generated section fill/stroke rules: {css}"
        );
        assert!(
            css.contains(
                "#k .node rect,#k .node circle,#k .node ellipse,#k .node polygon,#k .node path{fill:white;stroke:#9370DB;stroke-width:1px;}"
            ),
            "expected kanban item node background rules: {css}"
        );
        assert!(
            css.contains(
                "#k .kanban-ticket-link{fill:white;stroke:#9370DB;text-decoration:underline;}"
            ),
            "expected kanban ticket link styling: {css}"
        );
        assert!(
            css.contains(
                "#k .kanban-label{dy:1em;alignment-baseline:middle;text-anchor:middle;dominant-baseline:middle;text-align:center;}"
            ),
            "expected kanban label styling: {css}"
        );
    }

    #[test]
    fn kanban_dom_ids_are_scoped_by_diagram_id() {
        let layout = KanbanDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 240.0,
                max_y: 180.0,
            }),
            section_width: 200.0,
            padding: KANBAN_SECTION_PADDING_PX,
            max_label_height: KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX,
            viewbox_padding: 8.0,
            use_max_width: true,
            sections: vec![KanbanSectionLayout {
                id: "constructor".to_string(),
                label: "Todo".to_string(),
                index: 1,
                center_x: 100.0,
                center_y: 0.0,
                width: 200.0,
                rect_y: -300.0,
                rect_height: 100.0,
                rx: 5.0,
                ry: 5.0,
                label_width: 40.0,
                label_height: KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX,
            }],
            items: vec![
                KanbanItemLayout {
                    id: "task1".to_string(),
                    label: "Task".to_string(),
                    parent_id: "constructor".to_string(),
                    center_x: 100.0,
                    center_y: -240.0,
                    width: 185.0,
                    height: 44.0,
                    rx: 5.0,
                    ry: 5.0,
                    ticket: None,
                    assigned: None,
                    priority: None,
                    icon: None,
                },
                KanbanItemLayout {
                    id: "__proto__".to_string(),
                    label: "Prototype".to_string(),
                    parent_id: "constructor".to_string(),
                    center_x: 100.0,
                    center_y: -190.0,
                    width: 185.0,
                    height: 44.0,
                    rx: 5.0,
                    ry: 5.0,
                    ticket: None,
                    assigned: None,
                    priority: None,
                    icon: None,
                },
            ],
        };
        let options = SvgRenderOptions {
            diagram_id: Some("kanban_fixture".to_string()),
            ..Default::default()
        };

        let svg = render_test_kanban(&layout, &serde_json::json!({}), &options).unwrap();

        assert!(svg.contains(r#"id="kanban_fixture-constructor""#));
        assert!(svg.contains(r#"id="kanban_fixture-task1""#));
        assert!(svg.contains(r#"id="kanban_fixture-__proto__""#));
        assert!(svg.contains(
            r#"<span style="text-align:left !important" class="nodeLabel markdown-node-label"><p>Task</p></span>"#
        ));
        assert!(!svg.contains(r#"id="constructor""#));
        assert!(!svg.contains(r#"id="task1""#));
        assert!(!svg.contains(r#"id="__proto__""#));
    }

    #[test]
    fn kanban_sections_use_configured_look_in_dom_attributes() {
        let layout = KanbanDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -300.0,
                max_x: 220.0,
                max_y: 80.0,
            }),
            section_width: 200.0,
            padding: KANBAN_SECTION_PADDING_PX,
            max_label_height: KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX,
            viewbox_padding: 8.0,
            use_max_width: true,
            sections: vec![KanbanSectionLayout {
                id: "todo".to_string(),
                label: "Todo".to_string(),
                index: 1,
                center_x: 100.0,
                center_y: 0.0,
                width: 200.0,
                rect_y: -300.0,
                rect_height: 120.0,
                rx: 5.0,
                ry: 5.0,
                label_width: 40.0,
                label_height: KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX,
            }],
            items: Vec::new(),
        };
        let options = SvgRenderOptions {
            diagram_id: Some("kb".to_string()),
            ..Default::default()
        };

        let svg =
            render_test_kanban(&layout, &serde_json::json!({"look": "neo"}), &options).unwrap();

        assert!(
            svg.contains(r#"id="kb-todo" data-look="neo""#),
            "expected kanban section to propagate configured look: {svg}"
        );
        assert!(
            !svg.contains(r#"data-look="classic""#),
            "configured kanban look must not leave classic section attributes: {svg}"
        );
    }

    #[test]
    fn kanban_root_honors_disabled_max_width() {
        let layout = KanbanDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -300.0,
                max_x: 220.0,
                max_y: 80.0,
            }),
            section_width: 200.0,
            padding: KANBAN_SECTION_PADDING_PX,
            max_label_height: KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX,
            viewbox_padding: 8.0,
            use_max_width: false,
            sections: Vec::new(),
            items: Vec::new(),
        };
        let options = SvgRenderOptions {
            diagram_id: Some("kanbanFixed".to_string()),
            ..Default::default()
        };

        let svg = render_test_kanban(&layout, &serde_json::json!({}), &options).unwrap();
        let root_open = svg.split_once('>').expect("root svg open tag").0;
        let view_box = root_open
            .split_once(r#"viewBox=""#)
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(value, _)| {
                value
                    .split_whitespace()
                    .map(|part| part.parse::<f64>().expect("viewBox number"))
                    .collect::<Vec<_>>()
            })
            .expect("root viewBox");
        let bounds = layout.bounds.as_ref().expect("layout bounds");
        let expected_width = bounds.max_x - bounds.min_x;
        let expected_height = bounds.max_y - bounds.min_y;

        assert_eq!(attr_f64(root_open, "width"), expected_width);
        assert_eq!(attr_f64(root_open, "height"), expected_height);
        assert_eq!(
            view_box,
            vec![bounds.min_x, bounds.min_y, expected_width, expected_height]
        );
        assert!(
            root_open.contains(r#"style="background-color: white;""#),
            "{root_open}"
        );
        assert!(!root_open.contains("max-width"), "{root_open}");
    }

    #[test]
    fn kanban_item_title_foreign_object_uses_card_content_width() {
        let layout = KanbanDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -300.0,
                max_x: 220.0,
                max_y: 80.0,
            }),
            section_width: 200.0,
            padding: KANBAN_SECTION_PADDING_PX,
            max_label_height: KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX,
            viewbox_padding: 8.0,
            use_max_width: true,
            sections: vec![KanbanSectionLayout {
                id: "todo".to_string(),
                label: "Todo".to_string(),
                index: 1,
                center_x: 100.0,
                center_y: 0.0,
                width: 200.0,
                rect_y: -300.0,
                rect_height: 120.0,
                rx: 5.0,
                ry: 5.0,
                label_width: 40.0,
                label_height: KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX,
            }],
            items: vec![KanbanItemLayout {
                id: "renderer".to_string(),
                label: "Implement renderer".to_string(),
                parent_id: "todo".to_string(),
                center_x: 100.0,
                center_y: -240.0,
                width: 185.0,
                height: 44.0,
                rx: 5.0,
                ry: 5.0,
                ticket: None,
                assigned: None,
                priority: None,
                icon: None,
            }],
        };
        let options = SvgRenderOptions {
            diagram_id: Some("kb".to_string()),
            ..Default::default()
        };

        let svg = render_test_kanban(&layout, &serde_json::json!({}), &options).unwrap();

        let title_fo = foreign_object_before_text(&svg, "<p>Implement renderer</p>");
        let item = &layout.items[0];
        assert_eq!(
            attr_f64(title_fo, "width"),
            item.width - KANBAN_SECTION_PADDING_PX
        );
        assert!(
            attr_f64(title_fo, "height") >= KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX,
            "title foreignObject must accommodate at least one label row: {title_fo}"
        );
    }

    #[test]
    fn kanban_host_wrap_policy_changes_geometry_and_records_provenance() {
        let layout = KanbanDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -300.0,
                max_x: 220.0,
                max_y: 80.0,
            }),
            section_width: 200.0,
            padding: KANBAN_SECTION_PADDING_PX,
            max_label_height: KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX,
            viewbox_padding: 8.0,
            use_max_width: true,
            sections: Vec::new(),
            items: vec![KanbanItemLayout {
                id: "renderer".to_string(),
                label: "Implement renderer".to_string(),
                parent_id: "todo".to_string(),
                center_x: 100.0,
                center_y: -240.0,
                width: 185.0,
                height: 84.0,
                rx: 5.0,
                ry: 5.0,
                ticket: Some("KAN-1".to_string()),
                assigned: Some("Ada".to_string()),
                priority: None,
                icon: None,
            }],
        };
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.kanban-host").unwrap(),
            "v1",
        )
        .unwrap();
        let environment = RenderEnvironment::parity().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(
                identity,
                Arc::new(WideHost),
                [TextMeasurementPhase::Wrap],
            ),
        );
        let session = environment.begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let options = SvgExecution::new(&request, &debug, &session);

        let svg = render_kanban_diagram_svg(&layout, &serde_json::json!({}), &measurer, &options)
            .unwrap();

        let title_fo = foreign_object_before_text(&svg, "<p>Implement renderer</p>");
        assert_eq!(
            attr_f64(title_fo, "width"),
            layout.items[0].width - KANBAN_SECTION_PADDING_PX
        );
        assert_eq!(attr_f64(title_fo, "height"), 40.0);
        assert!(
            session
                .text_measurement_report()
                .entries()
                .iter()
                .any(|entry| {
                    entry.provenance().phase == TextMeasurementPhase::Wrap
                        && entry.provenance().source == TextMeasurementSource::Host
                })
        );
    }
}
