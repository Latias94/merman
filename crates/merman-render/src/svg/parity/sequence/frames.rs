use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use crate::model::SequenceFrameLayout;
use crate::sequence::SequenceNativeStyle;
use merman_model::families::sequence::{SequenceControlKind, SequenceControlRole, SequenceMessage};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy)]
pub(super) struct SequenceFrameRenderOptions<'a> {
    pub(super) actor_text_style: &'a TextStyle,
    pub(super) boxes: &'a [crate::model::SequenceParticipantBoxLayout],
    pub(super) box_indices: &'a [Option<usize>],
    pub(super) rect_default_fill: &'a str,
    pub(super) native_frames: Option<(&'a SequenceNativeStyle, &'a [SequenceFrameLayout])>,
}

pub(super) fn builtin_box_paint_bounds(
    boxes: &[crate::model::SequenceParticipantBoxLayout],
    theme: &crate::sequence::SequenceCompatBinding,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<Option<Bounds>> {
    if !theme.is_neo || theme.drop_shadow.as_str() != "url(#drop-shadow)" {
        return Ok(None);
    }
    let mut bounds: Option<Bounds> = None;
    for (index, box_layout) in boxes.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        let [x, y, width, height] = [
            box_layout.x,
            box_layout.y,
            box_layout.width,
            box_layout.height,
        ];
        // Box frames use SVG's default one-pixel stroke and the emitted zero-blur Neo filter.
        let paint =
            super::actor_effect::builtin_neo_rect_paint_bounds([x, y, width, height], 1.0, true);
        if let Some(bounds) = &mut bounds {
            bounds.min_x = bounds.min_x.min(paint.min_x);
            bounds.min_y = bounds.min_y.min(paint.min_y);
            bounds.max_x = bounds.max_x.max(paint.max_x);
            bounds.max_y = bounds.max_y.max(paint.max_y);
        } else {
            bounds = Some(paint);
        }
    }
    Ok(bounds)
}

pub(super) fn render_sequence_backgrounds(
    out: &mut impl SvgOutput,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    options: SequenceFrameRenderOptions<'_>,
    label_ctx: &super::actor_shapes::ActorLabelContext<'_>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
    let actor_typography = label_ctx.typography;
    // Mermaid renders "box" frames as root-level `<g><rect class="rect"/>...</g>` nodes before actors.
    // Mermaid renders boxes "behind" other elements; multiple boxes end up reversed in DOM order.
    checkpoints.checkpoint()?;
    for (emission_index, (_, box_index)) in model
        .boxes
        .iter()
        .zip(options.box_indices)
        .rev()
        .enumerate()
    {
        checkpoints.checkpoint_loop(emission_index)?;
        let Some(box_index) = *box_index else {
            continue;
        };
        let box_layout = &options.boxes[box_index];
        out.push_str("<g>");
        let _ = write!(
            out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" stroke="rgb(0,0,0, 0.5)" width="{w}" height="{h}" class="rect"/>"#,
            x = fmt(box_layout.x),
            y = fmt(box_layout.y),
            w = fmt(box_layout.width),
            h = fmt(box_layout.height),
            fill = escape_xml_display(&box_layout.fill),
        );
        if let Some(title) = box_layout.title.as_ref() {
            let cx = title.x;
            let text_y = title.y;
            let lines = crate::text::split_mermaid_common_br_lines(&title.text);
            let count = lines.len().max(1) as f64;
            for (index, raw) in lines.into_iter().enumerate() {
                checkpoints.checkpoint_loop(index)?;
                let decoded = crate::sequence::prepare_sequence_text_line(raw);
                let name = decoded.as_ref();
                let dy = (index as f64 - (count - 1.0) / 2.0) * options.actor_text_style.font_size;
                let style = actor_typography.terminal_style(
                    "text-anchor: middle",
                    format!(
                        "text-anchor: middle; {}",
                        super::settings::sequence_text_style_attribute(options.actor_text_style)
                    ),
                );
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="text" style="{style}"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                    x = fmt(cx),
                    y = fmt(text_y),
                    dy = fmt(dy),
                    style = escape_attr_display(&style),
                    text = super::escape_prepared_text(name)
                );
                out.checkpoint()?;
            }
        }
        out.push_str("</g>");
    }

    if let Some((native, frames)) = options.native_frames {
        // Frames are ordered by their opening message. Merge them with rect starts so each
        // background is painted before its descendants, beneath all control lines and labels.
        let mut frames = frames.iter().peekable();
        for (index, message) in model.messages.iter().enumerate() {
            checkpoints.checkpoint_loop(index)?;
            if let Some(frame) = frames.next_if(|frame| frame.id == message.id) {
                let _ = write!(
                    out,
                    r#"<rect class="merman-sequence-frame-surface" x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{}"/>"#,
                    fmt(frame.x),
                    fmt(frame.y),
                    fmt(frame.width),
                    fmt(frame.height),
                    fmt(native.form.container_radius),
                    native.container,
                );
            } else if let Some(rect) =
                RectBlock::for_message(message, nodes_by_id, options.rect_default_fill)
            {
                rect.write(out);
            }
        }
        return checkpoints.checkpoint();
    }

    let mut rects = Vec::with_capacity(model.messages.len());
    for (index, message) in model.messages.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        if let Some(rect) = RectBlock::for_message(message, nodes_by_id, options.rect_default_fill)
        {
            rects.push(rect);
        }
    }

    // Mermaid keeps contained rects after their parents and draws unrelated rects lower first.
    checkpoints.checkpoint()?;
    rects.sort_by(|a, b| {
        if a.contains(b) && !b.contains(a) {
            return std::cmp::Ordering::Less;
        }
        if b.contains(a) && !a.contains(b) {
            return std::cmp::Ordering::Greater;
        }
        b.y.partial_cmp(&a.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
    });

    checkpoints.checkpoint()?;
    for (index, rect) in rects.into_iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;
        rect.write(out);
    }
    checkpoints.checkpoint()
}

struct RectBlock<'a> {
    fill: &'a str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl<'a> RectBlock<'a> {
    fn for_message(
        message: &'a SequenceMessage,
        nodes_by_id: &FxHashMap<&str, &LayoutNode>,
        default_fill: &'a str,
    ) -> Option<Self> {
        let control = message.control_semantics()?;
        if control.kind != SequenceControlKind::Rect || control.role != SequenceControlRole::Start {
            return None;
        }
        let node_id = format!("rect-{}", message.id);
        let node = nodes_by_id.get(node_id.as_str()).copied()?;
        let (x, y) = node_left_top(node);
        let explicit_fill = message.message_text();
        let fill = if explicit_fill.is_empty() {
            default_fill
        } else {
            explicit_fill
        };
        Some(Self {
            fill,
            x,
            y,
            width: node.width,
            height: node.height,
        })
    }

    fn contains(&self, other: &Self) -> bool {
        const EPS: f64 = 1e-9;
        self.x <= other.x + EPS
            && self.y <= other.y + EPS
            && (self.x + self.width) >= (other.x + other.width) - EPS
            && (self.y + self.height) >= (other.y + other.height) - EPS
    }

    fn write(self, out: &mut impl SvgOutput) {
        let _ = write!(
            out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" width="{w}" height="{h}" class="rect"/>"#,
            x = fmt(self.x),
            y = fmt(self.y),
            w = fmt(self.width),
            h = fmt(self.height),
            fill = escape_xml_display(self.fill)
        );
    }
}
