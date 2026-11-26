use crate::elk_options::layout_options as class_elk_layout_options;
use crate::graph_layout::ir::Direction;
use crate::layout_backend::ElkRootAlgorithm;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::{
    ClassDiagramLayout, ClassRenderItem, ClassRenderRoot, ClassRenderRootId, ClassRenderTree,
    LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use merman_layout_elk as elk;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};

use super::measured::{MeasuredClassDiagram, measure_class_diagram};
use super::{
    ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig, EdgeTerminalMetrics, Rect,
    TerminalPos, apply_class_terminal_labels, calculate_point, class_namespace_ids_in_decl_order,
    class_namespace_label, compute_bounds, elk_terminals, normalize_dir,
    required_terminal_label_position, terminal_path_for_edge,
};

/// Lays out a Class diagram through ELK using the render operation's captured seed.
///
/// This remains crate-private so direct callers cannot accidentally turn ELK's unseeded
/// `randomSeed = 0` sentinel into a process-random layout.
#[allow(
    clippy::too_many_arguments,
    reason = "Class ELK receives immutable prepared settings and operation-owned execution inputs."
)]
pub(crate) fn layout_class_diagram_elk_typed_with_config_and_operation_seed(
    model: &ClassDiagramModel,
    effective_config: &merman_model::presentation::PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    algorithm: ElkRootAlgorithm,
    render_config: &ClassRenderConfig,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels)> {
    let settings = render_config.layout_settings();
    let mut measured = measure_class_diagram(
        model,
        effective_config,
        measurer,
        math_renderer,
        render_config,
        true,
        Some(work_control),
    )?;
    let prepared_labels = std::mem::take(&mut measured.prepared_labels);
    let settings = ClassElkLayoutSettings {
        // ClassDB.getData assigns class.padding ?? 16 to every namespace, independently
        // of class-node padding and ELK's fixed content inset.
        algorithm,
        namespace_padding: settings.namespace_padding,
        title_margin_top: settings.title_margin_top,
        title_margin_bottom: settings.title_margin_bottom,
        effective_config: effective_config.as_value(),
    };
    let mut elk_graph = class_measured_to_elk_graph(&measured, &settings, &mut Some(work_control))?;
    let orientation = crate::elk_feedback_edges::orient_feedback_edges(
        &mut elk_graph,
        settings.effective_config,
        &mut Some(work_control),
    )?;
    let mut layout = crate::layout_work::measure_layout_solve!(
        elk::layout_with_operation_seed_and_work_control(
            orientation.graph(),
            operation_seed,
            work_control,
        )
    )
    .map_err(|error| work_control.map_elk_error_with_context(error, "Class ELK"))?;
    orientation.restore(&mut layout, &mut Some(work_control))?;
    let layout = class_layout_from_elk(
        model,
        measured,
        &elk_graph,
        layout,
        settings,
        &mut Some(work_control),
    )?;
    Ok((layout, prepared_labels))
}

pub(super) struct ClassElkLayoutSettings<'a> {
    pub(super) algorithm: ElkRootAlgorithm,
    pub(super) namespace_padding: f64,
    pub(super) title_margin_top: f64,
    pub(super) title_margin_bottom: f64,
    pub(super) effective_config: &'a Value,
}

