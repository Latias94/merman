use super::RequirementDiagramLayout;
use super::{
    MeasuredRequirementEdge, RequirementEdgeIdentity, RequirementEdgeLabelPlan,
    RequirementEdgeLabels, RequirementMeasurements, RequirementNodeRenderPlan,
    RequirementPreparedArtifact, bounds_for_nodes_edges, measure_requirement_relationship,
    requirement_layout_node_from_center, validate_relationship_endpoints,
};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};
use merman_model::families::requirement::RequirementRelationship;
use merman_render_core::graph_layout::{Direction, Size};
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::text::TextMeasurer;
use merman_render_core::{Error, Result};
use merman_render_core::{LayoutLabel, LayoutPoint};
use std::collections::{HashMap, HashSet};

type DagreGraph = Graph<NodeLabel, EdgeLabel, GraphLabel>;

pub fn layout(
    relationships: &[RequirementRelationship],
    measured: RequirementMeasurements<'_>,
    settings: &super::config::RequirementLayoutSettings,
    text_measurer: &dyn TextMeasurer,
    work: &mut OperationLayoutWorkControl,
) -> Result<RequirementPreparedArtifact> {
    let RequirementMeasurements {
        direction,
        mut nodes,
        styles,
        binding: measurement_binding,
    } = measured;
    let mut graph = DagreGraph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    graph.set_graph(GraphLabel {
        rankdir: match direction {
            Direction::Down => dugong::RankDir::TB,
            Direction::Up => dugong::RankDir::BT,
            Direction::Right => dugong::RankDir::LR,
            Direction::Left => dugong::RankDir::RL,
        },
        nodesep: settings.nodesep,
        ranksep: settings.ranksep,
        marginx: 8.0,
        marginy: 8.0,
        ..Default::default()
    });
    for (&id, node) in &nodes {
        graph.set_node(
            id,
            NodeLabel {
                width: node.width,
                height: node.height,
                ..Default::default()
            },
        );
    }

    let mut edge_labels = RequirementEdgeLabels::new();
    let mut anchors = HashSet::new();
    for (index, relationship) in relationships.iter().enumerate() {
        // A later relationship can reference an anchor created by an earlier self-loop.
        validate_relationship_endpoints(relationship, |id| graph.has_node(id))?;
        let edge = measure_requirement_relationship(relationship, index, text_measurer, &styles)?;
        if edge.from == edge.to {
            insert_self_loop(&mut graph, &mut edge_labels, &mut anchors, edge);
        } else {
            insert_edge(
                &mut graph,
                &mut edge_labels,
                edge.identity().into_owned(),
                edge.parity_label_size(),
                edge.plan,
            );
        }
    }

    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        dugong::layout_controlled(&mut graph, work)
    };
    solved.map_err(|error| work.map_dugong_error(error))?;

    let mut out_nodes = Vec::new();
    for id in graph.nodes() {
        let Some(node) = graph.node(id) else {
            continue;
        };
        let (Some(x), Some(y)) = (node.x, node.y) else {
            continue;
        };
        out_nodes.push(requirement_layout_node_from_center(
            id.to_owned(),
            x,
            y,
            node.width,
            node.height,
        ));
    }

    let mut out_edges = Vec::new();
    let mut prepared_edges = RequirementEdgeLabels::new();
    for key in graph.edge_keys() {
        let Some(edge) = graph.edge_by_key(&key) else {
            continue;
        };
        let identity = key.name.as_deref().map(|name| RequirementEdgeIdentity {
            from: key.v.as_str(),
            to: key.w.as_str(),
            name,
        });
        let prepared_label = identity
            .and_then(|identity| edge_labels.swap_remove(&identity))
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Requirement edge label for {} -> {} ({:?})",
                    key.v, key.w, key.name
                ),
            })?;
        let identity = identity.expect("prepared Requirement edges are named");
        let points = edge
            .points
            .iter()
            .map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            })
            .collect();
        let label = match (edge.x, edge.y) {
            (Some(x), Some(y)) if edge.width > 0.0 && edge.height > 0.0 => Some(LayoutLabel {
                x,
                y,
                width: edge.width,
                height: edge.height,
            }),
            _ => None,
        };
        out_edges.push(identity.layout_edge(points, label));
        if prepared_edges
            .insert(identity.into_owned(), prepared_label)
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: "duplicate prepared Requirement edge identity after layout".to_owned(),
            });
        }
    }

    let mut prepared_nodes = HashMap::with_capacity(out_nodes.len());
    for node in &out_nodes {
        let plan = if anchors.contains(&node.id) {
            RequirementNodeRenderPlan::EdgeLabelAnchor
        } else {
            let measured =
                nodes
                    .swap_remove(node.id.as_str())
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("missing prepared Requirement node label for {}", node.id),
                    })?;
            RequirementNodeRenderPlan::Semantic(measured.label)
        };
        if prepared_nodes.insert(node.id.clone(), plan).is_some() {
            return Err(Error::InvalidModel {
                message: format!("duplicate prepared Requirement node identity: {}", node.id),
            });
        }
    }
    let bounds = bounds_for_nodes_edges(&out_nodes, &out_edges);
    Ok(RequirementPreparedArtifact {
        layout: RequirementDiagramLayout {
            nodes: out_nodes,
            edges: out_edges,
            bounds,
        },
        render_layout: None,
        nodes: prepared_nodes,
        edges: prepared_edges,
        measurement_binding,
        #[cfg(feature = "layout-grid")]
        grid: None,
        #[cfg(feature = "layout-layered")]
        layered: None,
    })
}

