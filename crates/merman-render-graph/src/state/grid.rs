//! StateDB nodes and transitions projected onto the measured Grid provider.

use std::sync::Arc;

use merman_model::{
    MermaidConfig, OperationPhase, families::state::StateDiagramRenderModel,
    presentation::PresentationConfig,
};
use merman_render_core::{
    Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    config::{json_f64, json_string_or_first_array},
    graph_layout::{
        Size,
        mermaid_grid::{
            GridEdgeLabel, GridGroupMeasure, GridItem, GridItemKind, GridMeasuredEdge,
            GridMeasuredItem, GridPaint, PreparedGrid, prepare_grid,
        },
    },
    resources::{OperationWorkMeter, PreparedTextRetainedReservation},
    text::TextMeasurer,
};

use super::flat_input::{
    FlatStateNode, charge_node_label, layout_bounds, resolve_directions, state_input,
};
use super::{
    StateConfigView, StateDiagramLayout, StateLabelOwner, StateLabelSidecarBuilder,
    StateLayoutSettings, StateNode, StateNodeStylePlan, StateStylePlan,
    measure_state_markdown_label, normalize_dir, state_fork_join_painted_dimensions,
    state_hidden_prefixes, state_node_dimensions, state_node_is_effective_group,
    state_node_label_text, title_label_metrics, validate_state_parent_cycles,
};
use crate::label_rect::LabelRect;

#[derive(Debug)]
pub(super) struct StateGridLabels {
    pub(super) labels: Vec<StateGridLabel>,
    _storage: PreparedTextRetainedReservation,
}

#[derive(Debug)]
pub(super) struct StateGridLabel {
    pub(super) edge_id: Box<str>,
    pub(super) terminal: LabelRect,
    _retained: PreparedTextRetainedReservation,
}

struct Measurements<'a> {
    config: &'a MermaidConfig,
    settings: StateLayoutSettings,
    style: &'a StateStylePlan,
    measurer: &'a dyn TextMeasurer,
    sidecar: &'a StateLabelSidecarBuilder,
    work: &'a OperationWorkMeter,
    flowchart_wrapping_width: f64,
}

