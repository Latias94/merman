//! Map prepared Swimlane labels and ownership to one shared rank grid.

use super::config::SwimlaneConfig;
use super::prepare::{normalize_direction, synthetic_node};
use super::working::{WorkingLayout, WorkingNode, WorkingNodeKind};
use crate::flowchart::{
    FlowchartEdgeKey, FlowchartRenderModelRef, FlowchartShape, FlowchartSvgLabelOwner,
    FlowchartSvgLabelSidecarBuilder, ProjectedFlowchartEdge, resolve_flowchart_shape,
};
use crate::graph_layout::axis_clearance::AxisClearance;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, RoutingStyle,
    Side, Size, SolvedGraph, TitlePosition,
};
use crate::graph_layout::route_paint::ObstacleOwner;
use crate::graph_layout::{AxisRoutes, RouteContract, RouteTopology, placement};
use crate::layout_frame::GraphPlacement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{LayoutNode, LayoutPoint, SwimlaneDirection, SwimlaneTitleRect};
use crate::{Error, LayoutExecution, Result};
use merman_model::presentation::PresentationConfig;
use rustc_hash::FxHashMap;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct SwimlaneShoal {
    pub(crate) routes: RouteContract,
    pub(crate) placement: GraphPlacement,
}

struct Bindings {
    node_slots: Vec<usize>,
    group_slots: Vec<usize>,
    profiles: Vec<ShapePortProfile>,
}

fn direction(value: SwimlaneDirection) -> Direction {
    match value {
        SwimlaneDirection::Tb => Direction::Down,
        SwimlaneDirection::Bt => Direction::Up,
        SwimlaneDirection::Lr => Direction::Right,
        SwimlaneDirection::Rl => Direction::Left,
    }
}

fn direction_name(value: Direction) -> &'static str {
    match value {
        Direction::Down => "TB",
        Direction::Up => "BT",
        Direction::Right => "LR",
        Direction::Left => "RL",
    }
}

fn index(value: usize, work: &OperationLayoutWorkControl) -> Result<u32> {
    u32::try_from(value).map_err(|_| work.record_arithmetic_overflow().into())
}

fn invalid_endpoint(id: &str) -> Error {
    Error::InvalidModel {
        message: format!("No measured Swimlane endpoint for `{id}`"),
    }
}

