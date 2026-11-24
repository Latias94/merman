//! State shape and label measurements shared by the layout adapters.

use super::config::{StateLayoutSettings, decode_html_entities_once};
use super::{StateNode, state_value_to_label_text};
use crate::graph_layout::ir::Direction;
use crate::text::{TextMeasurer, WrapMode};
use crate::{Error, Result};

pub(super) fn edge_label_metrics(
    edge_id: &str,
    label: &str,
    measurer: &dyn TextMeasurer,
    typography: &super::ResolvedLabelTypography,
    wrap_mode: WrapMode,
    sidecar: Option<&super::StateLabelSidecarBuilder>,
) -> (f64, f64) {
    if label.trim().is_empty() {
        return (0.0, 0.0);
    }
    let wrapping_width = crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
    let request = super::StateLabelMetricsRequest {
        owner: super::StateLabelOwner::Edge(edge_id),
        text: label,
        source_kind: super::StateLabelSourceKind::Markdown,
        measurer,
        typography,
        max_width_px: Some(wrapping_width),
        wrap_mode,
    };
    let mut metrics = sidecar
        .map_or_else(
            || {
                super::measure_state_markdown_label(
                    label,
                    measurer,
                    typography.text_style(),
                    Some(wrapping_width),
                    wrap_mode,
                )
            },
            |sidecar| sidecar.measure_for_layout(request),
        )
        .metrics;
    let owner = super::StateLabelOwner::Edge(edge_id);
    let measured_edge_geometry =
        sidecar.and_then(|sidecar| sidecar.measured_native_geometry(owner));
    // For SVG edge labels, `createText(..., addSvgBackground=true)` adds a background rect with a
    // 2px padding.
    if let Some(geometry) = measured_edge_geometry.as_ref() {
        metrics = geometry.layout_metrics();
    } else if wrap_mode == WrapMode::SvgLike {
        metrics.width += 4.0;
        metrics.height += 4.0;
    }

    (metrics.width.max(0.0), metrics.height.max(0.0))
}

fn node_label_metrics(
    node_id: &str,
    label: &str,
    wrapping_width: f64,
    measurer: &dyn TextMeasurer,
    typography: &super::ResolvedLabelTypography,
    wrap_mode: WrapMode,
    sidecar: Option<&super::StateLabelSidecarBuilder>,
) -> (f64, f64) {
    let request = super::StateLabelMetricsRequest {
        owner: super::StateLabelOwner::Node(node_id),
        text: label,
        source_kind: super::StateLabelSourceKind::Markdown,
        measurer,
        typography,
        max_width_px: Some(wrapping_width),
        wrap_mode,
    };
    let metrics = sidecar
        .map_or_else(
            || {
                super::measure_state_markdown_label(
                    label,
                    measurer,
                    typography.text_style(),
                    Some(wrapping_width),
                    wrap_mode,
                )
            },
            |sidecar| sidecar.measure_for_layout(request),
        )
        .metrics;

    (metrics.width.max(0.0), metrics.height.max(0.0))
}

pub(super) fn title_label_metrics(
    owner: super::StateLabelOwner<'_>,
    label: &str,
    measurer: &dyn TextMeasurer,
    typography: &super::ResolvedLabelTypography,
    wrap_mode: WrapMode,
    sidecar: Option<&super::StateLabelSidecarBuilder>,
) -> (f64, f64) {
    // Mermaid state diagram cluster titles use `createLabel(...)` (nowrap) rather than
    // `createText(...)` (width constrained).
    let request = super::StateLabelMetricsRequest {
        owner,
        text: label,
        source_kind: super::StateLabelSourceKind::Plain,
        measurer,
        typography,
        max_width_px: None,
        wrap_mode,
    };
    let metrics = sidecar
        .map_or_else(
            || {
                let metrics = if wrap_mode == WrapMode::HtmlLike {
                    let decoded = decode_html_entities_once(label);
                    measurer.measure_wrapped(
                        decoded.as_ref(),
                        typography.text_style(),
                        None,
                        wrap_mode,
                    )
                } else {
                    super::label::measure_state_svg_plain_label(
                        label,
                        measurer,
                        typography.text_style(),
                    )
                };
                super::StateLabelMeasurement {
                    metrics,
                    uses_html_wrapping_table: false,
                }
            },
            |sidecar| sidecar.measure_for_layout(request),
        )
        .metrics;

    (metrics.width.max(0.0), metrics.height.max(0.0))
}

pub(super) fn state_fork_join_painted_dimensions(direction: Direction) -> (f64, f64) {
    if matches!(direction, Direction::Right | Direction::Left) {
        (10.0, 70.0)
    } else {
        (70.0, 10.0)
    }
}

