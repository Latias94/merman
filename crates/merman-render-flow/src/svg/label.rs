//! Flowchart label rendering helpers (HTML/SVG text).

pub(crate) use crate::svg_label::{
    append_native_label_color, flowchart_label_html_with_prepared_label,
    write_flowchart_svg_label_plan_with_style,
    write_flowchart_svg_text_markdown_wrapped_with_style,
};

use merman_render_core::{
    mermaid_style::HtmlTextGeometryDeclarations,
    text::{
        TextMetrics,
        create_text_html::FlowchartLabelMetricsRequest,
        html_label_layout::{measure_flowchart_html_label, measure_flowchart_label_for_layout},
        trim_html_collapsible_ascii_whitespace,
    },
};

pub(super) fn flowchart_label_is_empty_for_render(label: &str) -> bool {
    label.is_empty()
}

pub(super) fn flowchart_label_text_is_empty_for_mode(text: &str, _html_labels: bool) -> bool {
    trim_html_collapsible_ascii_whitespace(text).is_empty()
}

pub(super) fn flowchart_label_metrics_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
) -> TextMetrics {
    measure_flowchart_label_for_layout(request, HtmlTextGeometryDeclarations::default(), true)
}

pub(super) fn flowchart_html_label_metrics_with_geometry(
    request: FlowchartLabelMetricsRequest<'_>,
    xhtml: &str,
    geometry: HtmlTextGeometryDeclarations,
) -> TextMetrics {
    measure_flowchart_html_label(request, xhtml, geometry, true)
}

pub(in crate::svg) fn flowchart_label_plain_text(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    merman_render_core::text::html_label_layout::flowchart_label_plain_text_for_layout(
        label,
        label_type,
        html_labels,
    )
}
