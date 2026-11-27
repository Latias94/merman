use super::{ErLayoutSettings, ErPreparedLabels, ErPreparedLayout, ErSubgraph, er_marker_id};
use crate::graph_layout::canonical::{CanonicalRoutes, CaptionClearance, theme_routing_style};
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Rect,
    RoutingStyle, Side, Size, SolvedGraph, TitlePosition, direction_from_rank,
};
use crate::graph_layout::placement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{
    Bounds, ErDiagramLayout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use kurbo::Insets;
use merman_model::families::er::ErDiagramModel;
use rustc_hash::FxHashMap;
use std::sync::Arc;

const GROUP_PADDING: f64 = 8.0;

struct MeasuredEr<'a> {
    graph: MeasuredGraph,
    entity_ids: Vec<&'a str>,
    groups: Vec<&'a ErSubgraph>,
    edge_ids: Vec<String>,
    profiles: Vec<ShapePortProfile>,
}

pub(super) fn layout(
    model: &ErDiagramModel,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    prepared_labels: Arc<ErPreparedLabels>,
    neo: bool,
    execution: Option<&crate::LayoutExecution<'_>>,
    work: &mut OperationLayoutWorkControl,
) -> Result<ErPreparedLayout> {
    let native = execution.and_then(crate::LayoutExecution::native_family_theme);
    let routing = theme_routing_style(native.map(|theme| theme.form.edge_style));
    let mut input = measure(
        model,
        measurer,
        settings,
        &prepared_labels,
        neo,
        routing,
        work,
    )?;
    let group_order = parent_first_groups(&input, work)?;
    reserve_marker_faces(
        &mut input,
        &group_order,
        native.map_or(0.0, |theme| theme.form.radius),
        work,
    )?;
    let (solved, placement) = placement::solve(
        &input.graph,
        input.entity_ids.iter().copied(),
        input.groups.iter().map(|group| group.id.as_str()),
        execution,
        Default::default(),
        work,
    )?;
    let mut canonical_routes = CanonicalRoutes::from_solved(
        &input.graph,
        &solved,
        &input.profiles,
        |_| 0.0,
        input.edge_ids.iter().map(String::as_str),
        CaptionClearance::Avoid,
        work,
    )?;
    canonical_routes.placement = placement;
    Ok(ErPreparedLayout {
        layout: project(model, input, solved, group_order, prepared_labels, work)?,
        route_contract: crate::graph_layout::RouteContract::canonical_graph(canonical_routes),
    })
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
    model: &'a ErDiagramModel,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
    prepared_labels: &ErPreparedLabels,
    neo: bool,
    routing: RoutingStyle,
    work: &mut OperationLayoutWorkControl,
) -> Result<MeasuredEr<'a>> {
    let group_count = model.subgraphs.len();
    let entity_count = model.entities.len();
    let edge_count = model.relationships.len();
    let endpoint_count = work.checked_add(group_count, entity_count)?;
    let index = |index: usize, work: &OperationLayoutWorkControl| {
        u32::try_from(index).map_err(|_| Error::from(work.record_arithmetic_overflow()))
    };
    let mut graph = MeasuredGraph {
        direction: settings.direction,
        routing,
        node_spacing: settings.node_spacing,
        rank_spacing: settings.rank_spacing,
        component_spacing: settings.node_spacing.max(settings.rank_spacing),
        inherit_crossed_group_direction: false,
        nodes: Vec::with_capacity(entity_count),
        groups: Vec::with_capacity(group_count),
        edges: Vec::with_capacity(edge_count),
    };
    let mut endpoints = FxHashMap::with_capacity_and_hasher(endpoint_count, Default::default());
    let groups: Vec<_> = model.subgraphs.iter().rev().collect();
    for (ordinal, subgraph) in groups.iter().enumerate() {
        work.charge_adapter(subgraph.title.len())?;
        let seq = index(ordinal, work)?;
        if endpoints
            .insert(subgraph.id.as_str(), Endpoint::Group(seq))
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!("duplicate ER subgraph {}", subgraph.id),
            });
        }
        let metrics = super::er_subgraph_title_metrics(
            subgraph,
            measurer,
            &settings.label_style,
            settings.relationship_html_labels,
        );
        work.charge_adapter(0)?;
        let title = (metrics.width > 0.0 && metrics.height > 0.0).then_some(Size {
            width: metrics.width,
            height: metrics.height,
        });
        graph.groups.push(MeasuredGroup {
            seq,
            parent: None,
            direction: subgraph.dir.as_deref().map(direction_from_rank),
            padding: Insets::new(
                GROUP_PADDING,
                title.map_or(GROUP_PADDING, |size| size.height + 2.0 * GROUP_PADDING),
                GROUP_PADDING,
                GROUP_PADDING,
            ),
            min_size: Size {
                width: 2.0 * GROUP_PADDING,
                height: 2.0 * GROUP_PADDING,
            },
            title,
            title_position: TitlePosition::TopCenter,
        });
    }

    let mut entity_ids = Vec::with_capacity(entity_count);
    let mut profiles = Vec::with_capacity(entity_count);
    let mut entity_by_name = FxHashMap::with_capacity_and_hasher(entity_count, Default::default());
    let (face, perpendicular) = match settings.direction {
        Direction::Down | Direction::Up => (Side::Top, Side::Left),
        Direction::Right | Direction::Left => (Side::Left, Side::Top),
    };
    for (name, entity) in &model.entities {
        work.charge_adapter(0)?;
        if matches!(endpoints.get(name.as_str()), Some(Endpoint::Group(_))) {
            continue;
        }
        let node = index(graph.nodes.len(), work)?;
        if endpoints
            .insert(entity.id.as_str(), Endpoint::Node(node))
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!("duplicate ER endpoint {}", entity.id),
            });
        }
        let measure = prepared_labels
            .entity(&entity.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared labels are missing entity {}", entity.id),
            })?;
        let size = Size {
            width: measure.width.max(1.0),
            height: measure.height.max(1.0),
        };
        let profile = ShapePortProfile::rectangle(size.width, size.height);
        let seq = index(work.checked_add(group_count, graph.nodes.len())?, work)?;
        graph.nodes.push(MeasuredNode {
            size,
            contour: profile.shoal_contour(size),
            seq,
            parent: None,
            port_span: Some(profile.port_span(face)),
            perpendicular_port_span: Some(profile.port_span(perpendicular)),
            anchor_y: None,
        });
        profiles.push(profile);
        entity_ids.push(entity.id.as_str());
        entity_by_name.insert(name.as_str(), node);
    }

    for (ordinal, subgraph) in groups.iter().enumerate().rev() {
        let parent = index(ordinal, work)?;
        for member in &subgraph.nodes {
            work.charge_adapter(0)?;
            if let Some(&Endpoint::Group(child)) = endpoints.get(member.as_str()) {
                if child != parent {
                    graph.groups[child as usize].parent = Some(parent);
                }
            } else if let Some(&child) = entity_by_name.get(member.as_str()) {
                graph.nodes[child as usize].parent = Some(parent);
            }
        }
    }

    let mut edge_ids = Vec::with_capacity(edge_count);
    for (index, relationship) in model.relationships.iter().enumerate() {
        work.charge_adapter(0)?;
        let (Some(&source), Some(&target)) = (
            endpoints.get(relationship.entity_a.as_str()),
            endpoints.get(relationship.entity_b.as_str()),
        ) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    relationship.entity_a, relationship.entity_b
                ),
            });
        };
        let (width, height) = prepared_labels
            .relationship(index)
            .map(super::ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        graph.edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: (width > 0.0 && height > 0.0).then_some(Size { width, height }),
            start_label: None,
            end_label: None,
            start_marker: crate::svg::er_canonical_marker(&relationship.rel_spec.card_b, true, neo),
            end_marker: crate::svg::er_canonical_marker(&relationship.rel_spec.card_a, false, neo),
        });
        edge_ids.push(format!("er-rel-{index}"));
    }
    Ok(MeasuredEr {
        graph,
        entity_ids,
        groups,
        edge_ids,
        profiles,
    })
}

