//! The MindmapDB flat graph at the measured Grid boundary.

use super::{
    MindmapDiagramLayout, MindmapModel, MindmapNodeModel, compute_bounds, outline, registered,
};
use kurbo::Point;
use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::{
    Error, LayoutEdge, LayoutNode, LayoutPoint, Result,
    graph_layout::{
        Size,
        mermaid_grid::{
            GridItem, GridItemKind, GridLineHops, GridMeasuredEdge, GridMeasuredItem, GridPaint,
            prepare_grid,
            routing::{
                outline::{GridOutline, GridOutlineQuery, clip_grid_endpoints},
                paint::{GridPaintedEdge, grid_line_hop_overrides},
            },
        },
    },
    resources::OperationWorkMeter,
    svg_edge_parity::{curve, edge_path::maybe_fix_corners, line_hops::LineHopEdge},
};
use std::collections::BTreeMap;

pub(super) fn layout(
    model: &MindmapModel,
    mut nodes: Vec<LayoutNode>,
    edge_indices: &[(usize, usize)],
    config: &PresentationConfig,
    work: &OperationWorkMeter,
) -> Result<(MindmapDiagramLayout, GridPaint)> {
    work.checkpoint(OperationPhase::Layout)?;
    if nodes.len() != model.nodes.len() || edge_indices.len() != model.edges.len() {
        return Err(invalid_projection(
            "measurement slots do not match the source model",
        ));
    }

    work.charge(nodes.len())?;
    let mut items = buffer(nodes.len())?;
    for (ordinal, (node, measured)) in model.nodes.iter().zip(&nodes).enumerate() {
        work.charge(1)?;
        items.push(GridMeasuredItem {
            item: GridItem {
                id: &node.id,
                authored_id: Some(&node.node_id),
                source_ordinal: Some(ordinal),
                inline: Default::default(),
            },
            // MindmapDB's preorder nodes all belong to the root container. The tree is
            // represented by ordinary edges and does not turn a parent into a Grid group.
            parent_id: None,
            kind: GridItemKind::Leaf(Size {
                width: measured.width,
                height: measured.height,
            }),
        });
    }
    work.charge(model.edges.len())?;
    let mut edges = buffer(model.edges.len())?;
    for (edge, &(source, target)) in model.edges.iter().zip(edge_indices) {
        work.charge(1)?;
        edges.push(GridMeasuredEdge {
            id: &edge.id,
            source,
            target,
            label: None,
        });
    }

    let prepared = prepare_grid(&items, &edges, config.get("grid"), work)?;
    let routes = prepared.route_edges(work)?;
    if routes.len() != edges.len() {
        return Err(invalid_projection(
            "route slots do not match the source model",
        ));
    }
    for (slot, node) in nodes.iter_mut().enumerate() {
        work.charge(1)?;
        let center = prepared
            .item_bounds(slot)
            .ok_or_else(|| invalid_projection("missing node bounds"))?
            .center();
        node.x = center.x;
        node.y = center.y;
    }
    work.charge(routes.len())?;
    let mut layout_edges = buffer(routes.len())?;
    for (edge, route) in model.edges.iter().zip(routes) {
        work.charge(edge.id.len())?;
        work.charge(edge.start.len())?;
        work.charge(edge.end.len())?;
        work.charge(route.points.len())?;
        work.charge(route.points.len())?;
        let mut points = buffer(route.points.len())?;
        points.extend(route.points.into_iter().map(layout_point));
        layout_edges.push(registered::edge(edge, points));
    }
    let bounds = compute_bounds(&nodes, &layout_edges);
    work.checkpoint(OperationPhase::Layout)?;
    Ok((
        MindmapDiagramLayout {
            nodes,
            edges: layout_edges,
            bounds,
            swimlane_lanes: Vec::new(),
        },
        prepared.paint(),
    ))
}

pub(super) struct GridEdgePaint {
    pub(super) points: Vec<LayoutPoint>,
    pub(super) path: String,
}

