//! RequirementDB projection onto the measured Mermaid Layered layout.

use super::{
    RequirementDiagramLayout, RequirementEdgeLabels, RequirementNodeRenderPlan,
    RequirementPreparedArtifact, bounds_for_nodes_edges, config::RequirementLayoutSettings,
    measure_requirement, measure_requirement_relationship, missing_relationship_node,
    requirement_layout_node_from_center,
};
use merman_model::{MermaidConfig, OperationPhase, families::requirement::RequirementDiagramModel};
use merman_render_core::{
    Error, LayoutEdge, LayoutLabel, LayoutPoint, RenderCapability, Result,
    graph_layout::{
        Endpoint, MeasuredEdge, MeasuredGraph, MeasuredNode, Size, SolvedEdge,
        mermaid_layered::{self, EdgeRouting, LayeredError, Options},
    },
    resources::OperationWorkMeter,
    svg_edge_parity::{
        layered_route::{self, Curve},
        line_hops::LineHopEdge,
        path_numbers::json_stringify_axis_points_into,
    },
    text::TextMeasurer,
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub(super) enum Paint {
    Axes {
        routing: EdgeRouting,
        routes: Vec<SolvedEdge>,
    },
    #[cfg(feature = "layout-dagre")]
    DagreFallback,
}

pub fn layout(
    model: &RequirementDiagramModel,
    config: &MermaidConfig,
    settings: &RequirementLayoutSettings,
    measurer: &dyn TextMeasurer,
    allow_dagre_retry: bool,
    work: &Arc<OperationWorkMeter>,
) -> Result<RequirementPreparedArtifact> {
    work.checkpoint(OperationPhase::Layout)?;
    let measured = measure_requirement(model, config, measurer, settings, work)?;
    let routing = EdgeRouting::from_name(
        config
            .get_path(&["layered", "edgeRouting"])
            .and_then(Value::as_str),
    );
    work.charge(measured.nodes.len())?;
    let mut nodes = buffer(measured.nodes.len())?;
    for (index, (id, node)) in measured.nodes.iter().enumerate() {
        work.charge(id.len().max(1))?;
        let size = Size {
            width: node.width,
            height: node.height,
        };
        nodes.push(MeasuredNode::new(size, slot(index, work)?, None));
    }

    work.charge(model.relationships.len())?;
    let mut edges = buffer(model.relationships.len())?;
    let mut relations = buffer(model.relationships.len())?;
    for (index, relation) in model.relationships.iter().enumerate() {
        work.charge(relation.src.len())?;
        work.charge(relation.dst.len())?;
        work.charge(relation.rel_type.len())?;
        let endpoint = |id: &str, terminal| {
            let index = measured
                .nodes
                .get_index_of(id)
                .ok_or_else(|| missing_relationship_node(terminal, id))?;
            slot(index, work).map(Endpoint::Node)
        };
        let source = endpoint(&relation.src, "src")?;
        let target = endpoint(&relation.dst, "dst")?;
        let label = measure_requirement_relationship(relation, index, measurer, &measured.styles)?;
        work.checkpoint(OperationPhase::Layout)?;
        edges.push(MeasuredEdge {
            source,
            target,
            min_rank_span: 1,
            weight: 1.0,
            label: Some(Size {
                width: label.label_size.width + 12.0,
                height: label.label_size.height + 4.0,
            }),
            start_label: None,
            end_label: None,
            start_marker: Default::default(),
            end_marker: Default::default(),
        });
        relations.push(label);
    }
    let input = MeasuredGraph {
        direction: measured.direction,
        routing: routing.geometry(),
        node_spacing: spacing(config, "nodeSpacing"),
        rank_spacing: spacing(config, "rankSpacing"),
        component_spacing: 0.0,
        inherit_crossed_group_direction: true,
        nodes,
        groups: Vec::new(),
        edges,
    };
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        mermaid_layered::layout(&input, Options::default(), work)
    };
    let solved = match solved {
        Ok(layout) => layout.graph,
        Err(LayeredError::Work(error)) => return Err(error.into()),
        Err(LayeredError::ArithmeticOverflow) => return Err(work.arithmetic_overflow().into()),
        Err(LayeredError::AllocationFailed) => return Err(allocation_error()),
        Err(error) if error.is_recoverable() => {
            drop((input, relations, measured));
            work.checkpoint(OperationPhase::Layout)?;
            #[cfg(feature = "layout-dagre")]
            if allow_dagre_retry {
                work.charge(super::requirement_layout_work_units(model))?;
                let measured = measure_requirement(model, config, measurer, settings, work)?;
                let mut control = merman_render_core::layout_work::OperationLayoutWorkControl::new(
                    Arc::clone(work),
                );
                let mut fallback = super::dagre::layout(
                    &model.relationships,
                    measured,
                    settings,
                    measurer,
                    &mut control,
                )?;
                fallback.layered = Some(Paint::DagreFallback);
                return Ok(fallback);
            }
            #[cfg(not(feature = "layout-dagre"))]
            let _ = allow_dagre_retry;
            return Err(Error::MissingCapability {
                capability: RenderCapability::LayoutDagre,
                diagram_type: "requirement".to_owned(),
            });
        }
        Err(error) => {
            return Err(Error::InvalidModel {
                message: error.to_string(),
            });
        }
    };
    if solved.nodes.len() != measured.nodes.len() || solved.edges.len() != relations.len() {
        return Err(Error::InvalidModel {
            message: "Requirement Layered geometry does not match its source slots".to_owned(),
        });
    }

    let mut nodes = buffer(solved.nodes.len())?;
    let mut node_plans = HashMap::new();
    node_plans
        .try_reserve(solved.nodes.len())
        .map_err(|_| allocation_error())?;
    for ((id, measured), rect) in measured.nodes.into_iter().zip(solved.nodes) {
        work.charge(id.len().max(1))?;
        let center = rect.center();
        nodes.push(requirement_layout_node_from_center(
            id.to_owned(),
            center.x,
            center.y,
            measured.width,
            measured.height,
        ));
        node_plans.insert(
            id.to_owned(),
            RequirementNodeRenderPlan::Semantic(measured.label),
        );
    }
    let mut edges = buffer(solved.edges.len())?;
    let mut edge_plans = RequirementEdgeLabels::new();
    edge_plans
        .try_reserve(solved.edges.len())
        .map_err(|_| allocation_error())?;
    for (measured, route) in relations.into_iter().zip(&solved.edges) {
        let identity = measured.identity();
        work.charge(identity.name.len().max(1))?;
        work.charge(route.points.len())?;
        let mut points = buffer(route.points.len())?;
        points.extend(route.points.iter().map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        }));
        let label = route.label.as_ref().map(|center| LayoutLabel {
            x: center.x,
            y: center.y,
            width: measured.label_size.width,
            height: measured.label_size.height,
        });
        edges.push(identity.layout_edge(points, label));
        edge_plans.insert(identity.into_owned(), measured.plan);
    }
    let bounds = bounds_for_nodes_edges(&nodes, &edges);
    work.checkpoint(OperationPhase::Layout)?;
    Ok(RequirementPreparedArtifact {
        layout: RequirementDiagramLayout {
            nodes,
            edges,
            bounds,
        },
        render_layout: None,
        nodes: node_plans,
        edges: edge_plans,
        measurement_binding: measured.binding,
        #[cfg(feature = "layout-grid")]
        grid: None,
        layered: Some(Paint::Axes {
            routing,
            routes: solved.edges,
        }),
    })
}

