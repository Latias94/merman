mod regions;

use super::config::{StateConfigView, rank_dir_from, state_node_is_effective_group};
use super::layout::{
    edge_label_metrics, note_group_owner_id, state_fork_join_painted_dimensions,
    state_hidden_prefixes, state_layout_adapter_work, state_node_dimensions, title_label_metrics,
    validate_state_parent_cycles,
};
use super::{
    StateDiagramModel, StateEdgeStylePlan, StateLabelOwner, StateLabelSidecarBuilder, StateNode,
    StateNodeStylePlan, StateStylePlan, state_value_to_label_text,
};
use crate::graph_layout::canonical::CanonicalRoutes;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Rect,
    RoutePoint, RoutingStyle, Side, Size, SolvedGraph, TitlePosition,
};
use crate::graph_layout::shoal_bridge::{self, LayoutScratch};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{
    Bounds, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, StateDiagramLayout,
};
use crate::text::TextMeasurer;
use crate::{Error, LayoutExecution, Result};
use dugong::RankDir;
use kurbo::Insets;
use merman_model::families::state::StateDiagramRenderEdge;
use serde_json::Value;
use std::collections::HashMap;

struct StateGroup<'a> {
    node: &'a StateNode,
    title: String,
    padding: f64,
}

struct MeasuredState<'a> {
    graph: MeasuredGraph,
    leaves: Vec<(u32, &'a StateNode)>,
    groups: Vec<StateGroup<'a>>,
    edges: Vec<&'a StateDiagramRenderEdge>,
    profiles: Vec<ShapePortProfile>,
}

pub(crate) fn layout_state_diagram_shoal(
    model: &StateDiagramModel,
    effective_config: &Value,
    style_plan: &StateStylePlan,
    label_sidecar: Option<&StateLabelSidecarBuilder>,
    execution: &LayoutExecution<'_>,
) -> Result<(StateDiagramLayout, CanonicalRoutes)> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    work.charge_adapter(state_layout_adapter_work(model, &work)?)?;
    validate_state_parent_cycles(model)?;
    let input = measure(
        model,
        effective_config,
        style_plan,
        label_sidecar,
        execution.text_measurer(),
        &mut work,
    )?;
    let mut solved = shoal_bridge::solve(&input.graph, &mut LayoutScratch::default(), &mut work)?;
    regions::stretch(&input, &mut solved, &mut work)?;
    let routes = CanonicalRoutes::from_solved(
        &input.graph,
        &solved,
        &input.profiles,
        input.edges.iter().map(|edge| edge.id.as_str()),
        &mut work,
    )?;
    Ok((project(input, solved, &mut work)?, routes))
}

fn direction(rankdir: RankDir) -> Direction {
    match rankdir {
        RankDir::TB => Direction::Down,
        RankDir::BT => Direction::Up,
        RankDir::LR => Direction::Right,
        RankDir::RL => Direction::Left,
    }
}

fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Down => "TB",
        Direction::Up => "BT",
        Direction::Right => "LR",
        Direction::Left => "RL",
    }
}