pub(super) fn solve(
    working: &mut WorkingLayout,
    view: &FlowchartRenderModelRef<'_>,
    visible_edges: &[(FlowchartEdgeKey, ProjectedFlowchartEdge<'_>)],
    effective_config: &PresentationConfig,
    config: SwimlaneConfig,
    execution: &LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
) -> Result<SwimlaneShoal> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    let (graph, lane_order, bindings) = measure(
        working,
        view,
        visible_edges,
        effective_config,
        config,
        labels,
        &mut work,
    )?;
    let ids = |&slot: &usize| working.nodes[slot].id.as_str();
    let (solved, placement) = placement::solve(
        &graph,
        bindings.node_slots.iter().map(ids),
        bindings.group_slots.iter().map(ids),
        Some(execution),
        placement::Constraints {
            lane_order: &lane_order,
            capture_ranks: true,
            ..Default::default()
        },
        &mut work,
    )?;
    drop(graph);
    let routes = bindings.project(working, solved, view.edges.len(), visible_edges, &mut work)?;
    Ok(SwimlaneShoal { routes, placement })
}

fn measure(
    working: &WorkingLayout,
    view: &FlowchartRenderModelRef<'_>,
    visible_edges: &[(FlowchartEdgeKey, ProjectedFlowchartEdge<'_>)],
    effective_config: &PresentationConfig,
    config: SwimlaneConfig,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    work: &mut OperationLayoutWorkControl,
) -> Result<(MeasuredGraph, Vec<u32>, Bindings)> {
    work.charge_adapter(working.nodes.len())?;
    let flow = direction(working.direction);
    let horizontal = matches!(flow, Direction::Left | Direction::Right);
    let faces = if horizontal {
        [Side::Left, Side::Top]
    } else {
        [Side::Top, Side::Left]
    };
    let mut node_slots = Vec::new();
    let mut group_slots = Vec::new();
    let mut endpoints =
        FxHashMap::with_capacity_and_hasher(working.nodes.len(), Default::default());
    for (slot, node) in working.nodes.values().enumerate() {
        let endpoint = match node.kind {
            WorkingNodeKind::Content => {
                let id = index(node_slots.len(), work)?;
                node_slots.push(slot);
                Endpoint::Node(id)
            }
            WorkingNodeKind::Group => {
                let id = index(group_slots.len(), work)?;
                group_slots.push(slot);
                Endpoint::Group(id)
            }
            _ => continue,
        };
        work.charge_adapter(node.id.len())?;
        endpoints.insert(node.id.as_str(), endpoint);
    }
    let group_of = |id: &str| match endpoints.get(id) {
        Some(&Endpoint::Group(group)) => Ok(group),
        _ => Err(invalid_endpoint(id)),
    };
    let parent = |id: Option<&str>| id.map(group_of).transpose();
    let mut authored_order = vec![None; group_slots.len()];
    for (ordinal, id) in working.top_lane_order.iter().enumerate() {
        work.charge_adapter(1)?;
        authored_order[group_of(id)? as usize] = Some(ordinal);
    }
    let mut graph = MeasuredGraph {
        direction: flow,
        routing: RoutingStyle::Orthogonal,
        node_spacing: config.node_gap,
        rank_spacing: config.layer_gap,
        component_spacing: config.node_gap,
        inherit_crossed_group_direction: false,
        nodes: Vec::with_capacity(node_slots.len()),
        groups: Vec::with_capacity(group_slots.len()),
        edges: Vec::with_capacity(visible_edges.len()),
    };
    let mut lane_order = Vec::new();
    for (id, &slot) in group_slots.iter().enumerate() {
        work.charge_adapter(1)?;
        let node = &working.nodes[slot];
        let parent = parent(node.parent_id.as_deref())?;
        let is_lane = parent.is_none();
        if is_lane {
            lane_order.push(index(id, work)?);
        }
        let mut title = (node.label_width > 0.0 && node.label_height > 0.0).then_some(Size {
            width: node.label_width,
            height: node.label_height,
        });
        if is_lane
            && horizontal
            && let Some(title) = &mut title
        {
            std::mem::swap(&mut title.width, &mut title.height);
        }
        let padding = node.padding.max(0.0);
        let mut insets = kurbo::Insets::uniform(padding);
        if !is_lane && let Some(title) = title {
            insets.y0 += title.height;
        }
        graph.groups.push(MeasuredGroup {
            seq: index(slot, work)?,
            parent,
            direction: node
                .requested_dir
                .as_deref()
                .map(|name| direction(normalize_direction(Some(name)))),
            padding: insets,
            min_size: title.map_or_else(Size::default, |title| Size {
                width: title.width + padding * 2.0,
                height: title.height + padding * 2.0,
            }),
            title,
            title_position: TitlePosition::TopCenter,
        });
    }
    let lane_levels = lane_order.len().checked_ilog2().unwrap_or(0) as usize + 1;
    work.charge_adapter(work.checked_mul(lane_order.len(), lane_levels)?)?;
    lane_order.sort_unstable_by_key(|&id| (authored_order[id as usize].unwrap_or(usize::MAX), id));

    let source_nodes: HashMap<_, _> = view
        .nodes
        .iter()
        .enumerate()
        .map(|(ordinal, node)| (node.id.as_str(), (node, ordinal)))
        .collect();
    let group_ordinals: HashMap<_, _> = view
        .subgraphs
        .iter()
        .enumerate()
        .rev()
        .map(|(ordinal, group)| (group.id.as_str(), ordinal))
        .collect();
    let neo = crate::config::mermaid_config_diagram_look(effective_config).is_neo();
    let configured_radius =
        crate::config::config_f64(effective_config, &["themeVariables", "radius"])
            .map(|radius| (radius.max(0.0), radius.max(0.0)));
    let process_radius = (neo
        && effective_config.explicit_config_owns_path("themeVariables.radius"))
    .then_some(configured_radius)
    .flatten();
    let mut profiles = Vec::with_capacity(node_slots.len());
    for &slot in &node_slots {
        work.charge_adapter(1)?;
        let node = &working.nodes[slot];
        let parent = parent(node.parent_id.as_deref())?;
        let authored = source_nodes.get(node.id.as_str()).copied();
        let owner = if node.shape == "collapsedGroup" {
            group_ordinals
                .get(node.id.as_str())
                .map(|&index| view.nodes.len() + index)
        } else {
            authored.map(|(_, ordinal)| ordinal)
        };
        let synthetic;
        let source = if node.shape != "collapsedGroup"
            && let Some((source, _)) = authored
        {
            source
        } else {
            synthetic = synthetic_node(&node.id, &node.label, Some(&node.label_type), &node.shape);
            &synthetic
        };
        let geometry = labels.zip(owner).and_then(|(labels, owner)| {
            labels.measured_node_geometry(FlowchartSvgLabelOwner::SwimlaneNode(owner))
        });
        let radii = match resolve_flowchart_shape(&node.shape)? {
            FlowchartShape::Process => process_radius,
            FlowchartShape::RoundedRectangle => configured_radius,
            _ => None,
        };
        let profile = ShapePortProfile::flowchart_with_paint_metrics(
            source,
            &LayoutNode {
                id: String::new(),
                x: 0.0,
                y: 0.0,
                width: node.width,
                height: node.height,
                is_cluster: false,
                label_width: Some(node.label_width),
                label_height: Some(node.label_height),
            },
            node.padding,
            neo,
            geometry,
            radii,
        )?;
        graph.nodes.push(MeasuredNode {
            size: Size {
                width: node.width,
                height: node.height,
            },
            seq: index(slot, work)?,
            parent,
            port_span: Some(profile.port_span(faces[0])),
            perpendicular_port_span: Some(profile.port_span(faces[1])),
            anchor_y: profile.anchor_y(),
        });
        profiles.push(profile);
    }
    for ((_, edge), binding) in visible_edges.iter().zip(&working.original_edges) {
        work.charge_adapter(1)?;
        let endpoint = |id: &str| {
            endpoints
                .get(id)
                .copied()
                .ok_or_else(|| invalid_endpoint(id))
        };
        let label = binding.label_node_id.as_ref().map(|id| &working.nodes[id]);
        let (start_marker, end_marker) = crate::svg::flowchart_marker_metrics(edge.semantic);
        graph.edges.push(MeasuredEdge {
            source: endpoint(edge.from)?,
            target: endpoint(edge.to)?,
            min_rank_span: index(edge.semantic.length.max(1), work)?,
            weight: 1.0,
            label: label.map(|label| Size {
                width: label.label_width,
                height: label.label_height,
            }),
            start_label: None,
            end_label: None,
            start_marker,
            end_marker,
        });
    }
    Ok((
        graph,
        lane_order,
        Bindings {
            node_slots,
            group_slots,
            profiles,
        },
    ))
}

impl Bindings {
    fn project(
        self,
        working: &mut WorkingLayout,
        mut solved: SolvedGraph,
        semantic_edge_count: usize,
        visible_edges: &[(FlowchartEdgeKey, ProjectedFlowchartEdge<'_>)],
        work: &mut OperationLayoutWorkControl,
    ) -> Result<RouteContract> {
        let Self {
            node_slots,
            group_slots,
            profiles,
        } = self;
        let horizontal = matches!(
            working.direction,
            SwimlaneDirection::Lr | SwimlaneDirection::Rl
        );
        let mut obstacles = Vec::new();
        for (id, (&slot, bounds)) in node_slots.iter().zip(solved.nodes).enumerate() {
            work.charge_adapter(1)?;
            let center = bounds.center();
            let node = &mut working.nodes[slot];
            node.x = center.x;
            node.y = center.y;
            node.layer = solved.node_ranks[id] as usize;
            obstacles.push((
                kurbo::Rect::from_origin_size((bounds.x, bounds.y), (bounds.width, bounds.height)),
                ObstacleOwner::Node(id as u32),
            ));
        }
        let mut node_slots = node_slots;
        let order_levels = node_slots.len().checked_ilog2().unwrap_or(0) as usize + 1;
        work.charge_adapter(work.checked_mul(node_slots.len(), order_levels)?)?;
        node_slots.sort_unstable_by(|&a, &b| {
            let across = |node: &WorkingNode| if horizontal { node.y } else { node.x };
            let (one, two) = (&working.nodes[a], &working.nodes[b]);
            one.layer
                .cmp(&two.layer)
                .then_with(|| across(one).total_cmp(&across(two)))
                .then(a.cmp(&b))
        });
        let mut rank = None;
        let mut order = 0;
        for slot in node_slots {
            let node = &mut working.nodes[slot];
            if rank != Some(node.layer) {
                rank = Some(node.layer);
                order = 0;
            }
            node.order = order;
            order += 1;
        }
        let group_parents: Vec<_> = solved.groups.iter().map(|group| group.parent).collect();
        for (id, (&slot, group)) in group_slots.iter().zip(solved.groups).enumerate() {
            work.charge_adapter(1)?;
            let parent_id = group
                .parent
                .map(|parent| working.nodes[group_slots[parent as usize]].id.clone());
            let node = &mut working.nodes[slot];
            let center = group.rect.center();
            node.x = center.x;
            node.y = center.y;
            node.width = group.rect.width;
            node.height = group.rect.height;
            node.parent_id = parent_id;
            node.requested_dir = Some(direction_name(group.direction).to_owned());
            if let Some(label) = group.title {
                let rect =
                    kurbo::Rect::from_center_size((label.x, label.y), (label.width, label.height));
                node.title_rect = Some(SwimlaneTitleRect {
                    left: rect.x0,
                    right: rect.x1,
                    top: rect.y0,
                    bottom: rect.y1,
                });
                obstacles.push((rect, ObstacleOwner::GroupTitle(id as u32)));
            }
        }
        work.charge_adapter(semantic_edge_count)?;
        let mut routes = vec![None; semantic_edge_count];
        for ((binding, route), (owner, _)) in working
            .original_edges
            .iter_mut()
            .zip(solved.edges)
            .zip(visible_edges)
        {
            work.charge_adapter(route.points.len())?;
            let edge = owner.semantic_index();
            let (points, knots) = route
                .points
                .into_iter()
                .map(|point| {
                    (
                        LayoutPoint {
                            x: point.x,
                            y: point.y,
                        },
                        (point.axis, point.kind),
                    )
                })
                .unzip();
            binding.points = points;
            binding.reversed_for_layout = route.reversed_for_ranking;
            if let Some((id, label)) = binding.label_node_id.as_ref().zip(route.label) {
                let node = &mut working.nodes[id];
                node.x = label.x;
                node.y = label.y;
                obstacles.push((
                    kurbo::Rect::from_center_size((label.x, label.y), (label.width, label.height)),
                    ObstacleOwner::EdgeLabel(edge),
                ));
            }
            routes[edge] = Some(RouteTopology {
                endpoints: (route.source, route.target),
                ports: route.ports,
                knots,
            });
        }
        work.charge_adapter(solved.border_stitches.len())?;
        for stitch in &mut solved.border_stitches {
            stitch.edge = visible_edges[stitch.edge].0.semantic_index();
        }
        let levels = obstacles.len().checked_ilog2().unwrap_or(0) as usize + 1;
        work.charge_adapter(work.checked_mul(obstacles.len(), levels)?)?;
        Ok(RouteContract::CanonicalAxis(
            AxisRoutes {
                nodes: profiles,
                group_parents,
                border_stitches: solved.border_stitches,
                clearance: AxisClearance::new(obstacles),
                routes,
                svg_edges: None,
            }
            .into(),
        ))
    }
}
