use super::bounds::recompute_nested_group_bounds;
use super::config::LR_TITLE_BAND_SIZE;
use super::geometry::Rect;
use super::working::{WorkingLayout, WorkingNodeKind};
use crate::model::{LayoutPoint, SwimlaneDirection, SwimlaneTitleRect};
use std::collections::HashMap;

mod detour_simplification;
mod endpoint_clip;
pub(super) mod geometry;
mod label_anchoring;
mod materialized_geometry;
mod port_swap;
mod shared_track_nudging;
mod sibling_shared_face_routing;
mod terminal_stub;
#[cfg(test)]
mod validation;

use geometry::{orthogonalize_polyline, simplify_polyline};

fn layout_content_ids(layout: &WorkingLayout) -> Vec<String> {
    layout
        .nodes
        .values()
        .filter(|node| !node.is_group() && node.kind != WorkingNodeKind::Dummy)
        .map(|node| node.id.clone())
        .collect()
}

fn mirror_axis(layout: &mut WorkingLayout, horizontal: bool) {
    let content_ids = layout_content_ids(layout);
    let values: Vec<f64> = content_ids
        .iter()
        .filter_map(|id| layout.nodes.get(id))
        .map(|node| if horizontal { node.x } else { node.y })
        .collect();
    let Some(minimum) = values.iter().copied().reduce(f64::min) else {
        return;
    };
    let Some(maximum) = values.iter().copied().reduce(f64::max) else {
        return;
    };
    let mirror = |value: f64| minimum + maximum - value;
    for node in layout
        .nodes
        .values_mut()
        .filter(|node| node.kind != WorkingNodeKind::Dummy)
    {
        if horizontal {
            node.x = mirror(node.x);
            if let Some(rect) = &mut node.title_rect {
                let left = mirror(rect.right);
                let right = mirror(rect.left);
                rect.left = left;
                rect.right = right;
            }
        } else {
            node.y = mirror(node.y);
            if let Some(rect) = &mut node.title_rect {
                let top = mirror(rect.bottom);
                let bottom = mirror(rect.top);
                rect.top = top;
                rect.bottom = bottom;
            }
        }
    }
    for edge in &mut layout.original_edges {
        for point in &mut edge.points {
            if horizontal {
                point.x = mirror(point.x);
            } else {
                point.y = mirror(point.y);
            }
        }
    }
}

fn transform_lr(layout: &mut WorkingLayout) {
    let content_ids = layout_content_ids(layout);
    if content_ids.is_empty() {
        return;
    }
    let min_x = content_ids
        .iter()
        .map(|id| layout.nodes[id].x)
        .fold(f64::INFINITY, f64::min);
    let min_y = content_ids
        .iter()
        .map(|id| layout.nodes[id].y)
        .fold(f64::INFINITY, f64::min);
    let (total_width, total_height) = content_ids.iter().fold((0.0, 0.0), |acc, id| {
        let node = &layout.nodes[id];
        (acc.0 + node.width, acc.1 + node.height)
    });
    let average_width = total_width / content_ids.len() as f64;
    let average_height = total_height / content_ids.len() as f64;
    let horizontal_scale = if average_height > 0.0 {
        (average_width / average_height).max(1.0)
    } else {
        1.0
    };
    let transform = |point: &LayoutPoint| LayoutPoint {
        x: (point.y - min_y) * horizontal_scale + LR_TITLE_BAND_SIZE,
        y: point.x - min_x,
    };

    for id in &content_ids {
        if let Some(node) = layout.nodes.get_mut(id) {
            let transformed = transform(&LayoutPoint {
                x: node.x,
                y: node.y,
            });
            node.x = transformed.x;
            node.y = transformed.y;
        }
    }
    for edge in &mut layout.original_edges {
        for point in &mut edge.points {
            *point = transform(point);
        }
    }

    recompute_nested_group_bounds(layout);
    let top_lane_ids: Vec<String> = layout
        .nodes
        .values()
        .filter(|node| node.is_group() && node.parent_id.is_none())
        .map(|node| node.id.clone())
        .collect();
    let mut children_by_lane: HashMap<String, Vec<String>> = HashMap::new();
    for id in &content_ids {
        let Some(lane_id) = layout.nodes[id].top_lane_id.clone() else {
            continue;
        };
        children_by_lane
            .entry(lane_id)
            .or_default()
            .push(id.clone());
    }

    let max_padding = top_lane_ids
        .iter()
        .map(|id| layout.nodes[id].padding)
        .fold(0.0, f64::max);
    let mut lane_bounds = Vec::new();
    let mut global_min_x = f64::INFINITY;
    let mut global_max_x = f64::NEG_INFINITY;
    for id in &top_lane_ids {
        let children = children_by_lane.get(id).cloned().unwrap_or_default();
        let mut bounds: Option<Rect> = None;
        for child_id in children {
            let child = &layout.nodes[&child_id];
            let rect = Rect::from_center(child.x, child.y, child.width, child.height);
            if let Some(current) = &mut bounds {
                current.union(rect);
            } else {
                bounds = Some(rect);
            }
        }
        let Some(bounds) = bounds else {
            continue;
        };
        global_min_x = global_min_x.min(bounds.left);
        global_max_x = global_max_x.max(bounds.right);
        lane_bounds.push((
            id.clone(),
            bounds.top,
            bounds.bottom,
            (bounds.top + bounds.bottom) / 2.0,
        ));
    }
    if !global_min_x.is_finite() || !global_max_x.is_finite() {
        return;
    }

    let horizontal_margin = max_padding.max(10.0);
    let body_width = global_max_x - global_min_x + 2.0 * horizontal_margin;
    let lane_width = LR_TITLE_BAND_SIZE + body_width;
    let body_center = (global_min_x + global_max_x) / 2.0;
    let body_left = body_center - body_width / 2.0;
    let lane_left = body_left - LR_TITLE_BAND_SIZE;
    let center_x = lane_left + lane_width / 2.0;
    let vertical_margin = max_padding.max(LR_TITLE_BAND_SIZE);
    lane_bounds.sort_by(|left, right| left.3.total_cmp(&right.3));

    for index in 0..lane_bounds.len() {
        let (id, content_top, content_bottom, _) = &lane_bounds[index];
        let top = if index == 0 {
            content_top - vertical_margin
        } else {
            (lane_bounds[index - 1].2 + content_top) / 2.0
        };
        let bottom = if index + 1 == lane_bounds.len() {
            content_bottom + vertical_margin
        } else {
            (content_bottom + lane_bounds[index + 1].1) / 2.0
        };
        if let Some(lane) = layout.nodes.get_mut(id) {
            lane.x = center_x;
            lane.y = (top + bottom) / 2.0;
            lane.width = lane_width;
            lane.height = (bottom - top).max(0.0);
            lane.content_top = Some(*content_top);
            lane.title_rect = Some(SwimlaneTitleRect {
                left: lane_left,
                right: lane_left + LR_TITLE_BAND_SIZE,
                top,
                bottom,
            });
        }
    }
}