fn insert_edge(
    graph: &mut DagreGraph,
    plans: &mut RequirementEdgeLabels,
    key: RequirementEdgeIdentity<String>,
    size: Size,
    plan: RequirementEdgeLabelPlan,
) {
    graph.set_edge_named(
        &key.from,
        &key.to,
        Some(&key.name),
        Some(EdgeLabel {
            width: size.width,
            height: size.height,
            labelpos: LabelPos::C,
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        }),
    );
    plans.insert(key, plan);
}

fn insert_self_loop(
    graph: &mut DagreGraph,
    plans: &mut RequirementEdgeLabels,
    anchors: &mut HashSet<String>,
    edge: MeasuredRequirementEdge<'_>,
) {
    // The pinned Dagre renderer uses two labelRect nodes and three named edges. Repeated loops
    // update those same nodes and edges in place, while SVG uses a separate ID for each segment.
    let node = edge.from;
    let first_anchor = format!("{node}---{node}---1");
    let second_anchor = format!("{node}---{node}---2");
    for anchor in [&first_anchor, &second_anchor] {
        graph.set_node(
            anchor.clone(),
            NodeLabel {
                width: 0.1,
                height: 0.1,
                ..Default::default()
            },
        );
        anchors.insert(anchor.clone());
    }

    let [first, middle, last] = [0, 1, 2].map(|part| format!("{node}-cyclic-special-{part}"));
    let outer_segment = |rendered_id, marker_start, marker_end| RequirementEdgeLabelPlan {
        relationship_type: edge.plan.relationship_type.clone(),
        display_text: String::new(),
        rendered_id,
        has_label: false,
        marker_start,
        marker_end,
    };
    let first_plan = outer_segment(middle.clone(), edge.plan.marker_start, false);
    let last_plan = outer_segment(last.clone(), false, edge.plan.marker_end);
    let label_size = edge.parity_label_size();
    let middle_plan = RequirementEdgeLabelPlan {
        rendered_id: format!("{node}-cyclic-special-mid"),
        marker_start: false,
        marker_end: false,
        ..edge.plan
    };
    for (from, to, name, size, plan) in [
        (
            node,
            first_anchor.as_str(),
            first,
            Size::default(),
            first_plan,
        ),
        (
            &first_anchor,
            &second_anchor,
            middle,
            label_size,
            middle_plan,
        ),
        (&second_anchor, node, last, Size::default(), last_plan),
    ] {
        insert_edge(
            graph,
            plans,
            RequirementEdgeIdentity {
                from: from.to_owned(),
                to: to.to_owned(),
                name,
            },
            size,
            plan,
        );
    }
}
