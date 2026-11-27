//! Complete ordinary Flow SVG edge labels on the caller's bounded sink.

use kurbo::Size;
use merman_render_core::{
    LayoutPoint,
    number_format::canonical_number as fmt_display,
    svg_output::SvgOutput,
    text::{TextMeasurer, TextStyle, WrapMode, literal_label::FlowchartSvgLabelSource, svg_label},
    xml::escape_attr_display,
};

// Edge labels use Mermaid's createText default independently of flowchart.wrappingWidth.
pub const FLOWCHART_EDGE_LABEL_WRAP_WIDTH: f64 = 200.0;

/// Text retained by the caller's validated label plan or still awaiting terminal wrapping.
pub enum SvgEdgeLabelText<'a> {
    Wrapped(&'a [Vec<String>]),
    Source {
        source: &'a FlowchartSvgLabelSource,
        wrap_style: &'a TextStyle,
    },
    Markdown(&'a str),
}

/// The resolved anchor, paint, and text of one ordinary edge label.
pub struct SvgEdgeLabel<'a> {
    pub edge_id: &'a str,
    pub position: LayoutPoint,
    /// A missing layout box uses the SVG text measurement fallback at this anchor.
    pub layout_size: Option<Size>,
    pub text: SvgEdgeLabelText<'a>,
    pub plain_text: &'a str,
    pub css: &'a str,
    pub measurer: &'a dyn TextMeasurer,
    pub metrics_style: &'a TextStyle,
}

impl SvgEdgeLabel<'_> {
    /// Emit the whole label. The layer caller owns the following output checkpoint.
    pub fn write(&self, out: &mut impl SvgOutput) {
        let max_width = Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH);
        let size = self.layout_size.unwrap_or_else(|| {
            let metrics = self.measurer.measure_wrapped(
                self.plain_text,
                self.metrics_style,
                max_width,
                WrapMode::SvgLike,
            );
            Size::new(
                (metrics.width + 4.0).max(1.0),
                (metrics.height + 4.0).max(1.0),
            )
        });
        let label_box = flowchart_svg_edge_label_box(
            size.width,
            size.height,
            self.measurer
                .measure_svg_create_text_bbox_y_offset_px(self.plain_text, self.metrics_style),
        );
        let _ = write!(
            out,
            r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><g><rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
            fmt_display(self.position.x),
            fmt_display(self.position.y),
            escape_attr_display(self.edge_id),
            fmt_display(label_box.translate_x),
            fmt_display(label_box.translate_y),
            fmt_display(label_box.background_x),
            fmt_display(label_box.background_y),
            fmt_display(label_box.background_width),
            fmt_display(label_box.background_height)
        );
        // Terminal wrapping follows the frame append even when the sink has recorded a failure.
        match self.text {
            SvgEdgeLabelText::Wrapped(lines) => {
                svg_label::write_svg_text_source_word_lines_with_style(
                    out, lines, self.css, true, None,
                );
            }
            SvgEdgeLabelText::Source { source, wrap_style } => {
                let lines = source.wrapped_lines(self.measurer, wrap_style, max_width, true);
                svg_label::write_svg_text_source_word_lines_with_style(
                    out, &lines, self.css, true, None,
                );
            }
            SvgEdgeLabelText::Markdown(source) if self.css.is_empty() => {
                svg_label::write_svg_text_markdown_wrapped_centered_from_create_text_source(
                    out,
                    source,
                    true,
                    self.measurer,
                    self.metrics_style,
                    max_width,
                );
            }
            SvgEdgeLabelText::Markdown(source) => {
                svg_label::write_svg_text_markdown_wrapped_from_create_text_source_with_style(
                    out,
                    source,
                    self.css,
                    self.measurer,
                    self.metrics_style,
                    max_width,
                    true,
                );
            }
        }
        out.push_str("</g></g></g>");
    }
}

/// Emit Mermaid's complete empty SVG edge label, including its centered text row.
pub fn write_empty_svg_edge_label(out: &mut impl SvgOutput, edge_id: &str) {
    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)">"#,
        escape_attr_display(edge_id)
    );
    svg_label::write_svg_text_source_word_lines(out, &[Vec::new()], false, true);
    out.push_str("</g></g>");
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SvgEdgeLabelBox {
    translate_x: f64,
    translate_y: f64,
    background_x: f64,
    background_y: f64,
    background_width: f64,
    background_height: f64,
}

fn flowchart_svg_edge_label_box(
    layout_width: f64,
    layout_height: f64,
    text_bbox_y: f64,
) -> SvgEdgeLabelBox {
    const BACKGROUND_PADDING_PX: f64 = 2.0;

    let background_width = layout_width.max(0.0);
    let background_height = layout_height.max(0.0);
    let text_height = (background_height - 2.0 * BACKGROUND_PADDING_PX).max(0.0);
    SvgEdgeLabelBox {
        // Mermaid centers the text bbox, so a symmetric bbox needs no horizontal translation.
        translate_x: 0.0,
        translate_y: -(text_bbox_y + text_height / 2.0),
        background_x: -background_width / 2.0,
        background_y: text_bbox_y - BACKGROUND_PADDING_PX,
        background_width,
        background_height,
    }
}

#[cfg(test)]
mod tests {
    use super::flowchart_svg_edge_label_box;

    #[test]
    fn svg_edge_label_box_centers_text_and_its_padded_background() {
        let label_box = flowchart_svg_edge_label_box(104.0, 24.0, 3.0);
        assert_eq!(label_box.translate_x, 0.0);
        assert_eq!(label_box.translate_y, -13.0);
        assert_eq!(label_box.background_x, -52.0);
        assert_eq!(label_box.background_y, 1.0);
        assert_eq!(label_box.background_width, 104.0);
        assert_eq!(label_box.background_height, 24.0);

        let translated_background_center_y =
            label_box.translate_y + label_box.background_y + label_box.background_height / 2.0;
        assert_eq!(translated_background_center_y, 0.0);
    }
}
