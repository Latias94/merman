use super::RequirementPreparedArtifact;
use crate::Result;
use crate::graph_layout::canonical::{CanonicalRoutes, CaptionClearance, theme_routing_style};
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, Marker, MeasuredEdge, MeasuredGraph, MeasuredNode, Side, Size,
};
use crate::graph_layout::placement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, LayoutPoint, RequirementDiagramLayout};
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use merman_model::MermaidConfig;
use merman_model::families::requirement::RequirementDiagramModel;
use merman_render_graph::requirement::{
    RequirementConfigView, RequirementCssBinding, RequirementEdgeLabels, RequirementMeasurements,
    RequirementNodeRenderPlan, RequirementPreparedArtifact as GraphPrepared, measure_requirement,
    measure_requirement_relationship, missing_relationship_node, requirement_layout_node,
};
use std::{collections::HashMap, sync::Arc};

pub(super) fn layout(
    model: &RequirementDiagramModel,
    config: &MermaidConfig,
    text_measurer: &dyn TextMeasurer,
    css_binding: &RequirementCssBinding,
    work_meter: &Arc<OperationWorkMeter>,
    execution: Option<&crate::LayoutExecution<'_>>,
) -> Result<RequirementPreparedArtifact> {
    let mut work = OperationLayoutWorkControl::new(Arc::clone(work_meter));
    let node_count = work.checked_add(model.requirements.len(), model.elements.len())?;
    work.charge_adapter(work.checked_add(node_count, model.relationships.len())?)?;
    let settings = &css_binding.layout;
    let RequirementMeasurements {
        direction,
        nodes,
        styles,
        binding: measurement_binding,
    } = measure_requirement(model, config, text_measurer, settings, work_meter)?;
    u32::try_from(nodes.len()).map_err(|_| work.record_arithmetic_overflow())?;

    let (face, perpendicular) = match direction {
        Direction::Down | Direction::Up => (Side::Top, Side::Left),
        Direction::Right | Direction::Left => (Side::Left, Side::Top),
    };
    let mut graph = MeasuredGraph {
        direction,
        routing: theme_routing_style(css_binding.edge_style()),
        node_spacing: settings.nodesep,
        rank_spacing: settings.ranksep,
        component_spacing: settings.nodesep.max(settings.ranksep),
        inherit_crossed_group_direction: false,
        nodes: Vec::with_capacity(nodes.len()),
        groups: Vec::new(),
        edges: Vec::with_capacity(model.relationships.len()),
    };
    let mut profiles = Vec::with_capacity(nodes.len());
    for (index, (id, measured)) in nodes.iter().enumerate() {
        work.charge_adapter(id.len())?;
        let size = Size {
            width: measured.width,
            height: measured.height,
        };
        let profile = ShapePortProfile::rectangle(size.width, size.height);
        graph.nodes.push(MeasuredNode {
            size,
            contour: profile.shoal_contour(size),
            seq: index as u32,
            parent: None,
            port_span: Some(profile.port_span(face)),
            perpendicular_port_span: Some(profile.port_span(perpendicular)),
            anchor_y: None,
        });
        profiles.push(profile);
    }

    let (contains_marker, arrow_marker) =
        css_binding.relation_markers(&RequirementConfigView::new(config.as_value()));
    let endpoint = |id: &str, terminal: &str| {
        nodes
            .get_index_of(id)
            .map(|index| Endpoint::Node(index as u32))
            .ok_or_else(|| missing_relationship_node(terminal, id))
    };
    let mut measured_edges = Vec::with_capacity(graph.edges.capacity());
    for (index, relationship) in model.relationships.iter().enumerate() {
        work.charge_adapter(work.checked_add(
            relationship.rel_type.len(),
            work.checked_add(relationship.src.len(), relationship.dst.len())?,
        )?)?;
        let source = endpoint(&relationship.src, "src")?;
        let target = endpoint(&relationship.dst, "dst")?;
        let edge = measure_requirement_relationship(relationship, index, text_measurer, &styles)?;
        let (start_marker, end_marker) = if edge.plan.marker_start {
            (contains_marker, Marker::default())
        } else {
            (Marker::default(), arrow_marker)
        };
        graph.edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: Some(edge.label_size),
            start_label: None,
            end_label: None,
            start_marker,
            end_marker,
        });
        measured_edges.push(edge);
    }

    let (solved, placement) = placement::solve(
        &graph,
        nodes.keys().copied(),
        std::iter::empty(),
        execution,
        Default::default(),
        &mut work,
    )?;
    let mut canonical_routes = CanonicalRoutes::from_solved(
        &graph,
        &solved,
        &profiles,
        |_| 0.0,
        measured_edges.iter().map(|edge| edge.identity().name),
        CaptionClearance::Avoid,
        &mut work,
    )?;
    canonical_routes.placement = placement;
    let mut layout_nodes = Vec::with_capacity(nodes.len());
    let mut prepared_nodes = HashMap::with_capacity(nodes.len());
    for ((id, measured), rect) in nodes.into_iter().zip(solved.nodes) {
        work.charge_adapter(id.len())?;
        layout_nodes.push(requirement_layout_node(id.to_owned(), rect));
        prepared_nodes.insert(
            id.to_owned(),
            RequirementNodeRenderPlan::Semantic(measured.label),
        );
    }

    let mut layout_edges = Vec::with_capacity(measured_edges.len());
    let mut prepared_edges = RequirementEdgeLabels::with_capacity(measured_edges.len());
    for (measured, edge) in measured_edges.into_iter().zip(solved.edges) {
        let identity = measured.identity();
        work.charge_adapter(work.checked_add(edge.points.len(), identity.name.len())?)?;
        layout_edges.push(
            identity.layout_edge(
                edge.points
                    .into_iter()
                    .map(|point| LayoutPoint {
                        x: point.x,
                        y: point.y,
                    })
                    .collect(),
                edge.label,
            ),
        );
        prepared_edges.insert(identity.into_owned(), measured.plan);
    }

    let bounds = (!layout_nodes.is_empty()).then_some(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    });
    Ok(RequirementPreparedArtifact {
        prepared: GraphPrepared::from_shoal(
            RequirementDiagramLayout {
                nodes: layout_nodes,
                edges: layout_edges,
                bounds,
            },
            prepared_nodes,
            prepared_edges,
            measurement_binding,
        ),
        route_contract: crate::graph_layout::RouteContract::canonical_graph(canonical_routes),
    })
}
