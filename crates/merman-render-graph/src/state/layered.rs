//! StateDB's measured nodes and transition occurrences for Mermaid Layered.

use std::fmt;

use merman_model::{MermaidConfig, OperationPhase, families::state::StateDiagramRenderModel};
use merman_render_core::{
    Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    config::json_string_or_first_array,
    graph_layout::{
        Axis, Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Rect,
        Size, SolvedGraph, TitlePosition, direction_from_rank,
        mermaid_layered::{self, EdgeRouting, LayeredError, Options},
    },
    resources::{OperationWorkError, OperationWorkMeter},
    svg_edge_parity::layered_route::Curve,
    text::TextMeasurer,
};
use rustc_hash::FxHashSet;
use serde_json::Value;

use super::{
    StateConfigView, StateDiagramLayout, StateLabelMetricsRequest, StateLabelOwner,
    StateLabelSidecarBuilder, StateLabelSourceKind, StateLayoutSettings, StateNode,
    StateNodeStylePlan, StateStylePlan, edge_label_metrics,
    flat_input::{
        FlatStateInput, charge_node_label, layout_bounds, resolve_directions, state_input,
    },
    state_fork_join_painted_dimensions, state_hidden_prefixes, state_node_dimensions,
    state_node_is_effective_group, state_node_label_text, title_label_metrics,
    validate_state_parent_cycles,
};

const GROUP_PADDING: f64 = 20.0;
const TITLE_GAP: f64 = 12.0;

#[derive(Debug)]
pub enum StateLayeredError {
    Provider(LayeredError),
    DuplicateId { kind: &'static str, id: String },
    UnroutedTransition(usize),
    Host(Error),
}

impl StateLayeredError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Provider(error) => error.is_recoverable(),
            Self::DuplicateId { .. } | Self::UnroutedTransition(_) => true,
            Self::Host(_) => false,
        }
    }
}

impl fmt::Display for StateLayeredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => error.fmt(f),
            Self::DuplicateId { kind, id } => write!(f, "duplicate State {kind} id '{id}'"),
            Self::UnroutedTransition(slot) => write!(f, "State transition {slot} has no endpoint"),
            Self::Host(error) => error.fmt(f),
        }
    }
}

impl From<Error> for StateLayeredError {
    fn from(error: Error) -> Self {
        Self::Host(error)
    }
}

impl From<OperationWorkError> for StateLayeredError {
    fn from(error: OperationWorkError) -> Self {
        Self::Host(error.into())
    }
}

/// Covers one successful Layered projection or a complete Dagre retry.
#[derive(Debug)]
pub struct StateLayeredPaint {
    pub(super) routes: Option<StateLayeredRoutes>,
}

impl StateLayeredPaint {
    pub const fn dagre_fallback() -> Self {
        Self { routes: None }
    }
}

#[derive(Debug)]
pub(super) struct StateLayeredRoutes {
    pub(super) edges: Vec<StateLayeredEdge>,
    pub(super) curve: Curve,
}

#[derive(Debug)]
pub(super) struct StateLayeredEdge {
    pub(super) source_slot: usize,
    pub(super) axes: Vec<Axis>,
}

struct Measurements<'a> {
    config: &'a MermaidConfig,
    settings: StateLayoutSettings,
    style: &'a StateStylePlan,
    measurer: &'a dyn TextMeasurer,
    sidecar: &'a StateLabelSidecarBuilder,
    work: &'a OperationWorkMeter,
}

struct MeasuredState<'a> {
    input: FlatStateInput<'a>,
    endpoints: Vec<Endpoint>,
    graph: MeasuredGraph,
    transitions: Vec<(usize, Option<Size>)>,
    margins: [f64; 2],
    curve: Curve,
    unrouted: Option<usize>,
}

pub fn layout(
    model: &StateDiagramRenderModel,
    config: &MermaidConfig,
    style: &StateStylePlan,
    measurer: &dyn TextMeasurer,
    sidecar: &StateLabelSidecarBuilder,
    work: &OperationWorkMeter,
) -> std::result::Result<(StateDiagramLayout, StateLayeredPaint), StateLayeredError> {
    work.checkpoint(OperationPhase::Layout)?;
    validate_state_parent_cycles(model)?;
    let measurements = Measurements {
        config,
        settings: StateConfigView::new(config.as_value())
            .layout_settings(&model.direction, style.compatibility()),
        style,
        measurer,
        sidecar,
        work,
    };
    let measured = measurements.prepare(model)?;
    if !measured.margins.iter().sum::<f64>().is_finite() {
        return Err(StateLayeredError::Provider(LayeredError::NonFiniteGeometry));
    }
    let group_order = group_paint_order(&measured.graph.groups, work)?;
    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        mermaid_layered::layout(&measured.graph, Options::default(), work)
    }
    .map_err(|error| match error {
        LayeredError::Work(error) => error.into(),
        LayeredError::ArithmeticOverflow => work.arithmetic_overflow().into(),
        LayeredError::AllocationFailed => allocation_error().into(),
        error => StateLayeredError::Provider(error),
    })?;
    if let Some(slot) = measured.unrouted {
        // The browser omits these from the solver and reaches them again in common paint.
        return Err(StateLayeredError::UnroutedTransition(slot));
    }
    Ok(measured.project(model, solved.graph, &group_order, &measurements)?)
}

