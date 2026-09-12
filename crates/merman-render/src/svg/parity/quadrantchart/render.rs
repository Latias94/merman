use super::super::*;
use merman_core::diagrams::quadrant_chart::QuadrantChartRenderModel;

// QuadrantChart diagram SVG renderer implementation (split from parity.rs).

fn quadrantchart_dominant_baseline(horizontal_pos: &str) -> &'static str {
    if horizontal_pos == "top" {
        "hanging"
    } else {
        "middle"
    }
}

fn quadrantchart_text_anchor(vertical_pos: &str) -> &'static str {
    if vertical_pos == "left" {
        "start"
    } else {
        "middle"
    }
}

fn quadrantchart_transform(x: f64, y: f64, rotation: f64) -> String {
    format!(
        "translate({}, {}) rotate({})",
        fmt(x),
        fmt(y),
        fmt(rotation)
    )
}

fn write_quadrantchart_axis_labels(
    out: &mut impl SvgOutput,
    axis_labels: &[crate::model::QuadrantChartAxisLabelData],
    mut record_text: impl FnMut(&crate::model::QuadrantChartTextData),
) -> Result<()> {
    for label in axis_labels {
        let _ = write!(
            out,
            r#"<g class="label"><text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text></g>"#,
            fill = escape_xml(&label.fill),
            font_size = fmt(label.font_size),
            dom = quadrantchart_dominant_baseline(&label.horizontal_pos),
            anchor = quadrantchart_text_anchor(&label.vertical_pos),
            transform = escape_xml(&quadrantchart_transform(label.x, label.y, label.rotation)),
            text = escape_xml(&label.text),
        );
        out.checkpoint()?;
        record_text(label);
    }
    Ok(())
}

fn quadrantchart_root_bounds(
    layout: &QuadrantChartDiagramLayout,
    point_theme: &crate::quadrantchart::QuadrantChartPointThemePlan,
) -> root_svg::DiagramBounds {
    let mut min_x = 0.0_f64;
    let mut min_y = 0.0_f64;
    let mut max_x = layout.width.max(1.0);
    let mut max_y = layout.height.max(1.0);

    for (point_index, point) in layout.points.iter().enumerate() {
        if point_theme.radius_override_px(point_index).is_none() {
            continue;
        }
        let stroke_outset =
            parse_quadrantchart_stroke_width_px(&point.stroke_width).unwrap_or(0.0) / 2.0;
        let paint_outset = point.radius.max(0.0) + stroke_outset;
        min_x = min_x.min(point.x - paint_outset);
        min_y = min_y.min(point.y - paint_outset);
        max_x = max_x.max(point.x + paint_outset);
        max_y = max_y.max(point.y + paint_outset);
    }

    root_svg::DiagramBounds::from_extents(min_x, min_y, max_x, max_y, 0.0)
}

