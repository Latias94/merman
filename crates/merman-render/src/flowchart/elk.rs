use crate::config::{config_bool, config_string};
use crate::math::MathRenderer;
use crate::model::{
    FlowchartLayout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::resources::{OperationWorkMeter, ResourceLimitExceeded};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use merman_core::MermaidConfig;
use merman_layout_elk as elk;
use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::sync::Arc;

use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph, FlowchartModel};

use super::config::{FlowchartConfigView, FlowchartLayoutSettings};
use super::label::compute_bounds;
use super::layout::flowchart_svg_plain_computed_width_px;
use super::node::{NodeLayoutDimensionsRequest, node_layout_dimensions};
use super::{
    FlowchartLabelMetricsRequest, flowchart_apply_html_node_class_box_metrics,
    flowchart_effective_text_style_for_classes, flowchart_effective_text_style_for_node_classes,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
};

struct ElkOperationWorkControl {
    meter: Arc<OperationWorkMeter>,
    rejection: Option<ResourceLimitExceeded>,
    #[cfg(test)]
    adapter_work: usize,
}

impl ElkOperationWorkControl {
    fn new(meter: Arc<OperationWorkMeter>) -> Self {
        Self {
            meter,
            rejection: None,
            #[cfg(test)]
            adapter_work: 0,
        }
    }

    fn charge_adapter(&mut self, units: usize) -> Result<()> {
        if let Some(error) = &self.rejection {
            return Err(error.clone().into());
        }
        #[cfg(test)]
        let next_adapter_work = self.checked_add(self.adapter_work, units)?;
        self.meter.charge(units).map_err(|error| {
            self.rejection = Some(error.clone());
            Error::from(error)
        })?;
        #[cfg(test)]
        {
            self.adapter_work = next_adapter_work;
        }
        Ok(())
    }

    #[cfg(test)]
    fn checked_add(&self, left: usize, right: usize) -> Result<usize> {
        left.checked_add(right)
            .ok_or_else(|| self.meter.arithmetic_overflow().into())
    }

    #[cfg(test)]
    fn checked_mul(&self, left: usize, right: usize) -> Result<usize> {
        left.checked_mul(right)
            .ok_or_else(|| self.meter.arithmetic_overflow().into())
    }

    fn map_elk_error(&mut self, error: elk::Error) -> Error {
        match error.work_error() {
            Some(elk::WorkError::Interrupted) => self
                .rejection
                .take()
                .unwrap_or_else(|| self.meter.arithmetic_overflow())
                .into(),
            Some(elk::WorkError::ArithmeticOverflow) => self.meter.arithmetic_overflow().into(),
            None => Error::InvalidModel {
                message: format!("ELK layout failed: {error}"),
            },
        }
    }

    #[cfg(test)]
    fn adapter_work(&self) -> usize {
        self.adapter_work
    }
}

impl elk::WorkControl for ElkOperationWorkControl {
    fn check(&mut self, units: usize) -> std::result::Result<(), elk::WorkError> {
        if self.rejection.is_some() {
            return Err(elk::WorkError::Interrupted);
        }
        self.meter.preflight(units).map_err(|error| {
            self.rejection = Some(error);
            elk::WorkError::Interrupted
        })
    }

    fn charge(&mut self, units: usize) -> std::result::Result<(), elk::WorkError> {
        if self.rejection.is_some() {
            return Err(elk::WorkError::Interrupted);
        }
        self.meter.charge(units).map_err(|error| {
            self.rejection = Some(error);
            elk::WorkError::Interrupted
        })
    }
}

