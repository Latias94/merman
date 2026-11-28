//! Usecase ELK input, seeded placement, and route preparation.

use super::{
    UsecaseDiagramLayout, UsecaseEdgePlan, UsecaseMeasured, UsecaseNodePlan, UsecasePrepared,
    layout::{edge, node},
    measure,
    parity_routes::{edge_points, prepare_edge_paths},
};
use merman_layout_elk as elk;
use merman_model::families::usecase::UsecaseDiagramModel;
use merman_render_core::{
    Bounds, Error, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    elk_adapter::{
        GroupFrame, apply_cyclic_entry_constraints,
        arrowheads::{Arrowhead, EdgeArrowheads},
        drawing_group_frames,
        feedback_edges::orient_feedback_edges_with_arrowheads,
        options::layout_options,
    },
    layout_work::OperationLayoutWorkControl,
};
use serde_json::Value;
use std::collections::HashMap;

pub fn prepare(
    model: &UsecaseDiagramModel,
    config: &Value,
    measured: UsecaseMeasured,
    work: &mut OperationLayoutWorkControl,
    algorithm: elk::Algorithm,
    operation_seed: elk::ElkOperationSeed,
) -> Result<UsecasePrepared> {
    let mut layout = layout(
        model,
        config,
        &measured.nodes,
        &measured.edges,
        work,
        algorithm,
        operation_seed,
    )?;
    let edge_paths = prepare_edge_paths(
        &mut layout,
        &measured.nodes,
        &measured.edges,
        config,
        true,
        work,
    )?;
    Ok(UsecasePrepared::new(measured, layout, edge_paths, true))
}

fn layout(
    model: &UsecaseDiagramModel,
    config: &Value,
    plans: &[UsecaseNodePlan],
    edge_plans: &[UsecaseEdgePlan],
    work: &mut OperationLayoutWorkControl,
    algorithm: elk::Algorithm,
    operation_seed: merman_layout_elk::ElkOperationSeed,
) -> Result<UsecaseDiagramLayout> {
    let direction = match model.direction.as_str() {
        "LR" => elk::Direction::Right,
        "RL" => elk::Direction::Left,
        "BT" => elk::Direction::Up,
        _ => elk::Direction::Down,
    };
    let mut graph = elk::Graph {
        id: "usecase".into(),
        direction,
        nodes: plans
            .iter()
            .map(|plan| elk::Node {
                id: plan.id.clone(),
                kind: if plan.is_boundary {
                    elk::NodeKind::Group
                } else {
                    elk::NodeKind::Leaf
                },
                container: elk::ContainerNodeOptions {
                    padding: if plan.is_boundary { 20.0 } else { 0.0 },
                    ..Default::default()
                },
                label_text: plan.is_boundary.then(|| plan.label.text.clone()),
                width: plan.width,
                height: plan.height,
                parent: plan.parent.clone(),
                direction: plan.is_boundary.then_some(direction),
                hierarchy_handling: plan
                    .is_boundary
                    .then_some(elk::HierarchyHandling::IncludeChildren),
                layer_constraint: None,
                port_alignment: plan.ellipse.then_some(elk::PortAlignment::Center),
                label: plan.is_boundary.then_some(elk::Label {
                    width: plan.label.metrics.width,
                    height: plan.label.metrics.height,
                }),
            })
            .collect(),
        edges: edge_plans
            .iter()
            .map(|plan| elk::Edge {
                id: plan.id.clone(),
                source: plan.source.clone(),
                target: plan.target.clone(),
                label: plan.label.as_ref().map(|label| elk::Label {
                    width: label.metrics.width,
                    height: label.metrics.height,
                }),
                minlen: plan.minlen,
                terminal_labels: Vec::new(),
                inside_self_loops_yo: false,
            })
            .collect(),
        spacing: elk::Spacing {
            node_node: measure::spacing(config, "nodeSpacing"),
            layer_layer: measure::spacing(config, "rankSpacing"),
            ..Default::default()
        },
        options: layout_options(config, algorithm),
    };
    if config
        .pointer("/elk/keepEntryNodeOnTop")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        apply_cyclic_entry_constraints(
            &mut graph.nodes,
            graph
                .edges
                .iter()
                .map(|edge| (edge.source.as_str(), edge.target.as_str())),
            &mut Some(&mut *work),
        )?;
    }
    let neo = config.get("look").and_then(Value::as_str) == Some("neo");
    let arrowhead = |marker: Option<&str>| match marker {
        Some("arrow_point") => Some(Arrowhead::Point),
        Some("arrow_circle") => Some(Arrowhead::Circle),
        Some("arrow_cross") => Some(Arrowhead::Cross),
        None | Some("" | "none") => None,
        Some(_) => Some(Arrowhead::Other),
    };
    let oriented = orient_feedback_edges_with_arrowheads(
        &mut graph,
        config,
        &mut Some(&mut *work),
        |index, _| {
            let plan = &edge_plans[index];
            Ok(EdgeArrowheads {
                start: arrowhead(plan.start_marker.as_deref()),
                end: arrowhead(plan.end_marker.as_deref()),
                margin: neo && !plan.animated,
            })
        },
    )?;
    let mut placed = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        elk::layout_with_operation_seed_and_work_control(oriented.graph(), operation_seed, work)
    }
    .map_err(|error| work.map_elk_error_with_context(error, "Usecase ELK"))?;
    oriented.restore(&mut placed, &mut Some(&mut *work))?;
    let frames = drawing_group_frames(
        &graph,
        &placed,
        |node| node.label.as_ref().map_or(0.0, |label| label.width) + node.container.padding,
        &mut Some(&mut *work),
    )?;
    let plans_by_id: HashMap<_, _> = plans.iter().map(|plan| (plan.id.as_str(), plan)).collect();
    let edges_by_id: HashMap<_, _> = edge_plans
        .iter()
        .map(|plan| (plan.id.as_str(), plan))
        .collect();
    let mut nodes = Vec::with_capacity(placed.nodes.len());
    for placed in placed.nodes {
        let plan = plans_by_id
            .get(placed.id.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ELK returned unknown Usecase node {}", placed.id),
            })?;
        // Project only the drawn group frame; provider routes keep their original origins.
        let frame = frames
            .get(placed.id.as_str())
            .copied()
            .unwrap_or_else(|| GroupFrame::from(&placed));
        nodes.push(node(plan, frame.x, frame.y, frame.width, frame.height));
    }
    let mut edges = Vec::with_capacity(placed.edges.len());
    for placed in placed.edges {
        let plan = edges_by_id
            .get(placed.id.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ELK returned unknown Usecase edge {}", placed.id),
            })?;
        let label = placed.labels.first().map(|label| LayoutLabel {
            x: label.x + label.width / 2.0,
            y: label.y + label.height / 2.0,
            width: label.width,
            height: label.height,
        });
        edges.push(edge(
            plan,
            placed
                .points
                .into_iter()
                .map(|point| LayoutPoint {
                    x: point.x,
                    y: point.y,
                })
                .collect(),
            label,
        ));
    }
    finish(nodes, edges, plans, edge_plans, work)
}