fn parse_quadrantchart_stroke_width_px(raw: &str) -> Option<f64> {
    let raw = raw.trim().trim_end_matches(';').trim();
    let raw = raw.trim_end_matches("!important").trim();
    let raw = raw.strip_suffix("px").unwrap_or(raw).trim();
    raw.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(crate) fn render_quadrantchart_diagram_svg(
    layout: &QuadrantChartDiagramLayout,
    model: &QuadrantChartRenderModel,
    point_theme: &crate::quadrantchart::QuadrantChartPointThemePlan,
    text_paint: &crate::quadrantchart::QuadrantChartPaintPlan,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    if point_theme.point_count() != layout.points.len() {
        return Err(crate::Error::InvalidModel {
            message: "Quadrant Chart point theme plan does not match the terminal layout"
                .to_string(),
        });
    }
    let diagram_id = options.diagram_id_or("quadrantchart");
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
    let use_max_width = crate::quadrantchart::QuadrantChartConfigView::new(effective_config)
        .render_settings()
        .use_max_width;

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_bounds = quadrantchart_root_bounds(layout, point_theme);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "quadrantChart");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        style_viewbox_order: root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::QUADRANT_CHART, diagram_id)
            .write_open(
                &mut out,
                root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width),
                root_chrome,
            )?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
        out.checkpoint()?;
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
        out.checkpoint()?;
    }

    let mut point_theme_receipt = point_theme.begin_terminal_receipt(layout);
    let mut text_paint_receipt = text_paint.begin_terminal_receipt();

    out.push_str("<style>");
    out.checkpoint()?;
    let css_write = write_info_css_with_font_family(
        &mut out,
        diagram_id.semantic_str(),
        effective_config,
        point_theme.font_family_css(),
    )?;
    if let Some(receipt) = point_theme_receipt.as_mut() {
        receipt.record_css_emission(
            css_write.root_font_family_css(),
            css_write.inherited_font_family_css(),
            css_write.root_variable_font_family_css(),
        );
    }
    out.checkpoint()?;
    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"</style><g/>"#);
    out.checkpoint()?;

    out.push_str(r#"<g class="main">"#);
    out.checkpoint()?;

    // Quadrants.
    out.push_str(r#"<g class="quadrants">"#);
    out.checkpoint()?;
    for quadrant in &layout.quadrants {
        out.push_str(r#"<g class="quadrant">"#);
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
            x = fmt(quadrant.x),
            y = fmt(quadrant.y),
            w = fmt(quadrant.width),
            h = fmt(quadrant.height),
            fill = escape_xml(&quadrant.fill),
        );
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&quadrant.text.fill),
            font_size = fmt(quadrant.text.font_size),
            dom = quadrantchart_dominant_baseline(&quadrant.text.horizontal_pos),
            anchor = quadrantchart_text_anchor(&quadrant.text.vertical_pos),
            transform = escape_xml(&quadrantchart_transform(
                quadrant.text.x,
                quadrant.text.y,
                quadrant.text.rotation
            )),
            text = escape_xml(&quadrant.text.text),
        );
        out.checkpoint()?;
        if let Some(receipt) = point_theme_receipt.as_mut() {
            receipt.record_quadrant_text(&quadrant.text.text);
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    // Borders.
    out.push_str(r#"<g class="border">"#);
    out.checkpoint()?;
    for line in &layout.border_lines {
        let _ = write!(
            &mut out,
            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" style="stroke: {stroke}; stroke-width: {w};"/>"#,
            x1 = fmt(line.x1),
            y1 = fmt(line.y1),
            x2 = fmt(line.x2),
            y2 = fmt(line.y2),
            stroke = escape_xml(&line.stroke_fill),
            w = fmt(line.stroke_width),
        );
        out.checkpoint()?;
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record_border(line);
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    // Points.
    out.push_str(r#"<g class="data-points">"#);
    out.checkpoint()?;
    for (point_index, point) in layout.points.iter().enumerate() {
        out.push_str(r#"<g class="data-point">"#);
        out.checkpoint()?;
        let themed_radius_token = point_theme.radius_override_token(point_index);
        let fallback_radius_token;
        let radius_token = if let Some(token) = themed_radius_token {
            token
        } else {
            fallback_radius_token = fmt(point.radius).to_string();
            fallback_radius_token.as_str()
        };
        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#,
            cx = fmt(point.x),
            cy = fmt(point.y),
            r = radius_token,
            fill = escape_xml(&point.fill),
            stroke = escape_xml(&point.stroke_color),
            stroke_width = escape_xml(&point.stroke_width),
        );
        out.checkpoint()?;
        if let Some(receipt) = point_theme_receipt.as_mut() {
            receipt.record_checkpointed_point(
                point_index,
                themed_radius_token.map(|_| radius_token),
                &point.fill,
            );
        }
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&point.text.fill),
            font_size = fmt(point.text.font_size),
            dom = quadrantchart_dominant_baseline(&point.text.horizontal_pos),
            anchor = quadrantchart_text_anchor(&point.text.vertical_pos),
            transform = escape_xml(&quadrantchart_transform(
                point.text.x,
                point.text.y,
                point.text.rotation
            )),
            text = escape_xml(&point.text.text),
        );
        out.checkpoint()?;
        if let Some(receipt) = point_theme_receipt.as_mut() {
            receipt.record_point_text(&point.text.text);
        }
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record(&point.text);
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    // Axis labels.
    out.push_str(r#"<g class="labels">"#);
    out.checkpoint()?;
    write_quadrantchart_axis_labels(&mut out, &layout.axis_labels, |label| {
        if let Some(receipt) = point_theme_receipt.as_mut() {
            receipt.record_axis_label(&label.text);
        }
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record(label);
        }
    })?;
    out.push_str("</g>");
    out.checkpoint()?;

    // Title.
    out.push_str(r#"<g class="title">"#);
    out.checkpoint()?;
    if let Some(t) = layout.title.as_ref() {
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&t.fill),
            font_size = fmt(t.font_size),
            dom = quadrantchart_dominant_baseline(&t.horizontal_pos),
            anchor = quadrantchart_text_anchor(&t.vertical_pos),
            transform = escape_xml(&quadrantchart_transform(t.x, t.y, t.rotation)),
            text = escape_xml(&t.text),
        );
        out.checkpoint()?;
        if let Some(receipt) = point_theme_receipt.as_mut() {
            receipt.record_title_text(&t.text);
        }
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record(t);
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str("</g></svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if point_theme_receipt.is_some_and(|receipt| !point_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "Quadrant Chart point theme receipt did not match the terminal SVG"
                .to_string(),
        });
    }
    if text_paint_receipt.is_some_and(|receipt| !text_paint.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "Quadrant Chart text paint receipt does not match terminal SVG".into(),
        });
    }
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn quadrantchart_axis_labels_stop_after_the_first_svg_sink_failure() {
        let axis_labels = (0..4)
            .map(|index| crate::model::QuadrantChartTextData {
                text: format!("axis-{index}"),
                x: index as f64,
                y: index as f64,
                fill: "#333333".to_string(),
                font_size: 16.0,
                rotation: 0.0,
                vertical_pos: "left".to_string(),
                horizontal_pos: "top".to_string(),
            })
            .collect::<Vec<_>>();
        let mut out = RejectAfterFirstWrite::default();

        let error = write_quadrantchart_axis_labels(&mut out, &axis_labels, |_| {})
            .expect_err("the rejecting sink must stop QuadrantChart axis-label emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "QuadrantChart axis-label emission must stop at the first failed sink checkpoint"
        );
    }
}