#[cfg(test)]
pub(crate) fn layout_flowchart_elk_typed(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<FlowchartLayout> {
    let graph = build_flowchart_elk_graph(model, effective_config, measurer, math_renderer)?;
    let layout = elk::layout(&graph).map_err(|err| Error::InvalidModel {
        message: format!("ELK layout failed: {err}"),
    })?;
    flowchart_layout_from_elk(model, effective_config, &graph, layout)
}

/// Lays out a Flowchart through ELK using the render operation's captured seed.
///
/// This is intentionally crate-private. The public typed function above is a raw diagnostic API
/// and fails closed when source configuration uses ELK's `randomSeed = 0` sentinel.
pub(crate) fn layout_flowchart_elk_typed_with_operation_seed(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let mut work_control = ElkOperationWorkControl::new(work_meter);
    let graph = build_flowchart_elk_graph_with_work_control(
        model,
        effective_config,
        measurer,
        math_renderer,
        Some(&mut work_control),
    )?;
    let layout = match elk::layout_with_operation_seed_and_work_control(
        &graph,
        operation_seed,
        &mut work_control,
    ) {
        Ok(layout) => layout,
        Err(error) => return Err(work_control.map_elk_error(error)),
    };
    flowchart_layout_from_elk_with_work_control(
        model,
        effective_config,
        &graph,
        layout,
        Some(&mut work_control),
    )
}

#[cfg(test)]
fn flowchart_layout_from_elk(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
) -> Result<FlowchartLayout> {
    flowchart_layout_from_elk_with_work_control(model, effective_config, graph, layout, None)
}

fn flowchart_layout_from_elk_with_work_control(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
    mut work_control: Option<&mut ElkOperationWorkControl>,
) -> Result<FlowchartLayout> {
    let effective_config_value = effective_config.as_value();
    let FlowchartLayoutSettings {
        cluster_padding,
        title_margin_top,
        title_margin_bottom,
        ..
    } = FlowchartConfigView::new(effective_config_value).layout_settings();

    let source_index_work =
        checked_adapter_add(&work_control, graph.nodes.len(), graph.edges.len())?;
    charge_adapter_work(&mut work_control, source_index_work)?;
    let source_node_by_id: HashMap<&str, &elk::Node> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let source_edge_by_id: HashMap<&str, &elk::Edge> = graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();

    charge_adapter_work(&mut work_control, layout.nodes.len())?;
    let mut out_nodes = Vec::with_capacity(layout.nodes.len());
    for node in layout.nodes {
        let Some(source) = source_node_by_id.get(node.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown node {}", node.id),
            });
        };
        out_nodes.push(LayoutNode {
            id: node.id,
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            is_cluster: source.kind == elk::NodeKind::Group,
            label_width: source.label.map(|label| label.width),
            label_height: source.label.map(|label| label.height),
        });
    }

    charge_adapter_work(&mut work_control, out_nodes.len())?;
    let layout_node_by_id: HashMap<&str, &LayoutNode> = out_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let diagram_direction = model.direction.as_deref().unwrap_or("TB");
    charge_adapter_work(&mut work_control, model.subgraphs.len())?;
    let mut clusters = Vec::new();
    for sg in &model.subgraphs {
        let Some(node) = layout_node_by_id.get(sg.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("missing ELK layout cluster {}", sg.id),
            });
        };
        let label = source_node_by_id
            .get(sg.id.as_str())
            .and_then(|node| node.label)
            .unwrap_or(elk::Label {
                width: 1.0,
                height: 1.0,
            });
        let title_label = LayoutLabel {
            x: node.x,
            y: node.y - node.height / 2.0 + title_margin_top + label.height / 2.0,
            width: label.width,
            height: label.height,
        };
        let title_w = label.width.max(1.0);
        let diff = if node.width <= title_w {
            (title_w - node.width) / 2.0 - cluster_padding / 2.0
        } else {
            -cluster_padding / 2.0
        };
        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            diff,
            offset_y: label.height - cluster_padding / 2.0,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|dir| normalize_flow_direction(dir)),
            effective_dir: sg
                .dir
                .as_deref()
                .map(normalize_flow_direction)
                .unwrap_or_else(|| normalize_flow_direction(diagram_direction)),
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });
    }
    let cluster_sort_work = comparison_sort_work_units(clusters.len(), &work_control)?;
    charge_adapter_work(&mut work_control, cluster_sort_work)?;
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let edge_projection_work = layout.edges.iter().try_fold(0usize, |total, edge| {
        let point_walks = checked_adapter_mul(&work_control, edge.points.len(), 2)?;
        let edge_work = checked_adapter_add(
            &work_control,
            checked_adapter_add(&work_control, point_walks, edge.labels.len())?,
            3,
        )?;
        checked_adapter_add(&work_control, total, edge_work)
    })?;
    // Reject the complete user-sized projection tranche before reserving its output vector.
    charge_adapter_work(&mut work_control, edge_projection_work)?;
    let mut out_edges = Vec::with_capacity(layout.edges.len());
    for edge in layout.edges {
        let Some(source) = source_edge_by_id.get(edge.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown edge {}", edge.id),
            });
        };
        let points = edge
            .points
            .into_iter()
            .map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        let label = source.label.and_then(|source_label| {
            edge.labels
                .first()
                .map(edge_label_layout)
                .or_else(|| edge_label_position(&points, source_label))
        });
        out_edges.push(LayoutEdge {
            id: edge.id,
            from: source.source.clone(),
            to: source.target.clone(),
            from_cluster: endpoint_cluster(source.source.as_str(), &layout_node_by_id),
            to_cluster: endpoint_cluster(source.target.as_str(), &layout_node_by_id),
            points,
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

    let bounds_points = out_edges.iter().try_fold(
        checked_adapter_mul(&work_control, out_nodes.len(), 2)?,
        |total, edge| {
            let label_points = usize::from(edge.label.is_some()) * 2;
            checked_adapter_add(
                &work_control,
                total,
                checked_adapter_add(&work_control, edge.points.len(), label_points)?,
            )
        },
    )?;
    let bounds_work = checked_adapter_mul(&work_control, bounds_points, 2)?;
    charge_adapter_work(&mut work_control, bounds_work)?;
    let bounds = compute_bounds(&out_nodes, &out_edges);
    let dom_work = checked_adapter_add(
        &work_control,
        checked_adapter_mul(&work_control, graph.nodes.len(), 2)?,
        1,
    )?;
    charge_adapter_work(&mut work_control, dom_work)?;
    let dom_node_order_by_root = flowchart_elk_dom_node_order_by_root(graph);

    Ok(FlowchartLayout {
        nodes: out_nodes,
        edges: out_edges,
        clusters,
        bounds,
        dom_node_order_by_root,
        uses_elk_adapter_dom: true,
    })
}

fn flowchart_elk_dom_node_order_by_root(graph: &elk::Graph) -> HashMap<String, Vec<String>> {
    let ids = mermaid_elk_adapter_dom_order(graph);
    std::iter::once((String::new(), ids)).collect()
}

fn mermaid_elk_adapter_dom_order(graph: &elk::Graph) -> Vec<String> {
    let mut children_by_parent: HashMap<Option<&str>, Vec<&elk::Node>> = HashMap::new();
    for node in &graph.nodes {
        children_by_parent
            .entry(node.parent.as_deref())
            .or_default()
            .push(node);
    }
    let mut out = Vec::with_capacity(graph.nodes.len());
    let mut stack = children_by_parent
        .get(&None)
        .into_iter()
        .flat_map(|children| children.iter().rev().copied())
        .collect::<Vec<_>>();
    while let Some(node) = stack.pop() {
        out.push(node.id.clone());
        if node.kind == elk::NodeKind::Group
            && let Some(children) = children_by_parent.get(&Some(node.id.as_str()))
        {
            stack.extend(children.iter().rev().copied());
        }
    }
    out
}

fn normalize_flow_direction(dir: &str) -> String {
    let upper = dir.trim().to_uppercase();
    if upper == "TD" {
        "TB".to_string()
    } else {
        upper
    }
}

fn endpoint_cluster(id: &str, layout_node_by_id: &HashMap<&str, &LayoutNode>) -> Option<String> {
    layout_node_by_id
        .get(id)
        .filter(|node| node.is_cluster)
        .map(|node| node.id.clone())
}

fn edge_label_layout(label: &elk::EdgeLabelLayout) -> LayoutLabel {
    LayoutLabel {
        x: label.x + label.width / 2.0,
        y: label.y + label.height / 2.0,
        width: label.width,
        height: label.height,
    }
}

fn edge_label_position(points: &[LayoutPoint], label: elk::Label) -> Option<LayoutLabel> {
    let point = polyline_midpoint(points)?;
    Some(LayoutLabel {
        x: point.x,
        y: point.y,
        width: label.width,
        height: label.height,
    })
}

fn polyline_midpoint(points: &[LayoutPoint]) -> Option<LayoutPoint> {
    match points {
        [] => None,
        [single] => Some(single.clone()),
        _ => {
            let total = points
                .windows(2)
                .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
                .sum::<f64>();
            if !total.is_finite() || total <= 0.0 {
                return points.first().cloned();
            }
            let mut remaining = total / 2.0;
            for pair in points.windows(2) {
                let len = (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y);
                if len <= 0.0 {
                    continue;
                }
                if remaining > len {
                    remaining -= len;
                    continue;
                }
                let t = remaining / len;
                return Some(LayoutPoint {
                    x: pair[0].x + (pair[1].x - pair[0].x) * t,
                    y: pair[0].y + (pair[1].y - pair[0].y) * t,
                });
            }
            points.last().cloned()
        }
    }
}

pub fn build_flowchart_elk_graph(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<elk::Graph> {
    build_flowchart_elk_graph_with_work_control(
        model,
        effective_config,
        measurer,
        math_renderer,
        None,
    )
}

fn charge_adapter_work(
    work_control: &mut Option<&mut ElkOperationWorkControl>,
    units: usize,
) -> Result<()> {
    match work_control.as_deref_mut() {
        Some(work_control) => work_control.charge_adapter(units),
        None => Ok(()),
    }
}

fn checked_adapter_add(
    work_control: &Option<&mut ElkOperationWorkControl>,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right).ok_or_else(|| {
        work_control
            .as_deref()
            .map(|work_control| work_control.meter.arithmetic_overflow())
            .unwrap_or_else(|| {
                OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .arithmetic_overflow()
            })
            .into()
    })
}

fn checked_adapter_mul(
    work_control: &Option<&mut ElkOperationWorkControl>,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_mul(right).ok_or_else(|| {
        work_control
            .as_deref()
            .map(|work_control| work_control.meter.arithmetic_overflow())
            .unwrap_or_else(|| {
                OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .arithmetic_overflow()
            })
            .into()
    })
}

fn comparison_sort_work_units(
    items: usize,
    work_control: &Option<&mut ElkOperationWorkControl>,
) -> Result<usize> {
    if items < 2 {
        return Ok(0);
    }
    let levels = usize::BITS as usize - (items - 1).leading_zeros() as usize;
    checked_adapter_mul(work_control, items, levels)
}

fn build_flowchart_elk_graph_with_work_control(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    mut work_control: Option<&mut ElkOperationWorkControl>,
) -> Result<elk::Graph> {
    // Shape validation only walks nodes. Charge that tranche before the validation scan; the
    // hierarchy and edge tranches are charged by their actual owners below.
    charge_adapter_work(&mut work_control, model.nodes.len())?;
    super::validate_flowchart_model_shapes(model)?;
    let effective_config_value = effective_config.as_value();
    let FlowchartLayoutSettings {
        node_padding,
        state_padding,
        wrapping_width,
        edge_label_wrapping_width,
        cluster_title_wrapping_width,
        edge_html_labels,
        node_wrap_mode,
        edge_wrap_mode,
        cluster_wrap_mode,
        cluster_padding,
        nodesep,
        ranksep,
        text_style,
        html_label_text_style,
        ..
    } = FlowchartConfigView::new(effective_config_value).layout_settings();

    let diagram_direction = model
        .direction
        .as_deref()
        .map(dir_to_elk_direction)
        .unwrap_or_default();
    let diagram_direction_text = model.direction.as_deref().unwrap_or("TB");

    let node_label_base_style = if node_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };
    let cluster_label_base_style = if cluster_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };
    let edge_label_base_style = if edge_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };

    let mut graph = elk::Graph {
        id: "root".to_string(),
        direction: diagram_direction,
        spacing: elk::Spacing {
            node_node: nodesep,
            layer_layer: ranksep,
            group_padding_x: cluster_padding,
            group_padding_y: cluster_padding,
            ..Default::default()
        },
        options: elk_layout_options(effective_config_value),
        ..Default::default()
    };

    charge_adapter_work(&mut work_control, model.subgraphs.len())?;
    let subgraph_ids: HashSet<&str> = model.subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let parent_by_id = parent_by_id(model, &mut work_control)?;
    let include_children_groups = include_children_groups(model, &parent_by_id, &mut work_control)?;

    let cluster_measure_ctx = ElkMeasureContext {
        model,
        effective_config,
        measurer,
        math_renderer,
        cluster_label_base_style,
        cluster_title_wrapping_width,
        cluster_wrap_mode,
    };
    let node_measure_ctx = NodeMeasureContext {
        model,
        effective_config,
        measurer,
        math_renderer,
        node_label_base_style,
        wrapping_width,
        diagram_direction_text,
        node_padding,
        state_padding,
        node_wrap_mode,
        class_html_labels: edge_html_labels,
    };
    let mut inserted_ids: HashSet<&str> = HashSet::new();
    // FlowDB emits subgraphs in reverse storage order before leaf vertices, and Mermaid derives
    // sibling lists by filtering that canonical array without sorting. Preserve that order here.
    for sg in model.subgraphs.iter().rev() {
        charge_adapter_work(&mut work_control, 1)?;
        if !inserted_ids.insert(sg.id.as_str()) {
            continue;
        }
        graph.nodes.push(subgraph_to_elk_node(
            sg,
            parent_by_id.get(&sg.id).cloned(),
            &include_children_groups,
            &cluster_measure_ctx,
        ));
    }

    for node in &model.nodes {
        charge_adapter_work(&mut work_control, 1)?;
        if subgraph_ids.contains(node.id.as_str()) || !inserted_ids.insert(node.id.as_str()) {
            continue;
        }
        graph.nodes.push(flow_node_to_elk_node(
            node,
            parent_by_id.get(&node.id).cloned(),
            node_measure_ctx,
        ));
    }

    apply_cyclic_entry_constraints(
        model,
        effective_config_value,
        &parent_by_id,
        &mut graph.nodes,
        &mut work_control,
    )?;

    charge_adapter_work(&mut work_control, model.edges.len())?;
    let mut edges = Vec::with_capacity(model.edges.len());
    for edge in &model.edges {
        let label = edge_label(
            edge,
            EdgeMeasureContext {
                model,
                effective_config,
                measurer,
                math_renderer,
                edge_label_base_style,
                edge_label_wrapping_width,
                edge_wrap_mode,
                edge_html_labels,
            },
        );
        edges.push(elk::Edge {
            id: edge.id.clone(),
            source: edge.from.clone(),
            target: edge.to.clone(),
            label,
            minlen: edge.length.max(1),
            inside_self_loops_yo: false,
        });
    }
    graph.edges = edges;

    Ok(graph)
}

#[derive(Clone, Copy)]
struct ElkMeasureContext<'a> {
    model: &'a FlowchartModel,
    effective_config: &'a MermaidConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    cluster_label_base_style: &'a TextStyle,
    cluster_title_wrapping_width: f64,
    cluster_wrap_mode: WrapMode,
}

#[derive(Clone, Copy)]
struct NodeMeasureContext<'a> {
    model: &'a FlowchartModel,
    effective_config: &'a MermaidConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    node_label_base_style: &'a TextStyle,
    wrapping_width: f64,
    diagram_direction_text: &'a str,
    node_padding: f64,
    state_padding: f64,
    node_wrap_mode: WrapMode,
    class_html_labels: bool,
}

#[derive(Clone, Copy)]
struct EdgeMeasureContext<'a> {
    model: &'a FlowchartModel,
    effective_config: &'a MermaidConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    edge_label_base_style: &'a TextStyle,
    edge_label_wrapping_width: f64,
    edge_wrap_mode: WrapMode,
    edge_html_labels: bool,
}

