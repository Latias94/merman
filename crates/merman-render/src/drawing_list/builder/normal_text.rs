//! Bounded plain-text layout for a centered anonymous flex item (`white-space: normal`).

use super::{DrawingListBuilder, allocation_failed, checked_increment, contract_error};
use crate::Result;
use crate::environment::TextMeasurementPhase;
use crate::text::{NormalLineMetrics, TextMeasurer as _, TextStyle as MeasurementStyle};
use merman_core::OperationPhase;
use merman_display_list::{
    Point, Rect, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle,
};
use std::ops::Range;

struct Line {
    range: Range<usize>,
    width: f64,
    metrics: NormalLineMetrics,
}

impl DrawingListBuilder<'_> {
    /// Resolves plain text once. Temporary normalized storage is at most twice its non-space
    /// payload; line records are admitted as commands before allocation. No host receives HTML.
    pub(crate) fn draw_normal_text(
        &mut self,
        text: &str,
        bounds: Rect,
        measurement: &MeasurementStyle,
        style: &TextStyle,
        obligation: &TextObligation,
    ) -> Result<()> {
        let normalized = self.normalize_normal_text(text)?;
        if normalized.is_empty() {
            return Ok(());
        }
        let session = self.session;
        let measurer =
            session.controlled_text_measurer(TextMeasurementPhase::Wrap, OperationPhase::Emit);
        let measure_width = |text: &str| -> Result<f64> {
            session
                .work_meter()
                .charge_at(text.len(), OperationPhase::Emit)?;
            let width = measurer.measure_canvas_text_width_px(text, measurement);
            session.checkpoint(OperationPhase::Emit)?;
            if !width.is_finite() || width < 0.0 {
                return Err(contract_error(
                    "Normal text measurement returned invalid width",
                ));
            }
            Ok(width)
        };

        // An anonymous flex item cannot shrink below its min-content width. Measuring complete
        // candidates (rather than summing word advances) preserves kerning and shaping.
        let mut available = bounds.width;
        for range in normal_segments(&normalized) {
            available = available.max(measure_width(normalized[range].trim_matches(' '))?);
        }
        let mut lines = Vec::<Line>::new();
        let mut projected = self.usage;
        let mut total_height = 0.0;
        let mut append_line = |range: Range<usize>, width: f64| -> Result<()> {
            checked_increment(&mut projected.commands, 1, "command count")?;
            checked_increment(&mut projected.text_bytes, range.len(), "text byte count")?;
            self.preflight(projected)?;
            lines
                .try_reserve(1)
                .map_err(|_| allocation_failed("normal text lines"))?;
            session
                .work_meter()
                .charge_at(range.len(), OperationPhase::Emit)?;
            let metrics =
                measurer.measure_normal_line_metrics(&normalized[range.clone()], measurement);
            session.checkpoint(OperationPhase::Emit)?;
            if !metrics.line_height.is_finite()
                || metrics.line_height < 0.0
                || !metrics.baseline_offset.is_finite()
            {
                return Err(contract_error(
                    "Normal text measurement returned invalid line metrics",
                ));
            }
            total_height += metrics.line_height;
            if !total_height.is_finite() {
                return Err(contract_error("Normal text line heights overflow"));
            }
            lines.push(Line {
                range,
                width,
                metrics,
            });
            Ok(())
        };
        let mut start = 0;
        let mut previous_end = 0;
        let mut previous_width = 0.0;
        for segment in normal_segments(&normalized) {
            let end = segment.start + normalized[segment.clone()].trim_end_matches(' ').len();
            let width = measure_width(&normalized[start..end])?;
            if width > available && previous_end > start {
                append_line(start..previous_end, previous_width)?;
                start = segment.start;
                while normalized.as_bytes().get(start) == Some(&b' ') {
                    start += 1;
                }
                previous_width = measure_width(&normalized[start..end])?;
            } else {
                previous_width = width;
            }
            previous_end = end;
        }
        if previous_end > start {
            append_line(start..previous_end, previous_width)?;
        }

        let mut top = bounds.y + (bounds.height - total_height) / 2.0;
        for line in lines {
            let origin = Point::new(
                bounds.x + bounds.width / 2.0,
                top + line.metrics.baseline_offset,
            );
            let line_bounds = Rect::new(
                origin.x - line.width / 2.0,
                top,
                line.width,
                line.metrics.line_height,
            );
            self.draw_host_text(&normalized[line.range], |text| {
                let mut style = style.clone();
                style.line_height = line.metrics.line_height;
                TextRun {
                    text,
                    origin,
                    bounds: line_bounds,
                    style,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Alphabetic,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: obligation.clone(),
                }
            })?;
            top += line.metrics.line_height;
        }
        Ok(())
    }

    pub(crate) fn normalize_normal_text(&self, text: &str) -> Result<String> {
        self.session
            .work_meter()
            .charge_at(text.len(), OperationPhase::Emit)?;
        let mut bytes = 0usize;
        let mut retained = 0usize;
        let mut pending_space = false;
        let mut projected = self.usage;
        for (index, ch) in text.char_indices() {
            if index % 1024 < ch.len_utf8() {
                self.session.checkpoint(OperationPhase::Emit)?;
            }
            if html_space(ch) {
                pending_space = bytes != 0;
                continue;
            }
            checked_increment(
                &mut bytes,
                ch.len_utf8() + usize::from(pending_space),
                "normal text byte count",
            )?;
            checked_increment(&mut retained, ch.len_utf8(), "text byte count")?;
            pending_space = false;
        }
        if bytes == 0 {
            return Ok(String::new());
        }
        checked_increment(&mut projected.commands, 1, "command count")?;
        // Spaces at chosen wrap boundaries are discarded. Admit the lower bound here, then
        // exact finalized line bytes below; rejecting the upper bound would break exact quotas.
        checked_increment(&mut projected.text_bytes, retained, "text byte count")?;
        self.preflight(projected)?;
        let mut result = String::new();
        result
            .try_reserve_exact(bytes)
            .map_err(|_| allocation_failed("normal text"))?;
        pending_space = false;
        for (index, ch) in text.char_indices() {
            if index % 1024 < ch.len_utf8() {
                self.session.checkpoint(OperationPhase::Emit)?;
            }
            if html_space(ch) {
                pending_space = !result.is_empty();
            } else {
                if pending_space {
                    result.push(' ');
                }
                result.push(ch);
                pending_space = false;
            }
        }
        Ok(result)
    }
}

fn html_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n' | '\u{000c}')
}

fn normal_segments(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut start = 0;
    unicode_linebreak::linebreaks(text)
        .filter(move |(end, opportunity)| {
            // Same Chromium-characterized slash rule as text::line_break, without the
            // preserved-space segmentation specific to `white-space: break-spaces`.
            *opportunity != unicode_linebreak::BreakOpportunity::Allowed
                || !text[..*end].ends_with('/')
        })
        .map(move |(end, _)| {
            let range = start..end;
            start = end;
            range
        })
}
