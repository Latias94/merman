//! Requirement projection of Mermaid 12's registered ELK renderer.
//!
//! Source: packages/mermaid/src/diagrams/requirement/requirementRenderer.ts and
//! rendering-util/layout-algorithms/elk/render.ts at 98a0945418c76238f15df2afaddbba4272656c3b.

use super::{
    RequirementEdgeLabels, RequirementMeasurements, RequirementNodeRenderPlan,
    RequirementPreparedArtifact, bounds_for_nodes_edges, measure_requirement_relationship,
    requirement_layout_node_from_center, validate_relationship_endpoints,
};
use crate::elk_edge_geometry::{self as geometry, Outline, Shape};
use crate::graph_layout::ir::{Direction, Size};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{LayoutLabel, LayoutPoint, RequirementDiagramLayout};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use indexmap::IndexMap;
use merman_layout_elk as elk;
use merman_model::families::requirement::RequirementRelationship;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn layout(
    relationships: &[RequirementRelationship],
    measured: RequirementMeasurements<'_>,
    config: &Value,
    text_measurer: &dyn TextMeasurer,
    operation_seed: elk::ElkOperationSeed,
    work: &mut OperationLayoutWorkControl,
) -> Result<RequirementPreparedArtifact> {
    let RequirementMeasurements {
        direction,
        mut nodes,
        styles,
        binding: measurement_binding,
    } = measured;
    let edges = relationships
        .iter()
        .enumerate()
        .map(|(index, relationship)| {
            validate_relationship_endpoints(relationship, |id| nodes.contains_key(id))?;
            measure_requirement_relationship(relationship, index, text_measurer, &styles)
        })
        .collect::<Result<Vec<_>>>()?;

    let mut input_nodes: Vec<_> = nodes
        .iter()
        .map(|(&id, node)| elk::Node {
            id: id.to_owned(),
            kind: elk::NodeKind::Leaf,
            container: Default::default(),
            label_text: None,
            width: node.width,
            height: node.height,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_alignment: None,
            label: None,
        })
        .collect();
    input_nodes.sort_by_key(|node| node_object_order_key(&node.id));
    let mut input = elk::Graph {
        id: "root".to_owned(),
        direction: match direction {
            Direction::Down => elk::Direction::Down,
            Direction::Up => elk::Direction::Up,
            Direction::Right => elk::Direction::Right,
            Direction::Left => elk::Direction::Left,
        },
        nodes: input_nodes,
        edges: edges
            .iter()
            .map(|edge| {
                let identity = edge.identity();
                let size = edge.parity_label_size();
                elk::Edge {
                    id: identity.name.to_owned(),
                    source: identity.from.to_owned(),
                    target: identity.to.to_owned(),
                    label: Some(elk::Label {
                        width: size.width,
                        height: size.height,
                    }),
                    minlen: 1,
                    terminal_labels: Vec::new(),
                    inside_self_loops_yo: false,
                }
            })
            .collect(),
        // The shared Mermaid ELK renderer sets spacing.baseValue=40, independently of
        // the Requirement renderer's Dagre nodeSpacing/rankSpacing fields.
        spacing: elk::Spacing::default(),
        options: crate::elk_options::layout_options(config),
    };
    if crate::config::config_bool(config, &["elk", "keepEntryNodeOnTop"]).unwrap_or(false) {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut input.nodes,
            input
                .edges
                .iter()
                .map(|edge| (edge.source.as_str(), edge.target.as_str())),
            &mut Some(&mut *work),
        )?;
    }
    work.charge_adapter(input.nodes.len().saturating_add(input.edges.len()))?;
    let oriented = crate::elk_feedback_edges::orient_feedback_edges(
        &mut input,
        config,
        &mut Some(&mut *work),
    )?;
    let mut placed =
        elk::layout_with_operation_seed_and_work_control(oriented.graph(), operation_seed, work)
            .map_err(|error| work.map_elk_error_with_context(error, "Requirement ELK"))?;
    oriented.restore(&mut placed, &mut Some(&mut *work))?;

    let mut node_slots: IndexMap<_, _> = input
        .nodes
        .into_iter()
        .map(|node| (node.id, None))
        .collect();
    for placed in placed.nodes {
        let slot = node_slots
            .get_mut(placed.id.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ELK returned unknown Requirement node {}", placed.id),
            })?;
        *slot = Some(requirement_layout_node_from_center(
            placed.id,
            placed.x,
            placed.y,
            placed.width,
            placed.height,
        ));
    }
    let out_nodes: Vec<_> = node_slots.into_values().flatten().collect();

    let edge_indices: HashMap<_, _> = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.identity().name, index))
        .collect();
    let mut edge_slots = vec![None; edges.len()];
    for placed in placed.edges {
        let &index = edge_indices
            .get(placed.id.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ELK returned unknown Requirement edge {}", placed.id),
            })?;
        edge_slots[index] = Some(placed);
    }
    drop(edge_indices);

    let mut out_edges = Vec::with_capacity(edges.len());
    let mut prepared_edges = RequirementEdgeLabels::with_capacity(edges.len());
    let mut label_sizes = Vec::with_capacity(edges.len());
    for (measured, placed) in edges.into_iter().zip(edge_slots) {
        let size = measured.parity_label_size();
        let (points, label) = if let Some(placed) = placed {
            let points = placed
                .points
                .into_iter()
                .map(|point| LayoutPoint {
                    x: point.x,
                    y: point.y,
                })
                .collect();
            let label = placed
                .labels
                .first()
                .filter(|_| size.width > 0.0 && size.height > 0.0)
                .map(|label| LayoutLabel {
                    x: label.x + label.width / 2.0,
                    y: label.y + label.height / 2.0,
                    width: size.width,
                    height: size.height,
                });
            (points, label)
        } else {
            (Vec::new(), None)
        };
        let identity = measured.identity();
        out_edges.push(identity.layout_edge(points, label));
        prepared_edges.insert(identity.into_owned(), measured.plan);
        label_sizes.push(size);
    }

    let mut prepared_nodes = HashMap::with_capacity(out_nodes.len());
    for node in &out_nodes {
        let measured = nodes
            .swap_remove(node.id.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Requirement node label for {}", node.id),
            })?;
        prepared_nodes.insert(
            node.id.clone(),
            RequirementNodeRenderPlan::Semantic(measured.label),
        );
    }
    let bounds = bounds_for_nodes_edges(&out_nodes, &out_edges);
    let layout = RequirementDiagramLayout {
        nodes: out_nodes,
        edges: out_edges,
        bounds,
    };
    let render_layout = render_layout(&layout, label_sizes, config, work)?;
    Ok(RequirementPreparedArtifact {
        layout,
        render_layout: Some(render_layout),
        route_contract: crate::graph_layout::RouteContract::parity(true),
        nodes: prepared_nodes,
        edges: prepared_edges,
        measurement_binding,
    })
}

