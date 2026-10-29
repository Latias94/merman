//! Shared native State label geometry for layout and SVG emission.

use std::{borrow::Cow, sync::Arc};

use crate::text::{MermaidMarkdownWordType, TextMetrics, WrapMode, svg_markdown_word_style};

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

#[derive(Debug, Clone, PartialEq)]
enum StateNativeLabelLines {
    Plain(Arc<[Box<str>]>),
    Markdown(Arc<[Vec<(String, MermaidMarkdownWordType)>]>),
}

impl StateNativeLabelLines {
    fn len(&self) -> usize {
        match self {
            Self::Plain(lines) => lines.len(),
            Self::Markdown(lines) => lines.len(),
        }
    }

    fn plain_text(&self, index: usize) -> Cow<'_, str> {
        match self {
            Self::Plain(lines) => Cow::Borrowed(&lines[index]),
            Self::Markdown(lines) => Cow::Owned(
                lines[index]
                    .iter()
                    .map(|(word, _)| crate::entities::decode_svg_text_content_entities(word))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        }
    }
}

fn measured_wrapped_lines(request: StateLabelMetricsRequest<'_>) -> StateNativeLabelLines {
    if request.source_kind == StateLabelSourceKind::Plain {
        return StateNativeLabelLines::Plain(
            super::label::state_svg_plain_lines(request.text)
                .into_iter()
                .map(String::into_boxed_str)
                .collect::<Vec<_>>()
                .into(),
        );
    }
    if request.max_width_px.is_none()
        && let Some(text) = super::state_markdown_label_plain_text(request.text)
    {
        return StateNativeLabelLines::Plain(vec![text.into_owned().into_boxed_str()].into());
    }
    let lines = super::label::state_svg_markdown_word_lines(
        request.text,
        request.measurer,
        request.typography.text_style(),
        request.max_width_px,
        request.wrap_mode,
    );
    if lines
        .iter()
        .flatten()
        .all(|(_, kind)| *kind == MermaidMarkdownWordType::Normal)
    {
        return StateNativeLabelLines::Plain(
            lines
                .into_iter()
                .map(|words| {
                    words
                        .iter()
                        .map(|(word, _)| crate::entities::decode_svg_text_content_entities(word))
                        .collect::<Vec<_>>()
                        .join(" ")
                        .into_boxed_str()
                })
                .collect::<Vec<_>>()
                .into(),
        );
    }
    StateNativeLabelLines::Markdown(lines.into())
}

struct MarkdownLineBbox {
    advance_px: f64,
    left_px: f64,
    right_px: f64,
    height_px: f64,
    y_px: f64,
}

fn markdown_line_bbox(
    request: StateLabelMetricsRequest<'_>,
    words: &[(String, MermaidMarkdownWordType)],
) -> MarkdownLineBbox {
    let mut advance = 0.0;
    let mut left = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut top = f64::INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for (index, (word, kind)) in words.iter().enumerate() {
        let visible = crate::entities::decode_svg_text_content_entities(word);
        let text = if index == 0 {
            visible
        } else {
            Cow::Owned(format!(" {visible}"))
        };
        let style = svg_markdown_word_style(request.typography.text_style(), *kind);
        let width = request
            .measurer
            .measure_svg_text_computed_length_px(&text, &style);
        let (word_left, word_right) = request.measurer.measure_svg_text_bbox_x(&text, &style);
        let center = advance + width / 2.0;
        left = left.min(center - word_left);
        right = right.max(center + word_right);
        let y = request
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(&text, &style);
        let height = request
            .measurer
            .measure_svg_tspan_text_bbox_height_px(&text, &style);
        top = top.min(y);
        bottom = bottom.max(y + height);
        advance += width;
    }
    if words.is_empty() {
        return MarkdownLineBbox {
            advance_px: 0.0,
            left_px: 0.0,
            right_px: 0.0,
            height_px: 0.0,
            y_px: 0.0,
        };
    }
    MarkdownLineBbox {
        advance_px: advance,
        left_px: left,
        right_px: right,
        height_px: bottom - top,
        y_px: top,
    }
}

