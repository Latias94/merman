use crate::config::{config_bool, config_string};
use crate::math::MathRenderer;
use crate::model::{
    FlowchartV2Layout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, FlowchartElkBackend, Result};
use merman_core::MermaidConfig;
use merman_layout_elk as elk;
use std::collections::{HashMap, HashSet};

use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph, FlowchartV2Model};

use super::config::{FlowchartConfigView, FlowchartLayoutSettings};
use super::label::compute_bounds;
use super::layout::{first_parent_cycle_assignment, flowchart_svg_plain_computed_width_px};
use super::node::{NodeLayoutDimensionsRequest, node_layout_dimensions};
use super::{
    FlowchartLabelMetricsRequest, flowchart_effective_font_style_for_classes,
    flowchart_effective_font_style_for_node_classes, flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_node_classes, flowchart_label_metrics_for_layout,
    flowchart_label_plain_text_for_layout, flowchart_node_has_span_css_height_parity,
    flowchart_whole_label_font_style_requests_italic,
};

pub fn layout_flowchart_elk(
    semantic: &serde_json::Value,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    backend: FlowchartElkBackend,
) -> Result<FlowchartV2Layout> {
    let model: FlowchartV2Model = crate::json::from_value_ref(semantic)?;
    layout_flowchart_elk_typed(&model, effective_config, measurer, math_renderer, backend)
}

pub fn layout_flowchart_elk_typed(
    model: &FlowchartV2Model,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    backend: FlowchartElkBackend,
) -> Result<FlowchartV2Layout> {
    let graph = build_flowchart_elk_graph(model, effective_config, measurer, math_renderer)?;
    let layout = layout_elk_graph(&graph, backend).map_err(|err| Error::InvalidModel {
        message: format!("ELK layout failed: {err}"),
    })?;
    flowchart_layout_from_elk(model, effective_config, &graph, layout)
}

fn layout_elk_graph(
    graph: &elk::Graph,
    backend: FlowchartElkBackend,
) -> std::result::Result<elk::LayoutResult, elk::Error> {
    match backend {
        FlowchartElkBackend::Compat => elk::layout(graph, elk::Algorithm::Layered),
        FlowchartElkBackend::SourcePorted => {
            elk::layout_source_ported(graph, elk::Algorithm::Layered)
        }
    }
}

