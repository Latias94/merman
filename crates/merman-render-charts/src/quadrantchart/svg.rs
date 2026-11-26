use merman_model::families::quadrant_chart::QuadrantChartPointModel;
use merman_render_core::{
    Error, Result, number_format::canonical_number as fmt, svg_output::SvgOutput, xml::escape_xml,
};

use super::{QuadrantChartAxisLabelData, QuadrantChartDiagramLayout};

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
    axis_labels: &[QuadrantChartAxisLabelData],
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

/// Writes the contents of an open Quadrant Chart main group.
///
/// Source points must be in layout order. A point-count mismatch fails before any output.
pub fn write_svg_body(
    out: &mut impl SvgOutput,
    layout: &QuadrantChartDiagramLayout,
    source_points: &[QuadrantChartPointModel],
) -> Result<()> {
    if source_points.len() != layout.points.len() {
        return Err(Error::InvalidModel {
            message: "Quadrant Chart source points do not match the terminal layout".to_string(),
        });
    }

    // Quadrants.
    out.push_str(r#"<g class="quadrants">"#);
    out.checkpoint()?;
    for quadrant in &layout.quadrants {
        out.push_str(r#"<g class="quadrant">"#);
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
            x = fmt(quadrant.x),
            y = fmt(quadrant.y),
            w = fmt(quadrant.width),
            h = fmt(quadrant.height),
            fill = escape_xml(&quadrant.fill),
        );
        out.checkpoint()?;
        let _ = write!(
            out,
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
            out,
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
    for (point, source_point) in layout.points.iter().zip(source_points) {
        out.push_str(r#"<g class="data-point">"#);
        out.checkpoint()?;
        let mut number_buffer = ryu_js::Buffer::new();
        out.push_str("<circle");
        // D3's unknown scale value is undefined, so attr() removes that coordinate.
        // Numeric overflow inside interpolation remains NaN and keeps the attribute.
        if !source_point.x.is_nan() {
            let _ = write!(out, r#" cx="{}""#, number_buffer.format(point.x));
        }
        if !source_point.y.is_nan() {
            let _ = write!(out, r#" cy="{}""#, number_buffer.format(point.y));
        }
        let _ = write!(
            out,
            r#" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#,
            r = number_buffer.format(point.radius),
            fill = escape_xml(&point.fill),
            stroke = escape_xml(&point.stroke_color),
            stroke_width = escape_xml(&point.stroke_width),
        );
        out.checkpoint()?;
        let text_x = if source_point.x.is_nan() {
            "undefined".to_string()
        } else {
            number_buffer.format(point.text.x).to_string()
        };
        // Adding text padding to an undefined y-coordinate produces numeric NaN.
        let text_y = number_buffer.format(point.text.y);
        let _ = write!(
            out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&point.text.fill),
            font_size = fmt(point.text.font_size),
            dom = quadrantchart_dominant_baseline(&point.text.horizontal_pos),
            anchor = quadrantchart_text_anchor(&point.text.vertical_pos),
            transform = escape_xml(&format!(
                "translate({text_x}, {text_y}) rotate({})",
                fmt(point.text.rotation)
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
    write_quadrantchart_axis_labels(out, &layout.axis_labels)?;
    out.push_str("</g>");
    out.checkpoint()?;

    // Title.
    out.push_str(r#"<g class="title">"#);
    out.checkpoint()?;
    if let Some(t) = layout.title.as_ref() {
        let _ = write!(
            out,
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadrantchart::QuadrantChartTextData;
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
    fn quadrantchart_axis_labels_stop_after_the_first_svg_sink_failure() {
        let axis_labels = (0..4)
            .map(|index| QuadrantChartTextData {
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

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "QuadrantChart axis-label emission must stop at the first failed sink checkpoint"
        );
    }
}