impl Measurements<'_> {
    fn prepare<'a>(
        &self,
        model: &'a StateDiagramRenderModel,
    ) -> std::result::Result<MeasuredState<'a>, StateLayeredError> {
        let work = self.work;
        let hidden = state_hidden_prefixes(model);
        let input = state_input(model, &hidden, work, "layered")?;
        let node_count = input.nodes.len();
        let mut endpoints = buffer(node_count, work)?;
        let mut groups = 0;
        let mut leaves = 0;
        for (index, input_node) in input.nodes.iter().enumerate() {
            let node = input_node.node;
            work.charge(node.id.len().max(1))?;
            if input.slots.get(node.id.as_str()) != Some(&index) {
                return Err(StateLayeredError::DuplicateId {
                    kind: "node",
                    id: owned(&node.id, work)?,
                });
            }
            let endpoint = if state_node_is_effective_group(node) {
                let slot = slot(groups, work)?;
                groups += 1;
                Endpoint::Group(slot)
            } else {
                let slot = slot(leaves, work)?;
                leaves += 1;
                Endpoint::Node(slot)
            };
            endpoints.push(endpoint);
        }
        let routing = EdgeRouting::from_name(self.string(&["layered", "edgeRouting"]));
        let curve = Curve::for_edge_routing(
            routing,
            self.string(&["flowchart", "curve"])
                .or_else(|| self.style.compatibility().native_edge_curve())
                .unwrap_or("basis"),
        );
        let margins = [self.title_margin("top"), self.title_margin("bottom")];
        let [_, node_scale, rank_scale] = self.style.compatibility().native_layout_scale;
        let mut graph = MeasuredGraph {
            direction: self.settings.direction,
            routing: routing.geometry(),
            node_spacing: self.spacing("nodeSpacing") * node_scale,
            rank_spacing: self.spacing("rankSpacing") * rank_scale,
            component_spacing: 0.0,
            inherit_crossed_group_direction: true,
            nodes: buffer(leaves, work)?,
            groups: buffer(groups, work)?,
            edges: buffer(model.edges.len(), work)?,
        };
        let endpoint = |id: &str| input.node_slot(id).map(|index| endpoints[index]);
        let directions =
            resolve_directions(&input.nodes, &input.slots, graph.direction, work, "layered")?;
        for ((input_node, &slot), direction) in input.nodes.iter().zip(&endpoints).zip(directions) {
            let node = input_node.node;
            charge_node_label(node, work)?;
            let parent = input_node
                .parent
                .and_then(endpoint)
                .and_then(|owner| match owner {
                    Endpoint::Group(index) => Some(index),
                    Endpoint::Node(_) => None,
                });
            match slot {
                Endpoint::Group(index) => {
                    let title = self.measure_group(node);
                    let top = if title.is_some() {
                        positive(margins[0]) + positive(margins[1]) + TITLE_GAP
                    } else {
                        GROUP_PADDING
                    };
                    graph.groups.push(MeasuredGroup {
                        seq: index,
                        parent,
                        direction: node.dir.as_deref().and_then(group_direction),
                        padding: kurbo::Insets::new(
                            GROUP_PADDING,
                            top,
                            GROUP_PADDING,
                            GROUP_PADDING,
                        ),
                        min_size: Size::default(),
                        title,
                        title_position: TitlePosition::TopCenter,
                    });
                }
                Endpoint::Node(index) => {
                    let mut leaf =
                        MeasuredNode::new(self.measure_leaf(node, direction)?, index, parent);
                    if matches!(node.shape.as_str(), "stateStart" | "stateEnd" | "choice") {
                        let width = if matches!(graph.direction, Direction::Left | Direction::Right)
                        {
                            leaf.size.height
                        } else {
                            leaf.size.width
                        };
                        leaf.port_span = Some(width * 0.6);
                    }
                    graph.nodes.push(leaf);
                }
            }
            work.checkpoint(OperationPhase::Layout)?;
        }

        let mut ids = FxHashSet::default();
        ids.try_reserve(model.edges.len())
            .map_err(|_| allocation_error())?;
        let mut unrouted = None;
        let mut transitions = buffer(model.edges.len(), work)?;
        for (index, edge) in model.edges.iter().enumerate() {
            for id in [&edge.id, &edge.start, &edge.end] {
                work.charge(id.len().max(1))?;
            }
            if [&edge.id, &edge.start, &edge.end]
                .into_iter()
                .any(|id| hidden.is_hidden(id))
            {
                continue;
            }
            let label = self.measure_transition(&edge.id, &edge.label)?;
            let (Some(source), Some(target)) = (endpoint(&edge.start), endpoint(&edge.end)) else {
                unrouted.get_or_insert(index);
                continue;
            };
            if !ids.insert(edge.id.as_str()) {
                return Err(StateLayeredError::DuplicateId {
                    kind: "transition",
                    id: owned(&edge.id, work)?,
                });
            }
            graph.edges.push(MeasuredEdge {
                source,
                target,
                min_rank_span: 1,
                weight: 1.0,
                label: label.map(|size| Size {
                    width: size.width + 12.0,
                    height: size.height + 4.0,
                }),
                start_label: None,
                end_label: None,
                start_marker: Default::default(),
                end_marker: Default::default(),
            });
            transitions.push((index, label));
        }
        Ok(MeasuredState {
            input,
            endpoints,
            graph,
            transitions,
            margins,
            curve,
            unrouted,
        })
    }

    fn measure_leaf(&self, node: &StateNode, direction: Option<Direction>) -> Result<Size> {
        let (width, height) = if matches!(node.shape.as_str(), "fork" | "join") {
            state_fork_join_painted_dimensions(direction.unwrap_or(self.settings.direction))
        } else {
            state_node_dimensions(
                node,
                &self.settings,
                self.style,
                self.measurer,
                Some(self.sidecar),
            )?
        };
        Ok(Size {
            width: positive(width),
            height: positive(height),
        })
    }

    fn measure_group(&self, node: &StateNode) -> Option<Size> {
        let label = node.label.as_ref().and_then(json_string_or_first_array)?;
        if label.is_empty() {
            return None;
        }
        let note = node.shape == "noteGroup";
        let request = StateLabelMetricsRequest {
            owner: StateLabelOwner::ClusterTitle(&node.id),
            text: &label,
            source_kind: if note {
                StateLabelSourceKind::Plain
            } else {
                StateLabelSourceKind::Markdown
            },
            measurer: self.measurer,
            typography: self.cluster_typography(node),
            max_width_px: (!note).then_some(self.settings.wrapping_width),
            wrap_mode: self.settings.wrap_mode,
        };
        // The solver sees createGraph's Markdown measurement. The State cluster writer paints
        // the separate unwrapped title, so it stores only the terminal measurement in the sidecar.
        let metrics = super::label_artifact::measure_label_for_layout(request).metrics;
        let size = Size {
            width: positive(metrics.width),
            height: positive(metrics.height),
        };
        (size.width > 0.0 && size.height > 0.0).then_some(size)
    }

    fn cluster_typography(&self, node: &StateNode) -> &super::ResolvedLabelTypography {
        self.style
            .node(&node.id)
            .map(StateNodeStylePlan::resolved_cluster_label_typography)
            .unwrap_or_else(|| self.style.composite_label_typography())
    }

    fn measure_transition(&self, id: &str, text: &str) -> Result<Option<Size>> {
        self.work.charge(text.len().max(1))?;
        let text = text.trim_matches(merman_model::text::is_ecmascript_whitespace);
        if text.is_empty() {
            return Ok(None);
        }
        let (width, height) = edge_label_metrics(
            id,
            text,
            self.measurer,
            self.style.transition_label_typography(),
            self.settings.wrap_mode,
            Some(self.sidecar),
        );
        self.work.checkpoint(OperationPhase::Layout)?;
        let width = positive(width);
        Ok((width > 0.0).then_some(Size {
            width,
            height: positive(height),
        }))
    }

    fn string(&self, path: &[&str]) -> Option<&str> {
        self.config.get_path(path).and_then(Value::as_str)
    }

    fn spacing(&self, key: &str) -> f64 {
        // The State renderer provides state[key] || 50, then Layered accepts only positive
        // numbers. A truthy invalid value lets the flowchart spacing supply the fallback.
        match self.config.get_path(&["state", key]) {
            None | Some(Value::Null | Value::Bool(false)) => 50.0,
            Some(Value::Number(value)) if value.as_f64() == Some(0.0) => 50.0,
            Some(Value::String(value)) if value.is_empty() => 50.0,
            Some(value) => value
                .as_f64()
                .filter(|value| positive(*value) > 0.0)
                .or_else(|| {
                    self.config
                        .get_path(&["flowchart", key])
                        .and_then(Value::as_f64)
                        .filter(|value| positive(*value) > 0.0)
                })
                .unwrap_or(50.0),
        }
    }

    fn title_margin(&self, side: &str) -> f64 {
        self.config
            .get_path(&["flowchart", "subGraphTitleMargin", side])
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    fn paint_cluster(
        &self,
        node: &StateNode,
        rect: Rect,
        direction: Direction,
        margins: [f64; 2],
    ) -> Result<LayoutCluster> {
        let work = self.work;
        charge_node_label(node, work)?;
        let titled = !matches!(node.shape.as_str(), "divider" | "noteGroup");
        let title = if titled {
            state_node_label_text(node)
        } else {
            String::new()
        };
        let (title_width, title_height) = if title.is_empty() {
            (0.0, 0.0)
        } else {
            title_label_metrics(
                StateLabelOwner::ClusterTitle(&node.id),
                &title,
                self.measurer,
                self.cluster_typography(node),
                self.settings.wrap_mode,
                Some(self.sidecar),
            )
        };
        work.checkpoint(OperationPhase::Layout)?;
        let padding = self
            .style
            .node(&node.id)
            .and_then(StateNodeStylePlan::padding_override)
            .or(node.padding)
            .unwrap_or(self.settings.state_padding)
            .max(0.0);
        let width = if titled {
            rect.width.max(title_width + padding)
        } else {
            rect.width
        };
        let center = rect.center();
        Ok(LayoutCluster {
            id: owned(&node.id, work)?,
            x: center.x,
            y: center.y,
            width,
            height: rect.height,
            diff: (width - rect.width) / 2.0 - padding,
            offset_y: if titled {
                title_height - padding / 2.0
            } else {
                0.0
            },
            title,
            title_label: LayoutLabel {
                x: center.x,
                y: rect.y + if self.settings.html_labels { 1.0 } else { -2.0 } + title_height / 2.0,
                width: title_width,
                height: title_height,
            },
            requested_dir: node
                .dir
                .as_deref()
                .map(|dir| owned(dir, work))
                .transpose()?,
            effective_dir: match direction {
                Direction::Down => "TB",
                Direction::Up => "BT",
                Direction::Right => "LR",
                Direction::Left => "RL",
            }
            .into(),
            padding,
            title_margin_top: margins[0],
            title_margin_bottom: margins[1],
        })
    }
}