fn flowchart_layout_from_elk(
    model: &FlowchartV2Model,
    effective_config: &MermaidConfig,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
) -> Result<FlowchartV2Layout> {
    let effective_config_value = effective_config.as_value();
    let FlowchartLayoutSettings {
        cluster_padding,
        title_margin_top,
        title_margin_bottom,
        ..
    } = FlowchartConfigView::new(effective_config_value).layout_settings();

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

    let layout_node_by_id: HashMap<&str, &LayoutNode> = out_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let diagram_direction = model.direction.as_deref().unwrap_or("TB");
    let mut clusters = Vec::new();
    for sg in &model.subgraphs {
        if sg.nodes.is_empty() {
            continue;
        }
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
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

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

    let bounds = compute_bounds(&out_nodes, &out_edges);
    let dom_node_order_by_root = std::iter::once((
        String::new(),
        graph.nodes.iter().map(|node| node.id.clone()).collect(),
    ))
    .collect();

    Ok(FlowchartV2Layout {
        nodes: out_nodes,
        edges: out_edges,
        clusters,
        bounds,
        dom_node_order_by_root,
    })
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
    model: &FlowchartV2Model,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<elk::Graph> {
    let effective_config_value = effective_config.as_value();
    let FlowchartLayoutSettings {
        node_padding,
        state_padding,
        wrapping_width,
        edge_label_wrapping_width,
        cluster_title_wrapping_width,
        edge_html_labels,
        node_html_label_css_parity,
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

    let subgraph_ids: HashSet<&str> = model.subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let parent_by_id = parent_by_id(model)?;

    for sg in model.subgraphs.iter().rev() {
        if sg.nodes.is_empty() {
            continue;
        }
        graph.nodes.push(elk::Node {
            id: sg.id.clone(),
            kind: elk::NodeKind::Group,
            width: 0.0,
            height: 0.0,
            parent: parent_by_id.get(&sg.id).cloned(),
            direction: sg.dir.as_deref().map(dir_to_elk_direction),
            label: subgraph_label(
                sg,
                &ElkMeasureContext {
                    model,
                    effective_config,
                    measurer,
                    math_renderer,
                    cluster_label_base_style,
                    cluster_title_wrapping_width,
                    cluster_wrap_mode,
                },
            ),
        });
    }

    let mut inserted_leaf_ids: HashSet<&str> = HashSet::new();
    for node in &model.nodes {
        if subgraph_ids.contains(node.id.as_str()) {
            continue;
        }
        let (width, height, label) = node_dimensions_and_label(
            node,
            NodeMeasureContext {
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
                node_html_label_css_parity,
            },
        );
        graph.nodes.push(elk::Node {
            id: node.id.clone(),
            kind: elk::NodeKind::Leaf,
            width,
            height,
            parent: parent_by_id.get(&node.id).cloned(),
            direction: None,
            label: Some(label),
        });
        inserted_leaf_ids.insert(node.id.as_str());
    }

    for sg in &model.subgraphs {
        if !sg.nodes.is_empty() {
            continue;
        }
        let (width, height, label) = empty_subgraph_dimensions_and_label(
            sg,
            EmptySubgraphMeasureContext {
                model,
                effective_config,
                measurer,
                math_renderer,
                cluster_label_base_style,
                cluster_title_wrapping_width,
                node_wrap_mode,
                node_html_label_css_parity,
                cluster_padding,
                state_padding,
                diagram_direction_text,
            },
        );
        graph.nodes.push(elk::Node {
            id: sg.id.clone(),
            kind: elk::NodeKind::Leaf,
            width,
            height,
            parent: parent_by_id.get(&sg.id).cloned(),
            direction: None,
            label: Some(label),
        });
        inserted_leaf_ids.insert(sg.id.as_str());
    }

    for id in &model.vertex_calls {
        if inserted_leaf_ids.contains(id.as_str()) || subgraph_ids.contains(id.as_str()) {
            continue;
        }
        if let Some(node) = model.nodes.iter().find(|node| node.id == *id) {
            let (width, height, label) = node_dimensions_and_label(
                node,
                NodeMeasureContext {
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
                    node_html_label_css_parity,
                },
            );
            graph.nodes.push(elk::Node {
                id: node.id.clone(),
                kind: elk::NodeKind::Leaf,
                width,
                height,
                parent: parent_by_id.get(&node.id).cloned(),
                direction: None,
                label: Some(label),
            });
            inserted_leaf_ids.insert(node.id.as_str());
        }
    }

    graph.edges = model
        .edges
        .iter()
        .map(|edge| {
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
            elk::Edge {
                id: edge.id.clone(),
                source: edge.from.clone(),
                target: edge.to.clone(),
                label,
                minlen: edge.length.max(1),
            }
        })
        .collect();

    Ok(graph)
}

struct ElkMeasureContext<'a> {
    model: &'a FlowchartV2Model,
    effective_config: &'a MermaidConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    cluster_label_base_style: &'a TextStyle,
    cluster_title_wrapping_width: f64,
    cluster_wrap_mode: WrapMode,
}

#[derive(Clone, Copy)]
struct NodeMeasureContext<'a> {
    model: &'a FlowchartV2Model,
    effective_config: &'a MermaidConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    node_label_base_style: &'a TextStyle,
    wrapping_width: f64,
    diagram_direction_text: &'a str,
    node_padding: f64,
    state_padding: f64,
    node_wrap_mode: WrapMode,
    node_html_label_css_parity: bool,
}

#[derive(Clone, Copy)]
struct EmptySubgraphMeasureContext<'a> {
    model: &'a FlowchartV2Model,
    effective_config: &'a MermaidConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    cluster_label_base_style: &'a TextStyle,
    cluster_title_wrapping_width: f64,
    node_wrap_mode: WrapMode,
    node_html_label_css_parity: bool,
    cluster_padding: f64,
    state_padding: f64,
    diagram_direction_text: &'a str,
}

