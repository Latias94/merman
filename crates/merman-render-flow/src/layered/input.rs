use std::collections::{HashMap, HashSet};

use merman_model::families::flowchart::{FlowNode, FlowSubgraph};
use merman_render_core::graph_layout::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredGroup, MeasuredNode, Size,
    TitlePosition, direction_from_rank,
};
use serde_json::Value;

use super::{
    FlowLayeredError, FlowLayeredRouting, FlowchartMeasurement, FlowchartRenderModelRef,
    OperationWorkMeter, PresentationConfig, allocation_error, owned, reserve,
};
use crate::measured::MeasuredLabel;
use crate::node_geometry::{
    flowchart_node_render_dimensions, validate_flowchart_node_asset_dimensions,
};
use crate::projection::{FlowchartEdgeKey, ProjectedFlowchartEdge};
use crate::svg_label::FlowchartSvgLabelOwner;

const GROUP_PADDING: f64 = 20.0;
const TITLE_GAP: f64 = 12.0;

pub(super) struct FlowLayeredInput<'a> {
    pub graph: MeasuredGraph,
    pub leaves: Vec<LeafMeasure<'a>>,
    pub groups: Vec<GroupMeasure<'a>>,
    pub edges: Vec<EdgeMeasure<'a>>,
    pub margins: [f64; 2],
}

pub(super) struct LeafMeasure<'a> {
    pub id: &'a str,
    pub label: MeasuredLabel,
}

pub(super) struct GroupMeasure<'a> {
    pub group: &'a FlowSubgraph,
    pub label: MeasuredLabel,
    pub depth: usize,
}

pub(super) struct EdgeMeasure<'a> {
    pub key: FlowchartEdgeKey,
    pub edge: ProjectedFlowchartEdge<'a>,
    pub label: Option<MeasuredLabel>,
}

