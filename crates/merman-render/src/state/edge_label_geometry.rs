//! Shared native State label geometry for layout and SVG emission.

use std::sync::Arc;

use crate::entities::decode_mermaid_entities_for_render_text;
use crate::text::{TextMetrics, WrapMode};

use super::StateLabelMeasurement;
use super::label_artifact::{StateLabelMetricsRequest, StateLabelOwner, StateLabelSourceKind};

pub(super) const STATE_SVG_EDGE_LABEL_BACKGROUND_PADDING_PX: f64 = 2.0;

fn native_padding_px(owner: StateLabelOwner<'_>) -> f64 {
    match owner {
        StateLabelOwner::Edge(_) => STATE_SVG_EDGE_LABEL_BACKGROUND_PADDING_PX,
        StateLabelOwner::Node(_)
        | StateLabelOwner::NodeTitle(_)
        | StateLabelOwner::NodeDescription(_)
        | StateLabelOwner::ClusterTitle(_) => 0.0,
    }
}

fn symmetric_edge_width_px(
    ink_left_from_anchor_px: f64,
    ink_right_from_anchor_px: f64,
    minimum_half_width_px: f64,
) -> Option<f64> {
    if !ink_left_from_anchor_px.is_finite()
        || !ink_right_from_anchor_px.is_finite()
        || !minimum_half_width_px.is_finite()
        || ink_left_from_anchor_px < 0.0
        || ink_right_from_anchor_px < 0.0
        || minimum_half_width_px < 0.0
    {
        return None;
    }
    let half_width_px = ink_left_from_anchor_px
        .max(ink_right_from_anchor_px)
        .max(minimum_half_width_px)
        + STATE_SVG_EDGE_LABEL_BACKGROUND_PADDING_PX;
    let width_px = half_width_px * 2.0;
    (width_px.is_finite() && width_px >= 0.0).then_some(width_px)
}

fn measured_wrapped_lines(request: StateLabelMetricsRequest<'_>) -> Vec<String> {
    let simple_text = match request.source_kind {
        StateLabelSourceKind::Markdown => super::state_markdown_label_plain_text(request.text),
        StateLabelSourceKind::Plain => Some(decode_mermaid_entities_for_render_text(request.text)),
    };
    if let Some(measured_text) = simple_text {
        return crate::text::DeterministicTextMeasurer::normalized_text_lines_for_wrap_mode(
            measured_text.as_ref(),
            request.wrap_mode,
        )
        .into_iter()
        .flat_map(|line| {
            crate::text::wrap_text_lines_measurer(
                &line,
                request.measurer,
                request.typography.text_style(),
                request.max_width_px,
            )
        })
        .collect();
    }

    let decoded = decode_mermaid_entities_for_render_text(request.text);
    crate::text::mermaid_markdown_to_wrapped_word_lines(
        request.measurer,
        decoded.as_ref(),
        request.typography.text_style(),
        request.max_width_px,
        request.wrap_mode,
    )
    .into_iter()
    .map(|words| {
        words
            .into_iter()
            .map(|(word, _)| crate::entities::decode_svg_text_content_entities(&word).into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    })
    .collect()
}

/// Operation-local geometry shared by State native-text layout and terminal SVG emission.
///
/// Baselines are retained in SVG user-space pixels relative to the top of the layout box.
/// Labels retain the wrapped rows and the
/// operation-selected SVG bbox profile after their first layout measurement, so the writer does not
/// segment the semantic label independently.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateNativeLabelGeometry {
    width_px: f64,
    height_px: f64,
    ink_left_from_anchor_px: f64,
    ink_right_from_anchor_px: f64,
    first_baseline_y_px: f64,
    line_height_px: f64,
    line_count: usize,
    measured_lines: Option<Arc<[Box<str>]>>,
}