#[derive(Clone, Copy)]
struct EdgeMeasureContext<'a> {
    model: &'a FlowchartV2Model,
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
    let consider_model_order = config_string(effective_config, &["elk", "considerModelOrder"])
        .map(|strategy| !strategy.trim().eq_ignore_ascii_case("NONE"))
        .unwrap_or(true);
    let cycle_breaking = config_string(effective_config, &["elk", "cycleBreakingStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "GREEDY" | "GREEDY_MODEL_ORDER" => elk::CycleBreakingStrategy::Greedy,
                _ => elk::CycleBreakingStrategy::ModelOrder,
            },
        )
        .unwrap_or_default();

    elk::LayoutOptions {
        layered: elk::LayeredOptions {
            merge_edges: config_bool(effective_config, &["elk", "mergeEdges"]).unwrap_or(false),
            force_node_model_order: config_bool(effective_config, &["elk", "forceNodeModelOrder"])
                .unwrap_or(false),
            consider_model_order,
            cycle_breaking,
            ..Default::default()
        },
    }
}

fn parent_by_id(model: &FlowchartV2Model) -> Result<HashMap<String, String>> {
    let mut parent_by_id = HashMap::new();
    for sg in model.subgraphs.iter().rev() {
        for child in &sg.nodes {
            parent_by_id.insert(child.clone(), sg.id.clone());
        }
    }

    if let Some((child, parent)) = first_parent_cycle_assignment(
        model.subgraphs.iter().rev().map(|sg| sg.id.as_str()),
        &parent_by_id,
    ) {
        return Err(Error::InvalidModel {
            message: format!("Setting {parent} as parent of {child} would create a cycle"),
        });
    }

    Ok(parent_by_id)
}

fn subgraph_label(sg: &FlowSubgraph, ctx: &ElkMeasureContext<'_>) -> Option<elk::Label> {
    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let title_font_style =
        flowchart_effective_font_style_for_classes(&ctx.model.class_defs, &sg.classes, &sg.styles);
    let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
        measurer: ctx.measurer,
        raw_label: &sg.title,
        label_type,
        style: ctx.cluster_label_base_style,
        max_width_px: Some(ctx.cluster_title_wrapping_width),
        wrap_mode: ctx.cluster_wrap_mode,
        config: ctx.effective_config,
        math_renderer: ctx.math_renderer,
        preserve_string_whitespace_height: false,
        whole_label_font_style: title_font_style.as_deref(),
    });
    Some(elk::Label {
        width: metrics.width.max(1.0),
        height: metrics.height.max(1.0),
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
    let node_font_style = flowchart_effective_font_style_for_node_classes(
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
        preserve_string_whitespace_height: ctx.node_html_label_css_parity,
        whole_label_font_style: node_font_style.as_deref(),
    });
    if ctx.node_html_label_css_parity
        && flowchart_node_has_span_css_height_parity(&ctx.model.class_defs, &node.classes)
    {
        crate::text::flowchart_apply_mermaid_styled_node_height_parity(
            &mut metrics,
            node_text_style.as_ref(),
        );
    }
    if ctx.node_wrap_mode == WrapMode::SvgLike
        && label_type != "markdown"
        && !raw_label.contains('<')
        && !raw_label.contains('>')
        && matches!(
            node.layout_shape.as_deref().unwrap_or("squareRect"),
            "squareRect"
        )
    {
        let plain = flowchart_label_plain_text_for_layout(raw_label, label_type, false);
        metrics.width = flowchart_svg_plain_computed_width_px(
            ctx.measurer,
            &plain,
            node_text_style.as_ref(),
            Some(ctx.wrapping_width),
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
        wrap_mode: ctx.node_wrap_mode,
        node_icon: node.icon.as_deref(),
        node_img: node.img.as_deref(),
        node_pos: node.pos.as_deref(),
        node_asset_width: node.asset_width,
        node_asset_height: node.asset_height,
    });

    (width, height, label)
}

