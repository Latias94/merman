use super::super::*;
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use rustc_hash::FxHashMap;

pub(super) fn write_actor_label(
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
    let raw_lines: Vec<String> = if wrap {
        crate::text::wrap_label_like_mermaid_lines(label, measurer, style, wrap_width_px)
    } else {
        crate::text::split_html_br_lines(label)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    };
    let n = raw_lines.len().max(1) as f64;
    for (i, raw) in raw_lines.into_iter().enumerate() {
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(&raw);
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

pub(super) fn render_sequence_bottom_actors(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_wrap_width: f64,
    label_box_height: f64,
    measurer: &dyn TextMeasurer,
    loop_text_style: &TextStyle,
) {
    // Mermaid draws bottom actors first (reverse DOM order).
    for actor_id in model.actor_order.iter().rev() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let node_id = format!("actor-bottom-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (x, y) = node_left_top(n);
        let actor_custom_class = actor
            .properties
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let actor_rect_fill = if actor_custom_class.is_some() {
            "#EDF2AE"
        } else {
            "#eaeaea"
        };
        let actor_bottom_class = actor_custom_class
            .map(|c| format!("{c} actor-bottom"))
            .unwrap_or_else(|| "actor actor-bottom".to_string());
        match actor_type {
            // Actor-man variants are drawn later (after `<defs>`), but Mermaid keeps stable
            // indices by emitting empty `<g/>` placeholders here.
            "actor" | "boundary" | "control" | "entity" => {
                out.push_str("<g/>");
            }
            "collections" => {
                const OFFSET: f64 = 6.0;
                let front_x = x - OFFSET;
                let front_y = y + OFFSET;
                let cx = front_x + (n.width / 2.0);
                let cy = front_y + (n.height / 2.0);
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor actor-bottom"/>"##,
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(n.width),
                    h = fmt(n.height),
                    name = escape_xml_display(actor_id)
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
                write_actor_label(
                    out,
                    cx,
                    cy,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g>");
            }
            "queue" => {
                let ry = n.height / 2.0;
                let rx = ry / (2.5 + n.height / 50.0);
                let body_w = n.width - 2.0 * rx;
                let y_mid = y + ry;
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<g transform="translate({tx1}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h} h {body_w} a {rx},{ry} 0 0 0 0,-{h} Z" class="actor actor-bottom"/></g>"##,
                    tx1 = fmt(rx),
                    ty = fmt(-n.height / 2.0),
                    x = fmt(x),
                    y_mid = fmt(y_mid),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(n.height),
                    body_w = fmt(body_w)
                );
                let _ = write!(
                    out,
                    r##"<g transform="translate({tx2}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}" stroke="#666" stroke-width="1px" class="actor actor-bottom"/></g>"##,
                    tx2 = fmt(n.width - rx),
                    ty = fmt(-n.height / 2.0),
                    x = fmt(x),
                    y_mid = fmt(y_mid),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(n.height)
                );
                write_actor_label(
                    out,
                    n.x,
                    y_mid,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g>");
            }
            "database" => {
                // Mermaid's database actor uses a cylinder glyph and updates the actor height after
                // the top render; the footer render uses that updated height (≈ width/4 + labelBoxHeight).
                let w = n.width / 4.0;
                let h = n.width / 4.0;
                let rx = w / 2.0;
                let ry = rx / (2.5 + w / 50.0);
                let footer_h = h + label_box_height;
                let tx = w * 1.5;
                let ty = (footer_h / 4.0) - 2.0 * ry;
                let y_text = y + ((footer_h + h) / 4.0) + (footer_h / 2.0);
                out.push_str("<g>");
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
                write_actor_label(
                    out,
                    n.x,
                    y_text,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g>");
            }
            _ => {
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="{class}"/>"##,
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(n.width),
                    h = fmt(n.height),
                    name = escape_xml(actor_id),
                    fill = escape_xml_display(actor_rect_fill),
                    class = escape_attr(&actor_bottom_class),
                );
                write_actor_label(
                    out,
                    n.x,
                    n.y,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g>");
            }
        }
    }
}

pub(super) fn render_sequence_top_actors_and_lifelines(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    actor_wrap_width: f64,
    actor_height: f64,
    measurer: &dyn TextMeasurer,
    loop_text_style: &TextStyle,
) {
    for (idx, actor_id) in model.actor_order.iter().enumerate().rev() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        let node_top_id = format!("actor-top-{actor_id}");
        let node_bottom_id = format!("actor-bottom-{actor_id}");
        let Some(top) = nodes_by_id.get(node_top_id.as_str()).copied() else {
            continue;
        };
        let Some(bottom) = nodes_by_id.get(node_bottom_id.as_str()).copied() else {
            continue;
        };
        let (top_x, top_y) = node_left_top(top);
        let (_bottom_x, bottom_y) = node_left_top(bottom);

        let (y1, y2) = edges_by_id
            .get(format!("lifeline-{actor_id}").as_str())
            .and_then(|e| Some((e.points.first()?.y, e.points.get(1)?.y)))
            .unwrap_or((top_y + top.height, bottom_y));
        let actor_custom_class = actor
            .properties
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let actor_rect_fill = if actor_custom_class.is_some() {
            "#EDF2AE"
        } else {
            "#eaeaea"
        };
        let actor_top_class = actor_custom_class
            .map(|c| format!("{c} actor-top"))
            .unwrap_or_else(|| "actor actor-top".to_string());

        match actor_type {
            "actor" | "boundary" | "control" | "entity" => {
                let _ = write!(
                    out,
                    r##"<g><line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/></g>"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id)
                );
            }
            "collections" => {
                const OFFSET: f64 = 6.0;
                let front_x = top_x - OFFSET;
                let front_y = top_y + OFFSET;
                let cx = front_x + (top.width / 2.0);
                let cy = front_y + (top.height / 2.0);
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    out,
                    r##"<rect x="{x}" y="{y}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor actor-top"/>"##,
                    x = fmt(top_x),
                    y = fmt(top_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    out,
                    r##"<rect x="{sx}" y="{sy}" fill="#eaeaea" stroke="#666" width="{w}" height="{h}" name="{name}" class="actor"/>"##,
                    sx = fmt(front_x),
                    sy = fmt(front_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                );
                write_actor_label(
                    out,
                    cx,
                    cy,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g></g>");
            }
            "queue" => {
                let ry = top.height / 2.0;
                let rx = ry / (2.5 + top.height / 50.0);
                let body_w = top.width - 2.0 * rx;
                let y_mid = top_y + ry;
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    out,
                    r##"<g transform="translate({tx1}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h} h {body_w} a {rx},{ry} 0 0 0 0,-{h} Z" class="actor actor-top"/></g>"##,
                    tx1 = fmt(rx),
                    ty = fmt(-top.height / 2.0),
                    x = fmt(top_x),
                    y_mid = fmt(y_mid),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(top.height),
                    body_w = fmt(body_w),
                );
                let _ = write!(
                    out,
                    r##"<g transform="translate({tx2}, {ty})"><path d="M {x},{y_mid} a {rx},{ry} 0 0 0 0,{h}" stroke="#666" stroke-width="1px" class="actor actor-top"/></g>"##,
                    tx2 = fmt(top.width - rx),
                    ty = fmt(-top.height / 2.0),
                    x = fmt(top_x),
                    y_mid = fmt(y_mid),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(top.height),
                );
                write_actor_label(
                    out,
                    top.x,
                    y_mid,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g></g>");
            }
            "database" => {
                let w = top.width / 4.0;
                let h = top.width / 4.0;
                let rx = w / 2.0;
                let ry = rx / (2.5 + w / 50.0);
                let tx = w * 1.5;
                let ty = (actor_height + ry) / 4.0;
                let y_text = top_y + actor_height + (ry / 2.0);
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    out,
                    r##"<g transform="translate({tx}, {ty})"><path d="M {x},{y1p} a {rx},{ry} 0 0 0 {w},0 a {rx},{ry} 0 0 0 -{w},0 l 0,{h2} a {rx},{ry} 0 0 0 {w},0 l 0,-{h2}" fill="#eaeaea" stroke="#000" stroke-width="1" class="actor actor-top"/></g>"##,
                    tx = fmt(tx),
                    ty = fmt(ty),
                    x = fmt(top_x),
                    y1p = fmt(top_y + ry),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    w = fmt(w),
                    h2 = fmt(h - 2.0 * ry),
                );
                write_actor_label(
                    out,
                    top.x,
                    y_text,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g></g>");
            }
            _ => {
                out.push_str("<g>");
                let _ = write!(
                    out,
                    r##"<line id="actor{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}" class="actor-line 200" stroke-width="0.5px" stroke="#999" name="{name}"/><g id="root-{idx}">"##,
                    idx = idx,
                    cx = fmt(top.x),
                    y1 = fmt(y1),
                    y2 = fmt(y2),
                    name = escape_xml(actor_id),
                );
                let _ = write!(
                    out,
                    r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" name="{name}" rx="3" ry="3" class="{class}"/>"##,
                    x = fmt(top_x),
                    y = fmt(top_y),
                    w = fmt(top.width),
                    h = fmt(top.height),
                    name = escape_xml(actor_id),
                    fill = escape_xml_display(actor_rect_fill),
                    class = escape_attr(&actor_top_class),
                );
                write_actor_label(
                    out,
                    top.x,
                    top.y,
                    &actor.description,
                    actor.wrap,
                    actor_wrap_width,
                    measurer,
                    loop_text_style,
                );
                out.push_str("</g></g>");
            }
        }
    }
}