#[derive(Clone, Copy)]
enum Leaf<'a> {
    Node(usize, &'a FlowNode),
    Collapsed(usize, &'a FlowSubgraph),
}

struct Visible<'a> {
    leaves: Vec<Leaf<'a>>,
    groups: Vec<(usize, &'a FlowSubgraph)>,
    order: Vec<Endpoint>,
    endpoints: HashMap<&'a str, Endpoint>,
    parents: HashMap<&'a str, u32>,
}

fn slot(index: usize, work: &OperationWorkMeter) -> Result<u32, FlowLayeredError> {
    u32::try_from(index).map_err(|_| work.arithmetic_overflow().into())
}

impl<'a> Visible<'a> {
    fn collect(
        view: FlowchartRenderModelRef<'a>,
        work: &OperationWorkMeter,
    ) -> Result<Self, FlowLayeredError> {
        let capacity = view
            .nodes
            .len()
            .checked_add(view.subgraphs.len())
            .ok_or_else(|| work.arithmetic_overflow())?;
        let mut leaves = reserve(capacity, work)?;
        let mut groups = reserve(view.subgraphs.len(), work)?;
        let mut order = reserve(capacity, work)?;
        let mut endpoints = HashMap::new();
        let mut parents = HashMap::new();
        work.charge(capacity)?;
        endpoints
            .try_reserve(capacity)
            .map_err(|_| allocation_error())?;
        work.charge(capacity)?;
        parents
            .try_reserve(capacity)
            .map_err(|_| allocation_error())?;

        // FlowDB inserts subgraphs in reverse close order, then visits its vertex map.
        for (index, group) in view.semantic.subgraphs.iter().enumerate().rev() {
            let id = group.id.as_str();
            work.charge(id.len().max(1))?;
            if view.collapsed_replacement(id).is_some() || endpoints.contains_key(id) {
                continue;
            }
            let endpoint = if view.is_subgraph_collapsed(id) {
                let slot = slot(leaves.len(), work)?;
                leaves.push(Leaf::Collapsed(index, group));
                Endpoint::Node(slot)
            } else {
                let slot = slot(groups.len(), work)?;
                groups.push((index, group));
                Endpoint::Group(slot)
            };
            endpoints.insert(id, endpoint);
            order.push(endpoint);
        }
        for (index, node) in view.semantic.nodes.iter().enumerate() {
            let id = node.id.as_str();
            work.charge(id.len().max(1))?;
            if view.collapsed_replacement(id).is_none() && !endpoints.contains_key(id) {
                let endpoint = Endpoint::Node(slot(leaves.len(), work)?);
                endpoints.insert(id, endpoint);
                order.push(endpoint);
                leaves.push(Leaf::Node(index, node));
            }
        }
        for (child, parent) in view.visible_memberships() {
            work.charge(child.len().max(1))?;
            work.charge(parent.len().max(1))?;
            if endpoints.contains_key(child)
                && let Some(Endpoint::Group(parent)) = endpoints.get(parent)
            {
                parents.insert(child, *parent);
            }
        }
        Ok(Self {
            leaves,
            groups,
            order,
            endpoints,
            parents,
        })
    }

    fn measurement_direction(
        &self,
        node_id: &str,
        root: Direction,
        work: &OperationWorkMeter,
    ) -> Result<Direction, FlowLayeredError> {
        let mut parent = self.parents.get(node_id).copied();
        for _ in 0..self.groups.len() {
            let Some(index) = parent else { break };
            work.charge(1)?;
            let group = self.groups[index as usize].1;
            if let Some(dir) = group.dir.as_deref().and_then(group_direction) {
                return Ok(dir);
            }
            work.charge(group.id.len().max(1))?;
            parent = self.parents.get(group.id.as_str()).copied();
        }
        Ok(root)
    }
}

impl<'a> FlowLayeredInput<'a> {
    pub(super) fn measure(
        view: FlowchartRenderModelRef<'a>,
        config: &PresentationConfig,
        measure: &FlowchartMeasurement<'_>,
        routing: FlowLayeredRouting,
        work: &OperationWorkMeter,
    ) -> Result<Self, FlowLayeredError> {
        let visible = Visible::collect(view, work)?;
        let direction = view
            .direction
            .as_deref()
            .map_or(Direction::Down, direction_from_rank);
        let flow_config = config.get("flowchart");
        let spacing_scale = measure.layered_spacing_scale();
        let spacing = |key| positive_config(flow_config, key).unwrap_or(50.0) * spacing_scale;
        let margin = flow_config.and_then(|flow| flow.get("subGraphTitleMargin"));
        let margins = [
            positive_config(margin, "top").unwrap_or(0.0),
            positive_config(margin, "bottom").unwrap_or(0.0),
        ];
        let mut graph = MeasuredGraph {
            direction,
            routing: routing.geometry(),
            node_spacing: spacing("nodeSpacing"),
            rank_spacing: spacing("rankSpacing"),
            component_spacing: 0.0,
            inherit_crossed_group_direction: true,
            nodes: reserve(visible.leaves.len(), work)?,
            groups: reserve(visible.groups.len(), work)?,
            edges: reserve(view.edges.len(), work)?,
        };
        let mut leaves = reserve(visible.leaves.len(), work)?;
        let mut groups = reserve(visible.groups.len(), work)?;
        let mut edges = reserve(view.edges.len(), work)?;
        let look = merman_render_core::config::mermaid_config_diagram_look(config);
        let charge = |units| Ok(work.charge(units)?);

        for &endpoint in &visible.order {
            match endpoint {
                Endpoint::Node(slot) => {
                    let leaf = visible.leaves[slot as usize];
                    work.charge(1)?;
                    let (id, shape, w, h, label) = match leaf {
                        Leaf::Node(index, node) => {
                            validate_flowchart_node_asset_dimensions(node)?;
                            let node_dir =
                                visible.measurement_direction(&node.id, direction, work)?;
                            let measured = measure.check_measurement(measure.node(
                                node,
                                FlowchartSvgLabelOwner::Node(index),
                                rank(node_dir),
                                charge,
                            ))?;
                            (
                                node.id.as_str(),
                                node.layout_shape.as_deref(),
                                measured.width,
                                measured.height,
                                measured.label,
                            )
                        }
                        Leaf::Collapsed(index, group) => {
                            let metrics = measure.check_measurement(
                                measure.layered_subgraph_title(group, index, charge),
                            )?;
                            let (w, h) = flowchart_node_render_dimensions(
                                Some("collapsedGroup"),
                                metrics,
                                measure.settings.cluster_padding,
                                look.is_neo(),
                            );
                            (
                                group.id.as_str(),
                                None,
                                w,
                                h,
                                MeasuredLabel {
                                    width: metrics.width,
                                    height: metrics.height,
                                },
                            )
                        }
                    };
                    let size = Size {
                        width: positive(w),
                        height: positive(h),
                    };
                    work.charge(id.len().max(1))?;
                    let mut node = MeasuredNode::new(size, slot, visible.parents.get(id).copied());
                    node.port_span = shape_span(
                        shape,
                        size,
                        matches!(direction, Direction::Right | Direction::Left),
                    );
                    graph.nodes.push(node);
                    leaves.push(LeafMeasure { id, label });
                }
                Endpoint::Group(slot) => {
                    let (index, group) = visible.groups[slot as usize];
                    work.charge(group.id.len().max(1))?;
                    let (width, height) = if view.subgraph_title_for_render(index, group).is_empty()
                    {
                        (0.0, 0.0)
                    } else {
                        let metrics = measure.check_measurement(
                            measure.layered_subgraph_title(group, index, charge),
                        )?;
                        (positive(metrics.width), positive(metrics.height))
                    };
                    let title = (width > 0.0 && height > 0.0).then_some(Size { width, height });
                    let top = if title.is_some() {
                        margins[0] + margins[1] + TITLE_GAP
                    } else {
                        GROUP_PADDING
                    };
                    graph.groups.push(MeasuredGroup {
                        seq: slot,
                        parent: visible.parents.get(group.id.as_str()).copied(),
                        direction: group.dir.as_deref().and_then(group_direction),
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
                    groups.push(GroupMeasure {
                        group,
                        label: MeasuredLabel { width, height },
                        depth: 0,
                    });
                }
            }
        }

        // Flowchart has no layout-only edges. Collapse, invisible strokes and authored loops
        // keep their FlowDB meanings. Only edges without both projected endpoints are excluded.
        work.charge(view.edges.len())?;
        for (key, edge) in view.projected_edges() {
            work.charge(edge.from.len().max(1))?;
            work.charge(edge.to.len().max(1))?;
            if edge.from.is_empty() || edge.to.is_empty() {
                continue;
            }
            let (Some(&source), Some(&target)) = (
                visible.endpoints.get(edge.from),
                visible.endpoints.get(edge.to),
            ) else {
                continue;
            };
            let label = measure
                .check_measurement(measure.layered_edge_label(edge.semantic, key))?
                .filter(|label| positive(label.width) > 0.0)
                .map(|label| MeasuredLabel {
                    width: label.width,
                    height: positive(label.height),
                });
            graph.edges.push(MeasuredEdge {
                source,
                target,
                min_rank_span: u32::try_from(edge.semantic.length.max(1)).unwrap_or(u32::MAX),
                weight: 1.0,
                label: label.as_ref().map(|label| Size {
                    width: label.width + 12.0,
                    height: label.height + 4.0,
                }),
                start_label: None,
                end_label: None,
                start_marker: Default::default(),
                end_marker: Default::default(),
            });
            edges.push(EdgeMeasure { key, edge, label });
        }

        group_depths(&graph.groups, &mut groups, work)?;
        check_edge_ids(&edges, work)?;
        Ok(Self {
            graph,
            leaves,
            groups,
            edges,
            margins,
        })
    }
}

fn group_depths(
    graph: &[MeasuredGroup],
    groups: &mut [GroupMeasure<'_>],
    work: &OperationWorkMeter,
) -> Result<(), FlowLayeredError> {
    #[derive(Clone, Copy)]
    enum Depth {
        Unvisited,
        Pending,
        Resolved(usize),
    }
    let mut depths = reserve(graph.len(), work)?;
    depths.resize(graph.len(), Depth::Unvisited);
    let mut stack = reserve(graph.len(), work)?;
    for start in 0..graph.len() {
        work.charge(1)?;
        let mut current = Some(start);
        let mut depth = 0;
        while let Some(index) = current {
            work.charge(1)?;
            match depths[index] {
                Depth::Resolved(found) => {
                    depth = found + 1;
                    break;
                }
                Depth::Pending => {
                    return Err(FlowLayeredError::InvalidNesting {
                        id: owned(&groups[index].group.id, work)?,
                    });
                }
                Depth::Unvisited => depths[index] = Depth::Pending,
            }
            stack.push(index);
            current = graph[index].parent.map(|index| index as usize);
        }
        while let Some(index) = stack.pop() {
            work.charge(1)?;
            depths[index] = Depth::Resolved(depth);
            groups[index].depth = depth;
            depth += 1;
        }
    }
    Ok(())
}

fn check_edge_ids(
    edges: &[EdgeMeasure<'_>],
    work: &OperationWorkMeter,
) -> Result<(), FlowLayeredError> {
    let mut ids = HashSet::new();
    work.charge(edges.len())?;
    ids.try_reserve(edges.len())
        .map_err(|_| allocation_error())?;
    for edge in edges {
        let id = edge.edge.semantic.id.as_str();
        work.charge(id.len().max(1))?;
        if !ids.insert(id) {
            return Err(FlowLayeredError::DuplicateEdgeId {
                id: owned(id, work)?,
            });
        }
    }
    Ok(())
}

pub(super) const fn rank(dir: Direction) -> &'static str {
    match dir {
        Direction::Down => "TB",
        Direction::Up => "BT",
        Direction::Right => "LR",
        Direction::Left => "RL",
    }
}

fn group_direction(dir: &str) -> Option<Direction> {
    ["TB", "TD", "BT", "LR", "RL"]
        .into_iter()
        .find(|name| dir.eq_ignore_ascii_case(name))
        .map(direction_from_rank)
}

fn positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn positive_config(config: Option<&Value>, key: &str) -> Option<f64> {
    config?
        .get(key)?
        .as_f64()
        .filter(|&value| positive(value) > 0.0)
}

fn shape_span(shape: Option<&str>, Size { width, height }: Size, sideways: bool) -> Option<f64> {
    let span = match shape? {
        "circle" | "circ" | "doublecircle" | "double-circle" | "dbl-circ" | "ellipse"
        | "diamond" | "diam" | "decision" | "question" | "sm-circ" | "small-circle" | "start"
        | "fr-circ" | "framed-circle" | "stop" | "f-circ" | "filled-circle" | "junction"
        | "cross-circ" | "crossed-circle" | "summary" | "stateStart" | "stateEnd" | "choice"
        | "cloud" | "bang" => (if sideways { height } else { width }) * 0.6,
        "stadium" | "terminal" | "pill" => {
            if sideways {
                height * 0.6
            } else {
                width - height
            }
        }
        "hexagon" | "hex" | "prepare" => {
            if sideways {
                height * 0.5
            } else {
                width - height / 2.0 - 12.0
            }
        }
        _ => return None,
    };
    Some(span.max(0.0))
}