#[derive(Clone, Copy, Default)]
struct MarkerDemand {
    count: usize,
    max_width: f64,
}

fn reserve_marker_faces(
    input: &mut MeasuredEr<'_>,
    group_order: &[usize],
    radius: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let graph = &mut input.graph;
    let mut group_directions = vec![graph.direction; graph.groups.len()];
    for &index in group_order {
        work.charge_adapter(0)?;
        let group = &graph.groups[index];
        group_directions[index] = group.direction.unwrap_or_else(|| {
            group
                .parent
                .map_or(graph.direction, |parent| group_directions[parent as usize])
        });
    }
    let spacing = merman_layout_shoal::Spacing::default();
    let mut demands = vec![MarkerDemand::default(); graph.nodes.len()];
    for edge in &graph.edges {
        work.charge_adapter(0)?;
        if edge.source == edge.target {
            // Shoal allocates self-loops on its separate boundary routes.
            continue;
        }
        for (endpoint, marker) in [
            (edge.source, edge.start_marker),
            (edge.target, edge.end_marker),
        ] {
            if let Endpoint::Node(index) = endpoint {
                let demand = &mut demands[index as usize];
                demand.count = work.checked_add(demand.count, 1)?;
                demand.max_width = demand.max_width.max(marker.width);
            }
        }
    }
    for ((node, profile), demand) in graph.nodes.iter_mut().zip(&mut input.profiles).zip(demands) {
        work.charge_adapter(0)?;
        if demand.count < 2 || demand.max_width <= 0.0 {
            continue;
        }
        // Backedges can be placed on the same side as outgoing edges, so reserve for incident
        // ends together. Shoal's main port fan uses a 4 px gap between marker footprints.
        let pitch = spacing.port_pitch.max(demand.max_width + 4.0);
        let span = (demand.count - 1) as f64 * pitch;
        let inset = radius.max(spacing.corner_clearance) + demand.max_width / 2.0;
        let minimum = span + 2.0 * inset;
        let direction = node
            .parent
            .map_or(graph.direction, |parent| group_directions[parent as usize]);
        let face = match direction {
            Direction::Down | Direction::Up => {
                node.size.width = node.size.width.max(minimum);
                Side::Top
            }
            Direction::Right | Direction::Left => {
                node.size.height = node.size.height.max(minimum);
                Side::Left
            }
        };
        *profile = ShapePortProfile::rectangle(node.size.width, node.size.height);
        node.contour = profile.shoal_contour(node.size);
        let horizontal_face = if face == Side::Top {
            (node.size.width - 2.0 * inset).max(span)
        } else {
            profile.port_span(Side::Top)
        };
        let vertical_face = if face == Side::Left {
            (node.size.height - 2.0 * inset).max(span)
        } else {
            profile.port_span(Side::Left)
        };
        let (main, perpendicular) = match graph.direction {
            Direction::Down | Direction::Up => (horizontal_face, vertical_face),
            Direction::Right | Direction::Left => (vertical_face, horizontal_face),
        };
        node.port_span = Some(main);
        node.perpendicular_port_span = Some(perpendicular);
    }
    Ok(())
}

