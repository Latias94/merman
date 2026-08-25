use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::math_label::{record_sequence_katex_terminal_emission, sequence_katex_label};
use crate::math::PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE;
use crate::sequence::SequenceMathHeightMode;
use merman_core::diagrams::sequence::SequenceActor;

#[derive(Clone, Copy)]
pub(super) struct ActorLabelContext<'a> {
    wrap_width_px: f64,
    measurer: &'a dyn TextMeasurer,
    style: &'a TextStyle,
    typography: &'a crate::sequence::SequenceResolvedTypography,
    typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
    math_sidecar: &'a crate::sequence::SequenceMathSidecar,
    actor_index: Option<usize>,
    checkpoints: SequenceEmitCheckpoints<'a>,
}

impl<'a> ActorLabelContext<'a> {
    pub(super) fn new(
        wrap_width_px: f64,
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        typography: &'a crate::sequence::SequenceResolvedTypography,
        typography_receipt: &'a crate::sequence::SequenceTypographyThemeReceipt,
        math_sidecar: &'a crate::sequence::SequenceMathSidecar,
        checkpoints: SequenceEmitCheckpoints<'a>,
    ) -> Self {
        Self {
            wrap_width_px,
            measurer,
            style,
            typography,
            typography_receipt,
            math_sidecar,
            actor_index: None,
            checkpoints,
        }
    }

    pub(super) fn for_actor(&self, actor_index: usize) -> Self {
        Self {
            actor_index: Some(actor_index),
            ..*self
        }
    }

    fn write_actor(
        &self,
        out: &mut impl SvgOutput,
        cx: f64,
        cy: f64,
        actor: &SequenceActor,
    ) -> Result<()> {
        write_actor_label(out, cx, cy, &actor.description, actor.wrap, self)
    }
}

pub(super) fn is_actor_man_variant(actor_type: &str) -> bool {
    matches!(actor_type, "actor" | "boundary" | "control" | "entity")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActorFillCoverage {
    TypedCss,
    Unhandled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActorStrokeCoverage {
    TypedCss,
    Unhandled,
}

/// Classifies which actor shapes are actually covered by the `.actor` CSS consumer.
///
/// The static Actor theme route may only claim evidence for shapes that receive the generated
/// CSS. Actor-man glyphs have child `circle`/`line` nodes targeted directly by the generated
/// selectors. Control remains conservative because its marker path still inherits the wrapper's
/// inline fill; a participant custom class likewise owns its rectangle fill.
pub(super) fn actor_fill_coverage(actor: &SequenceActor) -> ActorFillCoverage {
    match actor.actor_type.as_str() {
        // These actor-man glyphs use only circles/lines covered by the generated selectors.
        "actor" | "boundary" => ActorFillCoverage::TypedCss,
        "entity" | "collections" | "queue" | "database" => ActorFillCoverage::TypedCss,
        // The control arrow marker path still inherits the wrapper's inline fill. Do not sign the
        // whole Actor fill route until the marker writer participates in the typed receipt.
        "control" => ActorFillCoverage::Unhandled,
        // A participant with a custom class deliberately omits `.actor`; its source-owned fill
        // must not be counted as typed theme consumption. Treat it as uncovered so a strict
        // portable request cannot claim the typed route for a mixed or custom-only diagram.
        _ if actor_custom_class(actor).is_some() => ActorFillCoverage::Unhandled,
        _ => ActorFillCoverage::TypedCss,
    }
}

/// Classifies which actor shapes receive the generated actor stroke without an inline override.
pub(super) fn actor_stroke_coverage(actor: &SequenceActor) -> ActorStrokeCoverage {
    match actor.actor_type.as_str() {
        // Actor-man children, regular rectangles, collections, and queue paths all receive the
        // generated `.actor`/`.actor-man` stroke selectors directly or through an unstyled parent.
        "actor" | "boundary" | "entity" | "collections" | "queue" => ActorStrokeCoverage::TypedCss,
        // Mermaid's control and database glyphs put inline stroke on their wrappers. The control
        // marker path still inherits that stroke even though its circle/line children are covered.
        // Do not claim a typed stroke until those writers have dedicated terminal receipts.
        "control" | "database" => ActorStrokeCoverage::Unhandled,
        _ if actor_custom_class(actor).is_some() => ActorStrokeCoverage::Unhandled,
        _ => ActorStrokeCoverage::TypedCss,
    }
}

pub(super) fn write_actor_man_lifeline(
    out: &mut impl SvgOutput,
    idx: usize,
    cx: f64,
    y1: f64,
    y2: f64,
    actor_id: &str,
) {
    let _ = write!(
        out,
        r##"<g><line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}" data-et="life-line" data-id="{data_id}"/></g>"##,
        idx = idx,
        cx = fmt(cx),
        y1 = fmt(y1),
        y2 = fmt(y2),
        name = escape_xml(actor_id),
        data_id = escape_attr(actor_id),
    );
}

pub(super) fn write_lifeline_root_open(
    out: &mut impl SvgOutput,
    idx: usize,
    cx: f64,
    y1: f64,
    y2: f64,
    actor_id: &str,
    actor_type: &str,
) {
    out.push_str("<g>");
    let root_class = if actor_type == "queue" {
        r#" class="actor actor-top""#
    } else {
        ""
    };
    let _ = write!(
        out,
        r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}" data-et="life-line" data-id="{data_id}"/><g id="root-{idx}"{root_class} data-et="participant" data-type="{actor_type}" data-id="{data_id}">"##,
        idx = idx,
        cx = fmt(cx),
        y1 = fmt(y1),
        y2 = fmt(y2),
        name = escape_xml(actor_id),
        data_id = escape_attr(actor_id),
        root_class = root_class,
        actor_type = escape_attr(actor_type),
    );
}

pub(super) fn write_collection_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor_id: &str,
    actor: &SequenceActor,
    placement_class: &str,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    const OFFSET: f64 = 6.0;
    let (x, y) = node_left_top(n);
    let front_x = x - OFFSET;
    let front_y = y + OFFSET;
    let cx = front_x + (n.width / 2.0);
    let cy = front_y + (n.height / 2.0);
    let _ = write!(
        out,
        r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor {placement_class}"/>"##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height),
        name = escape_xml_display(actor_id),
        placement_class = placement_class,
    );
    let _ = write!(
        out,
        r##"<rect x="{sx}" y="{sy}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor"/>"##,
        sx = fmt(front_x),
        sy = fmt(front_y),
        w = fmt(n.width),
        h = fmt(n.height),
        name = escape_xml_display(actor_id)
    );
    label_ctx.write_actor(out, cx, cy, actor)
}

