use super::prelude::*;
use merman_render_core::{
    number_format::canonicalize_number, resources::OperationWorkMeter,
    theme_svg::NEO_SHADOW_OFFSET_PX,
};
pub(super) fn builtin_neo_rect_paint_bounds(
    rect: [f64; 4],
    stroke_width: f64,
    has_shadow: bool,
) -> Bounds {
    let [x, y, width, height] = rect;
    let half_stroke = stroke_width / 2.0;
    let offset = if has_shadow {
        NEO_SHADOW_OFFSET_PX
    } else {
        0.0
    };
    Bounds {
        min_x: x - half_stroke,
        min_y: y - half_stroke,
        max_x: x + width + half_stroke + offset,
        max_y: y + height + half_stroke + offset,
    }
}

pub(super) fn builtin_actor_paint_bounds(
    nodes: &rustc_hash::FxHashMap<&str, &LayoutNode>,
    model: &super::model::SequenceSvgModel,
    mirror: bool,
    has_root_canvas: bool,
    style: super::actor_shapes::SequenceActorRectStyle,
    compat: &crate::preparation::SequenceCompatBinding,
    work_meter: &OperationWorkMeter,
) -> Result<Option<Bounds>> {
    let mut total: Option<Bounds> = None;
    if !has_root_canvas || !compat.is_neo {
        return Ok(total);
    }
    // Read the same generated CSS value as the actor writer. Relative lengths cannot supply
    // a reliable filter envelope; retain a residual instead of guessing a pixel width.
    let width = style
        .stroke_width
        .map(f64::from)
        .or(compat.native_stroke_width);
    let Some(width) = width else {
        return Ok(total);
    };
    for actor_id in &model.actor_order {
        work_meter.charge(1)?;
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        if !super::actor_shapes::actor_rect_geometry_supported(actor) {
            continue;
        }
        for placement in ["top", "bottom"] {
            if placement == "bottom" && !mirror {
                continue;
            }
            let node_id = format!("actor-{placement}-{actor_id}");
            let Some(node) = nodes.get(node_id.as_str()) else {
                continue;
            };
            let (left, top) = super::geometry::node_left_top(node);
            let bounds =
                builtin_neo_rect_paint_bounds([left, top, node.width, node.height], width, true);
            if let Some(total) = &mut total {
                total.min_x = total.min_x.min(bounds.min_x);
                total.min_y = total.min_y.min(bounds.min_y);
                total.max_x = total.max_x.max(bounds.max_x);
                total.max_y = total.max_y.max(bounds.max_y);
            } else {
                total = Some(bounds);
            }
        }
    }
    Ok(total)
}

pub(super) fn include_lifeline_paint_bounds(
    total: &mut Option<Bounds>,
    x: f64,
    y1: f64,
    y2: f64,
    width: f64,
) {
    // Use the serialized coordinates, including descending or zero-length lines.
    let [x, y1, y2] = [x, y1, y2].map(canonicalize_number);
    let bounds = Bounds {
        min_x: x - width / 2.0,
        max_x: x + width / 2.0,
        min_y: y1.min(y2) - width / 2.0,
        max_y: y1.max(y2) + width / 2.0,
    };
    if let Some(total) = total {
        total.min_x = total.min_x.min(bounds.min_x);
        total.min_y = total.min_y.min(bounds.min_y);
        total.max_x = total.max_x.max(bounds.max_x);
        total.max_y = total.max_y.max(bounds.max_y);
    } else {
        *total = Some(bounds);
    }
}