fn measure<'a>(
    model: &'a StateDiagramModel,
    config: &Value,
    style_plan: &StateStylePlan,
    sidecar: Option<&StateLabelSidecarBuilder>,
    measurer: &dyn TextMeasurer,
    work: &mut OperationLayoutWorkControl,
) -> Result<MeasuredState<'a>> {
    let hidden = state_hidden_prefixes(model);
    let settings =
        StateConfigView::new(config).layout_settings(&model.direction, style_plan.compatibility());
    let rankdir = settings.graph.rankdir;
    let mut graph = MeasuredGraph {
        direction: direction(rankdir),
        routing: RoutingStyle::Curved,
        node_spacing: settings.graph.nodesep,
        rank_spacing: settings.graph.ranksep,
        component_spacing: settings.graph.nodesep.max(settings.graph.ranksep),
        inherit_crossed_group_direction: false,
        nodes: Vec::new(),
        groups: Vec::new(),
        edges: Vec::new(),
    };
    let aliases = note_group_aliases(model);
    let canonical = |id: &'a str| aliases.get(id).copied().unwrap_or(id);
    let index = |index: usize, work: &OperationLayoutWorkControl| {
        u32::try_from(index).map_err(|_| Error::from(work.record_arithmetic_overflow()))
    };
    let mut endpoints = HashMap::with_capacity(model.nodes.len());
    let mut leaves = Vec::new();
    let mut groups = Vec::new();
    for (sequence, node) in model.nodes.iter().enumerate() {
        if sequence.is_multiple_of(128) {
            work.charge_adapter(0)?;
        }
        if hidden.is_hidden(&node.id) || canonical(&node.id) != node.id {
            continue;
        }
        let seq = index(sequence, work)?;
        let endpoint = if state_node_is_effective_group(node) {
            let endpoint = Endpoint::Group(index(groups.len(), work)?);
            let style = style_plan.node(&node.id);
            let padding = style
                .and_then(StateNodeStylePlan::padding_override)
                .or(node.padding)
                .unwrap_or(settings.state_padding)
                .max(0.0);
            let title = node
                .label
                .as_ref()
                .map(state_value_to_label_text)
                .unwrap_or_else(|| node.id.clone());
            let title_size = if matches!(node.shape.as_str(), "divider" | "noteGroup")
                || title.trim().is_empty()
            {
                None
            } else {
                let (width, height) = title_label_metrics(
                    StateLabelOwner::ClusterTitle(&node.id),
                    &title,
                    measurer,
                    style
                        .map(StateNodeStylePlan::resolved_cluster_label_typography)
                        .unwrap_or_else(|| style_plan.composite_label_typography()),
                    settings.wrap_mode,
                    sidecar,
                );
                Some(Size { width, height })
            };
            let top = title_size.map_or(padding, |title| title.height + 2.0 * padding.max(1.0));
            graph.groups.push(MeasuredGroup {
                seq,
                parent: None,
                direction: node
                    .dir
                    .as_deref()
                    .filter(|_| node.explicit_dir != Some(false))
                    .map(|value| direction(rank_dir_from(value))),
                padding: Insets::new(padding, top, padding, padding),
                min_size: Size {
                    width: 1.0,
                    height: 1.0,
                },
                title: title_size,
                title_position: TitlePosition::TopCenter,
            });
            groups.push(StateGroup {
                node,
                title,
                padding,
            });
            endpoint
        } else {
            let endpoint = Endpoint::Node(index(leaves.len(), work)?);
            leaves.push((seq, node));
            endpoint
        };
        if endpoints.insert(node.id.as_str(), endpoint).is_some() {
            return Err(Error::InvalidModel {
                message: format!("duplicate State node {}", node.id),
            });
        }
    }

    let parent = |node: &'a StateNode| -> Result<Option<u32>> {
        let Some(id) = node.parent_id.as_deref().filter(|id| !hidden.is_hidden(id)) else {
            return Ok(None);
        };
        match endpoints.get(canonical(id)) {
            Some(&Endpoint::Group(group)) => Ok(Some(group)),
            _ => Err(Error::InvalidModel {
                message: format!("State parent `{id}` for `{}` is not a group", node.id),
            }),
        }
    };
    for (group, measured) in groups.iter().zip(&mut graph.groups) {
        work.charge_adapter(0)?;
        measured.parent = parent(group.node)?;
    }
    restore_concurrent_directions(&groups, &mut graph.groups, work)?;
    let group_directions = effective_group_directions(&graph, work)?;
    let mut profiles = Vec::with_capacity(leaves.len());
    let (face, perpendicular) = match rankdir {
        RankDir::TB | RankDir::BT => (Side::Top, Side::Left),
        RankDir::LR | RankDir::RL => (Side::Left, Side::Top),
    };
    for &(seq, node) in &leaves {
        work.charge_adapter(0)?;
        let parent = parent(node)?;
        let (width, height) = if matches!(node.shape.as_str(), "fork" | "join") {
            let direction = parent.map_or(graph.direction, |group| {
                group_directions[group as usize].unwrap_or(graph.direction)
            });
            state_fork_join_painted_dimensions(match direction {
                Direction::Down => RankDir::TB,
                Direction::Up => RankDir::BT,
                Direction::Right => RankDir::LR,
                Direction::Left => RankDir::RL,
            })
        } else {
            state_node_dimensions(node, &settings, style_plan, measurer, sidecar)?
        };
        let size = Size { width, height };
        let profile = port_profile(node, size, style_plan);
        graph.nodes.push(MeasuredNode {
            size,
            seq,
            parent,
            port_span: Some(profile.port_span(face)),
            perpendicular_port_span: Some(profile.port_span(perpendicular)),
            anchor_y: profile.anchor_y(),
        });
        profiles.push(profile);
    }

    let endpoint = |id, edge_id| {
        endpoints
            .get(canonical(id))
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing State endpoint `{id}` for edge `{edge_id}`"),
            })
    };
    let mut edges = Vec::with_capacity(model.edges.len());
    for edge in &model.edges {
        work.charge_adapter(0)?;
        if hidden.is_hidden(&edge.id)
            || hidden.is_hidden(&edge.start)
            || hidden.is_hidden(&edge.end)
        {
            continue;
        }
        let source = endpoint(&edge.start, &edge.id)?;
        let target = endpoint(&edge.end, &edge.id)?;
        let label = edge.label.trim();
        let label = (!label.is_empty()).then(|| {
            let (width, height) = edge_label_metrics(
                &edge.id,
                label,
                measurer,
                style_plan
                    .edge(&edge.id)
                    .map(StateEdgeStylePlan::resolved_label_typography)
                    .unwrap_or_else(|| style_plan.transition_label_typography()),
                settings.wrap_mode,
                sidecar,
            );
            Size { width, height }
        });
        graph.edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label,
            start_label: None,
            end_label: None,
            start_marker: Default::default(),
            end_marker: crate::svg::state_canonical_marker(
                &edge.arrow_type_end,
                style_plan.compatibility().is_neo(),
            ),
        });
        edges.push(edge);
    }
    work.charge_adapter(0)?;
    Ok(MeasuredState {
        graph,
        leaves,
        groups,
        edges,
        profiles,
    })
}

