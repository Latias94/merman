use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use merman_model::families::sequence::{SequenceControlKind, SequenceControlRole};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy)]
pub(super) struct SequenceFrameRenderOptions<'a> {
    pub(super) actor_text_style: &'a TextStyle,
    pub(super) boxes: &'a [crate::model::SequenceParticipantBoxLayout],
    pub(super) box_indices: &'a [Option<usize>],
    pub(super) rect_default_fill: &'a str,
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

pub(super) fn render_sequence_box_frames_and_rect_blocks(
    out: &mut impl SvgOutput,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    options: SequenceFrameRenderOptions<'_>,
    label_ctx: &super::actor_shapes::ActorLabelContext<'_>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
    let actor_typography = label_ctx.typography;
    let typography_receipt = label_ctx.typography_receipt;
    // Mermaid renders "box" frames as root-level `<g><rect class="rect"/>...</g>` nodes before actors.
    // Mermaid renders boxes "behind" other elements; multiple boxes end up reversed in DOM order.
    checkpoints.checkpoint()?;
    for (emission_index, (b, box_index)) in model
        .boxes
        .iter()
        .zip(options.box_indices)
        .rev()
        .enumerate()
    {
        checkpoints.checkpoint_loop(emission_index)?;
        let Some(box_index) = *box_index else {
            if b.name.is_some() {
                typography_receipt
                    .record_missing_text_effect(crate::sequence::SequenceTextSurface::BoxTitle);
            }
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
                let application = label_ctx.write_shadow(
                    out,
                    name,
                    cx,
                    text_y + dy,
                    options.actor_text_style.font_size,
                    super::text_effect::TextShadowBaseline::Middle,
                )?;
                let filter = application
                    .as_ref()
                    .map(|a| format!(" filter=\"{}\"", escape_attr(&a.filter)))
                    .unwrap_or_default();
                let style = actor_typography.terminal_style(
                    "text-anchor: middle",
                    format!(
                        "text-anchor: middle; {}",
                        super::settings::sequence_text_style_attribute(options.actor_text_style)
                    ),
                );
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="text" style="{style}"{filter}><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                    x = fmt(cx),
                    y = fmt(text_y),
                    dy = fmt(dy),
                    style = escape_attr_display(&style),
                    text = super::escape_prepared_text(name)
                );
                out.checkpoint()?;
                label_ctx.record_shadow(
                    application.as_ref(),
                    name,
                    crate::sequence::SequenceTextSurface::BoxTitle,
                    0.0,
                );
                typography_receipt
                    .record_terminal_text(crate::sequence::SequenceTextSurface::BoxTitle);
            }
        }
        out.push_str("</g>");
    }

    // Mermaid renders `rect` blocks as root-level `<rect class="rect"/>` nodes before actors.
    {
        #[derive(Debug, Clone, Copy)]
        struct RectBlock<'a> {
            fill: &'a str,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
        }

        fn contains(a: &RectBlock<'_>, b: &RectBlock<'_>) -> bool {
            const EPS: f64 = 1e-9;
            a.x <= b.x + EPS
                && a.y <= b.y + EPS
                && (a.x + a.w) >= (b.x + b.w) - EPS
                && (a.y + a.h) >= (b.y + b.h) - EPS
        }

        let mut rects: Vec<RectBlock<'_>> = Vec::with_capacity(model.messages.len());
        for (message_index, msg) in model.messages.iter().enumerate() {
            checkpoints.checkpoint_loop(message_index)?;
            if !msg.control_semantics().is_some_and(|semantics| {
                semantics.kind == SequenceControlKind::Rect
                    && semantics.role == SequenceControlRole::Start
            }) {
                continue;
            }
            let explicit_fill = msg.message_text();
            let fill = if explicit_fill.is_empty() {
                options.rect_default_fill
            } else {
                explicit_fill
            };
            let node_id = format!("rect-{}", msg.id);
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            let (x, y) = node_left_top(n);
            rects.push(RectBlock {
                fill,
                x,
                y,
                w: n.width,
                h: n.height,
            });
        }

        // Mermaid's emitted order for nested `rect` blocks is not strictly tied to parse order.
        // Match its DOM ordering semantics by keeping parents before contained children and
        // sorting unrelated rectangles by vertical position (lower blocks first).
        checkpoints.checkpoint()?;
        rects.sort_by(|a, b| {
            if contains(a, b) && !contains(b, a) {
                return std::cmp::Ordering::Less;
            }
            if contains(b, a) && !contains(a, b) {
                return std::cmp::Ordering::Greater;
            }
            b.y.partial_cmp(&a.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });

        checkpoints.checkpoint()?;
        for (rect_index, r) in rects.into_iter().enumerate() {
            checkpoints.checkpoint_loop(rect_index)?;
            let _ = write!(
                out,
                r#"<rect x="{x}" y="{y}" fill="{fill}" width="{w}" height="{h}" class="rect"/>"#,
                x = fmt(r.x),
                y = fmt(r.y),
                w = fmt(r.w),
                h = fmt(r.h),
                fill = escape_xml_display(r.fill)
            );
        }
    }
    checkpoints.checkpoint()
}