pub(super) fn state_node_dimensions(
    n: &StateNode,
    settings: &StateLayoutSettings,
    style_plan: &super::StateStylePlan,
    measurer: &dyn TextMeasurer,
    label_sidecar: Option<&super::StateLabelSidecarBuilder>,
) -> Result<(f64, f64)> {
    let node_style = style_plan.node(&n.id);
    let node_label_typography = node_style
        .map(super::StateNodeStylePlan::resolved_label_typography)
        .unwrap_or_else(|| style_plan.base_label_typography());
    let padding = node_style
        .and_then(super::StateNodeStylePlan::padding_override)
        .or(n.padding)
        .unwrap_or(settings.state_padding)
        .max(0.0);
    let label_text = n
        .label
        .as_ref()
        .map(state_value_to_label_text)
        .unwrap_or_else(|| n.id.clone());

    let (w, h) = match n.shape.as_str() {
        "stateStart" => (14.0, 14.0),
        "stateEnd" => (14.0, 14.0),
        "choice" => (28.0, 28.0),
        "fork" | "join" => {
            let (width, height) = state_fork_join_painted_dimensions(settings.direction);
            let half_padding = settings.state_padding / 2.0;
            (width + half_padding, height + half_padding)
        }
        "note" => {
            let (tw, th) = node_label_metrics(
                &n.id,
                &label_text,
                settings.wrapping_width,
                measurer,
                node_label_typography,
                settings.wrap_mode,
                label_sidecar,
            );
            let tw = crate::text::TextMetrics {
                width: tw,
                height: th,
                line_count: 1,
            }
            .with_label_min_width(&label_text, settings.label_min_width, None)
            .width;
            (tw + padding * 2.0, th + padding * 2.0)
        }
        "rectWithTitle" => {
            let desc = n
                .description
                .as_ref()
                .map(|v| v.join("\n"))
                .unwrap_or_default();
            let title_wrap_mode = if settings.html_labels {
                WrapMode::HtmlLike
            } else {
                WrapMode::SvgLikeSingleRun
            };
            let (title_w, title_h) = title_label_metrics(
                super::StateLabelOwner::NodeTitle(&n.id),
                &label_text,
                measurer,
                node_label_typography,
                title_wrap_mode,
                label_sidecar,
            );
            let (desc_w, desc_h) = title_label_metrics(
                super::StateLabelOwner::NodeDescription(&n.id),
                &desc,
                measurer,
                node_label_typography,
                title_wrap_mode,
                label_sidecar,
            );

            let geometry = super::RectWithTitleGeometry::from_metrics(
                title_w, title_h, desc_w, desc_h, padding,
            );
            (geometry.width, geometry.height)
        }
        "rect" => {
            let (tw, th) = node_label_metrics(
                &n.id,
                &label_text,
                settings.wrapping_width,
                measurer,
                node_label_typography,
                settings.wrap_mode,
                label_sidecar,
            );
            let tw = crate::text::TextMetrics {
                width: tw,
                height: th,
                line_count: 1,
            }
            .with_label_min_width(&label_text, settings.label_min_width, None)
            .width;
            // Mermaid converts `rect` into `roundedRect` when rx/ry is set.
            let radius = node_style
                .and_then(super::StateNodeStylePlan::radius_override)
                .unwrap_or_else(|| n.rx.unwrap_or(0.0).min(n.ry.unwrap_or(0.0)));
            let has_rounding = radius > 0.0;
            let pad_x = if has_rounding { padding } else { padding * 2.0 };
            let pad_y = padding;
            (tw + pad_x * 2.0, th + pad_y * 2.0)
        }
        other => {
            return Err(Error::InvalidModel {
                message: format!("unsupported state node shape: {other}"),
            });
        }
    };
    Ok((w.max(1.0), h.max(1.0)))
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::text::{TextMetrics, TextStyle};

    struct NonLatticeMeasurer {
        width: f64,
        height: f64,
    }

    impl TextMeasurer for NonLatticeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: self.width,
                height: self.height,
                line_count: 1,
            }
        }
    }

    #[test]
    fn state_html_metrics_preserve_host_measurement_precision() {
        let measurer = NonLatticeMeasurer {
            width: 73.123_456_789,
            height: 17.25,
        };
        let typography = crate::state::ResolvedLabelTypography::new(TextStyle::default());

        assert_eq!(
            edge_label_metrics(
                "edge",
                "edge",
                &measurer,
                &typography,
                WrapMode::HtmlLike,
                None,
            ),
            (73.123_456_789, 17.25)
        );
        assert_eq!(
            node_label_metrics(
                "node",
                "node",
                180.0,
                &measurer,
                &typography,
                WrapMode::HtmlLike,
                None,
            ),
            (73.123_456_789, 17.25)
        );
        assert_eq!(
            title_label_metrics(
                crate::state::StateLabelOwner::NodeTitle("node"),
                "title",
                &measurer,
                &typography,
                WrapMode::HtmlLike,
                None,
            ),
            (73.123_456_789, 17.25)
        );
    }

    #[test]
    fn state_label_metrics_keep_html_min_content_and_svg_background_padding() {
        let measurer = NonLatticeMeasurer {
            width: 250.123_456_789,
            height: 17.25,
        };
        let typography = crate::state::ResolvedLabelTypography::new(TextStyle::default());

        assert_eq!(
            edge_label_metrics(
                "edge",
                "edge",
                &measurer,
                &typography,
                WrapMode::HtmlLike,
                None,
            ),
            (250.123_456_789, 17.25)
        );
        assert_eq!(
            node_label_metrics(
                "node",
                "node",
                180.0,
                &measurer,
                &typography,
                WrapMode::HtmlLike,
                None,
            ),
            (250.123_456_789, 17.25)
        );
        // Keep this padding control below createText's 200px wrapping width.
        let measurer = NonLatticeMeasurer {
            width: 73.123_456_789,
            height: 17.25,
        };
        assert_eq!(
            edge_label_metrics(
                "edge",
                "edge",
                &measurer,
                &typography,
                WrapMode::SvgLike,
                None,
            ),
            (77.123_456_789, 21.25)
        );
    }
}
