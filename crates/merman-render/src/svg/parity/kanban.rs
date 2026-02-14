use super::*;

fn kanban_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let mut out = info_css(diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} .edge{{stroke-width:3;}}#{} .edge{{fill:none;}}#{} .cluster-label,#{} .label{{color:#333;fill:#333;}}"#,
        id, id, id, id
    );
    out
}

pub(super) fn render_kanban_diagram_svg(
    layout: &crate::model::KanbanDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

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
    let mut max_w_attr = fmt_max_width_px(vb_w);
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    if let Some((viewbox, max_w)) =
        crate::generated::kanban_root_overrides_11_12_2::lookup_kanban_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{viewbox}" role="graphics-document document" aria-roledescription="kanban">"#,
        diagram_id_esc = diagram_id_esc,
        max_w = max_w_attr,
        viewbox = viewbox_attr,
    );

    let css = kanban_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);

    // Mermaid emits a single empty <g/> before the diagram content for kanban.
    out.push_str(r#"<g/>"#);

    out.push_str(r#"<g class="sections">"#);
    for s in &layout.sections {
        let left = s.center_x - s.width / 2.0;
        let label_x = left + (s.width - s.label_width.max(0.0)) / 2.0;

        let _ = write!(
            &mut out,
            r##"<g class="cluster undefined section-{idx}" id="{id}" data-look="classic"><rect style="" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}"/><g class="cluster-label" transform="translate({lx}, {ly})"><foreignObject width="{lw}" height="24"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel"><p>{label}</p></span></div></foreignObject></g></g>"##,
            idx = s.index,
            id = escape_attr(&s.id),
            rx = fmt(s.rx),
            ry = fmt(s.ry),
            x = fmt(left),
            y = fmt(s.rect_y),
            w = fmt(s.width),
            h = fmt(s.rect_height),
            lx = fmt(label_x),
            ly = fmt(s.rect_y),
            lw = fmt(s.label_width.max(0.0)),
            label = escape_xml(&s.label),
        );
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="items">"#);

    // These helpers are used for positioning and foreignObject sizing. Keep them deterministic and
    // stable across platforms (DOM parity mode will mask numeric drift).
    fn measure_text_metrics_html(text: &str, max_width: Option<f64>) -> crate::text::TextMetrics {
        let style = crate::text::TextStyle::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        measurer.measure_wrapped(text, &style, max_width, crate::text::WrapMode::HtmlLike)
    }

    fn kanban_ticket_url(effective_config: &serde_json::Value, ticket: &str) -> Option<String> {
        let base = effective_config
            .get("kanban")
            .and_then(|v| v.get("ticketBaseUrl"))
            .and_then(|v| v.as_str())?;
        if base.trim().is_empty() {
            return None;
        }
        Some(base.replace("#TICKET#", ticket))
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
        let max_w = (n.width - 10.0).max(0.0);
        let rect_x = -n.width / 2.0;
        let rect_y = -n.height / 2.0;

        // The upstream kanban item shape (`kanbanItem.ts`) positions labels relative to the title
        // bbox and the "details row" bbox (ticket/assigned). Recompute the same anchor points here
        // so wrapped titles match exactly.
        let title_raw = measure_text_metrics_html(&n.label, None);
        let title_needs_wrap = max_w > 0.0 && title_raw.width > max_w;
        let title_metrics = if title_needs_wrap {
            measure_text_metrics_html(&n.label, Some(max_w))
        } else {
            title_raw
        };

        let ticket_h = n
            .ticket
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| measure_text_metrics_html(t, None).height)
            .unwrap_or(0.0);
        let assigned_metrics = n
            .assigned
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| measure_text_metrics_html(t, None))
            .unwrap_or(crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            });
        let height_adj = (ticket_h.max(assigned_metrics.height)) / 2.0;

        let left_x = rect_x + 10.0;
        let right_x = if assigned_metrics.width > 0.0 {
            n.width / 2.0 - 10.0 - assigned_metrics.width
        } else {
            n.width / 2.0 - 10.0
        };

        let title_y = -height_adj - title_metrics.height / 2.0;
        let details_y = -height_adj + title_metrics.height / 2.0;

        let _ = write!(
            &mut out,
            r##"<g class="node undefined" id="{id}" transform="translate({x}, {y})">"##,
            id = escape_attr(&n.id),
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
            x: f64,
            y: f64,
            max_w: f64,
            text: Option<&str>,
            div_class: Option<&str>,
            wrap_title: bool,
        ) {
            let (fo_w, fo_h, div_style_overrides) = match text {
                Some(t) if !t.is_empty() => {
                    if wrap_title && max_w > 0.0 {
                        let raw = measure_text_metrics_html(t, None);
                        if raw.width > max_w {
                            let wrapped = measure_text_metrics_html(t, Some(max_w));
                            (
                                wrapped.width,
                                wrapped.height,
                                Some(format!(
                                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; width: {mw}px;",
                                    mw = fmt(max_w),
                                )),
                            )
                        } else {
                            (
                                raw.width,
                                24.0,
                                Some(format!(
                                    "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px;",
                                    mw = fmt(max_w),
                                )),
                            )
                        }
                    } else {
                        let raw = measure_text_metrics_html(t, None);
                        (
                            raw.width,
                            24.0,
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
            let _ = write!(
                out,
                r##"<g class="label" style="text-align:left !important" transform="translate({x}, {y})"><rect/><foreignObject width="{w}" height="{h}"><div style="{div_style}" xmlns="http://www.w3.org/1999/xhtml"{class_attr}><span style="text-align:left !important" class="nodeLabel">"##,
                x = fmt(x),
                y = fmt(y),
                w = fmt(fo_w),
                h = fmt(fo_h),
                div_style = escape_attr(&div_style),
                class_attr = class_attr
            );
            if let Some(t) = text.filter(|t| !t.is_empty()) {
                let _ = write!(out, r#"<p>{}</p>"#, escape_xml(t));
            }
            out.push_str("</span></div></foreignObject></g>");
        }

        // Title label (may wrap).
        write_label_group(
            &mut out,
            left_x,
            title_y,
            max_w,
            Some(n.label.as_str()),
            n.icon.as_deref().map(|_| "labelBkg"),
            true,
        );

        // Ticket label: wrap in <a> when ticketBaseUrl is configured (upstream behavior).
        let ticket_text = n.ticket.as_deref();
        if let Some(t) = ticket_text.filter(|t| !t.is_empty()) {
            if let Some(url) = kanban_ticket_url(effective_config, t) {
                let _ = write!(
                    &mut out,
                    r#"<a class="kanban-ticket-link" xlink:href="{}">"#,
                    escape_attr(&url)
                );
                write_label_group(&mut out, left_x, details_y, max_w, Some(t), None, false);
                out.push_str("</a>");
            } else {
                write_label_group(&mut out, left_x, details_y, max_w, Some(t), None, false);
            }
        } else {
            write_label_group(&mut out, left_x, details_y, max_w, None, None, false);
        }

        // Assigned label.
        write_label_group(
            &mut out,
            right_x,
            details_y,
            max_w,
            n.assigned.as_deref(),
            None,
            false,
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
    Ok(out)
}