pub(super) fn write_queue_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor: &SequenceActor,
    _placement_class: &str,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    let (x, y) = node_left_top(n);
    let ry = n.height / 2.0;
    let rx = ry / (2.5 + n.height / 50.0);
    let body_w = n.width - 2.0 * rx;
    let y_mid = y + ry;
    let _ = write!(
        out,
        r##"<g transform="translate({tx1}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h} h {body_w} a {rx},{ry} 0 0 0 0,-{h} Z"/></g>"##,
        tx1 = fmt(rx),
        ty = fmt(-n.height / 2.0),
        x = fmt(x),
        y_mid = fmt(y_mid),
        rx = fmt(rx),
        ry = fmt(ry),
        h = fmt(n.height),
        body_w = fmt(body_w),
    );
    let _ = write!(
        out,
        r##"<g transform="translate({tx2}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}"/></g>"##,
        tx2 = fmt(n.width - rx),
        ty = fmt(-n.height / 2.0),
        x = fmt(x),
        y_mid = fmt(y_mid),
        rx = fmt(rx),
        ry = fmt(ry),
        h = fmt(n.height),
    );
    label_ctx.write_actor(out, n.x, y_mid, actor)
}

pub(super) fn write_database_top_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor: &SequenceActor,
    actor_height: f64,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    let (x, y) = node_left_top(n);
    let w = n.width / 3.0;
    let h = n.width / 3.0;
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);
    let tx = w;
    let ty = ry;
    let y_text = y + 35.0 + (actor_height / 2.0);
    let _ = write!(
        out,
        r##"<g class="actor actor-top" transform="translate({tx}, {ty})" style="stroke: rgb(147, 112, 219);"><path d="M {x},{y1p} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}"/></g>"##,
        tx = fmt(tx),
        ty = fmt(ty),
        x = fmt(x),
        y1p = fmt(y + ry),
        rx = fmt(rx),
        ry = fmt(ry),
        w = fmt(w),
        h2 = fmt(h - 2.0 * ry),
    );
    label_ctx.write_actor(out, n.x, y_text, actor)
}

pub(super) fn write_database_bottom_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor: &SequenceActor,
    label_box_height: f64,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    // Mermaid's database actor uses a cylinder glyph and updates the actor height after
    // the top render; the footer render uses that updated height (≈ width/3 + labelBoxHeight).
    let (x, y) = node_left_top(n);
    let w = n.width / 3.0;
    let h = n.width / 3.0;
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);
    let footer_h = h + label_box_height;
    let tx = w;
    let ty = ry;
    let y_text = y + 35.0 + (footer_h / 2.0);
    let _ = write!(
        out,
        r##"<g class="actor actor-bottom" transform="translate({tx}, {ty})" style="stroke: rgb(147, 112, 219);"><path d="M {x},{y1} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}"/></g>"##,
        tx = fmt(tx),
        ty = fmt(ty),
        x = fmt(x),
        y1 = fmt(y + ry),
        rx = fmt(rx),
        ry = fmt(ry),
        w = fmt(w),
        h2 = fmt(h - 2.0 * ry)
    );
    label_ctx.write_actor(out, n.x, y_text, actor)
}

