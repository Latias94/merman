use std::borrow::Cow;

use merman_model::families::sequence::{
    SequenceControlKind, SequenceControlRole, SequenceDiagramModel,
};
use merman_render_core::{
    Bounds, LayoutNode,
    resources::OperationWorkMeter,
    text::{TextMeasurer, TextStyle},
};
use rustc_hash::FxHashMap;

use super::super::{
    SequenceEmitCheckpoints, SequenceSvgRequest,
    block_text::{block_row_dy, display_block_label, wrap_svg_text_lines},
    blocks::{BlockLabelBox, block_frame_lines},
    control_paint::include_control_stroke_bounds,
    settings::SequenceRenderSettings,
};
use super::{
    Item, Label, Projected, Result, ScenePaint, SceneRole, SceneStroke, SceneTextAnchor,
    SceneTextBaseline, TextRole, admit_text, admit_token, allow_message, invalid, resolved_color,
    stroke, unsupported,
};
use crate::{
    frame::SequenceFrameLayout,
    preparation::{
        SequenceDrawnTextNode, SequencePreparedArtifact, measure_sequence_drawn_line_height,
        prepare_sequence_text_line, sequence_block_label_wrap_width,
        sequence_block_widths_for_render, sequence_drawn_text_first_y, sequence_drawn_text_style,
        sequence_drawn_text_y,
    },
};

pub(super) fn placed_frame(
    prepared: &SequencePreparedArtifact,
    message_index: usize,
) -> Option<&SequenceFrameLayout> {
    let index = prepared
        .frame_placements()
        .get(message_index)
        .copied()
        .flatten()?;
    prepared.layout().frames.get(index)
}

struct OpenFrame<'a> {
    index: usize,
    frame: &'a SequenceFrameLayout,
    next_section: usize,
}

pub(super) fn admit_messages(
    model: &SequenceDiagramModel,
    prepared: &SequencePreparedArtifact,
    meter: &OperationWorkMeter,
) -> Result<()> {
    let checks = SequenceEmitCheckpoints::for_emit(meter);
    let missing = || unsupported(meter, "sequence-frame-placement");
    let mut frames = prepared.layout().frames.iter().enumerate();
    let mut open = Vec::<OpenFrame<'_>>::new();
    for (index, message) in model.messages.iter().enumerate() {
        checks.checkpoint_loop(index)?;
        admit_token(&message.id, meter)?;
        let Some(control) = message.control_semantics() else {
            allow_message(message, meter)?;
            continue;
        };
        if !matches!(
            control.kind,
            SequenceControlKind::Loop | SequenceControlKind::Alt
        ) {
            return Err(unsupported(meter, "sequence-control"));
        }
        if prepared.native().is_some() {
            return Err(unsupported(meter, "sequence-native-frame"));
        }
        match control.role {
            SequenceControlRole::Start => {
                let (index, frame) = frames
                    .next()
                    .filter(|(_, frame)| frame.id == message.id && frame.kind == control.kind)
                    .ok_or_else(missing)?;
                admit_text(&frame.label, message.wrap, meter)?;
                meter.charge_emit_work(1)?;
                open.try_reserve(1)
                    .map_err(|_| invalid("unable to allocate Sequence compact frame admission"))?;
                open.push(OpenFrame {
                    index,
                    frame,
                    next_section: 0,
                });
            }
            SequenceControlRole::Separator => {
                let frame = open
                    .last_mut()
                    .filter(|open| open.frame.kind.accepts_separator(control.kind))
                    .ok_or_else(missing)?;
                let section = frame
                    .frame
                    .sections
                    .get(frame.next_section)
                    .filter(|section| section.id == message.id)
                    .ok_or_else(missing)?;
                admit_text(&section.label, message.wrap, meter)?;
                frame.next_section += 1;
            }
            SequenceControlRole::End => {
                let frame = open.pop().ok_or_else(missing)?;
                if !frame.frame.kind.accepts_end(control.kind)
                    || frame.next_section != frame.frame.sections.len()
                    || prepared.frame_placements().get(index) != Some(&Some(frame.index))
                {
                    return Err(missing());
                }
            }
        }
    }
    if !open.is_empty() || frames.next().is_some() {
        return Err(missing());
    }
    Ok(())
}

pub(super) struct FrameProjection<'a> {
    widths: FxHashMap<String, f64>,
    stroke: SceneStroke,
    keyword_paint: ScenePaint,
    style: &'a TextStyle,
    drawn_style: TextStyle,
    measurer: &'a dyn TextMeasurer,
    label_box_width: f64,
    label_box_drawn_width: f64,
    label_box_height: f64,
    box_margin: f64,
    box_text_margin: f64,
    wrap_padding: f64,
    pub(super) bounds: Option<Bounds>,
}

impl<'a> FrameProjection<'a> {
    pub(super) fn prepare(
        request: &SequenceSvgRequest<'a>,
        model: &SequenceDiagramModel,
        settings: &SequenceRenderSettings,
        nodes: &FxHashMap<&str, &LayoutNode>,
    ) -> Result<Self> {
        let meter = request.work_meter;
        let prepared = request.prepared;
        let typography = prepared.typography();
        let compat = typography.compat_binding();
        let border = resolved_color(&compat.label_box_border, meter)?;
        let keyword_paint = ScenePaint {
            fill: Some(resolved_color(&compat.label_box_fill, meter)?),
            stroke: Some(stroke(border, 1.0)),
        };

        // Admit both actor passes, the message scan, and its two width-table projections.
        // Preparation already charges the text envelope for layout and SVG replay.
        meter.charge_emit_work(model.actor_order.len().saturating_mul(2))?;
        meter.charge_emit_work(model.messages.len().saturating_mul(3))?;
        let widths = sequence_block_widths_for_render(
            model,
            prepared,
            nodes,
            request.config,
            request.measurer,
            meter,
        )?;
        let metrics = prepared.block_label_box_metrics();
        let style = typography.loop_label().measurement_style();
        Ok(Self {
            widths,
            stroke: stroke(border, 2.0),
            keyword_paint,
            style,
            drawn_style: sequence_drawn_text_style(style, request.config),
            measurer: request.measurer,
            label_box_width: metrics.width(),
            label_box_drawn_width: metrics.drawn_width(),
            label_box_height: settings.label_box_height,
            box_margin: settings.box_margin,
            box_text_margin: settings.box_text_margin,
            wrap_padding: settings.wrap_padding,
            bounds: None,
        })
    }