fn parent_first_groups(
    input: &MeasuredEr<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<usize>> {
    let groups = &input.graph.groups;
    work.charge_adapter(work.checked_mul(groups.len(), 3)?)?;
    let mut order = Vec::with_capacity(groups.len());
    let mut children = vec![Vec::new(); groups.len()];
    for (index, group) in groups.iter().enumerate() {
        work.charge_adapter(0)?;
        if let Some(parent) = group.parent {
            children[parent as usize].push(index);
        } else {
            order.push(index);
        }
    }
    let mut next = 0;
    while let Some(&parent) = order.get(next) {
        work.charge_adapter(0)?;
        order.extend_from_slice(&children[parent]);
        next += 1;
    }
    if order.len() != groups.len() {
        return Err(Error::InvalidModel {
            message: "ER parent cycle in subgraphs".into(),
        });
    }
    Ok(order)
}

fn layout_node(id: &str, rect: Rect, is_cluster: bool) -> LayoutNode {
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
    model: &ErDiagramModel,
    input: MeasuredEr<'_>,
    solved: SolvedGraph,
    group_order: Vec<usize>,
    prepared_labels: Arc<ErPreparedLabels>,
    work: &mut OperationLayoutWorkControl,
) -> Result<ErDiagramLayout> {
    let node_count = work.checked_add(input.entity_ids.len(), input.groups.len())?;
    work.charge_adapter(work.checked_add(node_count, input.edge_ids.len())?)?;
    let mut nodes = Vec::with_capacity(node_count);
    for (id, rect) in input.entity_ids.into_iter().zip(solved.nodes) {
        work.charge_adapter(id.len())?;
        nodes.push(layout_node(id, rect, false));
    }
    let mut clusters = Vec::with_capacity(input.groups.len());
    for index in group_order {
        let subgraph = input.groups[index];
        work.charge_adapter(work.checked_add(subgraph.id.len(), subgraph.title.len())?)?;
        let group = &solved.groups[index];
        let node = layout_node(&subgraph.id, group.rect, true);
        let title_label = group.title.clone().unwrap_or(LayoutLabel {
            x: node.x,
            y: group.rect.y + GROUP_PADDING,
            width: 0.0,
            height: 0.0,
        });
        clusters.push(LayoutCluster {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            diff: (title_label.width - node.width) / 2.0 - GROUP_PADDING / 2.0,
            offset_y: title_label.height - GROUP_PADDING / 2.0,
            title: subgraph.title.clone(),
            title_label,
            requested_dir: subgraph.dir.clone(),
            effective_dir: direction_name(group.direction).to_owned(),
            padding: GROUP_PADDING,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
        });
        nodes.push(node);
    }
    let mut edges = Vec::with_capacity(input.edge_ids.len());
    for (((id, relationship), measured), edge) in input
        .edge_ids
        .into_iter()
        .zip(&model.relationships)
        .zip(input.graph.edges)
        .zip(solved.edges)
    {
        work.charge_adapter(work.checked_add(
            edge.points.len(),
            work.checked_add(relationship.entity_a.len(), relationship.entity_b.len())?,
        )?)?;
        edges.push(LayoutEdge {
            id,
            from: relationship.entity_a.clone(),
            to: relationship.entity_b.clone(),
            from_cluster: matches!(measured.source, Endpoint::Group(_))
                .then(|| relationship.entity_a.clone()),
            to_cluster: matches!(measured.target, Endpoint::Group(_))
                .then(|| relationship.entity_b.clone()),
            points: edge
                .points
                .into_iter()
                .map(|point| LayoutPoint {
                    x: point.x,
                    y: point.y,
                })
                .collect(),
            label: edge.label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: er_marker_id(&relationship.rel_spec.card_b, "START"),
            end_marker: er_marker_id(&relationship.rel_spec.card_a, "END"),
            stroke_dasharray: (relationship.rel_spec.rel_type == "NON_IDENTIFYING")
                .then(|| "8,8".into()),
        });
    }
    let bounds = (!nodes.is_empty()).then_some(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    });
    Ok(ErDiagramLayout {
        nodes,
        edges,
        render_edges: Vec::new(),
        swimlane: None,
        clusters,
        bounds,
        prepared_labels,
    })
}