fn dir_to_elk_direction(dir: &str) -> elk::Direction {
    match dir.trim().to_uppercase().as_str() {
        "LR" => elk::Direction::Right,
        "RL" => elk::Direction::Left,
        "BT" => elk::Direction::Up,
        "TB" | "TD" => elk::Direction::Down,
        _ => elk::Direction::Down,
    }
}

fn elk_layout_options(effective_config: &serde_json::Value) -> elk::LayoutOptions {
    let model_order = config_string(effective_config, &["elk", "considerModelOrder"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "NONE" => elk::ModelOrderStrategy::None,
                "PREFER_EDGES" => elk::ModelOrderStrategy::PreferEdges,
                "PREFER_NODES" => elk::ModelOrderStrategy::PreferNodes,
                _ => elk::ModelOrderStrategy::NodesAndEdges,
            },
        )
        .unwrap_or_default();
    let cycle_breaking = config_string(effective_config, &["elk", "cycleBreakingStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "DEPTH_FIRST" => elk::CycleBreakingStrategy::DepthFirst,
                "INTERACTIVE" => elk::CycleBreakingStrategy::Interactive,
                "MODEL_ORDER" => elk::CycleBreakingStrategy::ModelOrder,
                "GREEDY_MODEL_ORDER" => elk::CycleBreakingStrategy::GreedyModelOrder,
                _ => elk::CycleBreakingStrategy::Greedy,
            },
        )
        .unwrap_or_default();
    let node_placement = config_string(effective_config, &["elk", "nodePlacementStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "SIMPLE" => elk::NodePlacementStrategy::Simple,
                "NETWORK_SIMPLEX" => elk::NodePlacementStrategy::NetworkSimplex,
                "LINEAR_SEGMENTS" => elk::NodePlacementStrategy::LinearSegments,
                _ => elk::NodePlacementStrategy::BrandesKoepf,
            },
        )
        .unwrap_or_default();
    let node_placement_alignment =
        config_string(effective_config, &["elk", "nodePlacementAlignment"])
            .map(
                |alignment| match alignment.trim().to_ascii_uppercase().as_str() {
                    "LEFTUP" => elk::NodePlacementAlignment::LeftUp,
                    "LEFTDOWN" => elk::NodePlacementAlignment::LeftDown,
                    "RIGHTUP" => elk::NodePlacementAlignment::RightUp,
                    "RIGHTDOWN" => elk::NodePlacementAlignment::RightDown,
                    "BALANCED" => elk::NodePlacementAlignment::Balanced,
                    _ => elk::NodePlacementAlignment::None,
                },
            )
            .unwrap_or_default();
    let self_loop_ordering = config_string(
        effective_config,
        &["elk", "layered", "edgeRouting", "selfLoopOrdering"],
    )
    .map(
        |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
            "REVERSE_STACKED" => elk::SelfLoopOrderingStrategy::ReverseStacked,
            "SEQUENCED" => elk::SelfLoopOrderingStrategy::Sequenced,
            _ => elk::SelfLoopOrderingStrategy::Stacked,
        },
    )
    .unwrap_or_default();

    elk::LayoutOptions {
        layered: elk::LayeredOptions {
            merge_edges: config_bool(effective_config, &["elk", "mergeEdges"]).unwrap_or(false),
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: true,
            inside_self_loops_activate: config_bool(
                effective_config,
                &["elk", "insideSelfLoops", "activate"],
            )
            .unwrap_or(false),
            self_loop_distribution: elk::SelfLoopDistributionStrategy::Equally,
            self_loop_ordering,
            force_node_model_order: config_bool(effective_config, &["elk", "forceNodeModelOrder"])
                .unwrap_or(false),
            consider_model_order: model_order != elk::ModelOrderStrategy::None,
            model_order,
            cycle_breaking,
            node_placement,
            node_placement_alignment,
            ..Default::default()
        },
    }
}

fn apply_cyclic_entry_constraints(
    model: &FlowchartModel,
    effective_config: &serde_json::Value,
    parent_by_id: &HashMap<String, String>,
    nodes: &mut [elk::Node],
    work_control: &mut Option<&mut ElkOperationWorkControl>,
) -> Result<()> {
    if !config_bool(effective_config, &["elk", "keepEntryNodeOnTop"]).unwrap_or(false) {
        return Ok(());
    }

    let entry_ids = find_cyclic_entry_nodes(model, parent_by_id, nodes, work_control)?;
    if entry_ids.is_empty() {
        return Ok(());
    }

    charge_adapter_work(work_control, nodes.len())?;
    for node in nodes {
        if entry_ids.contains(node.id.as_str()) {
            node.layer_constraint = Some(elk::LayerConstraint::First);
        }
    }
    Ok(())
}

fn find_cyclic_entry_nodes(
    model: &FlowchartModel,
    parent_by_id: &HashMap<String, String>,
    canonical_nodes: &[elk::Node],
    work_control: &mut Option<&mut ElkOperationWorkControl>,
) -> Result<HashSet<String>> {
    // Use the final Mermaid adapter node order (reverse subgraphs, then leaf vertices), because
    // keepEntryNodeOnTop nominates the first node in a source-less connected component.
    charge_adapter_work(work_control, canonical_nodes.len())?;
    let node_ids = canonical_nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    charge_adapter_work(work_control, node_ids.len())?;
    let node_id_set = node_ids.iter().copied().collect::<HashSet<_>>();
    let mut parent_group_index: HashMap<Option<&str>, usize> = HashMap::new();
    let mut by_parent = Vec::<(Option<&str>, Vec<&str>)>::new();
    for id in &node_ids {
        charge_adapter_work(work_control, 1)?;
        let parent = parent_by_id.get(*id).map(String::as_str);
        if let Some(&group) = parent_group_index.get(&parent) {
            by_parent[group].1.push(*id);
        } else {
            let group = by_parent.len();
            parent_group_index.insert(parent, group);
            by_parent.push((parent, vec![*id]));
        }
    }

    // Evaluate cyclic entry constraints within each direct hierarchy scope. Partition edges once
    // so a wide hierarchy does not rescan the complete edge list for every scope.
    charge_adapter_work(work_control, model.edges.len())?;
    let mut edges_by_parent: HashMap<Option<&str>, Vec<(&str, &str)>> = HashMap::new();
    for edge in &model.edges {
        let source = edge.from.as_str();
        let target = edge.to.as_str();
        if source == target || !node_id_set.contains(source) || !node_id_set.contains(target) {
            continue;
        }
        let source_parent = parent_by_id.get(source).map(String::as_str);
        if source_parent != parent_by_id.get(target).map(String::as_str) {
            continue;
        }
        edges_by_parent
            .entry(source_parent)
            .or_default()
            .push((source, target));
    }

    let mut entries = HashSet::new();
    for (parent, ids) in &by_parent {
        charge_adapter_work(work_control, ids.len())?;
        let mut incoming_count = ids
            .iter()
            .map(|id| (*id, 0usize))
            .collect::<HashMap<_, _>>();
        charge_adapter_work(work_control, ids.len())?;
        let mut adjacency = ids
            .iter()
            .map(|id| (*id, Vec::<&str>::new()))
            .collect::<HashMap<_, _>>();

        let local_edges = edges_by_parent
            .get(parent)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        charge_adapter_work(work_control, local_edges.len())?;
        for &(source, target) in local_edges {
            if let Some(count) = incoming_count.get_mut(target) {
                *count = checked_adapter_add(work_control, *count, 1)?;
            }
            adjacency.entry(source).or_default().push(target);
            adjacency.entry(target).or_default().push(source);
        }

        let mut component = HashMap::new();
        let mut component_count = 0usize;
        for id in ids {
            charge_adapter_work(work_control, 1)?;
            if component.contains_key(id) {
                continue;
            }
            charge_adapter_work(work_control, 1)?;
            let mut stack = vec![*id];
            while let Some(current) = stack.pop() {
                charge_adapter_work(work_control, 1)?;
                if component.insert(current, component_count).is_some() {
                    continue;
                }
                if let Some(neighbors) = adjacency.get(current) {
                    charge_adapter_work(work_control, neighbors.len())?;
                    for neighbor in neighbors {
                        if !component.contains_key(neighbor) {
                            stack.push(*neighbor);
                        }
                    }
                }
            }
            component_count = checked_adapter_add(work_control, component_count, 1)?;
        }

        charge_adapter_work(work_control, component_count)?;
        let mut has_source = vec![false; component_count];
        charge_adapter_work(work_control, ids.len())?;
        for id in ids {
            if incoming_count.get(id).copied().unwrap_or_default() == 0
                && let Some(component_index) = component.get(id).copied()
            {
                has_source[component_index] = true;
            }
        }

        charge_adapter_work(work_control, component_count)?;
        let mut nominated = vec![false; component_count];
        charge_adapter_work(work_control, ids.len())?;
        for id in ids {
            let Some(component_index) = component.get(id).copied() else {
                continue;
            };
            if !has_source[component_index] && !nominated[component_index] {
                entries.insert((*id).to_string());
                nominated[component_index] = true;
            }
        }
    }

    Ok(entries)
}

fn parent_by_id(
    model: &FlowchartModel,
    work_control: &mut Option<&mut ElkOperationWorkControl>,
) -> Result<HashMap<String, String>> {
    let mut parent_by_id = HashMap::new();
    for sg in model.subgraphs.iter().rev() {
        charge_adapter_work(work_control, sg.nodes.len())?;
        for child in &sg.nodes {
            parent_by_id.insert(child.clone(), sg.id.clone());
        }
    }

    if let Some((child, parent)) =
        first_elk_parent_cycle_assignment(model, &parent_by_id, work_control)?
    {
        return Err(Error::InvalidModel {
            message: format!("Setting {parent} as parent of {child} would create a cycle"),
        });
    }

    Ok(parent_by_id)
}

