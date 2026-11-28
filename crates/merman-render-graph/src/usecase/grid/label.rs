//! Retained Usecase Grid labelRect terminals in relationship order.

use super::allocation_error;
use crate::label_rect::LabelRect;
use merman_model::{
    OperationPhase, families::usecase::UsecaseLabelType, presentation::PresentationConfig,
};
use merman_render_core::{
    LayoutLabel, Result,
    config::{config_bool, config_f64},
    graph_layout::{Rect, Size},
    html::html_label_overflow_attr,
    math::{
        MathLabelMetricsRequest, MathRenderer, math_label_metrics_for_layout,
        render_math_html_label,
    },
    number_format::canonical_number as fmt,
    resources::{OperationWorkMeter, PreparedTextRetainedReservation},
    svg_output::SvgOutput,
    text::{
        TextMeasurer, TextMetrics, TextStyle, WrapMode,
        literal_label::{
            FlowchartSvgLabelSource, FlowchartSvgWidthMode, flowchart_svg_label_rows_heap_bytes,
        },
        measure_xhtml_label_fragment_with_terminal_fonts,
        svg_label::write_svg_text_source_word_lines_with_style,
    },
    xml::escape_attr_display,
};
use std::sync::Arc;

pub(in crate::usecase) const MERMAID_EDGE_LABEL_FONT_SIZE: f64 = 10.0;

pub(in crate::usecase) struct GridLabels<'a> {
    config: &'a PresentationConfig,
    font: &'a TextStyle,
    svg_edge_font: Option<TextStyle>,
    html: bool,
    wrapping_width: f64,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    work: &'a Arc<OperationWorkMeter>,
    labels: Vec<GridLabel>,
}

impl<'a> GridLabels<'a> {
    pub(in crate::usecase) fn new(
        config: &'a PresentationConfig,
        font: &'a TextStyle,
        native: bool,
        measurer: &'a dyn TextMeasurer,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        work: &'a Arc<OperationWorkMeter>,
    ) -> Self {
        let svg_edge_font = (!native).then(|| TextStyle {
            font_size: MERMAID_EDGE_LABEL_FONT_SIZE,
            font_weight: Some("normal".into()),
            ..font.clone()
        });
        Self {
            config,
            font,
            svg_edge_font,
            html: config_bool(config.as_value(), &["htmlLabels"]).unwrap_or(true),
            wrapping_width: config_f64(config.as_value(), &["flowchart", "wrappingWidth"])
                .filter(|width| width.is_finite() && *width > 0.0)
                .unwrap_or(120.0),
            measurer,
            math_renderer,
            work,
            labels: Vec::new(),
        }
    }

    pub(in crate::usecase) fn measure(
        &mut self,
        edge_slot: usize,
        source: &str,
        kind: UsecaseLabelType,
    ) -> Result<()> {
        if source.is_empty() {
            return Ok(());
        }
        self.work.charge(source.len())?;
        let capacity = self.labels.capacity();
        let next_capacity = if self.labels.len() == capacity {
            capacity.saturating_mul(2).max(1)
        } else {
            capacity
        };
        // The label that grows the buffer owns the entire new tranche, including empty slots.
        let owner_bytes = (next_capacity - capacity)
            .checked_mul(std::mem::size_of::<GridLabel>())
            .ok_or_else(|| self.work.arithmetic_overflow())?;
        self.work
            .preflight_prepared_text_retained_bytes(owner_bytes)?;
        let source = super::super::create_text_source(source, kind);
        let (body, box_, bytes) = if self.html {
            if let Some(fragment) = render_math_html_label(&source, self.config, self.math_renderer)
            {
                let metrics = math_label_metrics_for_layout(MathLabelMetricsRequest {
                    measurer: self.measurer,
                    raw_label: &source,
                    style: self.font,
                    max_width_px: Some(self.wrapping_width),
                    wrap_mode: WrapMode::HtmlLike,
                    config: self.config,
                    math_renderer: self.math_renderer,
                })
                .unwrap_or_else(|| {
                    measure_xhtml_label_fragment_with_terminal_fonts(
                        self.measurer,
                        &fragment,
                        self.font,
                        Some(self.wrapping_width),
                        16.0,
                    )
                });
                let bytes = fragment.capacity();
                (
                    GridLabelBody::Math {
                        fragment,
                        metrics,
                        wrapping_width: self.wrapping_width,
                    },
                    centered(metrics),
                    bytes,
                )
            } else {
                let terminal = LabelRect::prepare(
                    &source,
                    self.measurer,
                    self.font,
                    true,
                    self.config.as_config(),
                    self.wrapping_width,
                    true,
                );
                let rect = centered(terminal.metrics());
                let bytes = terminal.retained_buffer_bytes();
                (GridLabelBody::Html(terminal), rect, bytes)
            }
        } else {
            let source = FlowchartSvgLabelSource::new(&source);
            let rows =
                source.wrapped_lines(self.measurer, self.font, Some(self.wrapping_width), true);
            let measure = |style| {
                source.metrics_from_wrapped(
                    self.measurer,
                    style,
                    &rows,
                    FlowchartSvgWidthMode::Bbox,
                )
            };
            let bbox_top = |style| {
                if source.plain_text().is_empty() {
                    0.0
                } else {
                    self.measurer
                        .measure_svg_create_text_bbox_y_offset_px(source.plain_text(), style)
                }
            };
            let initial = measure(self.font);
            let x = -initial.width / 2.0;
            let top = bbox_top(self.font);
            let y = -initial.height / 2.0 - top;
            // labelRect computes the fixed transform before swapping .label for .edgeLabel.
            // createGraph then reads the completed group's bbox with the same rows at 10px.
            let (final_metrics, final_top) = self
                .svg_edge_font
                .as_ref()
                .map_or((initial, top), |font| (measure(font), bbox_top(font)));
            let rect = Rect {
                x,
                y: y + final_top,
                width: final_metrics.width,
                height: final_metrics.height,
            };
            let bytes = flowchart_svg_label_rows_heap_bytes(&rows);
            (GridLabelBody::Svg { rows, x, y }, rect, bytes)
        };
        self.work.checkpoint(OperationPhase::Layout)?;
        let _retained = self
            .work
            .reserve_prepared_text_retained_bytes(owner_bytes.saturating_add(bytes))?;
        if next_capacity > capacity {
            self.labels
                .try_reserve_exact(next_capacity - self.labels.len())
                .map_err(|_| allocation_error())?;
        }
        self.labels.push(GridLabel {
            edge_slot,
            body,
            box_,
            _retained,
        });
        Ok(())
    }

