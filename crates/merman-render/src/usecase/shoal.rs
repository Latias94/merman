use super::{UsecaseDiagramLayout, UsecaseEdgePlan, UsecaseLabelPlan, UsecaseNodePlan};
use crate::graph_layout::canonical::CanonicalRoutes;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, RoutingStyle,
    Side, Size, SolvedGroup, TitlePosition,
};
use crate::graph_layout::placement;
use crate::graph_layout::route_paint::RouteStyle;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, LayoutLabel, LayoutPoint};
use crate::{Error, LayoutExecution, Result};
use kurbo::Insets;
use merman_model::families::usecase::{UsecaseDiagramModel, UsecaseNodeKind};
use serde_json::Value;
use std::collections::HashMap;

mod marker;
pub(crate) use marker::UsecaseMarker;

const BOUNDARY_PADDING: f64 = 20.0;

#[derive(Debug)]
pub(super) struct UsecaseCanonicalLayout {
    pub(super) titles: HashMap<String, LayoutLabel>,
    pub(super) boundary_order: Vec<(usize, usize)>,
}

pub(crate) fn canonical_route_style(config: &Value) -> RouteStyle {
    let curve = config
        .pointer("/flowchart/curve")
        .and_then(Value::as_str)
        .unwrap_or("basis");
    let radius = config
        .pointer("/flowchart/roundedCornerRadius")
        .and_then(Value::as_f64)
        .filter(|radius| radius.is_finite() && *radius >= 0.0)
        .unwrap_or(8.0);
    RouteStyle::from_interpolate(curve, radius)
}

fn dimensions(label: &UsecaseLabelPlan) -> (f64, f64) {
    (label.metrics.width, label.metrics.height)
}

fn label_size(label: &UsecaseLabelPlan) -> Option<Size> {
    let (width, height) = dimensions(label);
    (width > 0.0 && height > 0.0).then_some(Size { width, height })
}

fn group_index(endpoint: Endpoint) -> Option<u32> {
    match endpoint {
        Endpoint::Node(_) => None,
        Endpoint::Group(index) => Some(index),
    }
}

fn set_parent(slot: &mut Option<u32>, group: u32, id: &str) -> Result<()> {
    if slot.is_some_and(|existing| existing != group) {
        return Err(Error::InvalidModel {
            message: format!("Usecase endpoint {id} belongs to more than one boundary"),
        });
    }
    *slot = Some(group);
    Ok(())
}

fn boundary_paint_order(
    groups: &[SolvedGroup],
    plan_indices: &[usize],
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<(usize, usize)>> {
    work.charge_adapter(work.checked_mul(groups.len(), 3)?)?;
    let mut children = vec![Vec::new(); groups.len()];
    let mut order = Vec::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        work.charge_adapter(0)?;
        if let Some(parent) = group.parent {
            children[parent as usize].push((index, plan_indices[index]));
        } else {
            order.push((index, plan_indices[index]));
        }
    }
    let mut next = 0;
    while let Some(&(parent, _)) = order.get(next) {
        work.charge_adapter(0)?;
        order.extend_from_slice(&children[parent]);
        next += 1;
    }
    Ok(order)
}