fn spacing(config: &MermaidConfig, key: &str) -> f64 {
    fn positive(value: &Value) -> Option<f64> {
        value
            .as_f64()
            .filter(|value| value.is_finite() && *value > 0.0)
    }
    match config.get_path(&["state", key]) {
        None | Some(Value::Null) => 50.0,
        Some(state) => positive(state)
            .or_else(|| config.get_path(&["flowchart", key]).and_then(positive))
            .unwrap_or(50.0),
    }
}

fn slot(index: usize, work: &OperationWorkMeter) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.arithmetic_overflow().into())
}

pub(super) fn axis_points_json(
    edge: &LayoutEdge,
    route: &SolvedEdge,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge_emit_work(route.points.len())?;
    let mut axes = buffer(route.points.len())?;
    axes.extend(route.points.iter().map(|point| point.axis));
    let mut out = String::new();
    json_stringify_axis_points_into(&mut out, &edge.points, &axes, &mut Default::default());
    work.charge_emit_work(out.len())?;
    Ok(out)
}

pub(super) fn line_hop_paths<'a>(
    edges: &'a [LayoutEdge],
    curve: impl Fn(&LayoutEdge) -> Result<Option<Curve>>,
    setting: Option<&Value>,
    work: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    let Some(style) = layered_route::line_hop_style(setting) else {
        return Ok(HashMap::new());
    };
    work.charge_emit_work(edges.len())?;
    let mut inputs = buffer(edges.len())?;
    for edge in edges {
        inputs.push(LineHopEdge {
            id: &edge.id,
            points: &edge.points,
            curve: curve(edge)?.map(Curve::as_str),
            arrow_type_start: None,
            arrow_type_end: None,
        });
    }
    let overrides = layered_route::line_hops(&inputs, style, work)?;
    let mut paths = HashMap::new();
    paths
        .try_reserve(overrides.len())
        .map_err(|_| allocation_error())?;
    for (index, path) in overrides {
        work.charge_emit_work(path.len())?;
        paths.insert(inputs[index].id, path);
    }
    Ok(paths)
}

fn buffer<T>(len: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| allocation_error())?;
    Ok(values)
}

fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}
