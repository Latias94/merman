use super::super::*;
use super::geometry::node_left_top;
use merman_core::diagrams::sequence::SequenceActor;

pub(super) struct ActorLabelContext<'a> {
    wrap_width_px: f64,
    measurer: &'a dyn TextMeasurer,
    style: &'a TextStyle,
}

impl<'a> ActorLabelContext<'a> {
    pub(super) fn new(
        wrap_width_px: f64,
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
    ) -> Self {
        Self {
            wrap_width_px,
            measurer,
            style,
        }
    }

    fn write_actor(&self, out: &mut String, cx: f64, cy: f64, actor: &SequenceActor) {
        write_actor_label(
            out,
            cx,
            cy,
            &actor.description,
            actor.wrap,
            self.wrap_width_px,
            self.measurer,
            self.style,
        );
    }
}

pub(super) fn is_actor_man_variant(actor_type: &str) -> bool {
    matches!(actor_type, "actor" | "boundary" | "control" | "entity")
}

pub(super) fn write_actor_man_lifeline(
    out: &mut String,
    idx: usize,
    cx: f64,
    y1: f64,
    y2: f64,
    actor_id: &str,
) {
    let _ = write!(
        out,
        r##"<g><line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/></g>"##,
        idx = idx,
        cx = fmt(cx),
        y1 = fmt(y1),
        y2 = fmt(y2),
        name = escape_xml(actor_id)
    );
}

pub(super) fn write_lifeline_root_open(
    out: &mut String,
    idx: usize,
    cx: f64,
    y1: f64,
    y2: f64,
    actor_id: &str,
) {
    out.push_str("<g>");
    let _ = write!(
        out,
        r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
        idx = idx,
        cx = fmt(cx),
        y1 = fmt(y1),
        y2 = fmt(y2),
        name = escape_xml(actor_id),
    );
}

pub(super) fn write_collection_actor_shape(
    out: &mut String,
    n: &LayoutNode,
    actor_id: &str,
    actor: &SequenceActor,
    placement_class: &str,
    label_ctx: &ActorLabelContext<'_>,
) {
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
    label_ctx.write_actor(out, cx, cy, actor);
}

pub(super) fn write_queue_actor_shape(
    out: &mut String,
    n: &LayoutNode,
    actor: &SequenceActor,
    placement_class: &str,
    label_ctx: &ActorLabelContext<'_>,
) {
    let (x, y) = node_left_top(n);
    let ry = n.height / 2.0;
    let rx = ry / (2.5 + n.height / 50.0);
    let body_w = n.width - 2.0 * rx;
    let y_mid = y + ry;
    let _ = write!(
        out,
        r##"<g transform="translate({tx1}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h} h {body_w} a {rx},{ry} 0 0 0 0,-{h} Z" class="actor {placement_class}"/></g>"##,
        tx1 = fmt(rx),
        ty = fmt(-n.height / 2.0),
        x = fmt(x),
        y_mid = fmt(y_mid),
        rx = fmt(rx),
        ry = fmt(ry),
        h = fmt(n.height),
        body_w = fmt(body_w),
        placement_class = placement_class,
    );
    let _ = write!(
        out,
        r##"<g transform="translate({tx2}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}" stroke="#666" stroke-width="1px" class="actor {placement_class}"/></g>"##,
        tx2 = fmt(n.width - rx),
        ty = fmt(-n.height / 2.0),
        x = fmt(x),
        y_mid = fmt(y_mid),
        rx = fmt(rx),
        ry = fmt(ry),
        h = fmt(n.height),
        placement_class = placement_class,
    );
    label_ctx.write_actor(out, n.x, y_mid, actor);
}

