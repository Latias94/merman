//! Flowchart edge bbox/path helpers.
//!
//! This module computes the edge path `d` and its bounds (bbox). It is used by the flowchart
//! renderer for tasks like cluster label placement and viewBox sizing.

use super::*;
use crate::svg::parity;

pub(super) fn flowchart_edge_path_d_for_bbox(
    layout_edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
    translate_x: f64,
    translate_y: f64,
    default_edge_interpolate: &str,
    edge_html_labels: bool,
    edge: &crate::flowchart::FlowEdge,
) -> Option<(String, parity::path_bounds::SvgPathBounds)> {
    flowchart_edge_path_d_for_bbox_impl(
        layout_edges_by_id,
        layout_clusters_by_id,
        translate_x,
        translate_y,
        default_edge_interpolate,
        edge_html_labels,
        edge,
    )
}

fn flowchart_edge_path_d_for_bbox_impl(
    layout_edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
    translate_x: f64,
    translate_y: f64,
    default_edge_interpolate: &str,
    edge_html_labels: bool,
    edge: &crate::flowchart::FlowEdge,
) -> Option<(String, parity::path_bounds::SvgPathBounds)> {
    let le = layout_edges_by_id.get(edge.id.as_str()).copied()?;
    if le.points.len() < 2 {
        return None;
    }

    let mut local_points: Vec<crate::model::LayoutPoint> = Vec::new();
    for p in &le.points {
        local_points.push(crate::model::LayoutPoint {
            x: p.x + translate_x,
            y: p.y + translate_y,
        });
    }

    fn boundary_for_cluster(
        layout_clusters_by_id: &FxHashMap<&str, &LayoutCluster>,
        cluster_id: &str,
        translate_x: f64,
        translate_y: f64,
    ) -> Option<super::edge_geom::BoundaryNode> {
        let n = layout_clusters_by_id.get(cluster_id).copied()?;
        Some(super::edge_geom::BoundaryNode {
            x: n.x + translate_x,
            y: n.y + translate_y,
            width: n.width,
            height: n.height,
        })
    }

    let is_cyclic_special = edge.id.contains("-cyclic-special-");

    let mut points_for_render: Vec<crate::model::LayoutPoint> = Vec::new();
    super::edge_geom::dedup_consecutive_points_into(&local_points, &mut points_for_render);
    if let Some(tc) = le.to_cluster.as_deref() {
        if let Some(boundary) =
            boundary_for_cluster(layout_clusters_by_id, tc, translate_x, translate_y)
        {
            let mut tmp: Vec<crate::model::LayoutPoint> = Vec::new();
            super::edge_geom::cut_path_at_intersect_into(&points_for_render, &boundary, &mut tmp);
            points_for_render = tmp;
        }
    }
    if let Some(fc) = le.from_cluster.as_deref() {
        if let Some(boundary) =
            boundary_for_cluster(layout_clusters_by_id, fc, translate_x, translate_y)
        {
            let mut rev = points_for_render.clone();
            rev.reverse();
            let mut tmp: Vec<crate::model::LayoutPoint> = Vec::new();
            super::edge_geom::cut_path_at_intersect_into(&rev, &boundary, &mut tmp);
            rev = tmp;
            rev.reverse();
            points_for_render = rev;
        }
    }

    let interpolate = edge
        .interpolate
        .as_deref()
        .unwrap_or(default_edge_interpolate);
    let is_basis = !matches!(
        interpolate,
        "linear"
            | "natural"
            | "step"
            | "stepAfter"
            | "stepBefore"
            | "cardinal"
            | "monotoneX"
            | "monotoneY"
    );

    let label_text = edge.label.as_deref().unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let label_text_plain = flowchart_label_plain_text(label_text, label_type, edge_html_labels);
    let has_label_text = !label_text_plain.trim().is_empty();
    let is_cluster_edge = le.to_cluster.is_some() || le.from_cluster.is_some();

    if is_basis
        && !has_label_text
        && !is_cyclic_special
        && edge.length <= 1
        && points_for_render.len() > 4
    {
        super::edge_geom::maybe_collapse_straight_except_one_endpoint(&mut points_for_render);
    }

    if is_basis && is_cluster_edge {
        super::edge_geom::maybe_remove_redundant_cluster_run_point(&mut points_for_render);
    }

    if is_basis
        && is_cyclic_special
        && edge.id.contains("-cyclic-special-mid")
        && points_for_render.len() > 3
    {
        points_for_render = vec![
            points_for_render[0].clone(),
            points_for_render[points_for_render.len() / 2].clone(),
            points_for_render[points_for_render.len() - 1].clone(),
        ];
    }
    if points_for_render.len() == 1 {
        points_for_render = local_points.clone();
    }

    if is_basis
        && points_for_render.len() == 2
        && interpolate != "linear"
        && (!is_cluster_edge || is_cyclic_special)
    {
        super::edge_geom::maybe_insert_midpoint_for_basis(
            &mut points_for_render,
            interpolate,
            is_cluster_edge,
            is_cyclic_special,
        );
    }

    if is_basis && is_cyclic_special {
        super::edge_geom::maybe_pad_cyclic_special_basis_route_for_layout_clusters(
            layout_clusters_by_id,
            edge,
            &mut points_for_render,
        );
    }

    let mut line_data: Vec<crate::model::LayoutPoint> = points_for_render
        .iter()
        .filter(|p| !p.y.is_nan())
        .cloned()
        .collect();

    super::edge_geom::maybe_fix_corners(&mut line_data);

    let line_data =
        super::edge_geom::line_with_offset_for_edge_type(&line_data, edge.edge_type.as_deref());

    let (d, pb, _skipped_bounds_for_viewbox) =
        super::edge_geom::curve_path_d_and_bounds(&line_data, interpolate, 0.0, 0.0, None);
    let pb = pb?;
    Some((d, pb))
}