pub(super) fn class_measured_to_elk_graph(
    measured: &MeasuredClassDiagram,
    settings: &ClassElkLayoutSettings<'_>,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    let graph = &measured.graph;
    let options = class_elk_layout_options(settings.effective_config, settings.algorithm);
    let direction = match graph.direction {
        Direction::Down => elk::Direction::Down,
        Direction::Up => elk::Direction::Up,
        Direction::Right => elk::Direction::Right,
        Direction::Left => elk::Direction::Left,
    };
    let parents = graph
        .parents
        .values()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut nodes: Vec<_> = graph
        .nodes
        .iter()
        .map(|(id, node)| {
            let is_group = node.is_namespace || parents.contains(id.as_str());
            let namespace_label =
                measured
                    .namespace_label_metrics_by_id
                    .get(id)
                    .map(|&(width, height)| elk::Label {
                        width: width.max(1.0),
                        height: height.max(1.0),
                    });
            elk::Node {
                id: id.clone(),
                container: elk::ContainerNodeOptions {
                    padding: if is_group {
                        settings.namespace_padding
                    } else {
                        0.0
                    },
                    ..Default::default()
                },
                label_text: is_group.then(|| id.clone()),
                kind: if is_group {
                    elk::NodeKind::Group
                } else {
                    elk::NodeKind::Leaf
                },
                width: node.width.max(if is_group { 0.0 } else { 1.0 }),
                height: node.height.max(if is_group { 0.0 } else { 1.0 }),
                parent: graph.parents.get(id).cloned(),
                direction: is_group.then_some(direction),
                hierarchy_handling: is_group.then_some(elk::HierarchyHandling::IncludeChildren),
                layer_constraint: None,
                port_alignment: None,
                label: namespace_label,
            }
        })
        .collect();
    if let Some(work) = work.as_deref_mut() {
        let terminal_work =
            work.checked_mul(graph.edges.len(), elk::TerminalLabelKey::ALL.len())?;
        work.charge_adapter(terminal_work)?;
    }
    let edges = graph
        .edges
        .iter()
        .map(|((source, target, id), edge)| elk::Edge {
            id: id.clone(),
            source: source.clone(),
            target: target.clone(),
            label: (edge.width > 0.0 && edge.height > 0.0).then_some(elk::Label {
                width: edge.width,
                height: edge.height,
            }),
            terminal_labels: if options.algorithm == elk::Algorithm::Layered {
                edge.terminals
                    .as_ref()
                    .map(|meta| {
                        elk::TerminalLabelKey::ALL
                            .into_iter()
                            .filter_map(|key| {
                                let (width, height) = class_terminal_metrics(meta, key)?;
                                // Mermaid's class markers are 12px wide. Padding reserves half that
                                // width on each side of the measured label beside a real marker.
                                let marker = if key.at_start() {
                                    meta.start_arrow_type
                                } else {
                                    meta.end_arrow_type
                                };
                                let padding = if marker.is_some() { 6.0 } else { 0.0 };
                                Some(elk::TerminalLabel {
                                    key,
                                    label: elk::Label {
                                        width: width + 2.0 * padding,
                                        height: height + 2.0 * padding,
                                    },
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            },
            minlen: 1,
            inside_self_loops_yo: false,
        })
        .collect();
    if settings
        .effective_config
        .pointer("/elk/keepEntryNodeOnTop")
        .and_then(Value::as_bool)
        != Some(false)
    {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut nodes,
            graph
                .edges
                .keys()
                .map(|(source, target, _)| (source.as_str(), target.as_str())),
            work,
        )?;
    }
    Ok(elk::Graph {
        id: "classDiagram".to_string(),
        direction,
        nodes,
        edges,
        spacing: elk::Spacing {
            node_node: graph.nodesep,
            layer_layer: graph.ranksep,
            group_padding_x: settings.namespace_padding,
            group_padding_y: settings.namespace_padding,
            ..Default::default()
        },
        options,
    })
}

pub(super) fn class_layout_from_elk(
    model: &ClassDiagramModel,
    measured: MeasuredClassDiagram,
    elk_graph: &elk::Graph,
    layout: elk::LayoutResult,
    settings: ClassElkLayoutSettings<'_>,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<ClassDiagramLayout> {
    let frames = crate::elk_adapter::drawing_group_frames(
        elk_graph,
        &layout,
        |node| node.label.map_or(0.0, |label| label.width) + node.container.padding,
        work,
    )?;
    let namespace_ids = class_namespace_ids_in_decl_order(model);
    let namespace_set: HashSet<&str> = namespace_ids.iter().copied().collect();
    let source_node_by_id: HashMap<&str, &elk::Node> = elk_graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let source_edge_by_id: HashMap<&str, &elk::Edge> = elk_graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    validate_class_elk_output_ids(
        "node",
        source_node_by_id.keys().copied(),
        layout.nodes.iter().map(|node| node.id.as_str()),
    )?;
    validate_class_elk_output_ids(
        "edge",
        source_edge_by_id.keys().copied(),
        layout.edges.iter().map(|edge| edge.id.as_str()),
    )?;

    let mut nodes = Vec::with_capacity(layout.nodes.len());
    for node in layout.nodes {
        let Some(source) = source_node_by_id.get(node.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown class node {}", node.id),
            });
        };
        let frame = frames.get(node.id.as_str());
        let label_metrics = measured
            .node_label_metrics_by_id
            .get(&node.id)
            .copied()
            .or_else(|| source.label.map(|label| (label.width, label.height)));
        nodes.push(LayoutNode {
            id: node.id,
            x: frame.map_or(node.x, |frame| frame.x),
            y: frame.map_or(node.y, |frame| frame.y),
            width: frame.map_or(node.width, |frame| frame.width),
            height: frame.map_or(node.height, |frame| frame.height),
            is_cluster: source.kind == elk::NodeKind::Group,
            label_width: label_metrics.map(|(width, _)| width),
            label_height: label_metrics.map(|(_, height)| height),
        });
    }

    let node_by_id: HashMap<&str, &LayoutNode> =
        nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let mut node_rect_by_id: HashMap<&str, Rect> = HashMap::new();
    for node in &nodes {
        node_rect_by_id.insert(
            node.id.as_str(),
            Rect::from_center(node.x, node.y, node.width, node.height),
        );
    }

    let edge_terminals_by_id = measured
        .graph
        .edges
        .iter()
        .filter_map(|((_, _, id), edge)| edge.terminals.as_ref().map(|meta| (id.as_str(), meta)))
        .collect::<HashMap<_, _>>();

    let mut edges = Vec::with_capacity(layout.edges.len());
    let mut placed_terminals = Vec::with_capacity(layout.edges.len());
    for edge in layout.edges {
        let Some(source) = source_edge_by_id.get(edge.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown class edge {}", edge.id),
            });
        };
        let terminal_meta = edge_terminals_by_id.get(edge.id.as_str());
        if let Some(work) = work.as_deref_mut() {
            work.charge_adapter(
                edge.points
                    .len()
                    .saturating_add(edge.labels.len())
                    .saturating_add(2),
            )?;
        }
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
                .iter()
                .find(|label| label.terminal.is_none())
                .map(|label| LayoutLabel {
                    x: label.x + label.width / 2.0,
                    y: label.y + label.height / 2.0,
                    width: label.width,
                    height: label.height,
                })
                .or_else(|| class_elk_edge_label_position(&points, source_label))
        });
        let terminal_points = if points.is_empty()
            && let (Some(from), Some(to)) = (
                node_by_id.get(source.source.as_str()),
                node_by_id.get(source.target.as_str()),
            ) {
            crate::elk_geometry::missing_rect_section_points(from, to)
        } else if let (Some(from), Some(to)) = (
            node_rect_by_id.get(source.source.as_str()).copied(),
            node_rect_by_id.get(source.target.as_str()).copied(),
        ) {
            terminal_path_for_edge(&points, from, to)
        } else {
            points.clone()
        };

        let mut placed = elk_terminals::PlacedTerminals {
            ports: points
                .first()
                .zip(points.last())
                .map(|(start, end)| (start.clone(), end.clone())),
            ..Default::default()
        };
        let routed_points = if !points.is_empty()
            && let (Some(start), Some(end)) = (
                node_by_id.get(source.source.as_str()),
                node_by_id.get(source.target.as_str()),
            ) {
            use crate::elk_edge_geometry::{self as geometry, Outline, Shape};
            let start_shape = Shape {
                node: start,
                outline: Outline::Rect,
                intersection: None,
            };
            let end_shape = Shape {
                node: end,
                outline: Outline::Rect,
                intersection: None,
            };
            let mut input = Vec::with_capacity(points.len() + 2);
            input.push(LayoutPoint {
                x: start.x,
                y: start.y,
            });
            input.extend_from_slice(&points);
            input.push(LayoutPoint { x: end.x, y: end.y });
            let mut clipped = geometry::sanitize(&input, start_shape, end_shape);
            if let Some(meta) = terminal_meta {
                geometry::marker_segment(&mut clipped, end_shape, meta.end_arrow_type, false);
                geometry::marker_segment(&mut clipped, start_shape, meta.start_arrow_type, true);
            }
            clipped
        } else {
            points
        };
        let mut out_edge = LayoutEdge {
            id: edge.id,
            from: source.source.clone(),
            to: source.target.clone(),
            from_cluster: node_by_id
                .get(source.source.as_str())
                .filter(|node| node.is_cluster)
                .map(|node| node.id.clone()),
            to_cluster: node_by_id
                .get(source.target.as_str())
                .filter(|node| node.is_cluster)
                .map(|node| node.id.clone()),
            points: routed_points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };
        if let Some(meta) = terminal_meta {
            for terminal in &edge.labels {
                let Some(key) = terminal.terminal else {
                    continue;
                };
                if terminal.x == 0.0 && terminal.y == 0.0 {
                    continue;
                }
                let Some((width, height)) = class_terminal_metrics(meta, key) else {
                    continue;
                };
                *elk_terminals::label_slot(&mut out_edge, key) = Some(LayoutLabel {
                    x: terminal.x + terminal.width / 2.0,
                    y: terminal.y + terminal.height / 2.0,
                    width,
                    height,
                });
                placed.keys.push(key);
            }
            apply_class_terminal_labels(&mut out_edge, meta, &terminal_points)?;
        }
        edges.push(out_edge);
        placed_terminals.push(placed);
    }

    let mut charge = |units| match work.as_deref_mut() {
        Some(work) => work.charge_adapter(units),
        None => Ok(()),
    };
    if settings
        .effective_config
        .pointer("/elk/straightenEdges")
        .and_then(Value::as_bool)
        != Some(false)
    {
        crate::elk_terminal_jogs::straighten_edge_terminals(&mut edges, &mut charge)?;
    }
    for (edge, placed) in edges.iter_mut().zip(&placed_terminals) {
        charge(1)?;
        elk_terminals::follow_moved_endpoints(edge, placed);
        // Only unplaced/non-layered labels retain Mermaid's legacy path fallback.
        if !edge.points.is_empty() {
            for (key, position) in [
                (elk::TerminalLabelKey::StartLeft, TerminalPos::StartLeft),
                (elk::TerminalLabelKey::StartRight, TerminalPos::StartRight),
                (elk::TerminalLabelKey::EndLeft, TerminalPos::EndLeft),
                (elk::TerminalLabelKey::EndRight, TerminalPos::EndRight),
            ] {
                if !placed.keys.contains(&key) && elk_terminals::label(edge, key).is_some() {
                    let (x, y) =
                        required_terminal_label_position(&edge.id, 10.0, position, &edge.points)?;
                    if let Some(label) = elk_terminals::label_slot(edge, key) {
                        label.x = x;
                        label.y = y;
                    }
                }
            }
        }
    }
    elk_terminals::slide_off_frames(&mut edges, &placed_terminals, &nodes, &mut charge)?;
    elk_terminals::put_on_own_side(&mut edges, &placed_terminals, &nodes, &mut charge)?;
    crate::elk_terminal_jogs::separate_opposite_edge_labels(
        edges
            .iter_mut()
            .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.label.as_mut())),
        &mut charge,
    )?;

    let mut clusters = Vec::new();
    for &id in &namespace_ids {
        if !namespace_set.contains(id) {
            continue;
        }
        let Some(node) = node_by_id.get(id).copied() else {
            continue;
        };
        let title = class_namespace_label(model, id).to_string();
        let (title_width, title_height) = measured.namespace_label_metrics_by_id[id];
        let title_label = LayoutLabel {
            x: node.x,
            y: node.y - node.height / 2.0 + settings.title_margin_top + title_height / 2.0,
            width: title_width,
            height: title_height,
        };
        // The title minimum is reserved before layout and bounded by the provider frame.
        // Reapplying it here would grow a contracted cluster without updating its node bounds.
        let width = node.width;
        let diff = -settings.namespace_padding;
        clusters.push(LayoutCluster {
            id: id.to_string(),
            x: node.x,
            y: node.y,
            width,
            height: node.height,
            diff,
            offset_y: title_height - settings.namespace_padding / 2.0,
            title,
            title_label,
            requested_dir: None,
            effective_dir: normalize_dir(&model.model().direction),
            padding: settings.namespace_padding,
            title_margin_top: settings.title_margin_top,
            title_margin_bottom: settings.title_margin_bottom,
        });
    }

    let namespace_order: HashMap<&str, usize> = namespace_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, id)| (id, idx))
        .collect();
    clusters.sort_by(|a, b| {
        namespace_order
            .get(a.id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
            .cmp(
                &namespace_order
                    .get(b.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
            )
            .then_with(|| a.id.cmp(&b.id))
    });

    let bounds = compute_bounds(&nodes, &edges, &clusters);
    let render_tree = ClassRenderTree {
        roots: vec![ClassRenderRoot {
            namespace_id: None,
            cluster_ids: clusters.iter().map(|cluster| cluster.id.clone()).collect(),
            edge_ids: edges.iter().map(|edge| edge.id.clone()).collect(),
            items: nodes
                .iter()
                .filter(|node| !node.is_cluster)
                .map(|node| ClassRenderItem::Node(node.id.clone()))
                .collect(),
        }],
        top: ClassRenderRootId(0),
    };
    Ok(ClassDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
        uses_elk_adapter_dom: true,
        class_label_plans_by_id: measured.class_label_plans_by_id,
        render_tree,
    })
}