pub fn layout(
    model: &StateDiagramRenderModel,
    effective_config: &PresentationConfig,
    style: &StateStylePlan,
    measurer: &dyn TextMeasurer,
    sidecar: &StateLabelSidecarBuilder,
    work: &Arc<OperationWorkMeter>,
) -> Result<(StateDiagramLayout, GridPaint)> {
    work.checkpoint(OperationPhase::Layout)?;
    validate_state_parent_cycles(model)?;
    let config = effective_config.as_config();
    let hidden = state_hidden_prefixes(model);
    let measurements = Measurements {
        config,
        settings: StateConfigView::new(config.as_value())
            .layout_settings(&model.direction, style.compatibility()),
        style,
        measurer,
        sidecar,
        work,
        flowchart_wrapping_width: config
            .get_path(&["flowchart", "wrappingWidth"])
            .and_then(json_f64)
            .unwrap_or(120.0),
    };

    let input = state_input(model, &hidden, work, "grid")?;
    let nodes = &input.nodes;
    let directions = resolve_directions(
        nodes,
        &input.slots,
        measurements.settings.direction,
        work,
        "grid",
    )?;
    let mut items = buffer(nodes.len())?;
    for (input, direction) in nodes.iter().zip(directions) {
        let node = input.node;
        work.charge(node.id.len().max(1))?;
        charge_node_label(node, work)?;
        let kind = if state_node_is_effective_group(node) {
            GridItemKind::Group(measurements.measure_group(node))
        } else {
            let (width, height) = if matches!(node.shape.as_str(), "fork" | "join") {
                // createGraph uses the painted bar bounds, with no Dagre half-padding.
                state_fork_join_painted_dimensions(
                    direction.unwrap_or(measurements.settings.direction),
                )
            } else {
                state_node_dimensions(node, &measurements.settings, style, measurer, Some(sidecar))?
            };
            GridItemKind::Leaf(Size { width, height })
        };
        work.checkpoint(OperationPhase::Layout)?;
        items.push(GridMeasuredItem {
            item: GridItem {
                id: &node.id,
                authored_id: input.placement_id,
                source_ordinal: Some(input.source_ordinal),
                inline: Default::default(),
            },
            parent_id: input.parent,
            kind,
        });
    }

    let mut edges = buffer(model.edges.len())?;
    work.charge(model.edges.len())?;
    let label_count = model
        .edges
        .iter()
        .filter(|edge| !edge.label.is_empty())
        .count();
    let storage_bytes = label_count
        .checked_mul(std::mem::size_of::<StateGridLabel>())
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut labels = StateGridLabels {
        _storage: work.reserve_prepared_text_retained_bytes(storage_bytes)?,
        labels: buffer(label_count)?,
    };
    let typography = style.transition_label_typography();
    let label_measurer = typography.svg_measurer(measurer, measurements.settings.wrap_mode);
    for edge in &model.edges {
        for id in [&edge.id, &edge.start, &edge.end] {
            work.charge(id.len().max(1))?;
        }
        if hidden.is_hidden(&edge.id)
            || hidden.is_hidden(&edge.start)
            || hidden.is_hidden(&edge.end)
        {
            continue;
        }
        let (Some(source), Some(target)) =
            (input.node_slot(&edge.start), input.node_slot(&edge.end))
        else {
            return Err(Error::InvalidModel {
                message: format!(
                    "GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{}\"",
                    edge.id
                ),
            });
        };
        let label = if edge.label.is_empty() {
            None
        } else {
            work.charge(edge.label.len())?;
            let terminal = LabelRect::prepare(
                &edge.label,
                label_measurer.as_measurer(),
                typography.text_style(),
                measurements.settings.html_labels,
                config,
                measurements.flowchart_wrapping_width,
                true,
            );
            work.checkpoint(OperationPhase::Layout)?;
            let metrics = terminal.metrics();
            if metrics.width.is_finite()
                && metrics.height.is_finite()
                && metrics.width > 0.0
                && metrics.height > 0.0
            {
                let bytes = terminal
                    .retained_buffer_bytes()
                    .checked_add(edge.id.len())
                    .ok_or_else(|| work.arithmetic_overflow())?;
                labels.labels.push(StateGridLabel {
                    _retained: work.reserve_prepared_text_retained_bytes(bytes)?,
                    edge_id: edge.id.as_str().into(),
                    terminal,
                });
                Some(GridEdgeLabel {
                    size: Size {
                        width: metrics.width,
                        height: metrics.height,
                    },
                    start_marker: false,
                    end_marker: !edge.arrow_type_end.is_empty(),
                })
            } else {
                None
            }
        };
        edges.push(GridMeasuredEdge {
            id: &edge.id,
            source,
            target,
            label,
        });
    }

    let grid = prepare_grid(&items, &edges, config.get("grid"), work)?;
    let output = measurements.project(&grid, nodes, &items, &edges, &model.direction)?;
    work.checkpoint(OperationPhase::Layout)?;
    sidecar.set_grid_labels(labels);
    let mut paint = grid.paint();
    if !effective_config.explicit_config_owns_path("grid.curve")
        && matches!(
            style.compatibility().native_edge_curve(),
            Some("step" | "linear")
        )
    {
        paint.curve = "linear";
    }
    Ok((output, paint))
}

