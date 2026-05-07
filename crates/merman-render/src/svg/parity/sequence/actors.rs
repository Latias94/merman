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

pub(super) fn render_sequence_actor_popup_menus(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    sanitize_config: &merman_core::MermaidConfig,
    force_menus: bool,
    mirror_actors: bool,
    actor_height: f64,
) {
    // Mermaid emits actor popup menus (links/link directives) as root-level
    // `<g class="actorPopupMenu">` groups after messages.
    for (actor_cnt, actor_id) in model.actor_order.iter().enumerate() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        if actor.links.is_empty() {
            continue;
        }
        let actor_custom_class = actor
            .properties
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        let popup_display = if force_menus {
            "block !important"
        } else {
            "none"
        };
        let popup_fill = if actor_custom_class.is_some() {
            "#EDF2AE"
        } else {
            "#eaeaea"
        };
        let popup_actor_pos_class = if mirror_actors {
            "actor-bottom"
        } else {
            "actor-top"
        };
        let popup_panel_class = actor_custom_class
            .map(|c| format!("actorPopupMenuPanel {c} {popup_actor_pos_class}"))
            .unwrap_or_else(|| format!("actorPopupMenuPanel actor {popup_actor_pos_class}"));

        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (x, _y) = node_left_top(n);

        let mut link_y: f64 = 20.0;
        let panel_height = 20.0 + (actor.links.len() as f64) * 30.0;

        let _ = write!(
            out,
            r##"<g id="actor{idx}_popup" class="actorPopupMenu" display="{display}">"##,
            idx = actor_cnt,
            display = escape_attr(popup_display),
        );
        let _ = write!(
            out,
            r##"<rect class="{class}" x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3"/>"##,
            class = escape_attr(&popup_panel_class),
            x = fmt(x),
            y = fmt(actor_height),
            w = fmt(n.width),
            h = fmt(panel_height),
            fill = escape_xml_display(popup_fill),
        );

        for (label, url) in &actor.links {
            let Some(href) = url.as_str() else {
                continue;
            };
            let href = url::Url::parse(href)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| href.to_string());
            let href = merman_core::utils::format_url(&href, sanitize_config)
                .filter(|u| u.trim() != merman_core::utils::BLANK_URL);
            let text_x = x + 10.0;
            let text_y = actor_height + link_y + 10.0;
            if let Some(href) = href {
                let _ = write!(
                    out,
                    r##"<a xlink:href="{href}"><text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor" style="text-anchor: start; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{label}</tspan></text></a>"##,
                    href = escape_xml(&href),
                    x = fmt(text_x),
                    y = fmt(text_y),
                    label = escape_xml(label)
                );
            } else {
                let _ = write!(
                    out,
                    r##"<a><text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="actor" style="text-anchor: start; font-size: 16px; font-weight: 400;"><tspan x="{x}" dy="0">{label}</tspan></text></a>"##,
                    x = fmt(text_x),
                    y = fmt(text_y),
                    label = escape_xml(label)
                );
            }
            link_y += 30.0;
        }

        out.push_str("</g>");
    }
}

