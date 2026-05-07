use super::super::*;
use super::geometry::node_left_top;

pub(super) fn write_actor_man_top_glyph(
    out: &mut String,
    actor_type: &str,
    actor_id: &str,
    label: &str,
    n: &LayoutNode,
    idx: usize,
    actor_height: f64,
) {
    let (_, actor_y) = node_left_top(n);
    let cx = n.x;

    match actor_type {
        "actor" => write_stick_actor_glyph(
            out,
            "actor-top",
            actor_id,
            label,
            n,
            idx,
            actor_y,
            cx,
            actor_height,
        ),
        "boundary" => {
            let radius = 30.0;
            // drawTextCandidate adds rect.height/2. Top render uses the config height.
            let label_y = actor_y + (radius / 2.0 + 3.0) + (actor_height / 2.0);
            write_boundary_actor_glyph(
                out,
                "actor-top",
                actor_id,
                label,
                idx,
                actor_y,
                cx,
                label_y,
            );
        }
        "control" => {
            let r = 18.0;
            let label_y = actor_y + (r + 10.0) + (actor_height / 2.0);
            write_control_actor_glyph(out, "actor-top", actor_id, label, actor_y, cx, label_y);
        }
        "entity" => {
            let r = 18.0;
            let cy = actor_y + 25.0;
            let label_y = actor_y + ((cy + r - actor_y) / 2.0) + (actor_height / 2.0);
            write_entity_actor_glyph(
                out,
                "actor-top",
                actor_id,
                label,
                n.width,
                actor_height,
                cx,
                cy,
                label_y,
            );
        }
        _ => {}
    }
}

pub(super) fn write_actor_man_bottom_glyph(
    out: &mut String,
    actor_type: &str,
    actor_id: &str,
    label: &str,
    n: &LayoutNode,
    idx: usize,
    actor_height: f64,
    label_box_height: f64,
) {
    let (_, actor_y) = node_left_top(n);
    let cx = n.x;

    match actor_type {
        "actor" => write_stick_actor_glyph(
            out,
            "actor-bottom",
            actor_id,
            label,
            n,
            idx,
            actor_y,
            cx,
            actor_height,
        ),
        "boundary" => {
            let radius = 30.0;
            let footer_h = 60.0 + label_box_height;
            let label_y = actor_y + (radius / 2.0 - 4.0) + (footer_h / 2.0);
            write_boundary_actor_glyph(
                out,
                "actor-bottom",
                actor_id,
                label,
                idx,
                actor_y,
                cx,
                label_y,
            );
        }
        "control" => {
            let r = 18.0;
            let footer_h = 36.0 + 2.0 * label_box_height;
            let label_y = actor_y + (r + 5.0) + (footer_h / 2.0);
            write_control_actor_glyph(out, "actor-bottom", actor_id, label, actor_y, cx, label_y);
        }
        "entity" => {
            let r = 18.0;
            let cy = actor_y + 10.0;
            let footer_h = 36.0 + label_box_height;
            let label_y = actor_y + ((cy - actor_y + r - 5.0) / 2.0) + (footer_h / 2.0);
            write_entity_actor_glyph(
                out,
                "actor-bottom",
                actor_id,
                label,
                n.width,
                footer_h,
                cx,
                cy,
                label_y,
            );
        }
        _ => {}
    }
}

fn write_stick_actor_glyph(
    out: &mut String,
    placement_class: &str,
    actor_id: &str,
    label: &str,
    n: &LayoutNode,
    idx: usize,
    actor_y: f64,
    cx: f64,
    actor_height: f64,
) {
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
        r##"<g class="actor-man {placement_class}" name="{name}"><line id="actor-man-torso{idx}" x1="{cx}" y1="{y1}" x2="{cx}" y2="{y2}"/><line id="actor-man-arms{idx}" x1="{ax1}" y1="{ay}" x2="{ax2}" y2="{ay}"/><line x1="{ax1}" y1="{ly}" x2="{cx}" y2="{y2}"/><line x1="{cx}" y1="{y2}" x2="{lx2}" y2="{ly}"/><circle cx="{cx}" cy="{cy}" r="15" width="{w}" height="{h}"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
        placement_class = placement_class,
        name = escape_xml(actor_id),
        idx = idx,
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
        label = escape_xml(label)
    );
}