impl MeasuredState<'_> {
    fn project(
        self,
        model: &StateDiagramRenderModel,
        solved: SolvedGraph,
        group_order: &[usize],
        measurements: &Measurements<'_>,
    ) -> Result<(StateDiagramLayout, StateLayeredPaint)> {
        let work = measurements.work;
        if solved.nodes.len() != self.graph.nodes.len()
            || solved.groups.len() != self.graph.groups.len()
            || solved.edges.len() != self.transitions.len()
        {
            return Err(invalid_projection(
                "solver geometry does not match State source slots",
            ));
        }
        let mut nodes = buffer(self.endpoints.len(), work)?;
        let mut group_nodes = buffer(solved.groups.len(), work)?;
        for (node, slot) in self.input.nodes.iter().zip(self.endpoints) {
            let (rect, is_cluster) = match slot {
                Endpoint::Group(index) => {
                    group_nodes.push(node.node);
                    (solved.groups[index as usize].rect, true)
                }
                Endpoint::Node(index) => (solved.nodes[index as usize], false),
            };
            let center = rect.center();
            nodes.push(LayoutNode {
                id: owned(&node.node.id, work)?,
                x: center.x,
                y: center.y,
                width: rect.width,
                height: rect.height,
                is_cluster,
                label_width: None,
                label_height: None,
            });
        }
        let mut clusters = buffer(solved.groups.len(), work)?;
        for &index in group_order {
            let group = &solved.groups[index];
            clusters.push(measurements.paint_cluster(
                group_nodes[index],
                group.rect,
                group.direction,
                self.margins,
            )?);
        }
        let mut edges = buffer(solved.edges.len(), work)?;
        let mut paint_edges = buffer(solved.edges.len(), work)?;
        let label_y_offset = (self.margins[0] + self.margins[1]) / 2.0;
        // The solver retains an additional deferred order for group-to-member edges. The common
        // State writer consumes original transitions, so retain the input slots here.
        for ((slot, size), route) in self.transitions.into_iter().zip(solved.edges) {
            let semantic = &model.edges[slot];
            let get_node = |id: &str| {
                self.input
                    .node_slot(id)
                    .and_then(|index| nodes.get(index))
                    .ok_or_else(|| invalid_projection("transition endpoint lost its node"))
            };
            let from = get_node(&semantic.start)?;
            let to = get_node(&semantic.end)?;
            let mut points = buffer(route.points.len(), work)?;
            let mut axes = buffer(route.points.len(), work)?;
            for point in route.points {
                points.push(LayoutPoint {
                    x: point.x,
                    y: point.y,
                });
                axes.push(point.axis);
            }
            let label = match (size, route.label) {
                (Some(size), Some(label)) => Some(LayoutLabel {
                    x: label.x,
                    y: label.y + label_y_offset,
                    width: size.width,
                    height: size.height,
                }),
                (None, None) => None,
                _ => {
                    return Err(invalid_projection(
                        "label anchor differs from transition measurement",
                    ));
                }
            };
            edges.push(LayoutEdge {
                id: owned(&semantic.id, work)?,
                from: owned(&from.id, work)?,
                to: owned(&to.id, work)?,
                from_cluster: from.is_cluster.then(|| owned(&from.id, work)).transpose()?,
                to_cluster: to.is_cluster.then(|| owned(&to.id, work)).transpose()?,
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
            paint_edges.push(StateLayeredEdge {
                source_slot: slot,
                axes,
            });
        }
        let bounds = layout_bounds(&nodes, &clusters, &edges, work)?;
        work.checkpoint(OperationPhase::Layout)?;
        let layout = StateDiagramLayout {
            nodes,
            edges,
            clusters,
            bounds,
            uses_elk_adapter_dom: false,
            elk_edge_paths: Default::default(),
        };
        Ok((
            layout,
            StateLayeredPaint {
                routes: Some(StateLayeredRoutes {
                    edges: paint_edges,
                    curve: self.curve,
                }),
            },
        ))
    }
}

fn group_paint_order(
    groups: &[MeasuredGroup],
    work: &OperationWorkMeter,
) -> std::result::Result<Vec<usize>, StateLayeredError> {
    let mut depths = buffer(groups.len(), work)?;
    depths.resize(groups.len(), None);
    let mut pending = buffer(groups.len(), work)?;
    for start in 0..groups.len() {
        let mut next = Some(start);
        let mut depth = 0usize;
        while let Some(index) = next {
            work.charge(1)?;
            match depths[index] {
                Some(usize::MAX) => {
                    return Err(StateLayeredError::Provider(
                        LayeredError::InconsistentLevel("State group nesting contains a cycle"),
                    ));
                }
                Some(ancestor) => {
                    depth = ancestor + 1;
                    break;
                }
                None => {
                    depths[index] = Some(usize::MAX);
                    pending.push(index);
                    next = groups[index].parent.map(|parent| parent as usize);
                }
            }
        }
        while let Some(index) = pending.pop() {
            work.charge(1)?;
            depths[index] = Some(depth);
            depth += 1;
        }
    }
    let mut order = buffer(groups.len(), work)?;
    order.extend(0..groups.len());
    let comparisons = (usize::BITS - groups.len().saturating_sub(1).leading_zeros()) as usize;
    work.charge(
        groups
            .len()
            .checked_mul(comparisons)
            .ok_or_else(|| work.arithmetic_overflow())?,
    )?;
    order.sort_unstable_by_key(|&index| (depths[index], index));
    Ok(order)
}

fn group_direction(direction: &str) -> Option<Direction> {
    ["TB", "TD", "BT", "LR", "RL"]
        .iter()
        .any(|rank| direction.eq_ignore_ascii_case(rank))
        .then(|| direction_from_rank(direction))
}

fn positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn slot(index: usize, work: &OperationWorkMeter) -> Result<u32> {
    u32::try_from(index).map_err(|_| work.arithmetic_overflow().into())
}

fn buffer<T>(count: usize, work: &OperationWorkMeter) -> Result<Vec<T>> {
    work.charge(count)?;
    super::flat_input::buffer(count, "layered")
}

fn owned(text: &str, work: &OperationWorkMeter) -> Result<String> {
    work.charge(text.len().max(1))?;
    let mut output = String::new();
    output
        .try_reserve_exact(text.len())
        .map_err(|_| allocation_error())?;
    output.push_str(text);
    Ok(output)
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "layered" }
}

pub(super) fn invalid_projection(detail: &str) -> Error {
    Error::InvalidModel {
        message: format!("Invalid State Layered projection: {detail}"),
    }
}
