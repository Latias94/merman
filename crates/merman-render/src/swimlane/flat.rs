//! Family-neutral boundary for already-measured Swimlane graphs.

use super::config::{DEFAULT_LANE_ID, DEFAULT_LANE_PADDING, SwimlaneConfig};
use super::working::{WorkingEdge, WorkingLayout, WorkingNode, WorkingNodeKind};
use crate::model::{LayoutEdge, LayoutNode, SwimlaneDirection, SwimlaneLayout};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use indexmap::IndexMap;
#[cfg(feature = "diagram-mindmap")]
use merman_model::presentation::PresentationConfig;
#[cfg(feature = "diagram-er")]
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;

/// Geometry and containment are supplied by the owning family, after its label preparation.
/// `LayoutNode::is_cluster` distinguishes groups without a Flowchart semantic projection.
pub(crate) struct MeasuredNode<'a> {
    pub node: &'a LayoutNode,
    pub shape: &'a str,
    pub label: &'a str,
    pub label_type: &'a str,
    pub parent_id: Option<&'a str>,
    pub requested_dir: Option<&'a str>,
    pub padding: f64,
}

#[cfg(feature = "diagram-mindmap")]
pub(crate) fn layout_flat(
    measured: &[(&LayoutNode, &str)],
    edges: &[LayoutEdge],
    config: &PresentationConfig,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    let measured = measured
        .iter()
        .map(|&(node, shape)| MeasuredNode {
            node,
            shape,
            label: "",
            label_type: "markdown",
            parent_id: None,
            requested_dir: None,
            padding: 0.0,
        })
        .collect::<Vec<_>>();
    layout_measured(
        &measured,
        edges,
        SwimlaneDirection::Tb,
        SwimlaneConfig::from_config(config),
        work_meter,
    )
}