    pub(super) fn project(
        &mut self,
        projected: &mut Projected<'a>,
        frame: &SequenceFrameLayout,
    ) -> Result<()> {
        let checks = SequenceEmitCheckpoints::for_emit(projected.meter);
        checks.checkpoint()?;
        let start = projected.items.len();
        let x1 = frame.x;
        let x2 = frame.x + frame.width;
        let y1 = frame.y;
        let y2 = frame.y + frame.height;
        for coordinates in block_frame_lines([x1, y1, x2, y2]) {
            self.line(projected, coordinates, [2.0, 2.0])?;
        }
        for (index, section) in frame.sections.iter().enumerate() {
            checks.checkpoint_loop(index)?;
            self.line(projected, [x1, section.y, x2, section.y], [3.0, 3.0])?;
        }

        let label_box = BlockLabelBox::new(
            x1,
            y1,
            self.label_box_drawn_width,
            self.label_box_height,
            self.box_text_margin,
        );
        include_control_stroke_bounds(&mut self.bounds, label_box.bounds(), 1.0, true);
        projected.keyword_box(label_box.points, self.keyword_paint)?;
        projected.item(Item::Text {
            label: Label {
                text: Cow::Borrowed(frame.kind.keyword()),
                _retained: None,
            },
            role: TextRole::ControlKeyword,
            position: label_box.text_position,
            dy: 0.0,
            anchor: SceneTextAnchor::Middle,
            baseline: SceneTextBaseline::Middle,
        })?;

        let label_box_right = x1 + self.label_box_width;
        let center_x = (x1 + x2) / 2.0;
        self.title_rows(
            projected,
            &frame.label,
            [
                (label_box_right + x2) / 2.0,
                y1 + self.box_margin + self.box_text_margin,
            ],
            self.wrap_width(&frame.id)
                .or(Some((x2 - label_box_right).max(0.0))),
            SequenceDrawnTextNode::Tspan,
        )?;
        for (index, section) in frame.sections.iter().enumerate() {
            checks.checkpoint_loop(index)?;
            self.title_rows(
                projected,
                &section.label,
                [center_x, section.y + self.box_margin + self.box_text_margin],
                self.wrap_width(&section.id),
                SequenceDrawnTextNode::Direct,
            )?;
        }
        projected.group(SceneRole::Cluster, start)
    }

    fn line(
        &mut self,
        projected: &mut Projected<'a>,
        coordinates: [f64; 4],
        dash: [f64; 2],
    ) -> Result<()> {
        include_control_stroke_bounds(&mut self.bounds, coordinates, self.stroke.width, false);
        let [x1, y1, x2, y2] = coordinates;
        projected.line(
            [x1, y1],
            [x2, y2],
            SceneStroke {
                dash: Some(dash),
                ..self.stroke
            },
            [None, None],
        )
    }

    fn wrap_width(&self, id: &str) -> Option<f64> {
        self.widths
            .get(id)
            .map(|width| sequence_block_label_wrap_width(*width, self.wrap_padding))
    }

    fn title_rows(
        &self,
        projected: &mut Projected<'a>,
        raw: &str,
        [x, y0]: [f64; 2],
        max_width: Option<f64>,
        node: SequenceDrawnTextNode,
    ) -> Result<()> {
        let checks = SequenceEmitCheckpoints::for_emit(projected.meter);
        checks.checkpoint()?;
        let Some(label) = display_block_label(raw, node == SequenceDrawnTextNode::Tspan) else {
            return Ok(());
        };
        let rows = wrap_svg_text_lines(&label, self.measurer, self.style, max_width, checks)?;
        let first_y = sequence_drawn_text_first_y(y0, self.box_text_margin);
        let mut preceding_height = 0.0;
        for (index, row) in rows.into_iter().enumerate() {
            checks.checkpoint_loop(index)?;
            projected
                .meter
                .charge_emit_work(1 + row.len().div_ceil(32))?;
            let text = match prepare_sequence_text_line(&row) {
                text if text.is_empty() => Cow::Borrowed("\u{200b}"),
                Cow::Borrowed(_) => Cow::Owned(row),
                Cow::Owned(text) => Cow::Owned(text),
            };
            let label = Label::retain(text, projected.meter)?;
            let y = sequence_drawn_text_y(first_y, self.box_text_margin, preceding_height);
            projected.reserve_item()?;
            if self.box_text_margin > 0.0 {
                preceding_height += measure_sequence_drawn_line_height(
                    self.measurer,
                    &label.text,
                    &self.drawn_style,
                    node,
                    checks.text(),
                )?;
            }
            projected.items.push(Item::Text {
                label,
                role: TextRole::ControlTitle,
                position: [x, y],
                dy: block_row_dy(index, self.box_text_margin, self.style.font_size).unwrap_or(0.0),
                anchor: SceneTextAnchor::Middle,
                baseline: SceneTextBaseline::Alphabetic,
            });
        }
        checks.checkpoint()?;
        Ok(())
    }
}