pub(super) fn render_sequence_actor_man_tops(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
) {
    // Actor-man variants (actor/boundary/control/entity) are emitted after `<defs>`.
    for (actor_idx, actor_id) in model.actor_order.iter().enumerate() {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        if !matches!(actor_type, "actor" | "boundary" | "control" | "entity") {
            continue;
        }
        let node_id = format!("actor-top-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (_x, actor_y) = node_left_top(n);
        let cx = n.x;

        match actor_type {
            "actor" => {
                let r = 15.0;
                let cy = actor_y + 10.0;
                let torso_top = cy + r;
                let torso_bottom = torso_top + 20.0;
                let arms_y = torso_top + 8.0;
                let arms_x1 = cx - 18.0;
                let arms_x2 = cx + 18.0;
                let leg_y = torso_bottom + 15.0;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-top" name="{name}"><line id="actor-man-torso{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}"/><line id="actor-man-arms{idx}" x1="{ax1}" y1="{ay}" x2="{ax2}" y2="{ay}"/><line x1="{ax1}" y1="{ly}" x2="{cx}" y2="{y2}"/><line x1="{cx}" y1="{y2}" x2="{lx2}" y2="{ly}"/><circle cx="{cx}" cy="{cy}" r="15" width="{w}" height="{h}"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    idx = actor_idx,
                    cx = fmt(cx),
                    y1 = fmt(torso_top),
                    y2 = fmt(torso_bottom),
                    ax1 = fmt(arms_x1),
                    ax2 = fmt(arms_x2),
                    ay = fmt(arms_y),
                    ly = fmt(leg_y),
                    lx2 = fmt(cx + 16.0),
                    cy = fmt(cy),
                    w = fmt(n.width),
                    h = fmt(actor_height),
                    ty = fmt(actor_y + actor_height + 2.5),
                    label = escape_xml(&actor.description)
                );
            }
            "boundary" => {
                let radius = 30.0;
                let x_left = cx - radius * 2.5;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-top" name="{name}" transform="translate(0,22)"><line id="actor-man-torso{idx}" x1="{x1}" y1="{y_t}" x2="{x2}" y2="{y_t}"/><line id="actor-man-arms{idx}" x1="{x1}" y1="{y0}" x2="{x1}" y2="{y20}"/><circle cx="{cx}" cy="{cy}" r="30"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    idx = actor_idx,
                    x1 = fmt(x_left),
                    x2 = fmt(cx - 15.0),
                    y_t = fmt(actor_y + 10.0),
                    y0 = fmt(actor_y + 0.0),
                    y20 = fmt(actor_y + 20.0),
                    cx = fmt(cx),
                    cy = fmt(actor_y + 10.0),
                    // drawTextCandidate adds rect.height/2. Top render uses the config height.
                    ty = fmt(actor_y + (radius / 2.0 + 3.0) + (actor_height / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            "control" => {
                let r = 18.0;
                let cy = actor_y + 30.0;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-top" name="{name}"><defs><marker id="filled-head-control" refX="11" refY="5.8" markerWidth="20" markerHeight="28" orient="172.5"><path d="M 14.4 5.6 L 7.2 10.4 L 8.8 5.6 L 7.2 0.8 Z"/></marker></defs><circle cx="{cx}" cy="{cy}" r="18" fill="#eaeaf7" stroke="#666" stroke-width="1.2"/><line marker-end="url(#filled-head-control)" transform="translate({cx}, {ly})"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                    ly = fmt(cy - r),
                    ty = fmt(actor_y + (r + 10.0) + (actor_height / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            "entity" => {
                let r = 18.0;
                let cy = actor_y + 25.0;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-top" name="{name}" transform="translate(0, 9)"><circle cx="{cx}" cy="{cy}" r="18" width="{w}" height="{h}"/><line x1="{x1}" x2="{x2}" y1="{y}" y2="{y}" stroke="#333" stroke-width="2"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                    w = fmt(n.width),
                    h = fmt(actor_height),
                    x1 = fmt(cx - r),
                    x2 = fmt(cx + r),
                    y = fmt(cy + r),
                    ty = fmt(actor_y + ((cy + r - actor_y) / 2.0) + (actor_height / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            _ => {}
        }
    }
}

pub(super) fn render_sequence_actor_man_bottoms(
    out: &mut String,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    actor_height: f64,
    label_box_height: f64,
) {
    // Actor-man footers (actor/boundary/control/entity) are emitted after messages.
    let last_idx = model.actor_order.len().saturating_sub(1);
    for actor_id in &model.actor_order {
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let actor_type = actor.actor_type.as_str();
        if !matches!(actor_type, "actor" | "boundary" | "control" | "entity") {
            continue;
        }
        let node_id = format!("actor-bottom-{actor_id}");
        let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
            continue;
        };
        let (_x, actor_y) = node_left_top(n);
        let cx = n.x;

        match actor_type {
            "actor" => {
                let r = 15.0;
                let cy = actor_y + 10.0;
                let torso_top = cy + r;
                let torso_bottom = torso_top + 20.0;
                let arms_y = torso_top + 8.0;
                let arms_x1 = cx - 18.0;
                let arms_x2 = cx + 18.0;
                let leg_y = torso_bottom + 15.0;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-bottom" name="{name}"><line id="actor-man-torso{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}"/><line id="actor-man-arms{idx}" x1="{ax1}" y1="{ay}" x2="{ax2}" y2="{ay}"/><line x1="{ax1}" y1="{ly}" x2="{cx}" y2="{y2}"/><line x1="{cx}" y1="{y2}" x2="{lx2}" y2="{ly}"/><circle cx="{cx}" cy="{cy}" r="15" width="{w}" height="{h}"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    idx = last_idx,
                    cx = fmt(cx),
                    y1 = fmt(torso_top),
                    y2 = fmt(torso_bottom),
                    ax1 = fmt(arms_x1),
                    ax2 = fmt(arms_x2),
                    ay = fmt(arms_y),
                    ly = fmt(leg_y),
                    lx2 = fmt(cx + 16.0),
                    cy = fmt(cy),
                    w = fmt(n.width),
                    h = fmt(actor_height),
                    ty = fmt(actor_y + actor_height + 2.5),
                    label = escape_xml(&actor.description)
                );
            }
            "boundary" => {
                let radius = 30.0;
                let x_left = cx - radius * 2.5;
                let footer_h = 60.0 + label_box_height;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-bottom" name="{name}" transform="translate(0,22)"><line id="actor-man-torso{idx}" x1="{x1}" y1="{y_t}" x2="{x2}" y2="{y_t}"/><line id="actor-man-arms{idx}" x1="{x1}" y1="{y0}" x2="{x1}" y2="{y20}"/><circle cx="{cx}" cy="{cy}" r="30"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    idx = last_idx,
                    x1 = fmt(x_left),
                    x2 = fmt(cx - 15.0),
                    y_t = fmt(actor_y + 10.0),
                    y0 = fmt(actor_y + 0.0),
                    y20 = fmt(actor_y + 20.0),
                    cx = fmt(cx),
                    cy = fmt(actor_y + 10.0),
                    ty = fmt(actor_y + (radius / 2.0 - 4.0) + (footer_h / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            "control" => {
                let r = 18.0;
                let cy = actor_y + 30.0;
                let footer_h = 36.0 + 2.0 * label_box_height;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-bottom" name="{name}"><defs><marker id="filled-head-control" refX="11" refY="5.8" markerWidth="20" markerHeight="28" orient="172.5"><path d="M 14.4 5.6 L 7.2 10.4 L 8.8 5.6 L 7.2 0.8 Z"/></marker></defs><circle cx="{cx}" cy="{cy}" r="18" fill="#eaeaf7" stroke="#666" stroke-width="1.2"/><line marker-end="url(#filled-head-control)" transform="translate({cx}, {ly})"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                    ly = fmt(cy - r),
                    ty = fmt(actor_y + (r + 5.0) + (footer_h / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            "entity" => {
                let r = 18.0;
                let cy = actor_y + 10.0;
                let footer_h = 36.0 + label_box_height;
                let _ = write!(
                    out,
                    r##"<g class="actor-man actor-bottom" name="{name}" transform="translate(0, 9)"><circle cx="{cx}" cy="{cy}" r="18" width="{w}" height="{h}"/><line x1="{x1}" x2="{x2}" y1="{y}" y2="{y}" stroke="#333" stroke-width="2"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
                    name = escape_xml(actor_id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                    w = fmt(n.width),
                    h = fmt(footer_h),
                    x1 = fmt(cx - r),
                    x2 = fmt(cx + r),
                    y = fmt(cy + r),
                    ty = fmt(actor_y + ((cy - actor_y + r - 5.0) / 2.0) + (footer_h / 2.0)),
                    label = escape_xml(&actor.description)
                );
            }
            _ => {}
        }
    }
}