/// Operation-local geometry shared by State native-text layout and terminal SVG emission.
///
/// Vertical baselines are retained in SVG user-space pixels relative to the layout box's top.
/// Start-anchored rows also retain their horizontal baseline origin relative to its left edge.
/// Labels retain the wrapped rows and the
/// operation-selected SVG bbox profile after their first layout measurement, so the writer does not
/// segment the semantic label independently.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StateNativeLabelGeometry {
    width_px: f64,
    height_px: f64,
    ink_left_from_anchor_px: f64,
    ink_right_from_anchor_px: f64,
    baseline_x_px: f64,
    first_baseline_y_px: f64,
    line_height_px: f64,
    line_count: usize,
    measured_lines: StateNativeLabelLines,
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
        if matches!(&lines, StateNativeLabelLines::Markdown(rows) if rows.iter().all(Vec::is_empty))
        {
            // Preserve an empty SVG projection instead of falling back to the literal source.
            return Some(Self {
                width_px: 0.0,
                height_px: native_padding_px(request.owner) * 2.0,
                ink_left_from_anchor_px: 0.0,
                ink_right_from_anchor_px: 0.0,
                baseline_x_px: 0.0,
                first_baseline_y_px: 0.0,
                line_height_px: 0.0,
                line_count: lines.len(),
                measured_lines: lines,
            });
        }
        let metrics = match &lines {
            StateNativeLabelLines::Plain(_) => measurement.metrics,
            StateNativeLabelLines::Markdown(words) => super::label::measure_state_svg_word_lines(
                words,
                request.measurer,
                request.typography.text_style(),
            ),
        };
        if metrics.line_count == 0 || lines.len() != metrics.line_count {
            return None;
        }

        let first_line = lines.plain_text(0);
        let row_style = svg_markdown_word_style(
            request.typography.text_style(),
            MermaidMarkdownWordType::Normal,
        );
        let first_line_bbox_height_px = request
            .measurer
            .measure_svg_tspan_text_bbox_height_px(&first_line, &row_style);
        let line_height_px = if matches!(&lines, StateNativeLabelLines::Markdown(_)) {
            // Styled ink extents already include face differences; row baselines stay 1.1em apart.
            request.typography.text_style().font_size * 1.1
        } else if metrics.line_count == 1 {
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
        let mut bbox_y_px = request
            .measurer
            .measure_svg_create_text_bbox_y_offset_px(&first_line, &row_style);
        drop(first_line);
        let mut content_height_px = metrics.height;
        let mut ink_left_from_anchor_px = 0.0_f64;
        let mut ink_right_from_anchor_px = 0.0_f64;
        let mut styled_left = f64::INFINITY;
        let mut styled_right = f64::NEG_INFINITY;
        let mut styled_top = f64::INFINITY;
        let mut styled_bottom = f64::NEG_INFINITY;
        for index in 0..lines.len() {
            let (left, right) = match &lines {
                StateNativeLabelLines::Plain(lines) => request
                    .measurer
                    .measure_svg_text_bbox_x(&lines[index], &row_style),
                StateNativeLabelLines::Markdown(lines) => {
                    let bbox = markdown_line_bbox(request, &lines[index]);
                    if !bbox.advance_px.is_finite()
                        || !bbox.left_px.is_finite()
                        || !bbox.right_px.is_finite()
                        || bbox.advance_px < 0.0
                        || bbox.right_px < bbox.left_px
                    {
                        return None;
                    }
                    styled_left = styled_left.min(bbox.left_px);
                    styled_right = styled_right.max(bbox.right_px);
                    let row_y = index as f64 * line_height_px;
                    styled_top = styled_top.min(row_y + bbox.y_px);
                    styled_bottom = styled_bottom.max(row_y + bbox.y_px + bbox.height_px);
                    (
                        bbox.advance_px / 2.0 - bbox.left_px,
                        bbox.right_px - bbox.advance_px / 2.0,
                    )
                }
            };
            if !left.is_finite()
                || !right.is_finite()
                || ((left < 0.0 || right < 0.0)
                    && (matches!(&lines, StateNativeLabelLines::Plain(_))
                        || matches!(request.owner, StateLabelOwner::Edge(_))))
            {
                return None;
            }
            ink_left_from_anchor_px = ink_left_from_anchor_px.max(left);
            ink_right_from_anchor_px = ink_right_from_anchor_px.max(right);
        }
        if matches!(&lines, StateNativeLabelLines::Markdown(_)) {
            bbox_y_px = styled_top;
            content_height_px = styled_bottom - styled_top;
        }

        let padding_px = native_padding_px(request.owner);
        let (width_px, baseline_x_px) = match request.owner {
            StateLabelOwner::Edge(_) => (
                symmetric_edge_width_px(
                    ink_left_from_anchor_px,
                    ink_right_from_anchor_px,
                    metrics.width / 2.0,
                )?,
                0.0,
            ),
            _ if matches!(&lines, StateNativeLabelLines::Markdown(_)) => {
                // Node rows share a start anchor, unlike independently centered edge rows.
                (styled_right - styled_left, -styled_left)
            }
            StateLabelOwner::Node(_)
            | StateLabelOwner::NodeTitle(_)
            | StateLabelOwner::NodeDescription(_)
            | StateLabelOwner::ClusterTitle(_) => (metrics.width, 0.0),
        };
        let height_px = content_height_px + padding_px * 2.0;
        // Mermaid's native createText recipe places its first alphabetic baseline one font-size
        // below the text origin. Retain the selected DOM bbox offset, then express that same
        // baseline directly from the layout box top instead of replaying y/dy declarations.
        let first_baseline_y_px =
            padding_px + request.typography.text_style().font_size.max(1.0) - bbox_y_px;
        if !width_px.is_finite()
            || !height_px.is_finite()
            || !baseline_x_px.is_finite()
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
            baseline_x_px,
            first_baseline_y_px,
            line_height_px,
            line_count: metrics.line_count,
            measured_lines: lines,
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

    pub(crate) const fn baseline_x_px(&self) -> f64 {
        self.baseline_x_px
    }

    pub(crate) fn baseline_y_px(&self, line_index: usize) -> Option<f64> {
        if line_index < self.line_count {
            Some(self.first_baseline_y_px + self.line_height_px * line_index as f64)
        } else {
            None
        }
    }

    pub(crate) fn measured_line(&self, index: usize) -> Option<&str> {
        match &self.measured_lines {
            StateNativeLabelLines::Plain(lines) => lines.get(index).map(AsRef::as_ref),
            StateNativeLabelLines::Markdown(_) => None,
        }
    }

    pub(crate) fn measured_word_line(
        &self,
        index: usize,
    ) -> Option<&[(String, MermaidMarkdownWordType)]> {
        match &self.measured_lines {
            StateNativeLabelLines::Plain(_) => None,
            StateNativeLabelLines::Markdown(lines) => lines.get(index).map(Vec::as_slice),
        }
    }

    pub(crate) fn layout_metrics(&self) -> TextMetrics {
        TextMetrics {
            width: self.width_px,
            height: self.height_px,
            line_count: self.line_count,
        }
    }
}
