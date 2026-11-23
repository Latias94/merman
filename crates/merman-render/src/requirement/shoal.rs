use super::{
    RequirementCssBinding, RequirementEdgeLabelPlan, RequirementMeasurements,
    RequirementNodeRenderPlan, RequirementPreparedArtifact, measure_requirement,
    measure_requirement_relationship, rank_dir_from,
};
use crate::graph_layout::canonical::CanonicalRoutes;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, Marker, MeasuredEdge, MeasuredGraph, MeasuredNode, RoutingStyle, Side,
    Size,
};
use crate::graph_layout::shoal_bridge::{self, LayoutScratch};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, RequirementDiagramLayout};
use crate::resources::OperationWorkMeter;
use crate::text::TextMeasurer;
use crate::{Error, Result};
use dugong::{RankDir, graphlib::EdgeKey};
use merman_model::MermaidConfig;
use merman_model::families::requirement::RequirementDiagramModel;
use std::{collections::HashMap, sync::Arc};

pub(super) fn layout(
    model: &RequirementDiagramModel,
    config: &MermaidConfig,
    text_measurer: &dyn TextMeasurer,
    css_binding: &RequirementCssBinding,
    work_meter: &Arc<OperationWorkMeter>,
) -> Result<RequirementPreparedArtifact> {
    let mut work = OperationLayoutWorkControl::new(Arc::clone(work_meter));
    let node_count = work.checked_add(model.requirements.len(), model.elements.len())?;
    work.charge_adapter(work.checked_add(node_count, model.relationships.len())?)?;
    let settings = &css_binding.layout;
    let RequirementMeasurements {
        nodes,
        styles,
        binding: measurement_binding,
    } = measure_requirement(model, config, text_measurer, settings, work_meter)?;
    u32::try_from(nodes.len()).map_err(|_| work.record_arithmetic_overflow())?;

    let (direction, face, perpendicular) = match rank_dir_from(&model.direction) {
        RankDir::TB => (Direction::Down, Side::Top, Side::Left),
        RankDir::BT => (Direction::Up, Side::Top, Side::Left),
        RankDir::LR => (Direction::Right, Side::Left, Side::Top),
        RankDir::RL => (Direction::Left, Side::Left, Side::Top),
    };
    let mut graph = MeasuredGraph {
        direction,
        routing: RoutingStyle::Curved,
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
            seq: index as u32,
            parent: None,
            port_span: Some(profile.port_span(face)),
            perpendicular_port_span: Some(profile.port_span(perpendicular)),
            anchor_y: None,
        });
        profiles.push(profile);
    }

    let effective_config = config.as_value();
    let neo = crate::config::config_diagram_look(effective_config).is_neo();
    let stroke_width = super::RequirementConfigView::new(effective_config).canonical_stroke_width();
    let (contains_marker, arrow_marker) = relation_markers(neo, stroke_width);
    let endpoint = |id: &str, terminal: &str| {
        nodes
            .get_index_of(id)
            .map(|index| Endpoint::Node(index as u32))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("relationship {terminal} node not found: {id}"),
            })
    };
    let mut plans = Vec::with_capacity(graph.edges.capacity());
    for (index, relationship) in model.relationships.iter().enumerate() {
        work.charge_adapter(work.checked_add(
            relationship.rel_type.len(),
            work.checked_add(relationship.src.len(), relationship.dst.len())?,
        )?)?;
        let source = endpoint(&relationship.src, "src")?;
        let target = endpoint(&relationship.dst, "dst")?;
        let (display, metrics) =
            measure_requirement_relationship(relationship, text_measurer, &styles)?;
        let plan = RequirementEdgeLabelPlan::for_relationship(relationship, index, display);
        let (start_marker, end_marker) = if plan.marker_start {
            (contains_marker, Marker::default())
        } else {
            (Marker::default(), arrow_marker)
        };
        graph.edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: Some(Size {
                width: metrics.width,
                height: metrics.height,
            }),
            start_label: None,
            end_label: None,
            start_marker,
            end_marker,
        });
        plans.push(plan);
    }

    let solved = shoal_bridge::solve(
        &graph,
        Default::default(),
        &mut LayoutScratch::default(),
        &mut work,
    )?;
    let canonical_routes = CanonicalRoutes::from_solved(
        &graph,
        &solved,
        &profiles,
        plans.iter().map(|plan| plan.rendered_id.as_str()),
        &mut work,
    )?;
    let mut layout_nodes = Vec::with_capacity(nodes.len());
    let mut prepared_nodes = HashMap::with_capacity(nodes.len());
    for ((id, measured), rect) in nodes.into_iter().zip(solved.nodes) {
        work.charge_adapter(id.len())?;
        layout_nodes.push(LayoutNode {
            id: id.to_owned(),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
        prepared_nodes.insert(
            id.to_owned(),
            RequirementNodeRenderPlan::Semantic(measured.label),
        );
    }

    let mut layout_edges = Vec::with_capacity(plans.len());
    let mut prepared_edges = HashMap::with_capacity(plans.len());
    for ((relationship, plan), edge) in model.relationships.iter().zip(plans).zip(solved.edges) {
        work.charge_adapter(work.checked_add(edge.points.len(), plan.rendered_id.len())?)?;
        let key = EdgeKey::new(
            &relationship.src,
            &relationship.dst,
            Some(&plan.rendered_id),
        );
        layout_edges.push(LayoutEdge {
            id: plan.rendered_id.clone(),
            from: relationship.src.clone(),
            to: relationship.dst.clone(),
            from_cluster: None,
            to_cluster: None,
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
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
        prepared_edges.insert(key, plan);
    }

    let bounds = (!layout_nodes.is_empty()).then_some(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: solved.bounds.width,
        max_y: solved.bounds.height,
    });
    Ok(RequirementPreparedArtifact {
        layout: RequirementDiagramLayout {
            nodes: layout_nodes,
            edges: layout_edges,
            bounds,
        },
        render_layout: None,
        canonical_routes: Some(canonical_routes),
        nodes: prepared_nodes,
        edges: prepared_edges,
        measurement_binding,
    })
}

fn relation_markers(neo: bool, stroke_width: f64) -> (Marker, Marker) {
    let half_stroke = stroke_width / 2.0;
    let contains_radius = (9.0 + half_stroke).min(10.0);
    let contains = Marker {
        trim: 0.0,
        depth: 10.0 + contains_radius,
        width: 2.0 * contains_radius,
    };
    let arrow_trim = half_stroke / 5.0_f64.sqrt();
    let arrow_half_width = 10.0 + 2.0 * arrow_trim;
    let arrow = if neo {
        let scale = 20.0 / 25.0;
        Marker {
            trim: arrow_trim.min(5.0) * scale,
            depth: 20.0 * scale,
            width: 2.0 * arrow_half_width.min(12.5) * scale,
        }
    } else {
        Marker {
            trim: 0.0,
            depth: 20.0,
            width: 20.0,
        }
    };
    (contains, arrow)
}