impl Measurements<'_> {
    fn measure_group(&self, node: &StateNode) -> GridGroupMeasure {
        let style = self.style.node(&node.id);
        let typography = style
            .map(StateNodeStylePlan::resolved_cluster_label_typography)
            .unwrap_or_else(|| self.style.composite_label_typography());
        let measurer = typography.svg_measurer(self.measurer, self.settings.wrap_mode);
        let width = node
            .wrapping_width
            .filter(|&width| width != 0.0)
            .unwrap_or(self.flowchart_wrapping_width);
        // createGraph's measurement uses the first label and State wrapping. Paint uses
        // the cluster's unwrapped label and may widen the outer rectangle.
        let title = node
            .label
            .as_ref()
            .and_then(json_string_or_first_array)
            .filter(|label| !label.is_empty())
            .map(|label| {
                let metrics = if node.shape == "noteGroup" {
                    LabelRect::prepare(
                        &label,
                        measurer.as_measurer(),
                        typography.text_style(),
                        self.settings.html_labels,
                        self.config,
                        width,
                        false,
                    )
                    .metrics()
                } else {
                    measure_state_markdown_label(
                        &label,
                        measurer.as_measurer(),
                        typography.text_style(),
                        Some(width),
                        self.settings.wrap_mode,
                    )
                    .metrics
                };
                Size {
                    width: metrics.width.max(0.0),
                    height: metrics.height.max(0.0),
                }
            });
        GridGroupMeasure {
            padding: style
                .and_then(StateNodeStylePlan::padding_override)
                .or(node.padding)
                .unwrap_or(self.settings.state_padding)
                .max(0.0),
            title,
        }
    }

    fn project(
        &self,
        grid: &PreparedGrid<'_>,
        source_nodes: &[FlatStateNode<'_>],
        items: &[GridMeasuredItem<'_>],
        source_edges: &[GridMeasuredEdge<'_>],
        root_direction: &str,
    ) -> Result<StateDiagramLayout> {
        let work = self.work;
        let routes = grid.route_edges(work)?;
        if routes.len() != source_edges.len() {
            return Err(invalid_projection(
                "route count differs from transition count",
            ));
        }
        work.charge(source_nodes.len())?;
        let mut nodes = buffer(source_nodes.len())?;
        let mut clusters = Vec::new();
        let mut edges = buffer(routes.len())?;
        for (slot, (input, item)) in source_nodes.iter().zip(items).enumerate() {
            let node = input.node;
            work.charge(node.id.len().max(1))?;
            let rect = grid
                .item_bounds(slot)
                .ok_or_else(|| invalid_projection("missing item geometry"))?;
            let center = rect.center();
            let is_cluster = matches!(item.kind, GridItemKind::Group(_));
            nodes.push(LayoutNode {
                id: node.id.clone(),
                x: center.x,
                y: center.y,
                width: rect.width,
                height: rect.height,
                is_cluster,
                label_width: None,
                label_height: None,
            });
            if let GridItemKind::Group(group) = item.kind {
                charge_node_label(node, work)?;
                let (title, metrics) = self.group_title(node);
                let width = if title.is_empty() {
                    rect.width
                } else {
                    rect.width.max(metrics.width + group.padding)
                };
                work.checkpoint(OperationPhase::Layout)?;
                clusters.try_reserve(1).map_err(|_| allocation_error())?;
                clusters.push(LayoutCluster {
                    id: node.id.clone(),
                    x: center.x,
                    y: center.y,
                    width,
                    height: rect.height,
                    diff: (width - rect.width) / 2.0 - group.padding,
                    offset_y: 0.0,
                    title,
                    title_label: LayoutLabel {
                        x: center.x,
                        y: rect.y
                            + if self.settings.html_labels { 1.0 } else { -2.0 }
                            + metrics.height / 2.0,
                        width: metrics.width,
                        height: metrics.height,
                    },
                    requested_dir: node.dir.clone(),
                    effective_dir: normalize_dir(node.dir.as_deref().unwrap_or(root_direction)),
                    padding: group.padding,
                    title_margin_top: 0.0,
                    title_margin_bottom: 0.0,
                });
            }
        }
        for (source, route) in source_edges.iter().zip(routes) {
            let from = &nodes[source.source];
            let to = &nodes[source.target];
            for id in [source.id, from.id.as_str(), to.id.as_str()] {
                work.charge(id.len().max(1))?;
            }
            work.charge(route.points.len())?;
            let mut points = buffer(route.points.len())?;
            points.extend(
                route
                    .points
                    .into_iter()
                    .map(|p| LayoutPoint { x: p.x, y: p.y }),
            );
            let label = match (source.label, route.label_center) {
                (Some(label), Some(center)) => Some(LayoutLabel {
                    x: center.x,
                    y: center.y,
                    width: label.size.width,
                    height: label.size.height,
                }),
                (None, None) => None,
                _ => {
                    return Err(invalid_projection(
                        "label anchor differs from transition measurement",
                    ));
                }
            };
            edges.push(LayoutEdge {
                id: source.id.into(),
                from: from.id.clone(),
                to: to.id.clone(),
                from_cluster: from.is_cluster.then(|| from.id.clone()),
                to_cluster: to.is_cluster.then(|| to.id.clone()),
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
        let bounds = layout_bounds(&nodes, &clusters, &edges, work)?;
        Ok(StateDiagramLayout {
            nodes,
            edges,
            clusters,
            bounds,
            uses_elk_adapter_dom: false,
            elk_edge_paths: Default::default(),
        })
    }

    fn group_title(&self, node: &StateNode) -> (String, Size) {
        if matches!(node.shape.as_str(), "divider" | "noteGroup") {
            return (String::new(), Size::default());
        }
        let title = state_node_label_text(node);
        let typography = self
            .style
            .node(&node.id)
            .map(StateNodeStylePlan::resolved_cluster_label_typography)
            .unwrap_or_else(|| self.style.composite_label_typography());
        let (width, height) = if title.is_empty() {
            (0.0, 0.0)
        } else {
            title_label_metrics(
                StateLabelOwner::ClusterTitle(&node.id),
                &title,
                self.measurer,
                typography,
                self.settings.wrap_mode,
                Some(self.sidecar),
            )
        };
        (title, Size { width, height })
    }
}

pub(super) fn buffer<T>(capacity: usize) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_error())?;
    Ok(output)
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid State Grid projection: {detail}"),
    }
}