fn finish(
    nodes: Vec<LayoutNode>,
    mut edges: Vec<LayoutEdge>,
    plans: &[UsecaseNodePlan],
    edge_plans: &[UsecaseEdgePlan],
    work: &mut OperationLayoutWorkControl,
) -> Result<UsecaseDiagramLayout> {
    let by_id: HashMap<_, _> = nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let edge_by_id: HashMap<_, _> = edge_plans
        .iter()
        .map(|plan| (plan.id.as_str(), plan))
        .collect();
    for edge in &mut edges {
        work.charge_adapter(edge.points.len().saturating_add(1))?;
        let Some(start) = by_id.get(edge.from.as_str()) else {
            continue;
        };
        let Some(end) = by_id.get(edge.to.as_str()) else {
            continue;
        };
        edge.from_cluster = start.is_cluster.then(|| start.id.clone());
        edge.to_cluster = end.is_cluster.then(|| end.id.clone());
        if edge.label.is_none()
            && let Some(label) = edge_by_id
                .get(edge.id.as_str())
                .and_then(|plan| plan.label.as_ref())
            && let Some((x, y)) = route_midpoint(&edge_points(edge, &nodes, plans))
        {
            edge.label = Some(LayoutLabel {
                x,
                y,
                width: label.metrics.width,
                height: label.metrics.height,
            });
        }
    }
    let bounds = Bounds::from_points(
        nodes
            .iter()
            .flat_map(|node| {
                [
                    (node.x - node.width / 2.0, node.y - node.height / 2.0),
                    (node.x + node.width / 2.0, node.y + node.height / 2.0),
                ]
            })
            .chain(
                edges
                    .iter()
                    .flat_map(|edge| edge.points.iter().map(|point| (point.x, point.y))),
            )
            .chain(
                edges
                    .iter()
                    .filter_map(|edge| edge.label.as_ref())
                    .flat_map(|label| {
                        [
                            (label.x - label.width / 2.0, label.y - label.height / 2.0),
                            (label.x + label.width / 2.0, label.y + label.height / 2.0),
                        ]
                    }),
            ),
    );
    Ok(UsecaseDiagramLayout {
        nodes,
        edges,
        bounds,
    })
}

fn route_midpoint(points: &[LayoutPoint]) -> Option<(f64, f64)> {
    let first = points.first()?;
    let distance = |pair: &[LayoutPoint]| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y);
    let mut remaining = points.windows(2).map(distance).sum::<f64>() / 2.0;
    for pair in points.windows(2) {
        let length = distance(pair);
        if length > 0.0 && remaining <= length {
            let t = remaining / length;
            return Some((
                pair[0].x + (pair[1].x - pair[0].x) * t,
                pair[0].y + (pair[1].y - pair[0].y) * t,
            ));
        }
        remaining -= length;
    }
    Some((first.x, first.y))
}
