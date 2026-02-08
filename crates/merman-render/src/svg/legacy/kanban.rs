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
    _effective_config: &serde_json::Value,
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
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" role="graphics-document document" aria-roledescription="kanban">"#,
        diagram_id_esc = diagram_id_esc,
        max_w = fmt(vb_w),
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
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

    fn measure_text_width(text: &str) -> f64 {
        // This width is used for positioning only; DOM parity mode masks numeric drift.
        // Keep it deterministic and stable across platforms.
        let style = crate::text::TextStyle::default();
        let measurer = crate::text::DeterministicTextMeasurer::default();
        measurer.measure(text, &style).width
    }

    for n in &layout.items {
        let max_w = (n.width - 10.0).max(0.0);
        let rect_x = -n.width / 2.0;
        let rect_y = -n.height / 2.0;

        let has_details_row = n.ticket.is_some() || n.assigned.is_some();
        let top_pad = if has_details_row { 4.0 } else { 10.0 };
        let row1_y = rect_y + top_pad;
        let row2_y = if has_details_row {
            rect_y + top_pad + 24.0
        } else {
            rect_y + 34.0
        };

        let left_x = rect_x + 10.0;
        let assigned_w = n.assigned.as_deref().map(measure_text_width).unwrap_or(0.0);
        let right_x = if assigned_w > 0.0 {
            n.width / 2.0 - 10.0 - assigned_w
        } else {
            n.width / 2.0 - 10.0
        };

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
        ) {
            let (fo_w, fo_h) = match text {
                Some(t) if !t.is_empty() => (measure_text_width(t), 24.0),
                _ => (0.0, 0.0),
            };
            let class_attr = div_class
                .map(|c| format!(r#" class="{}""#, escape_attr(c)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r##"<g class="label" style="text-align:left !important" transform="translate({x}, {y})"><rect/><foreignObject width="{w}" height="{h}"><div style="text-align: center; display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px;" xmlns="http://www.w3.org/1999/xhtml"{class_attr}><span style="text-align:left !important" class="nodeLabel">"##,
                x = fmt(x),
                y = fmt(y),
                w = fmt(fo_w),
                h = fmt(fo_h),
                mw = fmt(max_w),
                class_attr = class_attr
            );
            if let Some(t) = text.filter(|t| !t.is_empty()) {
                let _ = write!(out, r#"<p>{}</p>"#, escape_xml(t));
            }
            out.push_str("</span></div></foreignObject></g>");
        }

        write_label_group(
            &mut out,
            left_x,
            row1_y,
            max_w,
            Some(n.label.as_str()),
            n.icon.as_deref().map(|_| "labelBkg"),
        );
        write_label_group(&mut out, left_x, row2_y, max_w, n.ticket.as_deref(), None);
        write_label_group(
            &mut out,
            right_x,
            row2_y,
            max_w,
            n.assigned.as_deref(),
            None,
        );

        if n.priority.is_some() {
            let _ = write!(
                &mut out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4"/>"#,
                x1 = fmt(rect_x + 2.0),
                y1 = fmt(rect_y + 2.0),
                x2 = fmt(rect_x + 2.0),
                y2 = fmt(rect_y + n.height - 2.0),
            );
        }

        out.push_str("</g>");
    }

    out.push_str("</g>");
    out.push_str("</svg>\n");
    Ok(out)
}