fn first_elk_parent_cycle_assignment(
    model: &FlowchartModel,
    parent_by_id: &HashMap<String, String>,
    work_control: &mut Option<&mut ElkOperationWorkControl>,
) -> Result<Option<(String, String)>> {
    // ELK does not materialize a Graphlib compound graph, so it must validate the final FlowDB
    // parent map itself. Preserve Mermaid's reverse-subgraph assignment order, but keep this check
    // local to the ELK adapter instead of restoring Dagre's obsolete duplicate preflight.
    charge_adapter_work(work_control, model.subgraphs.len())?;
    let mut assigned = HashSet::with_capacity(model.subgraphs.len());
    let mut assignments = Vec::with_capacity(model.subgraphs.len());
    for child in model
        .subgraphs
        .iter()
        .rev()
        .map(|subgraph| subgraph.id.as_str())
    {
        let Some(parent) = parent_by_id.get(child) else {
            continue;
        };
        if assigned.insert(child) {
            assignments.push((child, parent.as_str()));
        }
    }
    if assignments.is_empty() {
        return Ok(None);
    }

    let node_capacity = checked_adapter_mul(work_control, assignments.len(), 2)?;
    let height = if node_capacity <= 1 {
        0
    } else {
        usize::BITS as usize - (node_capacity - 1).leading_zeros() as usize
    };
    let find_work = checked_adapter_mul(
        work_control,
        checked_adapter_add(work_control, height, 1)?,
        4,
    )?;
    let union_work = checked_adapter_mul(work_control, assignments.len(), find_work)?;
    let linear_work = checked_adapter_add(
        work_control,
        checked_adapter_mul(work_control, node_capacity, 6)?,
        checked_adapter_mul(work_control, assignments.len(), 2)?,
    )?;
    charge_adapter_work(
        work_control,
        checked_adapter_add(work_control, union_work, linear_work)?,
    )?;

    fn intern<'a>(
        id: &'a str,
        index_by_id: &mut HashMap<&'a str, usize>,
        parents: &mut Vec<usize>,
        ranks: &mut Vec<u8>,
    ) -> usize {
        if let Some(&index) = index_by_id.get(id) {
            return index;
        }
        let index = parents.len();
        parents.push(index);
        ranks.push(0);
        index_by_id.insert(id, index);
        index
    }

    fn find(parents: &mut [usize], mut index: usize) -> usize {
        let mut root = index;
        while parents[root] != root {
            root = parents[root];
        }
        while parents[index] != index {
            let next = parents[index];
            parents[index] = root;
            index = next;
        }
        root
    }

    let mut index_by_id = HashMap::with_capacity(node_capacity);
    let mut parents = Vec::with_capacity(node_capacity);
    let mut ranks = Vec::with_capacity(node_capacity);
    for (child, parent) in assignments {
        let child_index = intern(child, &mut index_by_id, &mut parents, &mut ranks);
        let parent_index = intern(parent, &mut index_by_id, &mut parents, &mut ranks);
        let child_root = find(&mut parents, child_index);
        let parent_root = find(&mut parents, parent_index);
        if child_root == parent_root {
            return Ok(Some((child.to_string(), parent.to_string())));
        }
        if ranks[child_root] < ranks[parent_root] {
            parents[child_root] = parent_root;
        } else {
            parents[parent_root] = child_root;
            if ranks[child_root] == ranks[parent_root] {
                ranks[child_root] = ranks[child_root].saturating_add(1);
            }
        }
    }

    Ok(None)
}

// Heavy-light decomposition keeps hierarchy preprocessing and retained memory linear while making
// repeated common-ancestor queries logarithmic in branching depth (and constant on a heavy chain).
struct FlowchartHierarchyIndex<'a> {
    ids: Vec<&'a str>,
    index_by_id: HashMap<&'a str, usize>,
    parent: Vec<Option<usize>>,
    depth: Vec<usize>,
    root: Vec<usize>,
    chain_head: Vec<usize>,
}

impl<'a> FlowchartHierarchyIndex<'a> {
    fn build(
        model: &'a FlowchartModel,
        parent_by_id: &HashMap<String, String>,
        work_control: &mut Option<&mut ElkOperationWorkControl>,
    ) -> Result<Self> {
        let item_capacity =
            checked_adapter_add(work_control, model.nodes.len(), model.subgraphs.len())?;
        charge_adapter_work(work_control, item_capacity)?;
        let mut ids = Vec::with_capacity(item_capacity);
        let mut index_by_id = HashMap::with_capacity(item_capacity);
        for id in model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .chain(model.subgraphs.iter().map(|subgraph| subgraph.id.as_str()))
        {
            if let Entry::Vacant(entry) = index_by_id.entry(id) {
                let index = ids.len();
                ids.push(id);
                entry.insert(index);
            }
        }

        charge_adapter_work(work_control, ids.len())?;
        let mut parent = vec![None; ids.len()];
        for (index, id) in ids.iter().copied().enumerate() {
            parent[index] = parent_by_id
                .get(id)
                .and_then(|parent| index_by_id.get(parent.as_str()).copied());
        }

        charge_adapter_work(work_control, ids.len())?;
        let mut children = vec![Vec::new(); ids.len()];
        let mut roots = Vec::new();
        for (index, parent) in parent.iter().copied().enumerate() {
            match parent {
                Some(parent) => children[parent].push(index),
                None => roots.push(index),
            }
        }

        let hierarchy_stage_work = checked_adapter_mul(work_control, ids.len(), 2)?;
        charge_adapter_work(work_control, hierarchy_stage_work)?;
        let mut depth = vec![0usize; ids.len()];
        let mut root = vec![0usize; ids.len()];
        let mut preorder = Vec::with_capacity(ids.len());
        let mut stack = roots
            .iter()
            .rev()
            .copied()
            .map(|node| (node, node))
            .collect::<Vec<_>>();
        while let Some((node, root_node)) = stack.pop() {
            root[node] = root_node;
            preorder.push(node);
            for child in children[node].iter().rev().copied() {
                depth[child] = checked_adapter_add(work_control, depth[node], 1)?;
                stack.push((child, root_node));
            }
        }

        charge_adapter_work(work_control, hierarchy_stage_work)?;
        let mut subtree_size = vec![1usize; ids.len()];
        let mut heavy_child = vec![None; ids.len()];
        for node in preorder.iter().rev().copied() {
            let mut largest_child = 0usize;
            for child in children[node].iter().copied() {
                subtree_size[node] =
                    checked_adapter_add(work_control, subtree_size[node], subtree_size[child])?;
                if subtree_size[child] > largest_child {
                    largest_child = subtree_size[child];
                    heavy_child[node] = Some(child);
                }
            }
        }

        charge_adapter_work(work_control, hierarchy_stage_work)?;
        let mut chain_head = vec![0usize; ids.len()];
        let mut chains = roots
            .iter()
            .rev()
            .copied()
            .map(|root| (root, root))
            .collect::<Vec<_>>();
        while let Some((start, head)) = chains.pop() {
            let mut current = Some(start);
            while let Some(node) = current {
                chain_head[node] = head;
                for child in children[node].iter().rev().copied() {
                    if Some(child) != heavy_child[node] {
                        chains.push((child, child));
                    }
                }
                current = heavy_child[node];
            }
        }

        Ok(Self {
            ids,
            index_by_id,
            parent,
            depth,
            root,
            chain_head,
        })
    }

    fn len(&self) -> usize {
        self.ids.len()
    }

    fn common_ancestor_index(
        &self,
        left: &str,
        right: &str,
        work_control: &mut Option<&mut ElkOperationWorkControl>,
    ) -> Result<Option<usize>> {
        let (Some(mut left), Some(mut right)) = (
            self.index_by_id.get(left).copied(),
            self.index_by_id.get(right).copied(),
        ) else {
            return Ok(None);
        };

        // Mermaid's findCommonAncestor is endpoint-inclusive, except that a self edge resolves to
        // the endpoint's parent (None here represents Mermaid's synthetic root).
        if left == right {
            return Ok(self.parent[left]);
        }
        if self.root[left] != self.root[right] {
            return Ok(None);
        }

        while self.chain_head[left] != self.chain_head[right] {
            charge_adapter_work(work_control, 1)?;
            let left_head = self.chain_head[left];
            let right_head = self.chain_head[right];
            if self.depth[left_head] > self.depth[right_head] {
                left = self.parent[left_head]
                    .expect("same-root heavy-light query has a parent above the deeper chain");
            } else {
                right = self.parent[right_head]
                    .expect("same-root heavy-light query has a parent above the deeper chain");
            }
        }
        charge_adapter_work(work_control, 1)?;
        Ok(Some(if self.depth[left] <= self.depth[right] {
            left
        } else {
            right
        }))
    }

    #[cfg(test)]
    fn common_ancestor_id(
        &self,
        left: &str,
        right: &str,
        work_control: &mut Option<&mut ElkOperationWorkControl>,
    ) -> Result<Option<&'a str>> {
        self.common_ancestor_index(left, right, work_control)
            .map(|ancestor| ancestor.map(|ancestor| self.ids[ancestor]))
    }
}

struct UnmarkedHierarchyPaths {
    next: Vec<usize>,
    sentinel: usize,
}

impl UnmarkedHierarchyPaths {
    fn new(node_count: usize) -> Self {
        Self {
            next: (0..=node_count).collect(),
            sentinel: node_count,
        }
    }

    fn find(&mut self, node: usize) -> usize {
        let mut root = node;
        while self.next[root] != root {
            root = self.next[root];
        }
        let mut current = node;
        while self.next[current] != current {
            let next = self.next[current];
            self.next[current] = root;
            current = next;
        }
        root
    }

    fn remove(&mut self, node: usize, parent: Option<usize>) {
        let parent = parent.unwrap_or(self.sentinel);
        let next = self.find(parent);
        self.next[node] = next;
    }
}