/// `spacing` is the family's renderer projection of (node gap, layer gap). ER's unified
/// renderer supplies its ER spacing here before the upstream Swimlane core reads Flowchart.
#[cfg(feature = "diagram-er")]
pub(crate) fn layout_compound(
    measured: &[MeasuredNode<'_>],
    edges: &[LayoutEdge],
    direction: SwimlaneDirection,
    config: &Value,
    spacing: (f64, f64),
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    use crate::config::config_bool;

    let settings = SwimlaneConfig {
        node_gap: spacing.0,
        layer_gap: spacing.1,
        ignore_cross_lane_edges: config_bool(config, &["swimlane", "ignoreCrossLaneEdges"])
            .unwrap_or(true),
        optimize_ranks_by_crossings: config_bool(config, &["swimlane", "optimizeRanksByCrossings"])
            .unwrap_or(true),
        automatic_lane_ordering: config_bool(config, &["swimlane", "automaticLaneOrdering"])
            .unwrap_or(false),
    };
    layout_measured(measured, edges, direction, settings, work_meter)
}

fn layout_measured(
    measured: &[MeasuredNode<'_>],
    edges: &[LayoutEdge],
    direction: SwimlaneDirection,
    settings: SwimlaneConfig,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    work_meter.preflight(super::swimlane_layout_preflight_work_units(
        measured.len(),
        edges.len(),
    ))?;
    work_meter.charge(super::swimlane_core_layout_work_units(
        measured.len(),
        edges.len(),
    ))?;
    let mut nodes = IndexMap::new();
    for measured in measured {
        let node = measured.node;
        let previous = nodes.insert(
            node.id.clone(),
            WorkingNode {
                id: node.id.clone(),
                label: measured.label.to_owned(),
                label_type: measured.label_type.to_owned(),
                shape: measured.shape.to_owned(),
                kind: if node.is_cluster {
                    WorkingNodeKind::Group
                } else {
                    WorkingNodeKind::Content
                },
                producer_lane_role: None,
                parent_id: measured.parent_id.map(str::to_owned),
                top_lane_id: None,
                requested_dir: measured.requested_dir.map(str::to_owned),
                padding: measured.padding,
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
                label_width: node.label_width.unwrap_or(0.0),
                label_height: node.label_height.unwrap_or(0.0),
                layer: 0,
                order: 0,
                content_top: None,
                title_rect: None,
            },
        );
        if previous.is_some() {
            return Err(Error::InvalidModel {
                message: format!("duplicate measured Swimlane node {}", node.id),
            });
        }
    }

    validate_parents(&nodes, &work_meter)?;
    for node in nodes.values_mut() {
        if node.is_group() && node.parent_id.is_none() {
            node.shape = "swimlane".into();
            node.requested_dir = Some(direction.as_str().into());
        }
    }

    // prepareLayoutForSwimlanes keeps explicit groups and places only loose content in the
    // implicit lane. An authored group with the reserved id retains its measured title.
    let loose_ids = nodes
        .values()
        .filter(|node| !node.is_group() && node.parent_id.is_none())
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    if !loose_ids.is_empty() {
        if let Some(lane) = nodes.get_mut(DEFAULT_LANE_ID) {
            if !lane.is_group() {
                return Err(Error::InvalidModel {
                    message: format!(
                        "Swimlane default lane id {DEFAULT_LANE_ID} is a content node"
                    ),
                });
            }
            lane.shape = "swimlane".into();
            lane.requested_dir = Some(direction.as_str().into());
        } else {
            nodes.insert(
                DEFAULT_LANE_ID.into(),
                WorkingNode {
                    id: DEFAULT_LANE_ID.into(),
                    label: String::new(),
                    label_type: "text".into(),
                    shape: "swimlane".into(),
                    kind: WorkingNodeKind::Group,
                    producer_lane_role: None,
                    parent_id: None,
                    top_lane_id: None,
                    requested_dir: Some(direction.as_str().into()),
                    padding: DEFAULT_LANE_PADDING,
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                    label_width: 0.0,
                    label_height: 0.0,
                    layer: 0,
                    order: 0,
                    content_top: None,
                    title_rect: None,
                },
            );
        }
        for id in loose_ids {
            if let Some(node) = nodes.get_mut(&id) {
                node.parent_id = Some(DEFAULT_LANE_ID.into());
            }
        }
    }

    let mut original_edges = Vec::with_capacity(edges.len());
    let mut edge_ids = HashSet::with_capacity(edges.len());
    for edge in edges {
        if !edge_ids.insert(edge.id.as_str()) {
            return Err(Error::InvalidModel {
                message: format!("duplicate measured Swimlane edge {}", edge.id),
            });
        }
        if !nodes.contains_key(&edge.from) || !nodes.contains_key(&edge.to) {
            return Err(Error::InvalidModel {
                message: format!(
                    "measured Swimlane edge {} references missing nodes: {} -> {}",
                    edge.id, edge.from, edge.to,
                ),
            });
        }
        let mut original = WorkingEdge {
            id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            reference_id: edge.id.clone(),
            label_node_id: None,
            reversed_for_layout: false,
            points: Vec::new(),
        };
        if let Some(label) = &edge.label {
            let base_id = format!("edge-label-{}-{}-{}", edge.from, edge.to, edge.id);
            let mut label_id = base_id.clone();
            let mut suffix = 0usize;
            while nodes.contains_key(&label_id) {
                work_meter.charge(1)?;
                suffix += 1;
                label_id = format!("{base_id}-{suffix}");
            }
            // Cross-lane labels belong to the target's immediate group, as in
            // createEdgeLabelNodes. Within one group both endpoint parents are identical.
            let parent_id = nodes.get(&edge.to).and_then(|node| node.parent_id.clone());
            nodes.insert(
                label_id.clone(),
                WorkingNode {
                    id: label_id.clone(),
                    label: String::new(),
                    label_type: "text".into(),
                    shape: "labelRect".into(),
                    kind: WorkingNodeKind::EdgeLabel,
                    producer_lane_role: None,
                    parent_id,
                    top_lane_id: None,
                    requested_dir: None,
                    padding: 0.0,
                    x: 0.0,
                    y: 0.0,
                    width: label.width.max(0.0),
                    height: label.height.max(0.0),
                    label_width: label.width.max(0.0),
                    label_height: label.height.max(0.0),
                    layer: 0,
                    order: 0,
                    content_top: None,
                    title_rect: None,
                },
            );
            original.label_node_id = Some(label_id);
        }
        original_edges.push(original);
    }
    let top_lane_order = nodes
        .values()
        .filter(|node| node.is_group() && node.parent_id.is_none())
        .map(|node| node.id.clone())
        .rev()
        .collect();
    let mut working = WorkingLayout {
        direction,
        top_lane_order,
        nodes,
        graph_edges: original_edges.clone(),
        original_edges,
    };
    working.refresh_top_lane_ids();
    super::run_layout_core(&mut working, settings, work_meter)?;
    Ok(super::project_layout(
        &working,
        &std::collections::HashMap::new(),
    ))
}

/// Validate the final parent forest once, before any core phase walks ancestors. The indexed
/// color walk is linear even for a long chain supplied through the direct model API.
fn validate_parents(
    nodes: &IndexMap<String, WorkingNode>,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let mut parents = Vec::with_capacity(nodes.len());
    for node in nodes.values() {
        work_meter.charge(1)?;
        let parent = node
            .parent_id
            .as_deref()
            .map(|id| {
                let Some((index, _, parent)) = nodes.get_full(id) else {
                    return Err(Error::InvalidModel {
                        message: format!("missing Swimlane parent {id} for node {}", node.id),
                    });
                };
                if !parent.is_group() {
                    return Err(Error::InvalidModel {
                        message: format!(
                            "Swimlane parent {id} for node {} is not a group",
                            node.id
                        ),
                    });
                }
                Ok(index)
            })
            .transpose()?;
        parents.push(parent);
    }
    let mut states = vec![0u8; nodes.len()];
    let mut path = Vec::new();
    for start in 0..nodes.len() {
        let mut current = Some(start);
        while let Some(index) = current {
            work_meter.charge(1)?;
            match states[index] {
                2 => break,
                1 => {
                    let id = nodes
                        .get_index(index)
                        .map(|(id, _)| id.as_str())
                        .unwrap_or("");
                    return Err(Error::InvalidModel {
                        message: format!("Swimlane parent cycle at {id}"),
                    });
                }
                _ => {}
            }
            states[index] = 1;
            path.push(index);
            current = parents[index];
        }
        for index in path.drain(..) {
            states[index] = 2;
        }
    }
    Ok(())
}
