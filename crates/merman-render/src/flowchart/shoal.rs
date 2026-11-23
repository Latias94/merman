//! Flat Flowchart projection for the experimental Shoal backend.

use super::measured::FlowchartMeasurement;
use super::{
    FlowchartEdgeOwners, FlowchartRenderModelRef, FlowchartSvgLabelOwner,
    FlowchartSvgLabelSidecarBuilder,
};
use crate::model::{Bounds, FlowchartLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::{Error, LayoutExecution, Result};
use merman_layout_shoal as shoal;
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
use std::collections::HashMap;

pub(crate) fn layout(
    model: &FlowchartModel,
    render_context: &FlowchartRenderContext,
    config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_styles: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<FlowchartLayout> {
    let meter = execution.work_meter_ref();
    let charge = |units| meter.charge(units).map_err(Error::from);
    charge(0)?;
    if !model.subgraphs.is_empty() {
        return Err(Error::UnsupportedLayoutConstruct {
            backend: "shoal",
            construct: "subgraphs",
        });
    }
    charge(model.nodes.len())?;
    super::validate_flowchart_model_shapes(model)?;
    let view = FlowchartRenderModelRef::new(model, render_context);
    let measurement = FlowchartMeasurement::new(
        view,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        labels,
        edge_styles,
    );
    let (direction, direction_name) = match model
        .direction
        .as_deref()
        .unwrap_or("TB")
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "LR" => (shoal::Direction::Right, "LR"),
        "RL" => (shoal::Direction::Left, "RL"),
        "BT" => (shoal::Direction::Up, "BT"),
        "TB" | "TD" => (shoal::Direction::Down, "TB"),
        other => {
            return Err(Error::InvalidModel {
                message: format!("Unsupported Shoal Flowchart direction `{other}`"),
            });
        }
    };
    charge(model.nodes.len())?;
    charge(model.edges.len())?;
    let mut graph = shoal::Graph {
        direction,
        node_spacing: measurement.settings.nodesep,
        rank_spacing: measurement.settings.ranksep,
        nodes: Vec::with_capacity(model.nodes.len()),
        edges: Vec::with_capacity(model.edges.len()),
    };
    let mut node_ids = HashMap::with_capacity(model.nodes.len());
    let mut nodes = Vec::with_capacity(model.nodes.len());
    for (index, node) in model.nodes.iter().enumerate() {
        charge(node.id.len())?;
        if node_ids
            .insert(node.id.as_str(), shoal::NodeId(index))
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!("Duplicate Shoal Flowchart node `{}`", node.id),
            });
        }
        super::validate_flowchart_node_asset_dimensions(node)?;
        let size = measurement.node(
            node,
            FlowchartSvgLabelOwner::Node(index),
            direction_name,
            charge,
        )?;
        graph.nodes.push(shoal::Size {
            width: size.width,
            height: size.height,
        });
        nodes.push(LayoutNode {
            id: node.id.clone(),
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
            is_cluster: false,
            label_width: Some(size.label.width),
            label_height: Some(size.label.height),
        });
    }

    let mut owners = Vec::with_capacity(model.edges.len());
    for (key, projected) in view.projected_edges() {
        let endpoint = |id: &str| {
            node_ids
                .get(id)
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "Missing node `{id}` for Shoal Flowchart edge `{}`",
                        projected.semantic.id
                    ),
                })
        };
        charge(1)?;
        let edge = projected.semantic;
        let label = measurement.edge_label(edge, key)?;
        let (start_clearance, end_clearance) =
            crate::svg::flowchart_marker_clearances(edge.edge_type.as_deref());
        graph.edges.push(shoal::Edge {
            source: endpoint(projected.from)?,
            target: endpoint(projected.to)?,
            min_rank_span: edge.length.max(1),
            label: label.map(|label| shoal::Size {
                width: label.width,
                height: label.height,
            }),
            start_clearance,
            end_clearance,
        });
        owners.push(key);
    }

    let solved = shoal::layout_with_control(&graph, charge).map_err(|error| match error {
        shoal::LayoutError::Interrupted(error) => error,
        shoal::LayoutError::Overflow => meter.arithmetic_overflow().into(),
        shoal::LayoutError::Unsupported(construct) => Error::UnsupportedLayoutConstruct {
            backend: "shoal",
            construct,
        },
        shoal::LayoutError::InvalidInput(detail) => Error::InvalidModel {
            message: format!("Invalid Shoal Flowchart: {detail}"),
        },
    })?;
    charge(nodes.len())?;
    for (node, center) in nodes.iter_mut().zip(solved.centers) {
        node.x = center.x;
        node.y = center.y;
    }
    charge(owners.len())?;
    let mut edges = Vec::with_capacity(owners.len());
    for ((key, measured), route) in owners.iter().zip(graph.edges).zip(solved.edges) {
        charge(route.points.len())?;
        let edge = &model.edges[key.semantic_index()];
        let label = measured
            .label
            .zip(route.label_center)
            .map(|(size, center)| LayoutLabel {
                x: center.x,
                y: center.y,
                width: size.width,
                height: size.height,
            });
        edges.push(LayoutEdge {
            id: edge.id.clone(),
            from: nodes[measured.source.0].id.clone(),
            to: nodes[measured.target.0].id.clone(),
            from_cluster: None,
            to_cluster: None,
            points: route
                .points
                .into_iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect(),
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }
    let bounds = (!nodes.is_empty()).then_some(Bounds {
        min_x: solved.bounds.min_x,
        min_y: solved.bounds.min_y,
        max_x: solved.bounds.max_x,
        max_y: solved.bounds.max_y,
    });
    Ok(FlowchartLayout {
        nodes,
        edges,
        clusters: Vec::new(),
        edge_owners: FlowchartEdgeOwners::new(owners),
        bounds,
        dom_node_order_by_root: HashMap::new(),
        uses_elk_adapter_dom: false,
    })
}