fn write_boundary_actor_glyph(
    out: &mut String,
    placement_class: &str,
    actor_id: &str,
    label: &str,
    idx: usize,
    actor_y: f64,
    cx: f64,
    label_y: f64,
) {
    let radius = 30.0;
    let x_left = cx - radius * 2.5;
    let _ = write!(
        out,
        r##"<g class="actor-man {placement_class}" name="{name}" transform="translate(0,22)"><line id="actor-man-torso{idx}" x1="{x1}" y1="{y_t}" x2="{x2}" y2="{y_t}"/><line id="actor-man-arms{idx}" x1="{x1}" y1="{y0}" x2="{x1}" y2="{y20}"/><circle cx="{cx}" cy="{cy}" r="30"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
        placement_class = placement_class,
        name = escape_xml(actor_id),
        idx = idx,
        x1 = fmt(x_left),
        x2 = fmt(cx - 15.0),
        y_t = fmt(actor_y + 10.0),
        y0 = fmt(actor_y + 0.0),
        y20 = fmt(actor_y + 20.0),
        cx = fmt(cx),
        cy = fmt(actor_y + 10.0),
        ty = fmt(label_y),
        label = escape_xml(label)
    );
}

fn write_control_actor_glyph(
    out: &mut String,
    placement_class: &str,
    actor_id: &str,
    label: &str,
    actor_y: f64,
    cx: f64,
    label_y: f64,
) {
    let r = 18.0;
    let cy = actor_y + 30.0;
    let _ = write!(
        out,
        r##"<g class="actor-man {placement_class}" name="{name}"><defs><marker id="filled-head-control" refX="11" refY="5.8" markerWidth="20" markerHeight="28" orient="172.5"><path d="M 14.4 5.6 L 7.2 10.4 L 8.8 5.6 L 7.2 0.8 Z"/></marker></defs><circle cx="{cx}" cy="{cy}" r="18" fill="#eaeaf7" stroke="#666" stroke-width="1.2"/><line marker-end="url(#filled-head-control)" transform="translate({cx}, {ly})"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
        placement_class = placement_class,
        name = escape_xml(actor_id),
        cx = fmt(cx),
        cy = fmt(cy),
        ly = fmt(cy - r),
        ty = fmt(label_y),
        label = escape_xml(label)
    );
}

fn write_entity_actor_glyph(
    out: &mut String,
    placement_class: &str,
    actor_id: &str,
    label: &str,
    width: f64,
    height: f64,
    cx: f64,
    cy: f64,
    label_y: f64,
) {
    let r = 18.0;
    let _ = write!(
        out,
        r##"<g class="actor-man {placement_class}" name="{name}" transform="translate(0, 9)"><circle cx="{cx}" cy="{cy}" r="18" width="{w}" height="{h}"/><line x1="{x1}" x2="{x2}" y1="{y}" y2="{y}" stroke="#333" stroke-width="2"/><text x="{cx}" y="{ty}" dominant-baseline="central" alignment-baseline="central" class="actor actor-man" style="text-anchor: middle; font-size: 16px; font-weight: 400;"><tspan x="{cx}" dy="0">{label}</tspan></text></g>"##,
        placement_class = placement_class,
        name = escape_xml(actor_id),
        cx = fmt(cx),
        cy = fmt(cy),
        w = fmt(width),
        h = fmt(height),
        x1 = fmt(cx - r),
        x2 = fmt(cx + r),
        y = fmt(cy + r),
        ty = fmt(label_y),
        label = escape_xml(label)
    );
}
