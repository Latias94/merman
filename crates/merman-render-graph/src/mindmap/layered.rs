//! Mindmap's measured leaves and tree edges at the Mermaid Layered boundary.

use super::{
    MindmapAttemptError, MindmapDiagramLayout, MindmapModel, compute_bounds, outline, registered,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use kurbo::Point;
use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::{
    Error, LayoutNode, LayoutPoint, Result,
    graph_layout::{
        Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredNode, RoutingStyle, Size,
        SolvedEdge,
        mermaid_layered::{self, LayeredError, Options},
    },
    resources::OperationWorkMeter,
    route_types::RoutePoint,
    svg_edge_parity::{
        layered_route::{self, Curve},
        line_hops::{LineHopEdge, LineHopStyle},
        path_numbers::json_stringify_axis_points_into,
    },
};
use rustc_hash::FxHashSet;
use serde_json::Value;
use std::fmt;

#[derive(Debug)]
pub(super) enum Rejection {
    Provider(LayeredError),
    DuplicateEdgeId(String),
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => fmt::Display::fmt(error, f),
            Self::DuplicateEdgeId(id) => write!(f, "duplicate edge id '{id}'"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Routing {
    Rounded,
    Curved,
    Stepped,
    Straight,
}

impl Routing {
    fn resolve(config: &PresentationConfig) -> Self {
        match config
            .get_path(&["layered", "edgeRouting"])
            .and_then(Value::as_str)
        {
            Some("curved") => Self::Curved,
            Some("stepped") => Self::Stepped,
            Some("straight") => Self::Straight,
            _ => Self::Rounded,
        }
    }

    const fn solver(self) -> RoutingStyle {
        match self {
            Self::Rounded | Self::Stepped => RoutingStyle::Orthogonal,
            Self::Curved => RoutingStyle::Curved,
            Self::Straight => RoutingStyle::Polyline,
        }
    }

    fn curve(self, source: &str) -> Curve {
        match self {
            Self::Rounded => Curve::Rounded,
            Self::Straight => Curve::Linear,
            Self::Curved => Curve::from_mermaid(source),
            Self::Stepped => match Curve::from_mermaid(source) {
                curve if curve.is_stepped() => curve,
                _ => Curve::Step,
            },
        }
    }
}

#[derive(Debug)]
pub(super) struct Paint {
    routing: Routing,
    // One route per DB edge. Internal solver ordering stays private to the kernel.
    routes: Vec<SolvedEdge>,
}

pub(super) fn layout(
    model: &MindmapModel,
    mut nodes: Vec<LayoutNode>,
    edge_indices: &[(usize, usize)],
    config: &PresentationConfig,
    work: &OperationWorkMeter,
) -> std::result::Result<(MindmapDiagramLayout, Paint), MindmapAttemptError> {
    use OperationPhase::Layout;
    work.checkpoint(Layout)?;
    if model.nodes.len() != nodes.len() || model.edges.len() != edge_indices.len() {
        return Err(invalid_projection("measurement slots do not match the model").into());
    }
    let routing = Routing::resolve(config);
    let mut measured_nodes = buffer(nodes.len(), Layout, work)?;
    for (index, (node, source)) in nodes.iter().zip(&model.nodes).enumerate() {
        work.charge(1)?;
        let size = Size {
            width: node.width,
            height: node.height,
        };
        let mut input = MeasuredNode::new(size, slot(index, work)?, None);
        input.port_span = match source.shape.as_str() {
            "cloud" | "bang" => Some(size.width * 0.6),
            "hexagon" => Some((size.width - size.height / 2.0 - 12.0).max(0.0)),
            _ => None,
        };
        measured_nodes.push(input);
    }
    let mut edges = buffer(edge_indices.len(), Layout, work)?;
    let mut ids = FxHashSet::default();
    work.charge(edge_indices.len())?;
    ids.try_reserve(edge_indices.len())
        .map_err(|_| allocation_error())?;
    for (edge, &(source, target)) in model.edges.iter().zip(edge_indices) {
        work.charge(edge.id.len().max(1))?;
        if !ids.insert(edge.id.as_str()) {
            let mut id = String::new();
            id.try_reserve_exact(edge.id.len())
                .map_err(|_| allocation_error())?;
            id.push_str(&edge.id);
            return Err(MindmapAttemptError::Layered(Rejection::DuplicateEdgeId(id)));
        }
        edges.push(MeasuredEdge {
            source: Endpoint::Node(slot(source, work)?),
            target: Endpoint::Node(slot(target, work)?),
            min_rank_span: 1,
            weight: 1.0,
            label: None,
            start_label: None,
            end_label: None,
            start_marker: Default::default(),
            end_marker: Default::default(),
        });
    }
    drop(ids);

    // MindmapDB emits only flat leaves in preorder, and supplies TB and 50/50 spacing.
    let graph = MeasuredGraph {
        direction: Direction::Down,
        routing: routing.solver(),
        node_spacing: 50.0,
        rank_spacing: 50.0,
        component_spacing: 0.0,
        inherit_crossed_group_direction: true,
        nodes: measured_nodes,
        groups: Vec::new(),
        edges,
    };
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        mermaid_layered::layout(&graph, Options::default(), work)
    }
    .map_err(|error| match error {
        LayeredError::Work(error) => error.into(),
        LayeredError::ArithmeticOverflow => work.arithmetic_overflow().into(),
        LayeredError::AllocationFailed => allocation_error().into(),
        error => MindmapAttemptError::Layered(Rejection::Provider(error)),
    })?
    .graph;
    if solved.nodes.len() != nodes.len() || solved.edges.len() != model.edges.len() {
        return Err(invalid_projection("solver slots do not match the model").into());
    }
    for (node, rect) in nodes.iter_mut().zip(solved.nodes) {
        work.charge(1)?;
        let center = rect.center();
        node.x = center.x;
        node.y = center.y;
    }
    let mut layout_edges = buffer(model.edges.len(), Layout, work)?;
    for (edge, route) in model.edges.iter().zip(&solved.edges) {
        for id in [&edge.id, &edge.start, &edge.end] {
            work.charge(id.len().max(1))?;
        }
        let mut points = buffer(route.points.len(), Layout, work)?;
        points.extend(route.points.iter().map(layout_point));
        work.charge(points.len())?;
        layout_edges.push(registered::edge(edge, points));
    }
    let bounds = compute_bounds(&nodes, &layout_edges);
    work.checkpoint(Layout)?;
    Ok((
        MindmapDiagramLayout {
            nodes,
            edges: layout_edges,
            bounds,
            swimlane_lanes: Vec::new(),
        },
        Paint {
            routing,
            routes: solved.edges,
        },
    ))
}

pub(super) struct PaintedEdge {
    pub(super) path: String,
    pub(super) data_points: String,
    points: Vec<LayoutPoint>,
    curve: Curve,
}

impl Paint {
    pub(super) fn edges(
        &self,
        model: &MindmapModel,
        layout: &MindmapDiagramLayout,
        config: &PresentationConfig,
        work: &OperationWorkMeter,
    ) -> Result<Vec<PaintedEdge>> {
        use OperationPhase::Emit;
        let mut painted = buffer(self.routes.len(), Emit, work)?;
        let mut route = Vec::new();
        let mut axes = Vec::new();
        let mut json = String::new();
        let mut number = Default::default();
        for (edge, solved) in model.edges.iter().zip(&self.routes) {
            work.charge_emit_work(solved.points.len())?;
            route.clear();
            route
                .try_reserve(solved.points.len())
                .map_err(|_| allocation_error())?;
            route.extend(solved.points.iter().copied());
            let (Endpoint::Node(source), Endpoint::Node(target)) = (solved.source, solved.target)
            else {
                return Err(invalid_projection("a tree edge has a group endpoint"));
            };
            let last = route
                .len()
                .checked_sub(1)
                .ok_or_else(|| invalid_projection("empty edge route"))?;
            for (knot, node) in [(0, source as usize), (last, target as usize)] {
                let source_node = &model.nodes[node];
                if !outline::needs_clip(source_node) {
                    continue;
                }
                let node = &layout.nodes[node];
                route[knot] = layered_route::clip_endpoint(
                    route[knot],
                    Point::new(node.x, node.y),
                    kurbo::Size::new(node.width, node.height),
                    |toward, origin| {
                        Ok(Some(
                            origin
                                .and_then(|origin| {
                                    outline::intersect_segment(node, source_node, origin, toward)
                                })
                                .unwrap_or_else(|| {
                                    outline::intersect_ray(node, source_node, toward)
                                }),
                        ))
                    },
                    work,
                )?;
            }
            let curve = self.routing.curve(&edge.curve);
            let path = layered_route::path(&route, curve, 0.0, 0.0, work)?;
            let mut points = buffer(route.len(), Emit, work)?;
            points.extend(route.iter().map(layout_point));
            axes.clear();
            axes.try_reserve(route.len())
                .map_err(|_| allocation_error())?;
            axes.extend(route.iter().map(|point| point.axis));
            json.clear();
            work.charge_emit_work(route.len())?;
            json_stringify_axis_points_into(&mut json, &points, &axes, &mut number);
            work.charge_emit_work(json.len())?;
            painted.push(PaintedEdge {
                path,
                data_points: STANDARD.encode(&json),
                points,
                curve,
            });
        }

        let style = match config.get_path(&["layered", "lineHops"]) {
            Some(Value::Bool(false)) => return Ok(painted),
            Some(Value::String(style)) if style == "gap" => LineHopStyle::Gap,
            _ => LineHopStyle::Arc,
        };
        let mut edges = buffer(painted.len(), Emit, work)?;
        edges.extend(
            model
                .edges
                .iter()
                .zip(&painted)
                .map(|(edge, paint)| LineHopEdge {
                    id: &edge.id,
                    points: &paint.points,
                    curve: Some(paint.curve.as_str()),
                    arrow_type_start: None,
                    arrow_type_end: None,
                }),
        );
        for (slot, path) in layered_route::line_hops(&edges, style, work)? {
            work.charge_emit_work(path.len())?;
            painted[slot].path = path;
        }
        Ok(painted)
    }
}

fn slot(index: usize, work: &OperationWorkMeter) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.arithmetic_overflow().into())
}

fn layout_point(point: &RoutePoint) -> LayoutPoint {
    LayoutPoint {
        x: point.x,
        y: point.y,
    }
}

fn buffer<T>(count: usize, phase: OperationPhase, work: &OperationWorkMeter) -> Result<Vec<T>> {
    work.charge_work(count, phase)?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(count)
        .map_err(|_| allocation_error())?;
    Ok(buffer)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}

pub(super) fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid Mindmap Layered projection: {detail}"),
    }
}

#[cfg(test)]
mod tests;
