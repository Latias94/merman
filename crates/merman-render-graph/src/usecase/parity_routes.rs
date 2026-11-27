//! Shared Dagre and ELK edge paths, label placement, and paint bounds.

use super::{UsecaseDiagramLayout, UsecaseEdgePlan, UsecaseNodePlan};
use merman_render_core::{
    Bounds, Error, LayoutEdge, LayoutNode, LayoutPoint, Result,
    elk_edge_geometry::{self as geometry, Outline, Shape},
    graph_layout::elk_terminal_jogs,
    layout_work::OperationLayoutWorkControl,
};
use serde_json::Value;
use std::collections::HashMap;

fn intersect(node: &LayoutNode, ellipse: bool, target: &LayoutPoint) -> LayoutPoint {
    let dx = target.x - node.x;
    let dy = target.y - node.y;
    let rx = node.width / 2.0;
    let ry = node.height / 2.0;
    if dx == 0.0 && dy == 0.0 {
        return LayoutPoint {
            x: node.x + rx,
            y: node.y,
        };
    }
    let denominator = if ellipse {
        (dx * dx / (rx * rx) + dy * dy / (ry * ry)).sqrt()
    } else {
        (dx.abs() / rx).max(dy.abs() / ry)
    };
    if !denominator.is_finite() || denominator == 0.0 {
        return LayoutPoint {
            x: node.x,
            y: node.y,
        };
    }
    LayoutPoint {
        x: node.x + dx / denominator,
        y: node.y + dy / denominator,
    }
}

pub(super) fn edge_points(
    edge: &LayoutEdge,
    nodes: &[LayoutNode],
    plans: &[UsecaseNodePlan],
) -> Vec<LayoutPoint> {
    let Some(start) = nodes.iter().find(|node| node.id == edge.from) else {
        return edge.points.clone();
    };
    let Some(end) = nodes.iter().find(|node| node.id == edge.to) else {
        return edge.points.clone();
    };
    let start_ellipse = plans
        .iter()
        .any(|plan| plan.id == edge.from && plan.ellipse);
    let end_ellipse = plans.iter().any(|plan| plan.id == edge.to && plan.ellipse);
    let mut points = edge.points.clone();
    if points.is_empty() {
        points = vec![
            LayoutPoint {
                x: start.x,
                y: start.y,
            },
            LayoutPoint { x: end.x, y: end.y },
        ];
        let centers = points.clone();
        if (end.x - start.x).abs() >= start.width / 2.0
            || (end.y - start.y).abs() >= start.height / 2.0
        {
            points[0] = intersect(start, start_ellipse, &points[1]);
        }
        points[1] = intersect(end, end_ellipse, &points[0]);
        if (points[1].x - points[0].x).hypot(points[1].y - points[0].y) < 2.0 {
            points = centers;
        }
        points.dedup_by(|right, left| {
            (right.x - left.x).abs() <= 1e-6 && (right.y - left.y).abs() <= 1e-6
        });
    } else if points.len() >= 2 {
        let last = points.len() - 1;
        points[0] = intersect(start, start_ellipse, &points[1]);
        points[last] = intersect(end, end_ellipse, &points[last - 1]);
    }
    points
}