pub(super) fn paint_edges(
    edges: &[LayoutEdge],
    nodes: &BTreeMap<String, &LayoutNode>,
    model_nodes: &BTreeMap<&str, &MindmapNodeModel>,
    settings: GridPaint,
    work: &OperationWorkMeter,
) -> Result<Vec<GridEdgePaint>> {
    work.charge_emit_work(edges.len())?;
    let mut paint = buffer(edges.len())?;
    let mut route = Vec::new();
    for edge in edges {
        work.charge_emit_work(edge.points.len())?;
        route.clear();
        route
            .try_reserve(edge.points.len())
            .map_err(|_| allocation_error())?;
        route.extend(edge.points.iter().map(|point| Point::new(point.x, point.y)));

        let source = nodes
            .get(&edge.from)
            .zip(model_nodes.get(edge.from.as_str()));
        let target = nodes.get(&edge.to).zip(model_nodes.get(edge.to.as_str()));
        let source_intersect =
            |query| Ok(source.and_then(|(node, model)| intersect(node, model, query)));
        let target_intersect =
            |query| Ok(target.and_then(|(node, model)| intersect(node, model, query)));
        let clipped = clip_grid_endpoints(
            &route,
            source.and_then(|(node, model)| outline(node, model, &source_intersect)),
            target.and_then(|(node, model)| outline(node, model, &target_intersect)),
            work,
        )?;
        work.charge_emit_work(clipped.len())?;
        let mut points = buffer(clipped.len())?;
        points.extend(clipped.iter().copied().map(layout_point));
        work.charge_emit_work(points.len())?;
        let path = match settings.curve {
            "rounded" => curve::curve_rounded_path_d(&points, settings.corner_radius),
            "linear" => curve::curve_linear_path_d(&points),
            name => {
                let mut line_data = buffer(points.len())?;
                line_data.extend(points.iter().cloned());
                maybe_fix_corners(&mut line_data);
                curve::interpolated_path_d(&line_data, name)
            }
        };
        work.charge_emit_work(path.len())?;
        paint.push(GridEdgePaint { points, path });
    }

    if settings.line_hops != GridLineHops::Disabled {
        work.charge_emit_work(edges.len())?;
        let mut rendered = buffer(edges.len())?;
        rendered.extend(
            edges
                .iter()
                .zip(&paint)
                .map(|(edge, paint)| GridPaintedEdge {
                    geometry: LineHopEdge {
                        id: &edge.id,
                        points: &paint.points,
                        curve: Some(settings.curve),
                        arrow_type_start: None,
                        arrow_type_end: None,
                    },
                    corner_radius: Some(settings.corner_radius),
                    original_path: &paint.path,
                }),
        );
        let overrides = grid_line_hop_overrides(settings.line_hops, &rendered, work)?;
        for (slot, path) in overrides {
            work.charge_emit_work(path.len())?;
            paint[slot].path = path;
        }
    }
    Ok(paint)
}

fn outline<'a>(
    node: &LayoutNode,
    model: &MindmapNodeModel,
    intersect: &'a dyn Fn(GridOutlineQuery) -> Result<Option<Point>>,
) -> Option<GridOutline<'a>> {
    outline::needs_clip(model).then_some(GridOutline {
        center: Some(Point::new(node.x, node.y)),
        size: Some(Size {
            width: node.width,
            height: node.height,
        }),
        intersect,
    })
}

fn intersect(
    node: &LayoutNode,
    model: &MindmapNodeModel,
    query: GridOutlineQuery,
) -> Option<Point> {
    outline::intersect_segment(node, model, query.line_origin?, query.adjacent)
}

fn layout_point(point: Point) -> LayoutPoint {
    LayoutPoint {
        x: point.x,
        y: point.y,
    }
}

fn buffer<T>(capacity: usize) -> Result<Vec<T>> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error())?;
    Ok(buffer)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

pub(super) fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid Mindmap Grid projection: {detail}"),
    }
}