impl StateNativeLabelGeometry {
    pub(super) fn from_measurement(
        request: StateLabelMetricsRequest<'_>,
        measurement: &StateLabelMeasurement,
    ) -> Option<Self> {
        if !matches!(
            request.wrap_mode,
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun
        ) {
            return None;
        }

        let lines = measured_wrapped_lines(request);
        let metrics = measurement.metrics;
        if metrics.line_count == 0 || lines.len() != metrics.line_count {
            return None;
        }

        let first_line = lines.first()?.as_str();
        let mut ink_left_from_anchor_px = 0.0_f64;
        let mut ink_right_from_anchor_px = 0.0_f64;
        for line in &lines {
            let (left, right) = request
                .measurer
                .measure_svg_text_bbox_x(line, request.typography.text_style());
            if !left.is_finite() || !right.is_finite() || left < 0.0 || right < 0.0 {
                return None;
            }
            ink_left_from_anchor_px = ink_left_from_anchor_px.max(left);
            ink_right_from_anchor_px = ink_right_from_anchor_px.max(right);
        }

        let padding_px = native_padding_px(request.owner);
        let width_px = match request.owner {
            StateLabelOwner::Edge(_) => symmetric_edge_width_px(
                ink_left_from_anchor_px,
                ink_right_from_anchor_px,
                metrics.width / 2.0,
            )?,
            StateLabelOwner::Node(_)
            | StateLabelOwner::NodeTitle(_)
            | StateLabelOwner::NodeDescription(_)
            | StateLabelOwner::ClusterTitle(_) => metrics.width,
        };
        let height_px = metrics.height + padding_px * 2.0;
        let first_line_bbox_height_px = request
            .measurer
            .measure_svg_tspan_text_bbox_height_px(first_line, request.typography.text_style());
        let line_height_px = if metrics.line_count == 1 {
            metrics.height.max(first_line_bbox_height_px)
        } else {
            let additional_height_px = metrics.height - first_line_bbox_height_px;
            let measured_line_height_px =
                additional_height_px / metrics.line_count.saturating_sub(1) as f64;
            if measured_line_height_px.is_finite() && measured_line_height_px > 0.0 {
                measured_line_height_px
            } else {
                metrics.height / metrics.line_count as f64
            }
        };
        let bbox_y_px = request
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(first_line, request.typography.text_style());
        // Mermaid's native createText recipe places its first alphabetic baseline one font-size
        // below the text origin. Retain the selected DOM bbox offset, then express that same
        // baseline directly from the layout box top instead of replaying y/dy declarations.
        let first_baseline_y_px =
            padding_px + request.typography.text_style().font_size.max(1.0) - bbox_y_px;
        if !width_px.is_finite()
            || !height_px.is_finite()
            || !first_baseline_y_px.is_finite()
            || !line_height_px.is_finite()
            || width_px < 0.0
            || height_px < 0.0
            || line_height_px < 0.0
        {
            return None;
        }

        Some(Self {
            width_px,
            height_px,
            ink_left_from_anchor_px,
            ink_right_from_anchor_px,
            first_baseline_y_px,
            line_height_px,
            line_count: metrics.line_count,
            measured_lines: Some(
                lines
                    .into_iter()
                    .map(String::into_boxed_str)
                    .collect::<Vec<_>>()
                    .into(),
            ),
        })
    }

    pub(crate) const fn width_px(&self) -> f64 {
        self.width_px
    }

    pub(crate) const fn height_px(&self) -> f64 {
        self.height_px
    }

    pub(crate) const fn line_count(&self) -> usize {
        self.line_count
    }

    pub(crate) fn baseline_y_px(&self, line_index: usize) -> Option<f64> {
        if line_index < self.line_count {
            Some(self.first_baseline_y_px + self.line_height_px * line_index as f64)
        } else {
            None
        }
    }

    pub(crate) fn measured_lines(&self) -> Option<impl ExactSizeIterator<Item = &str> + '_> {
        self.measured_lines
            .as_ref()
            .map(|lines| lines.iter().map(|line| line.as_ref()))
    }

    pub(crate) fn layout_metrics(&self) -> TextMetrics {
        TextMetrics {
            width: self.width_px,
            height: self.height_px,
            line_count: self.line_count,
        }
    }
}