    pub(in crate::usecase) fn finish(self) -> Vec<GridLabel> {
        self.labels
    }
}

#[derive(Debug)]
pub(in crate::usecase) struct GridLabel {
    pub(in crate::usecase) edge_slot: usize,
    body: GridLabelBody,
    box_: Rect,
    _retained: PreparedTextRetainedReservation,
}

#[derive(Debug)]
enum GridLabelBody {
    Html(LabelRect),
    Math {
        fragment: String,
        metrics: TextMetrics,
        wrapping_width: f64,
    },
    Svg {
        rows: Vec<Vec<String>>,
        x: f64,
        y: f64,
    },
}

impl GridLabel {
    pub(in crate::usecase) fn size(&self) -> Size {
        let rect = self.bounds();
        Size {
            width: rect.width,
            height: rect.height,
        }
    }

    pub(in crate::usecase) fn bounds(&self) -> Rect {
        let x = self.box_.x.min(0.0);
        let y = self.box_.y.min(0.0);
        Rect {
            x,
            y,
            width: (self.box_.x + self.box_.width).max(0.1) - x,
            height: (self.box_.y + self.box_.height).max(0.1) - y,
        }
    }

    pub(in crate::usecase) fn write(
        &self,
        out: &mut impl SvgOutput,
        id: &str,
        label: &LayoutLabel,
        overflow: bool,
    ) -> Result<()> {
        if let GridLabelBody::Html(terminal) = &self.body {
            let metrics = terminal.metrics();
            let label = LayoutLabel {
                width: metrics.width,
                height: metrics.height,
                ..label.clone()
            };
            return terminal.write(out, &label, id, (0.0, 0.0), overflow);
        }
        let (x, y) = match &self.body {
            GridLabelBody::Svg { x, y, .. } => (*x, *y),
            GridLabelBody::Math { metrics, .. } => (-metrics.width / 2.0, -metrics.height / 2.0),
            GridLabelBody::Html(_) => unreachable!(),
        };
        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate({}, {})"><rect/>"#,
            escape_attr_display(id),
            fmt(label.x),
            fmt(label.y),
            fmt(x),
            fmt(y),
        );
        match &self.body {
            GridLabelBody::Svg { rows, .. } => {
                out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                write_svg_text_source_word_lines_with_style(out, rows, "", false, None);
                out.push_str("</g>");
            }
            GridLabelBody::Math {
                fragment,
                metrics,
                wrapping_width,
            } => {
                let wraps = metrics.width >= *wrapping_width;
                let (display, whitespace) = if wraps {
                    ("table", "break-spaces")
                } else {
                    ("table-cell", "nowrap")
                };
                let _ = write!(
                    out,
                    r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="display: {display}; white-space: {whitespace}; line-height: 1.5; max-width: {}px; text-align: center;"#,
                    fmt(metrics.width),
                    fmt(metrics.height),
                    html_label_overflow_attr(overflow, metrics.width, metrics.height),
                    fmt(*wrapping_width)
                );
                if wraps {
                    let _ = write!(out, " width: {}px;", fmt(*wrapping_width));
                }
                out.push_str(r#""><span class="nodeLabel">"#);
                out.push_str(fragment);
                out.push_str("</span></div></foreignObject>");
            }
            GridLabelBody::Html(_) => unreachable!(),
        }
        out.push_str("</g></g>");
        out.checkpoint()
    }
}

fn centered(metrics: TextMetrics) -> Rect {
    Rect {
        x: -metrics.width / 2.0,
        y: -metrics.height / 2.0,
        width: metrics.width,
        height: metrics.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::{resources::RenderResourcePolicy, text::DeterministicTextMeasurer};

    #[test]
    fn retained_svg_label_charge_includes_unused_owner_slots() {
        let config =
            crate::test_support::presentation_config(serde_json::json!({"htmlLabels": false}));
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let font = TextStyle::default();
        let measurer = DeterministicTextMeasurer::default();
        let mut labels = GridLabels::new(&config, &font, false, &measurer, None, &meter);
        for slot in 0..3 {
            labels
                .measure(slot, "ordinary words", UsecaseLabelType::Text)
                .unwrap();
        }
        let retained = labels.finish();
        assert!(retained.capacity() > retained.len());
        let word_bytes = retained
            .iter()
            .map(|label| {
                let GridLabelBody::Svg { rows, .. } = &label.body else {
                    panic!("SVG terminal")
                };
                flowchart_svg_label_rows_heap_bytes(rows)
            })
            .sum::<usize>();
        assert_eq!(
            meter.prepared_text_retained_bytes(),
            retained.capacity() * std::mem::size_of::<GridLabel>() + word_bytes
        );
        drop(retained);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}
