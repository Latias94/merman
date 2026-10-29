use super::super::theme::EventModelingTheme;
use super::super::*;
use merman_core::diagrams::eventmodeling::EventModelingDiagramRenderModel;

const BOX_TEXT_PADDING: f64 = 10.0;

pub(crate) fn render_eventmodeling_diagram_svg(
    layout: &EventModelingDiagramLayout,
    model: &EventModelingDiagramRenderModel,
    prepared_labels: &crate::eventmodeling::EventModelingPreparedLabels,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("eventmodeling");
    let acc_title = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(|description| description.trim_end_matches('\n'))
        .filter(|description| !description.trim().is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let theme = PresentationTheme::new(effective_config).eventmodeling();
    let mut out = String::new();
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        layout.viewbox_x,
        layout.viewbox_y,
        layout.total_width,
        layout.total_height,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::SvgNumber(layout.total_width));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "eventmodeling");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document = root_svg::RootViewportContext::new(
        crate::family::RenderFamilyKind::EventModeling,
        diagram_id,
    )
    .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
    }

    let css = eventmodeling_css(diagram_id, &theme);
    let _ = write!(&mut out, "<style>{css}</style>");
    out.push_str("<g/>");
    options.checkpoint_emit()?;

    for swimlane in &layout.swimlanes {
        let _ = write!(
            &mut out,
            r#"<g class="em-swimlane"><rect x="{}" y="{}" rx="3" width="{}" height="{}" fill="{}" stroke="{}"></rect><text font-weight="bold" x="{}" y="{}">"#,
            fmt(swimlane.x),
            fmt(swimlane.y),
            fmt(swimlane.width),
            fmt(swimlane.height),
            escape_attr_display(&theme.swimlane_background_fill),
            escape_attr_display(&theme.swimlane_background_stroke),
            fmt(swimlane.x + 30.0),
            fmt(swimlane.y + 30.0)
        );
        escape_xml_into(&mut out, &swimlane.label);
        out.push_str("</text></g>");
    }

    for box_layout in &layout.boxes {
        let _ = write!(
            &mut out,
            r#"<g class="em-box"><rect x="{}" y="{}" rx="3" width="{}" height="{}" stroke="{}" fill="{}"></rect><foreignObject x="{}" y="{}" width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><span style="display: table-cell; text-align: center; vertical-align: middle;">"#,
            fmt(box_layout.x),
            fmt(box_layout.y),
            fmt(box_layout.width),
            fmt(box_layout.height),
            escape_attr_display(&box_layout.stroke),
            escape_attr_display(&box_layout.fill),
            fmt(box_layout.x + BOX_TEXT_PADDING),
            fmt(box_layout.y + BOX_TEXT_PADDING),
            fmt((box_layout.width - 2.0 * BOX_TEXT_PADDING).max(1.0)),
            fmt((box_layout.height - 2.0 * BOX_TEXT_PADDING).max(1.0))
        );
        push_box_html_label(&mut out, prepared_labels, box_layout.index)?;
        out.push_str("</span></div></foreignObject></g>");
    }

    for relation in &layout.relations {
        let _ = write!(
            &mut out,
            r#"<path class="em-relation" fill="none" stroke="{}" stroke-width="1" marker-end="url(#em-arrowhead-{diagram_id})" d="M{} {} L{} {}"></path>"#,
            escape_attr_display(&relation.stroke),
            fmt(relation.x1),
            fmt(relation.y1),
            fmt(relation.x2),
            fmt(relation.y2)
        );
        options.checkpoint_emit()?;
    }

    let marker_fill = &theme.arrowhead_fill;
    let _ = write!(
        &mut out,
        r#"<defs><marker id="em-arrowhead-{diagram_id}" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto"><polygon points="0 0, 10 3.5, 0 7" fill=""#,
    );
    escape_xml_into(&mut out, marker_fill);
    out.push_str(r#""></polygon></marker></defs></svg>"#);
    out.push('\n');
    options.checkpoint_emit()?;
    root_document.complete(out)
}

fn push_box_html_label(
    out: &mut String,
    prepared_labels: &crate::eventmodeling::EventModelingPreparedLabels,
    index: usize,
) -> Result<()> {
    out.push_str(prepared_labels.box_xhtml(index)?);
    Ok(())
}

fn eventmodeling_css(diagram_id: SvgDiagramId<'_>, theme: &EventModelingTheme) -> String {
    format!(
        "#{diagram_id} .em-swimlane text,#{diagram_id} .em-box span {{ font-family: {}; color: {}; }}\
#{diagram_id} .em-relation {{ fill: none; }}",
        theme.font_family_css, theme.text_color
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn emission_requires_prepared_xhtml() {
        let mut out = String::new();
        let error = super::push_box_html_label(
            &mut out,
            &crate::eventmodeling::EventModelingPreparedLabels::default(),
            0,
        )
        .expect_err("missing prepared XHTML must fail without rebuilding layout text");
        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert!(out.is_empty());
    }
}