/// JavaScript array indices precede the other keys, whose declaration order is stable.
fn node_object_order_key(id: &str) -> u32 {
    let bytes = id.as_bytes();
    if bytes.len() > 10
        || (bytes.len() > 1 && bytes[0] == b'0')
        || !bytes.iter().all(u8::is_ascii_digit)
    {
        return u32::MAX;
    }
    id.parse().unwrap_or(u32::MAX)
}

/// Prepare the source renderer's clipped paint routes without rewriting provider sections.
fn render_layout(
    layout: &RequirementDiagramLayout,
    label_sizes: Vec<Size>,
    config: &Value,
    work: &mut OperationLayoutWorkControl,
) -> Result<RequirementDiagramLayout> {
    let mut render = layout.clone();
    // Requirement's public geometry uses top-left nodes; the shared shape adapter uses centers.
    let centers: HashMap<_, _> = layout
        .nodes
        .iter()
        .map(|node| {
            let mut center = node.clone();
            center.x += center.width / 2.0;
            center.y += center.height / 2.0;
            (node.id.as_str(), center)
        })
        .collect();
    let mut routes = Vec::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        work.charge_adapter(edge.points.len().saturating_add(2))?;
        let start = centers
            .get(edge.from.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing Requirement edge source {}", edge.from),
            })?;
        let end = centers
            .get(edge.to.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing Requirement edge target {}", edge.to),
            })?;
        let mut points = Vec::with_capacity(edge.points.len() + 2);
        points.push(LayoutPoint {
            x: start.x,
            y: start.y,
        });
        points.extend_from_slice(&edge.points);
        points.push(LayoutPoint { x: end.x, y: end.y });
        routes.push(geometry::sanitize(
            &points,
            Shape {
                intersection: None,
                node: start,
                outline: Outline::Rect,
            },
            Shape {
                intersection: None,
                node: end,
                outline: Outline::Rect,
            },
        ));
    }
    if config
        .pointer("/elk/straightenEdges")
        .and_then(Value::as_bool)
        != Some(false)
    {
        work.charge_adapter(routes.len())?;
        let mut labelled_routes: Vec<_> = routes
            .iter_mut()
            .zip(render.edges.iter_mut())
            .map(
                |(points, edge)| crate::elk_terminal_jogs::LabeledTerminalRoute {
                    points,
                    label: edge.label.as_mut(),
                },
            )
            .collect();
        crate::elk_terminal_jogs::straighten_edge_terminals(&mut labelled_routes, |units| {
            work.charge_adapter(units)
        })?;
    }
    for ((edge, raw), (points, size)) in render
        .edges
        .iter_mut()
        .zip(&layout.edges)
        .zip(routes.into_iter().zip(label_sizes))
    {
        if raw.points.is_empty() {
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                edge.label = Some(LayoutLabel {
                    x: (first.x + last.x) / 2.0,
                    y: (first.y + last.y) / 2.0,
                    width: size.width,
                    height: size.height,
                });
            }
        }
        edge.points = points;
    }
    crate::elk_terminal_jogs::separate_opposite_edge_labels(
        render
            .edges
            .iter_mut()
            .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.label.as_mut())),
        |units| work.charge_adapter(units),
    )?;
    // The SVG renderer reconstructs final viewport bounds from these painted paths and labels.
    Ok(render)
}