fn include_children_groups<'a>(
    model: &'a FlowchartModel,
    parent_by_id: &HashMap<String, String>,
    work_control: &mut Option<&mut ElkOperationWorkControl>,
) -> Result<HashSet<&'a str>> {
    if model.subgraphs.is_empty() || model.edges.is_empty() {
        return Ok(HashSet::new());
    }

    let item_count = checked_adapter_add(work_control, model.nodes.len(), model.subgraphs.len())?;
    charge_adapter_work(work_control, item_count)?;
    let valid_ids = model
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .chain(model.subgraphs.iter().map(|subgraph| subgraph.id.as_str()))
        .collect::<HashSet<_>>();
    charge_adapter_work(work_control, model.edges.len())?;
    let cross_parent_edges = model
        .edges
        .iter()
        .filter(|edge| {
            valid_ids.contains(edge.from.as_str())
                && valid_ids.contains(edge.to.as_str())
                && parent_by_id.get(&edge.from) != parent_by_id.get(&edge.to)
        })
        .collect::<Vec<_>>();
    if cross_parent_edges.is_empty() {
        return Ok(HashSet::new());
    }

    let hierarchy = FlowchartHierarchyIndex::build(model, parent_by_id, work_control)?;
    // Cross-parent edges override direction-induced SeparateChildren on both endpoint-to-LCA
    // paths. Mermaid's walk includes both endpoints and the common ancestor; path compression may
    // skip only nodes already marked by an earlier edge.
    charge_adapter_work(work_control, hierarchy.len())?;
    let mut unmarked = UnmarkedHierarchyPaths::new(hierarchy.len());
    let mut include_children = HashSet::new();
    for edge in cross_parent_edges {
        let ancestor =
            hierarchy.common_ancestor_index(edge.from.as_str(), edge.to.as_str(), work_control)?;
        mark_include_children_path(
            edge.from.as_str(),
            ancestor,
            &hierarchy,
            &mut unmarked,
            &mut include_children,
            work_control,
        )?;
        mark_include_children_path(
            edge.to.as_str(),
            ancestor,
            &hierarchy,
            &mut unmarked,
            &mut include_children,
            work_control,
        )?;
    }
    Ok(include_children)
}

fn mark_include_children_path<'a>(
    node_id: &str,
    ancestor: Option<usize>,
    hierarchy: &FlowchartHierarchyIndex<'a>,
    unmarked: &mut UnmarkedHierarchyPaths,
    include_children: &mut HashSet<&'a str>,
    work_control: &mut Option<&mut ElkOperationWorkControl>,
) -> Result<()> {
    let Some(start) = hierarchy.index_by_id.get(node_id).copied() else {
        return Ok(());
    };
    let stop_depth = ancestor.map(|ancestor| hierarchy.depth[ancestor]);
    loop {
        let node = unmarked.find(start);
        if node == unmarked.sentinel
            || stop_depth.is_some_and(|stop_depth| hierarchy.depth[node] < stop_depth)
        {
            break;
        }
        charge_adapter_work(work_control, 1)?;
        include_children.insert(hierarchy.ids[node]);
        let reached_ancestor = Some(node) == ancestor;
        unmarked.remove(node, hierarchy.parent[node]);
        if reached_ancestor {
            break;
        }
    }
    Ok(())
}

fn subgraph_label(sg: &FlowSubgraph, ctx: &ElkMeasureContext<'_>) -> Option<elk::Label> {
    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let text_style = flowchart_effective_text_style_for_classes(
        ctx.cluster_label_base_style,
        &ctx.model.class_defs,
        &sg.classes,
        &sg.styles,
    );
    let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
        measurer: ctx.measurer,
        raw_label: &sg.title,
        label_type,
        style: text_style.as_ref(),
        max_width_px: Some(ctx.cluster_title_wrapping_width),
        wrap_mode: ctx.cluster_wrap_mode,
        config: ctx.effective_config,
        math_renderer: ctx.math_renderer,
    });
    Some(elk::Label {
        width: metrics.width.max(1.0),
        height: (metrics.height - 2.0).max(1.0),
    })
}

fn node_dimensions_and_label(
    node: &FlowNode,
    ctx: NodeMeasureContext<'_>,
) -> (f64, f64, elk::Label) {
    let raw_label = node.label.as_deref().unwrap_or(&node.id);
    let label_type = node.label_type.as_deref().unwrap_or("text");
    let node_text_style = flowchart_effective_text_style_for_node_classes(
        ctx.node_label_base_style,
        &ctx.model.class_defs,
        &node.classes,
        &node.styles,
    );
    let mut metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
        measurer: ctx.measurer,
        raw_label,
        label_type,
        style: node_text_style.as_ref(),
        max_width_px: Some(ctx.wrapping_width),
        wrap_mode: ctx.node_wrap_mode,
        config: ctx.effective_config,
        math_renderer: ctx.math_renderer,
    });
    if ctx.node_wrap_mode == WrapMode::SvgLike
        && label_type != "markdown"
        && !raw_label.contains('<')
        && !raw_label.contains('>')
        && super::is_flowchart_process_shape(node.layout_shape.as_deref().unwrap_or("squareRect"))
    {
        let plain = flowchart_label_plain_text_for_layout(raw_label, label_type, false);
        metrics.width = flowchart_svg_plain_computed_width_px(
            ctx.measurer,
            &plain,
            node_text_style.as_ref(),
            Some(ctx.wrapping_width),
        );
    }
    if ctx.node_wrap_mode == WrapMode::HtmlLike && ctx.class_html_labels {
        flowchart_apply_html_node_class_box_metrics(
            &mut metrics,
            raw_label,
            label_type,
            node_text_style.as_ref(),
            &ctx.model.class_defs,
            &node.classes,
        );
    }

    let label = elk::Label {
        width: metrics.width,
        height: metrics.height,
    };
    let (width, height) = node_layout_dimensions(NodeLayoutDimensionsRequest {
        layout_shape: node.layout_shape.as_deref(),
        layout_direction: ctx.diagram_direction_text,
        metrics,
        padding: ctx.node_padding,
        state_padding: ctx.state_padding,
        node_icon: node.icon.as_deref(),
        node_img: node.img.as_deref(),
        node_pos: node.pos.as_deref(),
        node_asset_width: node.asset_width,
        node_asset_height: node.asset_height,
    });

    (width, height, label)
}

fn edge_label(edge: &FlowEdge, ctx: EdgeMeasureContext<'_>) -> Option<elk::Label> {
    if !edge_label_is_non_empty(edge) {
        return None;
    }

    let label_text = edge.label.as_deref().unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let edge_text_style = flowchart_effective_text_style_for_classes(
        ctx.edge_label_base_style,
        &ctx.model.class_defs,
        &edge.classes,
        &edge.style,
    );
    let metrics = if label_type == "markdown" && ctx.edge_wrap_mode != WrapMode::HtmlLike {
        crate::text::measure_wrapped_markdown_with_inline_styles(
            ctx.measurer,
            label_text,
            edge_text_style.as_ref(),
            Some(ctx.edge_label_wrapping_width),
            ctx.edge_wrap_mode,
        )
    } else {
        flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
            measurer: ctx.measurer,
            raw_label: label_text,
            label_type,
            style: edge_text_style.as_ref(),
            max_width_px: Some(ctx.edge_label_wrapping_width),
            wrap_mode: ctx.edge_wrap_mode,
            config: ctx.effective_config,
            math_renderer: ctx.math_renderer,
        })
    };

    let (width, height) = if ctx.edge_html_labels {
        (metrics.width.max(1.0), metrics.height.max(1.0))
    } else {
        (
            (metrics.width + 4.0).max(1.0),
            (metrics.height + 4.0).max(1.0),
        )
    };

    Some(elk::Label { width, height })
}