pub(super) fn validate_class_elk_output_ids<'a>(
    kind: &'static str,
    expected: impl IntoIterator<Item = &'a str>,
    observed: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let expected = expected
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut observed_ids = BTreeSet::new();
    for id in observed {
        if !observed_ids.insert(id.to_owned()) {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned duplicate class {kind} {id}"),
            });
        }
    }
    if let Some(id) = observed_ids.difference(&expected).next() {
        return Err(Error::InvalidModel {
            message: format!("ELK layout returned unknown class {kind} {id}"),
        });
    }
    if let Some(id) = expected.difference(&observed_ids).next() {
        return Err(Error::InvalidModel {
            message: format!("ELK layout omitted class {kind} {id}"),
        });
    }
    Ok(())
}

fn class_terminal_metrics(
    meta: &EdgeTerminalMetrics,
    key: elk::TerminalLabelKey,
) -> Option<(f64, f64)> {
    match key {
        elk::TerminalLabelKey::StartLeft => meta.start_left,
        elk::TerminalLabelKey::StartRight => meta.start_right,
        elk::TerminalLabelKey::EndLeft => meta.end_left,
        elk::TerminalLabelKey::EndRight => meta.end_right,
    }
}

fn class_elk_edge_label_position(points: &[LayoutPoint], label: elk::Label) -> Option<LayoutLabel> {
    calculate_point(points, class_elk_polyline_len(points) / 2.0).map(|point| LayoutLabel {
        x: point.x,
        y: point.y,
        width: label.width,
        height: label.height,
    })
}

fn class_elk_polyline_len(points: &[LayoutPoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .sum::<f64>()
}