fn empty_subgraph_dimensions_and_label(
    sg: &FlowSubgraph,
    ctx: EmptySubgraphMeasureContext<'_>,
) -> (f64, f64, elk::Label) {
    let label_type = sg.label_type.as_deref().unwrap_or("text");
    let sg_text_style = flowchart_effective_text_style_for_classes(
        ctx.cluster_label_base_style,
        &ctx.model.class_defs,
        &sg.classes,
        &sg.styles,
    );
    let sg_font_style =
        flowchart_effective_font_style_for_classes(&ctx.model.class_defs, &sg.classes, &sg.styles);
    let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
        measurer: ctx.measurer,
        raw_label: &sg.title,
        label_type,
        style: sg_text_style.as_ref(),
        max_width_px: Some(ctx.cluster_title_wrapping_width),
        wrap_mode: ctx.node_wrap_mode,
        config: ctx.effective_config,
        math_renderer: ctx.math_renderer,
        preserve_string_whitespace_height: ctx.node_html_label_css_parity,
        whole_label_font_style: sg_font_style.as_deref(),
    });
    let label = elk::Label {
        width: metrics.width,
        height: metrics.height,
    };
    let (width, height) = node_layout_dimensions(NodeLayoutDimensionsRequest {
        layout_shape: Some("squareRect"),
        layout_direction: ctx.diagram_direction_text,
        metrics,
        padding: ctx.cluster_padding,
        state_padding: ctx.state_padding,
        wrap_mode: ctx.node_wrap_mode,
        node_icon: None,
        node_img: None,
        node_pos: None,
        node_asset_width: None,
        node_asset_height: None,
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
    let edge_font_style = flowchart_effective_font_style_for_classes(
        &ctx.model.class_defs,
        &edge.classes,
        &edge.style,
    );
    let metrics = if label_type == "markdown" && ctx.edge_wrap_mode != WrapMode::HtmlLike {
        let mut metrics = crate::text::measure_wrapped_markdown_with_flowchart_bold_deltas(
            ctx.measurer,
            label_text,
            edge_text_style.as_ref(),
            Some(ctx.edge_label_wrapping_width),
            ctx.edge_wrap_mode,
        );
        if flowchart_whole_label_font_style_requests_italic(edge_font_style.as_deref()) {
            let plain = flowchart_label_plain_text_for_layout(
                label_text,
                label_type,
                ctx.edge_wrap_mode == WrapMode::HtmlLike,
            );
            let italic_delta = crate::text::mermaid_default_italic_width_delta_px(
                &plain,
                edge_text_style.as_ref(),
            );
            if italic_delta > 0.0 {
                metrics.width = crate::text::round_to_1_64_px(metrics.width + italic_delta);
            }
        }
        metrics
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
            preserve_string_whitespace_height: false,
            whole_label_font_style: edge_font_style.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use serde_json::json;

    fn node(id: &str, label: Option<&str>, label_type: Option<&str>) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            label: label.map(str::to_string),
            label_type: label_type.map(str::to_string),
            layout_shape: Some("squareRect".to_string()),
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
            stroke: Some("normal".to_string()),
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    fn model(nodes: Vec<FlowNode>, edges: Vec<FlowEdge>) -> FlowchartV2Model {
        FlowchartV2Model {
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
        }
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
                "forceNodeModelOrder": true,
                "considerModelOrder": "NONE",
                "cycleBreakingStrategy": "GREEDY"
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
        assert!(!graph.options.layered.consider_model_order);
        assert_eq!(
            graph.options.layered.cycle_breaking,
            elk::CycleBreakingStrategy::Greedy
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
            FlowchartElkBackend::Compat,
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
    #[cfg(feature = "elk-layout")]
    fn flowchart_source_ported_elk_uses_exported_edge_label_position() {
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
        let raw_layout = elk::layout_source_ported(&graph, elk::Algorithm::Layered).unwrap();
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
            FlowchartElkBackend::SourcePorted,
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
    fn flowchart_elk_layout_uses_subgraph_direction_for_child_geometry() {
        let mut model = model(
            vec![
                node("A", Some("Alpha"), None),
                node("B", Some("Beta"), None),
                node("C", Some("Gamma"), None),
            ],
            vec![edge("L-A-B", "A", "B", None), edge("L-B-C", "B", "C", None)],
        );
        model.subgraphs.push(FlowSubgraph {
            id: "cluster".to_string(),
            title: "Cluster".to_string(),
            dir: Some("LR".to_string()),
            label_type: Some("text".to_string()),
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string(), "B".to_string()],
        });

        let layout = layout_flowchart_elk_typed(
            &model,
            &MermaidConfig::default(),
            &crate::text::VendoredFontMetricsTextMeasurer::default(),
            None,
            FlowchartElkBackend::Compat,
        )
        .unwrap();
        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();
        let c = layout.nodes.iter().find(|node| node.id == "C").unwrap();

        assert!(b.x > a.x);
        assert!(c.y > b.y);
    }
}