pub(super) fn write_rect_actor_shape(
    out: &mut impl SvgOutput,
    n: &LayoutNode,
    actor_id: &str,
    actor: &SequenceActor,
    placement_class: &str,
    label_ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    let (x, y) = node_left_top(n);
    let custom_class = actor_custom_class(actor);
    let fill = if custom_class.is_some() {
        "#EDF2AE"
    } else {
        "#eaeaea"
    };
    let class = custom_class
        .map(|c| format!("{c} {placement_class}"))
        .unwrap_or_else(|| format!("actor {placement_class}"));
    let _ = write!(
        out,
        r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="{class}"/>"##,
        x = fmt(x),
        y = fmt(y),
        w = fmt(n.width),
        h = fmt(n.height),
        name = escape_xml(actor_id),
        fill = escape_xml_display(fill),
        class = escape_attr(&class),
    );
    label_ctx.write_actor(out, n.x, n.y, actor)
}

fn actor_custom_class(actor: &SequenceActor) -> Option<&str> {
    actor
        .properties
        .get("class")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn write_actor_label(
    out: &mut impl SvgOutput,
    cx: f64,
    cy: f64,
    label: &str,
    wrap: bool,
    ctx: &ActorLabelContext<'_>,
) -> Result<()> {
    ctx.checkpoints.checkpoint()?;
    let wrapped_label = if wrap {
        Some(
            crate::sequence::wrap_sequence_label_like_mermaid_lines(
                label,
                ctx.measurer,
                ctx.style,
                ctx.wrap_width_px,
                ctx.checkpoints.text(),
            )?
            .join("<br>"),
        )
    } else {
        None
    };
    let rendered_label = wrapped_label.as_deref().unwrap_or(label);

    let prepared_math = ctx.actor_index.and_then(|actor_index| {
        ctx.math_sidecar.terminal(
            &crate::sequence::SequenceMathOccurrence::Actor(actor_index),
            rendered_label,
        )
    });
    if let Some(katex) =
        sequence_katex_label(prepared_math, ctx.style, SequenceMathHeightMode::Actor)
    {
        let x = cx - katex.width / 2.0;
        let y = cy - katex.height / 2.0;
        out.push_str("<switch ");
        out.push_str(PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE);
        out.push('>');
        let _ = write!(
            out,
            r#"<foreignObject x="{x}" y="{y}" width="{w}" height="{h}"><div class="actor actor-box" xmlns="http://www.w3.org/1999/xhtml" style="height: 100%; width: 100%;"><div style="text-align: center; vertical-align: middle;">{html}</div></div></foreignObject>"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(katex.width),
            h = fmt(katex.height),
            html = katex.html,
        );
        let raw_lines = crate::text::split_html_br_lines(rendered_label);
        let line_count = raw_lines.len();
        write_actor_label_lines(out, cx, cy, raw_lines, line_count, ctx, false)?;
        out.push_str("</switch>");
        record_sequence_katex_terminal_emission(
            ctx.typography_receipt,
            crate::sequence::SequenceTextSurface::ParticipantLabel,
            &katex,
        );
        return ctx.checkpoints.checkpoint();
    }

    // Split/wrap before decoding Mermaid entities so escaped `<br>` (`#lt;br#gt;`) remains
    // literal text rather than being treated as an actual `<br>` break.
    if let Some(wrapped_label) = wrapped_label {
        let raw_lines = crate::text::split_html_br_lines(&wrapped_label);
        write_actor_label_lines(
            out,
            cx,
            cy,
            raw_lines.iter().copied(),
            raw_lines.len(),
            ctx,
            true,
        )?;
    } else {
        let raw_lines = crate::text::split_html_br_lines(label);
        let line_count = raw_lines.len();
        write_actor_label_lines(out, cx, cy, raw_lines, line_count, ctx, true)?;
    }
    ctx.checkpoints.checkpoint()
}

fn write_actor_label_lines<'a>(
    out: &mut impl SvgOutput,
    cx: f64,
    cy: f64,
    raw_lines: impl IntoIterator<Item = &'a str>,
    line_count: usize,
    ctx: &ActorLabelContext<'_>,
    record_receipt: bool,
) -> Result<()> {
    let n = line_count.max(1) as f64;
    for (i, raw) in raw_lines.into_iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(i)?;
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw);
        let dy = if n <= 1.0 {
            0.0
        } else {
            (i as f64 - (n - 1.0) / 2.0) * ctx.style.font_size
        };
        let legacy_style = format!(
            "text-anchor: middle; font-size: {}px; font-weight: 400;",
            fmt(ctx.style.font_size)
        );
        let inline_style = ctx
            .typography
            .terminal_style("text-anchor: middle", legacy_style);
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor actor-box" style="{style}"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(cy),
            style = escape_attr_display(&inline_style),
            dy = fmt(dy),
            text = escape_xml_display(decoded.as_ref())
        );
        if record_receipt {
            ctx.typography_receipt
                .record_terminal_text(crate::sequence::SequenceTextSurface::ParticipantLabel);
        }
    }
    ctx.checkpoints.checkpoint()
}