pub(super) fn prepare_edge_paths(
    layout: &mut UsecaseDiagramLayout,
    plans: &[UsecaseNodePlan],
    edge_plans: &[UsecaseEdgePlan],
    config: &Value,
    elk: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<HashMap<String, Vec<LayoutPoint>>> {
    // Dagre's recursive paint measurement includes title labels which may extend beyond
    // a rect boundary's frame. Retain that measured contribution when clipping edge paths.
    let dagre_paint_bounds = (!elk).then(|| layout.bounds.clone()).flatten();
    let node_indexes: HashMap<_, _> = layout
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.clone(), index))
        .collect();
    let edge_indexes: HashMap<_, _> = layout
        .edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.id.as_str(), index))
        .collect();
    let plan_by_id: HashMap<_, _> = plans.iter().map(|plan| (plan.id.as_str(), plan)).collect();
    if elk {
        // The source resolves every node's first tiny-side anchor before building any route.
        let mut aligned = std::collections::HashSet::new();
        for edge in &layout.edges {
            for (id, anchor) in [
                (&edge.from, edge.points.first()),
                (&edge.to, edge.points.last()),
            ] {
                work.charge_adapter(1)?;
                let (Some(&index), Some(anchor)) = (node_indexes.get(id), anchor) else {
                    continue;
                };
                let node = &mut layout.nodes[index];
                let along_width = (anchor.y - (node.y - node.height / 2.0)).abs() <= 0.5
                    || (anchor.y - (node.y + node.height / 2.0)).abs() <= 0.5;
                if (if along_width { node.width } else { node.height }) >= 24.0
                    || !aligned.insert(index)
                {
                    continue;
                }
                let delta = if along_width {
                    anchor.x - node.x
                } else {
                    anchor.y - node.y
                };
                if delta.abs() < 0.01 {
                    continue;
                }
                if along_width {
                    node.x += delta;
                } else {
                    node.y += delta;
                }
            }
        }
    }
    work.charge_adapter(edge_plans.len())?;
    let mut route_layout_indexes = Vec::with_capacity(edge_plans.len());
    let mut routes = Vec::with_capacity(edge_plans.len());
    for plan in edge_plans {
        let Some(&index) = edge_indexes.get(plan.id.as_str()) else {
            return Err(Error::InvalidModel {
                message: format!("Usecase layout omitted edge {}", plan.id),
            });
        };
        let edge = &layout.edges[index];
        work.charge_adapter(work.checked_add(edge.points.len(), 50)?)?;
        let points = if elk {
            let start = Shape {
                intersection: None,
                node: &layout.nodes[node_indexes[&edge.from]],
                outline: if plan_by_id[edge.from.as_str()].ellipse {
                    Outline::Ellipse
                } else {
                    Outline::Rect
                },
            };
            let end = Shape {
                intersection: None,
                node: &layout.nodes[node_indexes[&edge.to]],
                outline: if plan_by_id[edge.to.as_str()].ellipse {
                    Outline::Ellipse
                } else {
                    Outline::Rect
                },
            };
            let mut input = Vec::with_capacity(edge.points.len() + 2);
            input.push(LayoutPoint {
                x: start.node.x,
                y: start.node.y,
            });
            input.extend_from_slice(&edge.points);
            input.push(LayoutPoint {
                x: end.node.x,
                y: end.node.y,
            });
            let mut points = geometry::sanitize(&input, start, end);
            if !edge.points.is_empty() {
                geometry::marker_segment(&mut points, end, plan.end_marker.as_deref(), false);
                geometry::marker_segment(&mut points, start, plan.start_marker.as_deref(), true);
            }
            points
        } else if let Some(id) = plan.dagre_self_loop_endpoint() {
            // Top-level normalization retains the original self-edge endpoints. Recursive
            // Dagre paint instead receives the actual helper-node endpoints.
            let mut paint_edge = edge.clone();
            paint_edge.from = id.to_owned();
            paint_edge.to = id.to_owned();
            edge_points(&paint_edge, &layout.nodes, plans)
        } else {
            edge_points(edge, &layout.nodes, plans)
        };
        // Empty diagrams can have one coincident endpoint after source deduplication.
        // Do not synthesize a second terminal, which would change the source fallback.
        route_layout_indexes.push(index);
        routes.push(points);
    }
    drop(edge_indexes);
    if elk
        && config
            .pointer("/elk/straightenEdges")
            .and_then(Value::as_bool)
            != Some(false)
    {
        work.charge_adapter(work.checked_add(layout.edges.len(), routes.len())?)?;
        let mut labels: Vec<_> = layout
            .edges
            .iter_mut()
            .map(|edge| edge.label.as_mut())
            .collect();
        let mut labelled_routes: Vec<_> = routes
            .iter_mut()
            .zip(&route_layout_indexes)
            .map(|(points, &index)| elk_terminal_jogs::LabeledTerminalRoute {
                points,
                label: labels[index].take(),
            })
            .collect();
        elk_terminal_jogs::straighten_edge_terminals(&mut labelled_routes, |units| {
            work.charge_adapter(units)
        })?;
    }
    let paths: HashMap<_, _> = edge_plans
        .iter()
        .zip(routes)
        .map(|(plan, route)| (plan.id.clone(), route))
        .collect();
    // Missing sections use the two terminal midpoint before painting; routed labels
    // retain the provider's own label placement.
    if elk {
        for edge in &mut layout.edges {
            if edge.points.is_empty()
                && let Some(label) = edge.label.as_mut()
                && let Some(points) = paths.get(&edge.id)
                && let (Some(first), Some(last)) = (points.first(), points.last())
            {
                label.x = (first.x + last.x) / 2.0;
                label.y = (first.y + last.y) / 2.0;
            }
        }
    }
    if elk {
        elk_terminal_jogs::separate_opposite_edge_labels(
            layout
                .edges
                .iter_mut()
                .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.label.as_mut())),
            |units| work.charge_adapter(units),
        )?;
    }
    layout.bounds = Bounds::from_points(
        layout
            .nodes
            .iter()
            .flat_map(|node| {
                [
                    (node.x - node.width / 2.0, node.y - node.height / 2.0),
                    (node.x + node.width / 2.0, node.y + node.height / 2.0),
                ]
            })
            .chain(
                paths
                    .values()
                    .flat_map(|points| points.iter().map(|p| (p.x, p.y))),
            )
            .chain(
                layout
                    .edges
                    .iter()
                    .filter_map(|edge| edge.label.as_ref())
                    .flat_map(|label| {
                        [
                            (label.x - label.width / 2.0, label.y - label.height / 2.0),
                            (label.x + label.width / 2.0, label.y + label.height / 2.0),
                        ]
                    }),
            ),
    );
    if let Some(previous) = dagre_paint_bounds {
        if let Some(bounds) = &mut layout.bounds {
            bounds.min_x = bounds.min_x.min(previous.min_x);
            bounds.min_y = bounds.min_y.min(previous.min_y);
            bounds.max_x = bounds.max_x.max(previous.max_x);
            bounds.max_y = bounds.max_y.max(previous.max_y);
        } else {
            layout.bounds = Some(previous);
        }
    }
    Ok(paths)
}