fn finalize_rendered_edges(layout: &mut WorkingLayout) {
    materialized_geometry::resolve_rendered_orthogonal_crossings(layout);
    materialized_geometry::reassign_crossing_external_rail_channels(layout);
    materialized_geometry::shortcut_redundant_orthogonal_jogs(layout);
    label_anchoring::anchor_labels_to_polyline(layout);
    endpoint_clip::prepare_edge_endpoints_for_renderer(layout);
    materialized_geometry::lift_obstacle_hugging_same_side_rails(layout);
    label_anchoring::anchor_labels_to_polyline(layout);
    endpoint_clip::prepare_edge_endpoints_for_renderer(layout);
}

pub(super) fn post_process(layout: &mut WorkingLayout) {
    match layout.direction {
        SwimlaneDirection::Tb => {}
        SwimlaneDirection::Bt => mirror_axis(layout, false),
        SwimlaneDirection::Lr => transform_lr(layout),
        SwimlaneDirection::Rl => {
            transform_lr(layout);
            mirror_axis(layout, true);
        }
    }
    for edge in &mut layout.original_edges {
        edge.points = simplify_polyline(&orthogonalize_polyline(&edge.points));
    }
    detour_simplification::simplify_detoured_edges(layout);
    sibling_shared_face_routing::straighten_collinear_sibling_detours(layout);
    port_swap::port_swap_to_l_shape(layout);
    label_anchoring::anchor_labels_to_polyline(layout);
    endpoint_clip::clip_edge_endpoints_to_node_boundaries(layout);
    terminal_stub::collapse_short_terminal_stub(layout);
    shared_track_nudging::nudge_shared_interior_subpaths(layout);
    materialized_geometry::separate_shared_rendered_terminal_lanes(layout);
    materialized_geometry::collapse_redundant_rectangular_doglegs(layout);
    materialized_geometry::lift_obstacle_hugging_same_side_rails(layout);
    materialized_geometry::swap_destination_terminal_tails_to_reduce_crossings(layout);

    finalize_rendered_edges(layout);
    shared_track_nudging::nudge_shared_interior_subpaths(layout);
    finalize_rendered_edges(layout);

    for _ in 0..2 {
        materialized_geometry::lift_top_lane_title_bands_above_rails(layout);
        materialized_geometry::shift_left_lane_title_bands_left_of_rails(layout);
    }
}
