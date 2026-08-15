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
    }
    Ok(())
}

pub(crate) fn render_quadrantchart_diagram_svg(
    layout: &QuadrantChartDiagramLayout,
    model: &QuadrantChartRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("quadrantchart");
    let diagram_id_esc = escape_xml(diagram_id);
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
    let w = layout.width.max(1.0);
    let h = layout.height.max(1.0);
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
                root_svg::RootViewportSpec::mermaid(
                    root_svg::DiagramBounds::from_view_box(0.0, 0.0, w, h),
                    use_max_width,
                ),
                root_chrome,
            )?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id_esc}">{}</title>"#,
            escape_xml(title)
        );
        out.checkpoint()?;
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id_esc}">{}</desc>"#,
            escape_xml(description)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    let css = info_css_with_config(diagram_id, effective_config);
    out.push_str(&css);
    drop(css);
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
    }
    out.push_str("</g>");
    out.checkpoint()?;

    // Points.
    out.push_str(r#"<g class="data-points">"#);
    out.checkpoint()?;
    for point in &layout.points {
        out.push_str(r#"<g class="data-point">"#);
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#,
            cx = fmt(point.x),
            cy = fmt(point.y),
            r = fmt(point.radius),
            fill = escape_xml(&point.fill),
            stroke = escape_xml(&point.stroke_color),
            stroke_width = escape_xml(&point.stroke_width),
        );
        out.checkpoint()?;
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
        out.push_str("</g>");
        out.checkpoint()?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    // Axis labels.
    out.push_str(r#"<g class="labels">"#);
    out.checkpoint()?;
    write_quadrantchart_axis_labels(&mut out, &layout.axis_labels)?;
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
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
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

        let error = write_quadrantchart_axis_labels(&mut out, &axis_labels)
            .expect_err("the rejecting sink must stop QuadrantChart axis-label emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "QuadrantChart axis-label emission must stop at the first failed sink checkpoint"
        );
    }
}
