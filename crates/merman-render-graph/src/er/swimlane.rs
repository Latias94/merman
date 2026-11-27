//! ER's retained Swimlane terminals and lane paint geometry.

use merman_render_core::{
    LayoutLabel, SwimlaneDirection, SwimlaneLaneLayout,
    text::{
        self, TextMeasurer, TextMetrics, TextStyle,
        literal_label::{
            FlowchartSvgLabelSource, FlowchartSvgWidthMode, flowchart_non_markdown_label_for_html,
        },
    },
};

/// The selected provider creates a fresh plain labelRect node. Its frozen terminal supplies
/// layout metrics, painted content, and HTML mode to every consumer.
#[derive(Debug, Clone)]
pub(super) struct ErSwimlaneRelationshipLabel {
    metrics: TextMetrics,
    content: ErSwimlaneLabelContent,
}

#[derive(Debug, Clone)]
enum ErSwimlaneLabelContent {
    Html {
        fragment: String,
        wrapper_style: String,
    },
    Svg {
        lines: Vec<Vec<String>>,
    },
}

pub fn relationship_html_source(source: &str) -> String {
    if source.is_empty() {
        String::new()
    } else {
        let label = flowchart_non_markdown_label_for_html(source, "text");
        text::replace_fontawesome_icons(&format!("<p>{label}</p>"))
    }
}

impl ErSwimlaneRelationshipLabel {
    pub(super) fn retained_buffer_bytes(&self) -> usize {
        match &self.content {
            ErSwimlaneLabelContent::Html {
                fragment,
                wrapper_style,
            } => fragment.capacity().saturating_add(wrapper_style.capacity()),
            ErSwimlaneLabelContent::Svg { lines } => lines.iter().fold(
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

    pub(super) fn prepare(
        source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        html_labels: bool,
        config: &merman_model::MermaidConfig,
    ) -> Self {
        let view = super::ErConfigView::new(config.as_value());
        let wrapping_width = view.swimlane_relationship_wrapping_width();
        let (metrics, content) = if html_labels {
            // nonMarkdownToHTML wraps literal text in one paragraph. The sanitizer retains
            // authored HTML semantics, while Markdown punctuation remains ordinary text.
            let html = relationship_html_source(source);
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
                    super::labels::HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
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
                ErSwimlaneLabelContent::Html {
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
            (metrics, ErSwimlaneLabelContent::Svg { lines })
        };
        Self { metrics, content }
    }

    pub(super) fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub(super) fn xhtml_fragment(&self) -> Option<&str> {
        self.html().map(|(fragment, _)| fragment)
    }

    pub(super) fn html(&self) -> Option<(&str, &str)> {
        match &self.content {
            ErSwimlaneLabelContent::Html {
                fragment,
                wrapper_style,
            } => Some((fragment, wrapper_style)),
            ErSwimlaneLabelContent::Svg { .. } => None,
        }
    }

    pub(super) fn svg_lines(&self) -> Option<&[Vec<String>]> {
        match &self.content {
            ErSwimlaneLabelContent::Svg { lines } => Some(lines),
            ErSwimlaneLabelContent::Html { .. } => None,
        }
    }
}

/// Prepared ER lane paint data. Label anchors remain on the ER relationships; lane metadata
/// carries the content boundary and parent roles that ordinary LayoutCluster does not expose.
#[derive(Debug, Clone)]
pub struct ErSwimlanePaintLayout {
    pub direction: SwimlaneDirection,
    pub lanes: Vec<SwimlaneLaneLayout>,
}

pub struct ErSwimlaneLaneGeometry {
    pub width: f64,
    pub height: f64,
    pub title_band_width: f64,
    pub title_band_height: f64,
    pub title_label: LayoutLabel,
}

impl ErSwimlanePaintLayout {
    /// Geometry from rendering-elements/clusters/swimlane.js, shared by bounds and painting.
    pub fn lane_geometry(&self, lane: &SwimlaneLaneLayout) -> ErSwimlaneLaneGeometry {
        let label_width = lane.title_label_width.max(0.0);
        let label_height = lane.title_label_height.max(0.0);
        let is_lr = self.direction == SwimlaneDirection::Lr;
        let title_size = label_height + if is_lr { 8.0 } else { 0.0 };
        let width = lane.width.max(label_width + lane.padding.max(0.0));
        let height = lane.height.max(0.0);
        let top = lane.y - height / 2.0;
        if is_lr {
            ErSwimlaneLaneGeometry {
                width,
                height,
                title_band_width: title_size,
                title_band_height: height,
                title_label: LayoutLabel {
                    x: lane.x - width / 2.0 + title_size / 2.0,
                    y: lane.y,
                    width: label_height,
                    height: label_width,
                },
            }
        } else {
            let content_top = lane.content_top.unwrap_or(top + height / 3.0);
            let title_height = title_size.min((content_top - top).max(0.0));
            ErSwimlaneLaneGeometry {
                width,
                height,
                title_band_width: width,
                title_band_height: title_height,
                title_label: LayoutLabel {
                    x: lane.x,
                    y: top + title_height / 2.0,
                    width: label_width,
                    height: label_height,
                },
            }
        }
    }
}