fn edge_label_is_non_empty(edge: &FlowEdge) -> bool {
    edge.label
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

fn flow_node_to_elk_node(
    node: &FlowNode,
    parent: Option<String>,
    ctx: NodeMeasureContext<'_>,
) -> elk::Node {
    let (width, height, label) = node_dimensions_and_label(node, ctx);
    elk::Node {
        id: node.id.clone(),
        kind: elk::NodeKind::Leaf,
        width,
        height,
        parent,
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        label: Some(label),
    }
}

fn subgraph_to_elk_node(
    sg: &FlowSubgraph,
    parent: Option<String>,
    include_children_groups: &HashSet<&str>,
    ctx: &ElkMeasureContext<'_>,
) -> elk::Node {
    elk::Node {
        id: sg.id.clone(),
        kind: elk::NodeKind::Group,
        width: 0.0,
        height: 0.0,
        parent,
        // Use the resolved dir, not has_explicit_dir: FlowDB may populate it through inheritDir,
        // and Mermaid applies SeparateChildren whenever that resolved direction exists.
        direction: sg.dir.as_deref().map(dir_to_elk_direction),
        hierarchy_handling: if include_children_groups.contains(sg.id.as_str()) {
            Some(elk::HierarchyHandling::IncludeChildren)
        } else if sg.dir.is_some() {
            Some(elk::HierarchyHandling::SeparateChildren)
        } else {
            None
        },
        layer_constraint: None,
        label: subgraph_label(sg, ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};
    use serde_json::json;

    const NON_LATTICE_COMPUTED_LENGTH_PX: f64 = 73.123_456_789;

    struct NonLatticeComputedLengthMeasurer;

    impl TextMeasurer for NonLatticeComputedLengthMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> crate::text::TextMetrics {
            crate::text::TextMetrics {
                width: 40.0,
                height: 18.0,
                line_count: 1,
            }
        }

        fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            NON_LATTICE_COMPUTED_LENGTH_PX
        }
    }

    #[test]
    fn elk_preserves_operation_computed_length_precision() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"htmlLabels\": false, \"flowchart\": {\"htmlLabels\": false}}}%%\nflowchart TB\nA[alpha]\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected");
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let graph = build_flowchart_elk_graph(
            model,
            &parsed.metadata().effective_config,
            &NonLatticeComputedLengthMeasurer,
            None,
        )
        .expect("ELK graph");
        let label = graph
            .nodes
            .iter()
            .find(|node| node.id == "A")
            .and_then(|node| node.label)
            .expect("node A label");

        assert_eq!(label.width, NON_LATTICE_COMPUTED_LENGTH_PX);
    }

    fn node(id: &str, label: Option<&str>, label_type: Option<&str>) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            label: label.map(str::to_string),
            label_type: label_type.map(str::to_string),
            layout_shape: Some("squareRect".to_string()),
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }
    }

    fn edge(id: &str, from: &str, to: &str, label: Option<&str>) -> FlowEdge {
        FlowEdge {
            id: id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            label: label.map(str::to_string),
            label_type: Some("text".to_string()),
            edge_type: Some("arrow_point".to_string()),
            arrow: "-->".to_string(),
            is_user_defined_id: false,
            stroke: Some("normal".to_string()),
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    fn model(nodes: Vec<FlowNode>, edges: Vec<FlowEdge>) -> FlowchartModel {
        FlowchartModel {
            keyword: "graph".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: IndexMap::new(),
            direction: Some("TD".to_string()),
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes,
            edges,
            subgraphs: Vec::new(),
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        }
    }

    fn subgraph(id: String, nodes: Vec<String>) -> FlowSubgraph {
        FlowSubgraph {
            title: id.clone(),
            id,
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes,
        }
    }

    fn nested_model(depth: usize, target: Option<&str>) -> FlowchartModel {
        let mut nested = model(
            vec![
                node("leaf", Some("leaf"), None),
                node("sibling", Some("sibling"), None),
                node("outside", Some("outside"), None),
            ],
            target
                .map(|target| vec![edge("cross", "leaf", target, None)])
                .unwrap_or_default(),
        );
        if depth == 0 {
            return nested;
        }
        nested.subgraphs.push(subgraph(
            "group-0".to_string(),
            vec!["leaf".to_string(), "sibling".to_string()],
        ));
        for level in 1..depth {
            nested.subgraphs.push(subgraph(
                format!("group-{level}"),
                vec![format!("group-{}", level - 1)],
            ));
        }
        nested
    }

    fn nested_model_with_repeated_cross_edges(depth: usize, edge_count: usize) -> FlowchartModel {
        let mut nested = nested_model(depth, None);
        nested.edges = (0..edge_count)
            .map(|index| edge(&format!("cross-{index}"), "leaf", "outside", None))
            .collect();
        nested
    }

    fn projection_fixture() -> (FlowchartModel, elk::Graph, elk::LayoutResult) {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", None)],
        );
        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();
        let layout = elk::LayoutResult {
            nodes: vec![
                elk::NodeLayout {
                    id: "A".to_string(),
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                elk::NodeLayout {
                    id: "B".to_string(),
                    x: 20.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
            ],
            edges: vec![elk::EdgeLayout {
                id: "L-A-B".to_string(),
                points: vec![
                    elk::Point { x: 5.0, y: 0.0 },
                    elk::Point { x: 15.0, y: 0.0 },
                ],
                labels: Vec::new(),
            }],
        };
        (model, graph, layout)
    }

    fn adapter_graph_and_work(
        model: &FlowchartModel,
        config: &MermaidConfig,
    ) -> (elk::Graph, usize) {
        let meter = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut work_control = ElkOperationWorkControl::new(meter);
        let graph = build_flowchart_elk_graph_with_work_control(
            model,
            config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
            Some(&mut work_control),
        )
        .unwrap();
        (graph, work_control.adapter_work())
    }

    fn operation_seed() -> elk::ElkOperationSeed {
        elk::ElkOperationSeed::from_operation_seed(
            std::num::NonZeroU64::new(0x656c_6b2d_776f_726b).expect("nonzero operation seed"),
        )
    }

    #[test]
    fn flowchart_elk_kernel_interruption_maps_to_layout_work_resource_error() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", None)],
        );
        let config = MermaidConfig::default();
        let (_, adapter_work) = adapter_graph_and_work(&model, &config);
        let meter = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, adapter_work)
                .unwrap(),
        ));

        let error = layout_flowchart_elk_typed_with_operation_seed(
            &model,
            &config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
            operation_seed(),
            meter,
        )
        .unwrap_err();

        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.cause, crate::resources::ResourceLimitCause::Ceiling);
        assert_eq!(limit.limit, "max_layout_work_units");
        assert_eq!(limit.max, adapter_work);
        assert!(limit.actual > limit.max);
    }

    #[test]
    fn flowchart_elk_checked_work_overflow_maps_to_the_resource_contract() {
        let work_control = ElkOperationWorkControl::new(Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )));
        let error = work_control.checked_mul(usize::MAX, 2).unwrap_err();
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(
            limit.cause,
            crate::resources::ResourceLimitCause::ArithmeticOverflow
        );
        assert_eq!(limit.limit, "max_layout_work_units");
    }

    #[test]
    fn flowchart_elk_adapter_work_is_linear_for_deep_edgeless_hierarchies() {
        let config = MermaidConfig::default();
        let (_, work_16) = adapter_graph_and_work(&nested_model(16, None), &config);
        let (_, work_32) = adapter_graph_and_work(&nested_model(32, None), &config);
        let (_, work_48) = adapter_graph_and_work(&nested_model(48, None), &config);

        assert_eq!(work_32 - work_16, work_48 - work_32);
    }

    #[test]
    fn flowchart_elk_first_cross_scope_edge_pays_linear_index_and_path_work() {
        let config = MermaidConfig::default();
        let (_, local_16) = adapter_graph_and_work(&nested_model(16, Some("sibling")), &config);
        let (_, cross_16) = adapter_graph_and_work(&nested_model(16, Some("outside")), &config);
        let (_, local_32) = adapter_graph_and_work(&nested_model(32, Some("sibling")), &config);
        let (_, cross_32) = adapter_graph_and_work(&nested_model(32, Some("outside")), &config);

        let ancestor_work_16 = cross_16 - local_16;
        let ancestor_work_32 = cross_32 - local_32;
        assert!(ancestor_work_16 > 0);
        assert!(ancestor_work_32 > ancestor_work_16);
        // Each added chain-level scope contributes nine HLD-index units, one DSU slot, and one
        // first-time path mark. The heavy-chain LCA query itself stays constant for this topology.
        assert_eq!(ancestor_work_32 - ancestor_work_16, (32 - 16) * 11);
    }

    #[test]
    fn flowchart_elk_cross_scope_work_has_no_depth_times_repeated_edge_term() {
        let config = MermaidConfig::default();
        let work = |depth, edge_count| {
            adapter_graph_and_work(
                &nested_model_with_repeated_cross_edges(depth, edge_count),
                &config,
            )
            .1
        };

        let work_8_1 = work(8, 1);
        let work_8_16 = work(8, 16);
        let work_32_1 = work(32, 1);
        let work_32_16 = work(32, 16);

        assert_eq!(work_32_16 - work_8_16, work_32_1 - work_8_1);
        assert_eq!(work_32_16 - work_32_1, work_8_16 - work_8_1);
    }

    #[test]
    fn flowchart_elk_projection_work_has_an_independent_exact_budget() {
        // 3 source-index rows + 2 projected nodes + 2 layout-index rows + 7 edge units
        // + 12 bounds units + 5 DOM-order units.
        const EXPECTED_PROJECTION_WORK: usize = 31;

        let (model, graph, layout) = projection_fixture();
        let meter = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::ResourceLimitId::MaxLayoutWorkUnits,
                    EXPECTED_PROJECTION_WORK,
                )
                .unwrap(),
        ));
        let mut work_control = ElkOperationWorkControl::new(meter);
        let measured = flowchart_layout_from_elk_with_work_control(
            &model,
            &MermaidConfig::default(),
            &graph,
            layout,
            Some(&mut work_control),
        )
        .unwrap();

        assert_eq!(work_control.adapter_work(), EXPECTED_PROJECTION_WORK);

        let (model, graph, layout) = projection_fixture();
        let unmetered =
            flowchart_layout_from_elk(&model, &MermaidConfig::default(), &graph, layout).unwrap();
        assert_eq!(
            serde_json::to_value(&measured).unwrap(),
            serde_json::to_value(&unmetered).unwrap()
        );
        assert_eq!(
            measured.dom_node_order_by_root,
            unmetered.dom_node_order_by_root
        );
    }

    #[test]
    fn flowchart_elk_projection_rejection_does_not_advance_past_completed_work() {
        const WORK_BEFORE_DOM_ORDER: usize = 26;

        let (model, graph, layout) = projection_fixture();
        let meter = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::ResourceLimitId::MaxLayoutWorkUnits,
                    WORK_BEFORE_DOM_ORDER,
                )
                .unwrap(),
        ));
        let mut work_control = ElkOperationWorkControl::new(meter);
        let error = flowchart_layout_from_elk_with_work_control(
            &model,
            &MermaidConfig::default(),
            &graph,
            layout,
            Some(&mut work_control),
        )
        .unwrap_err();

        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.max, WORK_BEFORE_DOM_ORDER);
        assert_eq!(limit.actual, WORK_BEFORE_DOM_ORDER + 5);
        assert_eq!(work_control.adapter_work(), WORK_BEFORE_DOM_ORDER);
        assert!(work_control.charge_adapter(1).is_err());
        assert_eq!(work_control.adapter_work(), WORK_BEFORE_DOM_ORDER);
    }

    #[test]
    fn flowchart_elk_projection_rejects_the_complete_edge_tranche_atomically() {
        // 4 source-index rows + 2 projected nodes + 2 layout-index rows. Two seven-unit edges must
        // be accepted together before the output allocation starts.
        const WORK_BEFORE_EDGES: usize = 8;
        const ONE_EDGE_WORK: usize = 7;

        let (mut model, _, mut layout) = projection_fixture();
        let mut second_source = model.edges[0].clone();
        second_source.id = "L-A-B-2".to_string();
        model.edges.push(second_source);
        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();
        let mut second_layout = layout.edges[0].clone();
        second_layout.id = "L-A-B-2".to_string();
        layout.edges.push(second_layout);

        let max_work = WORK_BEFORE_EDGES + 2 * ONE_EDGE_WORK - 1;
        let meter = Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, max_work)
                .unwrap(),
        ));
        let mut work_control = ElkOperationWorkControl::new(meter);
        let error = flowchart_layout_from_elk_with_work_control(
            &model,
            &MermaidConfig::default(),
            &graph,
            layout,
            Some(&mut work_control),
        )
        .unwrap_err();

        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.actual, WORK_BEFORE_EDGES + 2 * ONE_EDGE_WORK);
        assert_eq!(work_control.adapter_work(), WORK_BEFORE_EDGES);
    }

    #[test]
    fn flowchart_elk_lca_matches_mermaid_endpoint_inclusive_semantics() {
        fn mermaid_common_ancestor(
            left: &str,
            right: &str,
            parent_by_id: &HashMap<String, String>,
        ) -> Option<String> {
            if left == right {
                return parent_by_id.get(left).cloned();
            }
            let mut visited = HashSet::new();
            let mut current = Some(left);
            while let Some(id) = current {
                visited.insert(id);
                if id == right {
                    return Some(id.to_string());
                }
                current = parent_by_id.get(id).map(String::as_str);
            }
            current = Some(right);
            while let Some(id) = current {
                if visited.contains(id) {
                    return Some(id.to_string());
                }
                current = parent_by_id.get(id).map(String::as_str);
            }
            None
        }

        let mut model = nested_model(3, None);
        model.subgraphs.push(subgraph(
            "other-root".to_string(),
            vec!["outside".to_string()],
        ));
        let mut no_work_control = None;
        let parent_by_id = parent_by_id(&model, &mut no_work_control).unwrap();
        let hierarchy =
            FlowchartHierarchyIndex::build(&model, &parent_by_id, &mut no_work_control).unwrap();
        let ids = [
            "leaf",
            "sibling",
            "outside",
            "group-0",
            "group-1",
            "group-2",
            "other-root",
        ];

        for left in ids {
            for right in ids {
                assert_eq!(
                    hierarchy
                        .common_ancestor_id(left, right, &mut no_work_control)
                        .unwrap()
                        .map(str::to_string),
                    mermaid_common_ancestor(left, right, &parent_by_id),
                    "left={left} right={right}"
                );
            }
        }
    }

    #[test]
    fn flowchart_elk_adapter_budget_gates_are_exact_and_rejections_do_not_advance() {
        let model = nested_model(8, Some("outside"));
        let config = MermaidConfig::default();
        let (expected, exact_work) = adapter_graph_and_work(&model, &config);
        let policy = |limit| {
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap()
        };

        for limit in [exact_work, exact_work + 1] {
            let mut work_control =
                ElkOperationWorkControl::new(Arc::new(OperationWorkMeter::new(policy(limit))));
            let actual = build_flowchart_elk_graph_with_work_control(
                &model,
                &config,
                &crate::text::VendoredFontMetricsTextMeasurer::default(),
                None,
                Some(&mut work_control),
            )
            .unwrap();
            assert_eq!(work_control.adapter_work(), exact_work);
            assert_eq!(actual.nodes.len(), expected.nodes.len());
            assert_eq!(actual.edges.len(), expected.edges.len());
        }

        let mut below =
            ElkOperationWorkControl::new(Arc::new(OperationWorkMeter::new(policy(exact_work - 1))));
        let error = build_flowchart_elk_graph_with_work_control(
            &model,
            &config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
            Some(&mut below),
        )
        .unwrap_err();
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.max, exact_work - 1);
        let charged_before_retry = below.adapter_work();
        assert!(charged_before_retry < exact_work);
        assert!(below.charge_adapter(1).is_err());
        assert_eq!(below.adapter_work(), charged_before_retry);
    }

    #[test]
    fn flowchart_elk_adapter_rejects_before_graph_mutation_or_label_measurement() {
        struct CountingMeasurer(std::cell::Cell<usize>);

        impl TextMeasurer for CountingMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> crate::text::TextMetrics {
                self.0.set(self.0.get() + 1);
                crate::text::TextMetrics {
                    width: 1.0,
                    height: 1.0,
                    line_count: 1,
                }
            }
        }

        let model = nested_model(1, None);
        let config = MermaidConfig::default();
        let measurer = CountingMeasurer(std::cell::Cell::new(0));
        let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let mut work_control =
            ElkOperationWorkControl::new(Arc::new(OperationWorkMeter::new(policy)));

        let error = build_flowchart_elk_graph_with_work_control(
            &model,
            &config,
            &measurer,
            None,
            Some(&mut work_control),
        )
        .unwrap_err();

        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(work_control.adapter_work(), 0);
        assert_eq!(measurer.0.get(), 0);
    }

    #[test]
    fn flowchart_elk_graph_adapter_preserves_basic_nodes_and_edges() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", Some("go"))],
        );
        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        assert_eq!(graph.id, "root");
        assert_eq!(graph.direction, elk::Direction::Down);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert!(graph.nodes.iter().all(|n| n.kind == elk::NodeKind::Leaf));
        assert!(graph.nodes.iter().all(|n| n.width > 0.0 && n.height > 0.0));
        assert_eq!(graph.edges[0].source, "A");
        assert_eq!(graph.edges[0].target, "B");
        assert!(graph.edges[0].label.is_some());
    }

    #[test]
    fn flowchart_elk_graph_adapter_preserves_subgraph_parent_mapping() {
        let mut model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", None)],
        );
        model.subgraphs.push(FlowSubgraph {
            id: "cluster".to_string(),
            title: "Cluster".to_string(),
            dir: Some("LR".to_string()),
            has_explicit_dir: true,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string()],
        });

        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let cluster = graph.nodes.iter().find(|n| n.id == "cluster").unwrap();
        let child = graph.nodes.iter().find(|n| n.id == "A").unwrap();
        let outside = graph.nodes.iter().find(|n| n.id == "B").unwrap();

        assert_eq!(cluster.kind, elk::NodeKind::Group);
        assert_eq!(cluster.direction, Some(elk::Direction::Right));
        assert_eq!(child.parent.as_deref(), Some("cluster"));
        assert_eq!(outside.parent, None);
    }

    #[test]
    fn flowchart_elk_parent_validation_reports_mermaids_first_assignment_cycle() {
        let mut model = model(Vec::new(), Vec::new());
        model.subgraphs = [("A", "B"), ("B", "A"), ("C", "D"), ("D", "C")]
            .into_iter()
            .map(|(parent, child)| subgraph(parent.to_string(), vec![child.to_string()]))
            .collect();

        let error = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap_err();

        let Error::InvalidModel { message } = error else {
            panic!("expected InvalidModel");
        };
        assert_eq!(message, "Setting D as parent of C would create a cycle");
    }

    #[test]
    fn flowchart_elk_adapter_keeps_empty_subgraphs_as_groups() {
        let mut model = model(
            vec![node("a", Some("a"), None), node("b", Some("b"), None)],
            vec![edge("L-a-b", "a", "b", None)],
        );
        model.direction = Some("LR".to_string());
        model.subgraphs.push(FlowSubgraph {
            id: "A".to_string(),
            title: "A".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["a".to_string(), "b".to_string()],
        });
        model.subgraphs.push(FlowSubgraph {
            id: "B".to_string(),
            title: "B".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: Vec::new(),
        });

        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let empty = graph.nodes.iter().find(|node| node.id == "B").unwrap();

        assert_eq!(empty.kind, elk::NodeKind::Group);
        assert_eq!(empty.label.map(|label| label.height), Some(22.0));
    }

    #[test]
    fn flowchart_elk_graph_adapter_matches_mermaid_get_data_node_order() {
        let mut model = model(
            vec![
                node("root-a", Some("root-a"), None),
                node("cluster", Some("cluster"), None),
                node("cluster-a", Some("cluster-a"), None),
                node("cluster-b", Some("cluster-b"), None),
                node("later-cluster", Some("later-cluster"), None),
                node("later-a", Some("later-a"), None),
                node("root-b", Some("root-b"), None),
            ],
            vec![],
        );
        model.subgraphs.push(FlowSubgraph {
            id: "cluster".to_string(),
            title: "Cluster".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["cluster-a".to_string(), "cluster-b".to_string()],
        });
        model.subgraphs.push(FlowSubgraph {
            id: "later-cluster".to_string(),
            title: "Later Cluster".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["later-a".to_string()],
        });

        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let ids: Vec<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "later-cluster",
                "cluster",
                "root-a",
                "cluster-a",
                "cluster-b",
                "later-a",
                "root-b"
            ]
        );
    }

    #[test]
    fn flowchart_elk_dom_order_matches_mermaid_add_vertices_recursion() {
        let mut model = model(
            vec![
                node("A", Some("A"), None),
                node("B", Some("B"), None),
                node("C", Some("C"), None),
                node("D", Some("D"), None),
                node("E", Some("E"), None),
                node("F", Some("F"), None),
                node("G", Some("G"), None),
            ],
            vec![],
        );
        model.subgraphs.push(FlowSubgraph {
            id: "foo".to_string(),
            title: "Foo SubGraph".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["C".to_string(), "D".to_string()],
        });
        model.subgraphs.push(FlowSubgraph {
            id: "bar".to_string(),
            title: "Bar SubGraph".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["E".to_string(), "F".to_string()],
        });

        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let ids = mermaid_elk_adapter_dom_order(&graph);
        let actual: Vec<&str> = ids.iter().map(String::as_str).collect();
        assert_eq!(
            actual,
            vec!["bar", "E", "F", "foo", "C", "D", "A", "B", "G"]
        );
    }

    #[test]
    fn flowchart_elk_graph_adapter_maps_elk_layout_options() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", None)],
        );
        let config = MermaidConfig::from_value(json!({
            "elk": {
                "mergeEdges": true,
                "nodePlacementStrategy": "LINEAR_SEGMENTS",
                "nodePlacementAlignment": "RIGHTDOWN",
                "forceNodeModelOrder": true,
                "considerModelOrder": "PREFER_EDGES",
                "cycleBreakingStrategy": "GREEDY_MODEL_ORDER",
                "layered": {
                    "edgeRouting": {
                        "selfLoopOrdering": "SEQUENCED"
                    }
                },
                "insideSelfLoops": {
                    "activate": true
                }
            }
        }));

        let graph = build_flowchart_elk_graph(
            &model,
            &config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        assert!(graph.options.layered.merge_edges);
        assert!(graph.options.layered.force_node_model_order);
        assert!(graph.options.layered.consider_model_order);
        assert_eq!(
            graph.options.layered.model_order,
            elk::ModelOrderStrategy::PreferEdges
        );
        assert_eq!(
            graph.options.layered.cycle_breaking,
            elk::CycleBreakingStrategy::GreedyModelOrder
        );
        assert_eq!(
            graph.options.layered.node_placement,
            elk::NodePlacementStrategy::LinearSegments
        );
        assert_eq!(
            graph.options.layered.node_placement_alignment,
            elk::NodePlacementAlignment::RightDown
        );
        assert!(graph.options.layered.inside_self_loops_activate);
        assert!(graph.options.layered.unnecessary_bendpoints);
        assert!(graph.options.layered.merge_hierarchy_edges);
        assert_eq!(
            graph.options.layered.self_loop_distribution,
            elk::SelfLoopDistributionStrategy::Equally
        );
        assert_eq!(
            graph.options.layered.self_loop_ordering,
            elk::SelfLoopOrderingStrategy::Sequenced
        );
    }

    #[test]
    fn flowchart_elk_keeps_the_source_default_nonzero_seed() {
        assert_eq!(
            elk_layout_options(&serde_json::Value::Null)
                .layered
                .random_seed,
            1
        );
    }

    #[test]
    fn flowchart_elk_zero_seed_adapter_graph_requires_an_operation_seed() {
        use std::num::NonZeroU64;

        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", Some("go"))],
        );
        let mut graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();
        graph.options.layered.random_seed = 0;

        assert!(elk::layout(&graph).is_err());

        let operation_seed = elk::ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x666c_6f77_6368_6172).expect("nonzero operation seed"),
        );
        let first = elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("seeded Flowchart layout");
        let replayed = elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("replayed seeded Flowchart layout");

        assert_eq!(first, replayed);
    }

    #[test]
    fn flowchart_elk_graph_adapter_defaults_inside_self_loop_edges_to_false() {
        let model = model(
            vec![node("A", Some("Alpha"), None)],
            vec![edge("A-A", "A", "A", None)],
        );

        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        assert!(!graph.edges[0].inside_self_loops_yo);
    }

    #[test]
    fn flowchart_elk_graph_adapter_maps_disabled_model_order() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", None)],
        );
        let config = MermaidConfig::from_value(json!({
            "elk": {
                "considerModelOrder": "NONE",
                "cycleBreakingStrategy": "MODEL_ORDER",
                "nodePlacementStrategy": "NETWORK_SIMPLEX"
            }
        }));

        let graph = build_flowchart_elk_graph(
            &model,
            &config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        assert!(!graph.options.layered.consider_model_order);
        assert_eq!(
            graph.options.layered.model_order,
            elk::ModelOrderStrategy::None
        );
        assert_eq!(
            graph.options.layered.cycle_breaking,
            elk::CycleBreakingStrategy::ModelOrder
        );
        assert_eq!(
            graph.options.layered.node_placement,
            elk::NodePlacementStrategy::NetworkSimplex
        );
    }

    #[test]
    fn flowchart_elk_graph_adapter_marks_cyclic_entry_node_on_top() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
                node("C", Some("Gamma"), None),
            ],
            vec![
                edge("L-A-B", "A", "B", None),
                edge("L-B-C", "B", "C", None),
                edge("L-C-A", "C", "A", None),
            ],
        );
        let config = MermaidConfig::from_value(json!({
            "elk": {
                "keepEntryNodeOnTop": true
            }
        }));

        let graph = build_flowchart_elk_graph(
            &model,
            &config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let a = graph.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = graph.nodes.iter().find(|node| node.id == "B").unwrap();
        assert_eq!(a.layer_constraint, Some(elk::LayerConstraint::First));
        assert_eq!(b.layer_constraint, None);
    }

    #[test]
    fn flowchart_elk_cyclic_entry_uses_mermaid_subgraph_first_node_order() {
        let mut model = model(
            vec![node("A", Some("Alpha"), None)],
            vec![
                edge("group-to-a", "group", "A", None),
                edge("a-to-group", "A", "group", None),
            ],
        );
        model
            .subgraphs
            .push(subgraph("group".to_string(), Vec::new()));
        let config = MermaidConfig::from_value(json!({
            "elk": {
                "keepEntryNodeOnTop": true
            }
        }));

        let graph = build_flowchart_elk_graph(
            &model,
            &config,
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let group = graph.nodes.iter().find(|node| node.id == "group").unwrap();
        let a = graph.nodes.iter().find(|node| node.id == "A").unwrap();
        assert_eq!(group.layer_constraint, Some(elk::LayerConstraint::First));
        assert_eq!(a.layer_constraint, None);
    }

    #[test]
    fn flowchart_elk_missing_endpoint_does_not_override_separate_children() {
        let mut model = model(
            vec![node("A", Some("Alpha"), None)],
            vec![edge("missing-target", "A", "missing", None)],
        );
        let mut group = subgraph("group".to_string(), vec!["A".to_string()]);
        group.dir = Some("LR".to_string());
        group.has_explicit_dir = true;
        model.subgraphs.push(group);

        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();
        let group = graph.nodes.iter().find(|node| node.id == "group").unwrap();
        assert_eq!(
            group.hierarchy_handling,
            Some(elk::HierarchyHandling::SeparateChildren)
        );
    }

    #[test]
    fn flowchart_elk_graph_adapter_measures_markdown_and_html_labels() {
        let model = model(
            vec![
                node("A", Some("**bold** label"), Some("markdown")),
                node("B", Some("<span>html</span>"), Some("html")),
            ],
            vec![edge("L-A-B", "A", "B", Some("edge"))],
        );
        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        for node in &graph.nodes {
            let label = node.label.expect("node should carry measured label bounds");
            assert!(label.width > 0.0, "label width for {}", node.id);
            assert!(label.height > 0.0, "label height for {}", node.id);
            assert!(node.width >= label.width, "node width for {}", node.id);
            assert!(node.height >= label.height, "node height for {}", node.id);
        }
    }

    #[test]
    fn flowchart_elk_layout_produces_nodes_edges_and_bounds() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
            ],
            vec![edge("L-A-B", "A", "B", Some("go"))],
        );
        let layout = layout_flowchart_elk_typed(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.y > a.y);
        assert_eq!(layout.edges.len(), 1);
        assert!(layout.edges[0].points.len() >= 2);
        assert!(layout.edges[0].label.is_some());
        assert!(layout.bounds.is_some());
    }

    #[test]
    #[cfg(feature = "layout-elk")]
    fn flowchart_source_backed_elk_uses_exported_edge_label_position() {
        let model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
                node("C", Some("Gamma"), None),
            ],
            vec![
                edge("L-A-B", "A", "B", None),
                edge("L-B-C", "B", "C", None),
                edge("L-A-C", "A", "C", Some("choice")),
            ],
        );
        let graph = build_flowchart_elk_graph(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();
        let raw_layout = elk::layout(&graph).unwrap();
        let raw_edge = raw_layout
            .edges
            .iter()
            .find(|edge| edge.id == "L-A-C")
            .unwrap();
        let raw_label = raw_edge.labels.first().unwrap();

        let layout = layout_flowchart_elk_typed(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        let edge = layout.edges.iter().find(|edge| edge.id == "L-A-C").unwrap();
        let label = edge.label.as_ref().unwrap();
        assert_eq!(label.x, raw_label.x + raw_label.width / 2.0);
        assert_eq!(label.y, raw_label.y + raw_label.height / 2.0);
        assert_eq!(label.width, raw_label.width);
        assert_eq!(label.height, raw_label.height);
    }

    #[test]
    #[cfg(feature = "layout-elk")]
    fn flowchart_source_backed_elk_recursively_lays_out_directed_subgraphs() {
        let mut model = model(
            vec![
                node("A", Some("A"), None),
                node("B", Some("B"), None),
                node("i1", Some("i1"), None),
                node("f1", Some("f1"), None),
                node("i2", Some("i2"), None),
                node("f2", Some("f2"), None),
            ],
            vec![
                edge("L-i1-f1", "i1", "f1", None),
                edge("L-i2-f2", "i2", "f2", None),
                edge("L-A-TOP", "A", "TOP", None),
                edge("L-TOP-B", "TOP", "B", None),
                edge("L-B1-B2", "B1", "B2", None),
            ],
        );
        model.direction = Some("LR".to_string());
        model.subgraphs.push(FlowSubgraph {
            id: "TOP".to_string(),
            title: "TOP".to_string(),
            dir: Some("TB".to_string()),
            has_explicit_dir: true,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["B1".to_string(), "B2".to_string()],
        });
        model.subgraphs.push(FlowSubgraph {
            id: "B1".to_string(),
            title: "B1".to_string(),
            dir: Some("RL".to_string()),
            has_explicit_dir: true,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["i1".to_string(), "f1".to_string()],
        });
        model.subgraphs.push(FlowSubgraph {
            id: "B2".to_string(),
            title: "B2".to_string(),
            dir: Some("BT".to_string()),
            has_explicit_dir: true,
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["i2".to_string(), "f2".to_string()],
        });

        let layout = layout_flowchart_elk_typed(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
        )
        .unwrap();

        assert_eq!(layout.clusters.len(), 3);
        for id in ["TOP", "B1", "B2"] {
            assert!(layout.clusters.iter().any(|cluster| cluster.id == id));
        }
        let top = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "TOP")
            .unwrap();
        let b1 = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "B1")
            .unwrap();
        let b2 = layout
            .clusters
            .iter()
            .find(|cluster| cluster.id == "B2")
            .unwrap();
        let i1 = layout.nodes.iter().find(|node| node.id == "i1").unwrap();
        let f1 = layout.nodes.iter().find(|node| node.id == "f1").unwrap();
        let i2 = layout.nodes.iter().find(|node| node.id == "i2").unwrap();
        let f2 = layout.nodes.iter().find(|node| node.id == "f2").unwrap();
        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();

        assert!(a.x < top.x);
        assert!(b.x > top.x);
        assert!(b2.y > b1.y);
        assert!(f1.x < i1.x);
        assert!(f2.y < i2.y);
    }
}