pub(super) fn layout(
    model: &UsecaseDiagramModel,
    config: &Value,
    plans: &[UsecaseNodePlan],
    edge_plans: &[UsecaseEdgePlan],
    execution: Option<&LayoutExecution<'_>>,
    work: &mut OperationLayoutWorkControl,
) -> Result<(
    UsecaseDiagramLayout,
    UsecaseCanonicalLayout,
    CanonicalRoutes,
)> {
    work.charge_adapter(work.checked_add(plans.len(), model.nodes.len())?)?;
    let direction = match model.direction.as_str() {
        "LR" => Direction::Right,
        "RL" => Direction::Left,
        "BT" => Direction::Up,
        _ => Direction::Down,
    };
    let (face, perpendicular) = match direction {
        Direction::Up | Direction::Down => (Side::Top, Side::Left),
        Direction::Left | Direction::Right => (Side::Left, Side::Top),
    };
    let node_spacing = super::measure::number(config, "nodeSpacing", 50.0);
    let rank_spacing = super::measure::number(config, "rankSpacing", 50.0);
    let routing = match canonical_route_style(config) {
        RouteStyle::Rounded(_)
        | RouteStyle::Step
        | RouteStyle::StepBefore
        | RouteStyle::StepAfter => RoutingStyle::Orthogonal,
        RouteStyle::Linear => RoutingStyle::Polyline,
        RouteStyle::Curved => RoutingStyle::Curved,
    };
    let mut graph = MeasuredGraph {
        direction,
        routing,
        node_spacing,
        rank_spacing,
        component_spacing: node_spacing.max(rank_spacing),
        inherit_crossed_group_direction: false,
        nodes: Vec::new(),
        groups: Vec::with_capacity(model.boundaries.len()),
        edges: Vec::new(),
    };
    let mut endpoints = HashMap::with_capacity(plans.len());
    let actors: HashMap<_, _> = model
        .nodes
        .iter()
        .filter(|node| node.kind == UsecaseNodeKind::Actor)
        .map(|node| (node.id.as_str(), node.actor_type))
        .collect();
    let mut profiles = Vec::new();
    let mut node_ids = Vec::with_capacity(plans.len().saturating_sub(model.boundaries.len()));
    let mut boundary_plans = Vec::with_capacity(model.boundaries.len());
    let index = |len: usize, work: &OperationLayoutWorkControl| {
        u32::try_from(len).map_err(|_| Error::from(work.record_arithmetic_overflow()))
    };
    let title_margin = config
        .pointer("/flowchart/subGraphTitleMargin/top")
        .and_then(Value::as_f64)
        .filter(|margin| margin.is_finite() && *margin >= 0.0)
        .unwrap_or_default();
    for (plan_index, plan) in plans.iter().enumerate() {
        work.charge_adapter(0)?;
        let seq = index(plan_index, work)?;
        let endpoint = if plan.is_boundary {
            let group = Endpoint::Group(index(graph.groups.len(), work)?);
            boundary_plans.push(plan_index);
            let title = label_size(&plan.label);
            let title_band = title.map_or(0.0, |title| title.height + title_margin);
            graph.groups.push(MeasuredGroup {
                seq,
                parent: None,
                direction: None,
                padding: Insets::new(
                    BOUNDARY_PADDING,
                    2.0 * BOUNDARY_PADDING + title_band,
                    BOUNDARY_PADDING,
                    BOUNDARY_PADDING,
                ),
                min_size: Size {
                    width: plan.width,
                    height: plan.height,
                },
                title,
                title_position: if plan.package {
                    TitlePosition::TopLeft
                } else {
                    TitlePosition::TopCenter
                },
            });
            group
        } else {
            let node = Endpoint::Node(index(graph.nodes.len(), work)?);
            let (profile, size, anchor_y) = if let Some(&variant) = actors.get(plan.id.as_str()) {
                let actor = super::actor::ActorPlacement::canonical(plan, variant);
                (
                    ShapePortProfile::usecase_actor(
                        variant,
                        actor.placement,
                        dimensions(&plan.label),
                        plan.stereotype.as_ref().map(dimensions),
                    ),
                    actor.size,
                    Some(actor.anchor_y),
                )
            } else {
                let profile = if plan.ellipse {
                    ShapePortProfile::ellipse(plan.width, plan.height)
                } else {
                    ShapePortProfile::rectangle(plan.width, plan.height)
                };
                (
                    profile,
                    Size {
                        width: plan.width,
                        height: plan.height,
                    },
                    None,
                )
            };
            graph.nodes.push(MeasuredNode {
                size,
                seq,
                parent: None,
                port_span: Some(profile.port_span(face)),
                perpendicular_port_span: Some(profile.port_span(perpendicular)),
                anchor_y,
            });
            node_ids.push(plan.id.as_str());
            profiles.push(profile);
            node
        };
        endpoints.insert(plan.id.as_str(), endpoint);
    }
    let endpoint = |id: &str| {
        endpoints
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing Usecase endpoint {id}"),
            })
    };
    let group = |id: &str| {
        group_index(endpoint(id)?).ok_or_else(|| Error::InvalidModel {
            message: format!("invalid Usecase boundary {id}"),
        })
    };
    for plan in plans {
        if let Some(parent) = &plan.parent {
            work.charge_adapter(1)?;
            let parent = group(parent)?;
            let slot = match endpoint(&plan.id)? {
                Endpoint::Node(node) => &mut graph.nodes[node as usize].parent,
                Endpoint::Group(node) => &mut graph.groups[node as usize].parent,
            };
            set_parent(slot, parent, &plan.id)?;
        }
    }
    for boundary in &model.boundaries {
        work.charge_adapter(boundary.members.len().saturating_add(1))?;
        let parent = group(&boundary.id)?;
        for id in &boundary.members {
            work.charge_adapter(0)?;
            let slot = match endpoint(id)? {
                Endpoint::Node(node) => &mut graph.nodes[node as usize].parent,
                Endpoint::Group(node) => &mut graph.groups[node as usize].parent,
            };
            set_parent(slot, parent, id)?;
        }
    }
    work.charge_adapter(edge_plans.len())?;
    graph.edges.reserve(edge_plans.len());
    let neo = config.get("look").and_then(Value::as_str) == Some("neo");
    for plan in edge_plans {
        work.charge_adapter(0)?;
        let marker = |start| {
            UsecaseMarker::for_edge(plan, start, neo)
                .map_or_else(Default::default, |marker| marker.metrics())
        };
        graph.edges.push(MeasuredEdge {
            source: endpoint(&plan.source)?,
            target: endpoint(&plan.target)?,
            min_rank_span: index(plan.minlen.max(1), work)?,
            weight: 1.0,
            label: plan.label.as_ref().and_then(label_size),
            start_label: None,
            end_label: None,
            start_marker: marker(true),
            end_marker: marker(false),
        });
    }
    let (mut solved, placement) = placement::solve(
        &graph,
        node_ids.iter().copied(),
        boundary_plans.iter().map(|&index| plans[index].id.as_str()),
        execution,
        Default::default(),
        work,
    )?;
    let mut routes = CanonicalRoutes::from_solved(
        &graph,
        &solved,
        &profiles,
        |_| 0.0,
        edge_plans.iter().map(|plan| plan.id.as_str()),
        work,
    )?;
    routes.placement = placement;
    let boundary_order = boundary_paint_order(&solved.groups, &boundary_plans, work)?;
    work.charge_adapter(work.checked_add(plans.len(), edge_plans.len())?)?;
    let mut nodes = Vec::with_capacity(plans.len());
    let mut titles = HashMap::with_capacity(solved.groups.len());
    for plan in plans {
        work.charge_adapter(plan.id.len())?;
        let rect = match endpoint(&plan.id)? {
            Endpoint::Node(node) => solved.nodes[node as usize],
            Endpoint::Group(group) => {
                let group = &mut solved.groups[group as usize];
                if let Some(title) = group.title.take() {
                    titles.insert(plan.id.clone(), title);
                }
                group.rect
            }
        };
        let center = rect.center();
        nodes.push(super::layout::node(
            plan,
            center.x,
            center.y,
            rect.width,
            rect.height,
        ));
    }
    let mut edges = Vec::with_capacity(edge_plans.len());
    for (plan, edge) in edge_plans.iter().zip(solved.edges) {
        let strings = work.checked_add(
            plan.id.len(),
            work.checked_add(plan.source.len(), plan.target.len())?,
        )?;
        work.charge_adapter(work.checked_add(strings, edge.points.len())?)?;
        let mut projected = super::layout::edge(
            plan,
            edge.points
                .into_iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect(),
            edge.label,
        );
        projected.from_cluster = group_index(endpoint(&plan.source)?).map(|_| plan.source.clone());
        projected.to_cluster = group_index(endpoint(&plan.target)?).map(|_| plan.target.clone());
        edges.push(projected);
    }
    let bounds = (!nodes.is_empty()).then_some(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    });
    Ok((
        UsecaseDiagramLayout {
            nodes,
            edges,
            bounds,
        },
        UsecaseCanonicalLayout {
            titles,
            boundary_order,
        },
        routes,
    ))
}
