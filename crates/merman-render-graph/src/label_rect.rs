//! Shared literal labelRect measurement and retained SVG or HTML content.

use merman_render_core::{
    LayoutLabel, Result,
    html::html_label_overflow_attr,
    number_format::canonical_number as fmt,
    svg_output::SvgOutput,
    text::{
        self, TextMeasurer, TextMetrics, TextStyle,
        literal_label::{
            FlowchartSvgLabelSource, FlowchartSvgWidthMode, flowchart_non_markdown_label_for_html,
        },
        svg_label::write_svg_text_source_word_lines_with_style,
    },
    xml::escape_attr,
};

/// The selected provider creates a fresh plain labelRect node. Its frozen terminal supplies
/// layout metrics, painted content, and HTML mode to every consumer.
#[derive(Debug, Clone)]
pub(crate) struct LabelRect {
    metrics: TextMetrics,
    content: LabelRectContent,
}

#[derive(Debug, Clone)]
enum LabelRectContent {
    Html {
        fragment: String,
        wrapper_style: String,
    },
    Svg {
        lines: Vec<Vec<String>>,
        grid_bbox_top: Option<f64>,
    },
}

pub fn label_rect_html_source(source: &str) -> String {
    if source.is_empty() {
        String::new()
    } else {
        let label = flowchart_non_markdown_label_for_html(source, "text");
        text::replace_fontawesome_icons(&format!("<p>{label}</p>"))
    }
}

impl LabelRect {
    pub(crate) fn retained_buffer_bytes(&self) -> usize {
        match &self.content {
            LabelRectContent::Html {
                fragment,
                wrapper_style,
            } => fragment.capacity().saturating_add(wrapper_style.capacity()),
            LabelRectContent::Svg { lines, .. } => lines.iter().fold(
                lines
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Vec<String>>()),
                |bytes, row| {
                    row.iter().fold(
                        bytes.saturating_add(
                            row.capacity().saturating_mul(std::mem::size_of::<String>()),
                        ),
                        |bytes, word| bytes.saturating_add(word.capacity()),
                    )
                },
            ),
        }
    }

    pub(crate) fn prepare(
        source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        html_labels: bool,
        config: &merman_model::MermaidConfig,
        wrapping_width: f64,
        grid: bool,
    ) -> Self {
        let (metrics, content) = if html_labels {
            // nonMarkdownToHTML wraps literal text in one paragraph. The sanitizer retains
            // authored HTML semantics, while Markdown punctuation remains ordinary text.
            let html = label_rect_html_source(source);
            let fragment = merman_markup::sanitize::sanitize_xhtml(&html, config);
            let metrics = if text::trim_html_collapsible_ascii_whitespace(source).is_empty() {
                TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                }
            } else {
                text::measure_xhtml_label_fragment_with_terminal_fonts(
                    measurer,
                    &fragment,
                    style,
                    Some(wrapping_width),
                    16.0,
                )
            };
            // addHtmlSpan switches a constrained table-cell to a fixed-width break-spaces
            // table. Retain this choice with the measured terminal instead of deciding in SVG.
            let wrapped = wrapping_width.is_finite()
                && wrapping_width > 0.0
                && metrics.width >= wrapping_width;
            let wrapper_style = if wrapped {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {wrapping_width}px; width: {wrapping_width}px; text-align: center;"
                )
            } else {
                format!(
                    "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {wrapping_width}px; text-align: center;"
                )
            };
            (
                metrics,
                LabelRectContent::Html {
                    fragment,
                    wrapper_style,
                },
            )
        } else {
            // This source/measurement utility is shared text machinery, independent of the
            // Flowchart family feature. Freeze its selected word rows for the SVG writer.
            let source = FlowchartSvgLabelSource::new(source);
            let lines = source.wrapped_lines(measurer, style, Some(wrapping_width), true);
            let metrics =
                source.metrics_from_wrapped(measurer, style, &lines, FlowchartSvgWidthMode::Bbox);
            let grid_bbox_top = grid.then(|| {
                if source.plain_text().is_empty() {
                    0.0
                } else {
                    measurer.measure_svg_create_text_bbox_y_offset_px(source.plain_text(), style)
                }
            });
            (
                metrics,
                LabelRectContent::Svg {
                    lines,
                    grid_bbox_top,
                },
            )
        };
        Self { metrics, content }
    }

    pub(crate) fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    fn grid_svg_bbox_top(&self) -> Option<f64> {
        match &self.content {
            LabelRectContent::Svg { grid_bbox_top, .. } => *grid_bbox_top,
            LabelRectContent::Html { .. } => None,
        }
    }

    #[cfg(feature = "diagram-er")]
    pub(crate) fn xhtml_fragment(&self) -> Option<&str> {
        match &self.content {
            LabelRectContent::Html { fragment, .. } => Some(fragment),
            LabelRectContent::Svg { .. } => None,
        }
    }

    pub(crate) fn write(
        &self,
        out: &mut impl SvgOutput,
        label: &LayoutLabel,
        label_id: &str,
        (tx, ty): (f64, f64),
        allow_html_overflow: bool,
    ) -> Result<()> {
        let grid_bbox_top = self.grid_svg_bbox_top();
        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate({}, {})"><rect/>"#,
            escape_attr(label_id),
            fmt(label.x + tx),
            fmt(label.y + ty),
            fmt(-label.width / 2.0),
            fmt(-label.height / 2.0 - grid_bbox_top.unwrap_or(0.0)),
        );
        match &self.content {
            LabelRectContent::Html {
                fragment,
                wrapper_style,
            } => {
                let _ = write!(
                    out,
                    r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel">{}</span></div></foreignObject>"#,
                    fmt(label.width),
                    fmt(label.height),
                    html_label_overflow_attr(allow_html_overflow, label.width, label.height),
                    escape_attr(wrapper_style),
                    fragment,
                );
            }
            LabelRectContent::Svg { lines, .. } => {
                out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                if grid_bbox_top.is_none() {
                    out.push_str("</g>");
                }
                write_svg_text_source_word_lines_with_style(out, lines, "", false, None);
                if grid_bbox_top.is_some() {
                    out.push_str("</g>");
                }
            }
        }
        out.push_str("</g></g>");
        out.checkpoint()?;
        Ok(())
    }
}