pub(super) fn write_database_top_actor_shape(
    out: &mut String,
    n: &LayoutNode,
    actor: &SequenceActor,
    actor_height: f64,
    label_ctx: &ActorLabelContext<'_>,
) {
    let (x, y) = node_left_top(n);
    let w = n.width / 4.0;
    let h = n.width / 4.0;
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);
    let tx = w * 1.5;
    let ty = (actor_height + ry) / 4.0;
    let y_text = y + actor_height + (ry / 2.0);
    let _ = write!(
        out,
        r##"<g transform="translate({tx}, {ty})"><path d="M {x},{y1p} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}" fill="#eaeaea" stroke="#000" stroke-width="1" class="actor actor-top"/></g>"##,
        tx = fmt(tx),
        ty = fmt(ty),
        x = fmt(x),
        y1p = fmt(y + ry),
        rx = fmt(rx),
        ry = fmt(ry),
        w = fmt(w),
        h2 = fmt(h - 2.0 * ry),
    );
    label_ctx.write_actor(out, n.x, y_text, actor);
}

pub(super) fn write_database_bottom_actor_shape(
    out: &mut String,
    n: &LayoutNode,
    actor: &SequenceActor,
    label_box_height: f64,
    label_ctx: &ActorLabelContext<'_>,
) {
    // Mermaid's database actor uses a cylinder glyph and updates the actor height after
    // the top render; the footer render uses that updated height (≈ width/4 + labelBoxHeight).
    let (x, y) = node_left_top(n);
    let w = n.width / 4.0;
    let h = n.width / 4.0;
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);
    let footer_h = h + label_box_height;
    let tx = w * 1.5;
    let ty = (footer_h / 4.0) - 2.0 * ry;
    let y_text = y + ((footer_h + h) / 4.0) + (footer_h / 2.0);
    let _ = write!(
        out,
        r##"<g transform="translate({tx}, {ty})"><path d="M {x},{y1} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}" fill="#eaeaea" stroke="#000" stroke-width="1" class="actor actor-bottom"/></g>"##,
        tx = fmt(tx),
        ty = fmt(ty),
        x = fmt(x),
        y1 = fmt(y + ry),
        rx = fmt(rx),
        ry = fmt(ry),
        w = fmt(w),
        h2 = fmt(h - 2.0 * ry)
    );
    label_ctx.write_actor(out, n.x, y_text, actor);
}

pub(super) fn write_rect_actor_shape(
    out: &mut String,
    n: &LayoutNode,
    actor_id: &str,
    actor: &SequenceActor,
    placement_class: &str,
    label_ctx: &ActorLabelContext<'_>,
) {
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
    label_ctx.write_actor(out, n.x, n.y, actor);
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
    out: &mut String,
    cx: f64,
    cy: f64,
    label: &str,
    wrap: bool,
    wrap_width_px: f64,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) {
    // Split/wrap before decoding Mermaid entities so escaped `<br>` (`#lt;br#gt;`) remains
    // literal text rather than being treated as an actual `<br>` break.
    if wrap {
        let raw_lines =
            crate::text::wrap_label_like_mermaid_lines(label, measurer, style, wrap_width_px);
        write_actor_label_lines(
            out,
            cx,
            cy,
            raw_lines.iter().map(String::as_str),
            raw_lines.len(),
            style,
        );
    } else {
        let raw_lines = crate::text::split_html_br_lines(label);
        let line_count = raw_lines.len();
        write_actor_label_lines(out, cx, cy, raw_lines, line_count, style);
    }
}

fn write_actor_label_lines<'a>(
    out: &mut String,
    cx: f64,
    cy: f64,
    raw_lines: impl IntoIterator<Item = &'a str>,
    line_count: usize,
    style: &TextStyle,
) {
    let n = line_count.max(1) as f64;
    for (i, raw) in raw_lines.into_iter().enumerate() {
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(raw);
        let dy = if n <= 1.0 {
            0.0
        } else {
            (i as f64 - (n - 1.0) / 2.0) * style.font_size
        };
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor actor-box" style="text-anchor: middle; font-size: {fs}px; font-weight: 400;"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
            x = fmt(cx),
            y = fmt(cy),
            fs = fmt(style.font_size),
            dy = fmt(dy),
            text = escape_xml_display(decoded.as_ref())
        );
    }
}