fn restore_concurrent_directions(
    groups: &[StateGroup<'_>],
    measured: &mut [MeasuredGroup],
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let mut seen_region = vec![false; groups.len()];
    for (region, group) in groups.iter().enumerate() {
        work.charge_adapter(0)?;
        if group.node.shape != "divider" {
            continue;
        }
        let Some(parent) = measured[region].parent else {
            continue;
        };
        let parent = parent as usize;
        if !std::mem::replace(&mut seen_region[parent], true)
            && groups[parent].node.explicit_dir == Some(false)
            && group.node.explicit_dir == Some(true)
        {
            measured[parent].direction = measured[region].direction.take();
        }
    }
    Ok(())
}

fn effective_group_directions(
    graph: &MeasuredGraph,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<Option<Direction>>> {
    let mut directions: Vec<_> = graph.groups.iter().map(|group| group.direction).collect();
    let mut ancestors = Vec::new();
    for start in 0..graph.groups.len() {
        work.charge_adapter(0)?;
        let mut next = Some(start);
        let direction = loop {
            let Some(group) = next else {
                break graph.direction;
            };
            if let Some(direction) = directions[group] {
                break direction;
            }
            if ancestors.len() == graph.groups.len() {
                return Err(Error::InvalidModel {
                    message: "State note-group aliases form a parent cycle".into(),
                });
            }
            if ancestors.len().is_multiple_of(128) {
                work.charge_adapter(0)?;
            }
            ancestors.push(group);
            next = graph.groups[group].parent.map(|parent| parent as usize);
        };
        for group in ancestors.drain(..) {
            directions[group] = Some(direction);
        }
    }
    Ok(directions)
}

fn note_group_aliases(model: &StateDiagramModel) -> HashMap<&str, &str> {
    let declared_groups = || {
        model.nodes.iter().filter_map(|node| {
            let id = node.id.as_str();
            (node.shape == "noteGroup")
                .then(|| note_group_owner_id(id).map(|owner| (owner, id)))
                .flatten()
        })
    };
    let last_by_owner: HashMap<_, _> = declared_groups().collect();
    declared_groups()
        .map(|(owner, id)| (id, last_by_owner[owner]))
        .collect()
}

fn port_profile(node: &StateNode, size: Size, style_plan: &StateStylePlan) -> ShapePortProfile {
    let Size { width, height } = size;
    let radius = style_plan
        .node(&node.id)
        .and_then(StateNodeStylePlan::radius_override);
    match node.shape.as_str() {
        "stateStart" | "stateEnd" => ShapePortProfile::ellipse(width, height),
        "choice" => ShapePortProfile::diamond(width, height),
        "rect" | "rectWithTitle" | "note" => {
            let default_radius = match node.shape.as_str() {
                "rect" if style_plan.compatibility().diagram_look() == "handDrawn" => 5.0,
                "rect" => style_plan.compatibility().rect_radius,
                _ => 0.0,
            };
            let radius = radius
                .unwrap_or(default_radius)
                .min(width.min(height) / 2.0);
            ShapePortProfile::rounded_rectangle(width, height, radius, radius)
        }
        _ => ShapePortProfile::rectangle(width, height),
    }
}

impl MeasuredState<'_> {
    fn id(&self, endpoint: Endpoint) -> &str {
        match endpoint {
            Endpoint::Node(node) => &self.leaves[node as usize].1.id,
            Endpoint::Group(group) => &self.groups[group as usize].node.id,
        }
    }
}

fn positioned_node(id: &str, rect: Rect, is_cluster: bool) -> LayoutNode {
    let center = rect.center();
    LayoutNode {
        id: id.to_owned(),
        x: center.x,
        y: center.y,
        width: rect.width,
        height: rect.height,
        is_cluster,
        label_width: None,
        label_height: None,
    }
}

fn project(
    input: MeasuredState<'_>,
    solved: SolvedGraph,
    work: &mut OperationLayoutWorkControl,
) -> Result<StateDiagramLayout> {
    let node_count = work.checked_add(solved.nodes.len(), solved.groups.len())?;
    work.charge_adapter(work.checked_add(node_count, solved.edges.len())?)?;
    let mut nodes = Vec::with_capacity(node_count);
    for ((_, leaf), &rect) in input.leaves.iter().zip(&solved.nodes) {
        nodes.push(positioned_node(&leaf.id, rect, false));
    }
    let mut clusters = Vec::with_capacity(input.groups.len());
    for ((group, measured), placed) in input
        .groups
        .iter()
        .zip(&input.graph.groups)
        .zip(&solved.groups)
    {
        let node = positioned_node(&group.node.id, placed.rect, true);
        let title_label = placed.title.clone().unwrap_or(LayoutLabel {
            x: node.x,
            y: placed.rect.y,
            width: 0.0,
            height: 0.0,
        });
        clusters.push(LayoutCluster {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            diff: 0.0,
            offset_y: 0.0,
            title: group.title.clone(),
            title_margin_top: title_label.y - title_label.height / 2.0 - placed.rect.y,
            title_margin_bottom: 0.0,
            title_label,
            requested_dir: measured.direction.map(direction_name).map(str::to_owned),
            effective_dir: direction_name(placed.direction).to_owned(),
            padding: group.padding,
        });
        nodes.push(node);
    }

    let contact = |endpoint, side, point| match endpoint {
        Endpoint::Node(node) => {
            let index = node as usize;
            let center = solved.nodes[index].center();
            input.profiles[index].contact(
                &LayoutPoint {
                    x: center.x,
                    y: center.y,
                },
                side,
                point,
            )
        }
        Endpoint::Group(_) => point,
    };
    let mut edges = Vec::with_capacity(input.edges.len());
    for ((edge, spec), placed) in input.edges.iter().zip(&input.graph.edges).zip(solved.edges) {
        work.charge_adapter(placed.points.len())?;
        let from = input.id(spec.source).to_owned();
        let to = input.id(spec.target).to_owned();
        let mut points = placed.points;
        if let [start, .., end] = points.as_mut_slice() {
            *start = contact(placed.source, placed.ports.0, *start);
            *end = contact(placed.target, placed.ports.1, *end);
        }
        edges.push(LayoutEdge {
            id: edge.id.clone(),
            from_cluster: matches!(spec.source, Endpoint::Group(_)).then(|| from.clone()),
            to_cluster: matches!(spec.target, Endpoint::Group(_)).then(|| to.clone()),
            from,
            to,
            points: points
                .into_iter()
                .map(|RoutePoint { x, y, .. }| LayoutPoint { x, y })
                .collect(),
            label: placed.label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }
    Ok(StateDiagramLayout {
        bounds: (!nodes.is_empty()).then_some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: solved.bounds.width,
            max_y: solved.bounds.height,
        }),
        nodes,
        edges,
        clusters,
        uses_elk_adapter_dom: false,
        elk_edge_paths: HashMap::new(),
    })
}
