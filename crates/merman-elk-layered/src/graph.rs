//! Internal layered graph model.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraph.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LNode.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LPort.java

use std::{fmt, sync::Arc};

use super::options::{
    Alignment, ElkDirection, LayerConstraint, LayeredOptions, NodeLabelPlacement, PortAlignment,
    PortConstraints,
};
use crate::random::{
    GraphSeedScope, JavaRandom, RandomSeedAuthority, RandomSeedError, RandomSeedPhase,
};
use crate::work::{NoopWorkControl, WorkControl, WorkError, checked_add, checked_mul, checked_sum};

mod clone_tree;
mod port_remap;
pub(crate) use port_remap::{FlatPortRemap, PendingPortRemap};

pub(crate) struct ReorderNodePortsWorkContext {
    shared_reference_work: usize,
    self_loop_holder_scan_work: usize,
    self_loop_payload_by_node: Vec<usize>,
    deferred_scan_extra: Result<usize, WorkError>,
}

impl ReorderNodePortsWorkContext {
    pub(crate) fn new(graph: &LGraph) -> Result<Self, WorkError> {
        Self::new_with_work_control(graph, &mut NoopWorkControl, false).map(|(context, _)| context)
    }

    pub(crate) fn new_with_work_control(
        graph: &LGraph,
        work: &mut dyn WorkControl,
        mut can_defer: bool,
    ) -> Result<(Self, bool), WorkError> {
        let initial_work = checked_add(graph.layerless_nodes.len(), graph.self_loop_holders.len())?;
        work.check(initial_work)?;
        work.charge(initial_work)?;
        let mut self_loop_payload_by_node = vec![0usize; graph.layerless_nodes.len()];
        let mut holder_hyper_loops = 0;
        for holder in &graph.self_loop_holders {
            let Some(payload) = self_loop_payload_by_node.get_mut(holder.node) else {
                continue;
            };
            for hyper_loop in &holder.hyper_loops {
                let loop_payload = checked_add(
                    hyper_loop.ports.len(),
                    checked_mul(hyper_loop.edges.len(), 2)?,
                )?;
                let loop_work = checked_add(1, loop_payload)?;
                work.check(loop_work)?;
                work.charge(loop_work)?;
                *payload = checked_add(*payload, loop_payload)?;
                if can_defer {
                    holder_hyper_loops += 1;
                    let port_count = graph.layerless_nodes[holder.node].ports.len();
                    can_defer = hyper_loop.ports.iter().all(|port| port.port < port_count)
                        && hyper_loop.edges.iter().all(|edge| {
                            edge.source_port < port_count && edge.target_port < port_count
                        });
                }
            }
        }

        // Checking slots during the charged census is only a batching eligibility test.
        // An origin outside the local slot range can retain the immediate path even if it
        // belongs to another graph. Leave graph ID comparisons to the remapping walk.
        let mut same_length_origin_ids = 0usize;
        let mut origin_slot_is_valid = |node: &LNode| {
            node.origin_port.as_ref().is_none_or(|origin| {
                same_length_origin_ids += usize::from(origin.graph_id.len() == graph.id.len());
                port_exists(graph, origin.port)
            })
        };
        let mut descendant_nodes = 0usize;
        let mut descendant_graphs = 0;
        let mut stack = graph
            .layerless_nodes
            .iter()
            .filter_map(|node| {
                if can_defer {
                    can_defer = origin_slot_is_valid(node)
                        && node
                            .long_edge_source
                            .is_none_or(|port| port_exists(graph, port))
                        && node
                            .long_edge_target
                            .is_none_or(|port| port_exists(graph, port));
                }
                node.nested_graph.as_deref()
            })
            .collect::<Vec<_>>();
        while let Some(current) = stack.pop() {
            let scan_work = checked_add(1, current.layerless_nodes.len())?;
            work.check(scan_work)?;
            work.charge(scan_work)?;
            descendant_nodes = checked_add(descendant_nodes, current.layerless_nodes.len())?;
            if can_defer {
                descendant_graphs += 1;
            }
            stack.extend(current.layerless_nodes.iter().filter_map(|node| {
                if can_defer {
                    can_defer = origin_slot_is_valid(node);
                }
                node.nested_graph.as_deref()
            }));
        }

        let context = Self {
            shared_reference_work: checked_sum([
                checked_mul(graph.edges.len(), 3)?,
                checked_mul(graph.layerless_nodes.len(), 3)?,
                descendant_nodes,
                graph.id.len(),
            ])?,
            self_loop_holder_scan_work: graph.self_loop_holders.len(),
            self_loop_payload_by_node,
            deferred_scan_extra: if can_defer {
                checked_mul(same_length_origin_ids.saturating_sub(1), graph.id.len()).and_then(
                    |id_work| checked_sum([id_work, descendant_graphs, holder_hyper_loops]),
                )
            } else {
                Ok(0)
            },
        };
        Ok((context, can_defer))
    }

    pub(crate) fn node_work(&self, graph: &LGraph, node_index: usize) -> Result<usize, WorkError> {
        let port_count = graph.layerless_nodes[node_index].ports.len();
        checked_add(
            checked_mul(port_count, 4)?,
            self.reference_work(node_index)?,
        )
    }

    pub(crate) fn reference_work(&self, node_index: usize) -> Result<usize, WorkError> {
        checked_sum([
            self.shared_reference_work,
            self.self_loop_holder_scan_work,
            self.self_loop_payload_by_node
                .get(node_index)
                .copied()
                .unwrap_or(0),
        ])
    }

    fn deferred_reference_scan_work(&self) -> Result<usize, WorkError> {
        checked_sum([
            self.shared_reference_work,
            self.self_loop_holder_scan_work,
            self.deferred_scan_extra?,
        ])
    }
}

#[derive(Debug, PartialEq)]
pub struct LGraph {
    pub id: String,
    pub options: LayeredOptions,
    pub size: LSize,
    pub padding: LPadding,
    pub offset: LPoint,
    pub layerless_nodes: Vec<LNode>,
    pub layers: Vec<Layer>,
    pub edges: Vec<LayeredEdge>,
    pub graph_properties: GraphProperties,
    pub cyclic: bool,
    pub max_model_order_nodes: usize,
    pub(crate) random: JavaRandom,
    pub parent_node_id: Option<String>,
    pub hidden_nodes: Vec<usize>,
    pub replaced_external_port_dummies: Vec<usize>,
    pub hierarchy_edges: Vec<HierarchyEdge>,
    pub cross_hierarchy_edges: Vec<CrossHierarchyEdge>,
    pub self_loop_holders: Vec<SelfLoopHolder>,
    pub in_layer_successor_constraints_between_non_dummies: bool,
    random_seed_context: GraphRandomSeedContext,
}

#[derive(Clone)]
struct GraphRandomSeedContext {
    authority: RandomSeedAuthority,
    domain: GraphReferenceDomain,
    configuration_invocations: u64,
}

impl GraphRandomSeedContext {
    fn seed_scope(&self) -> &GraphSeedScope {
        self.domain.0.as_ref()
    }
}

impl PartialEq for GraphRandomSeedContext {
    fn eq(&self, other: &Self) -> bool {
        self.authority == other.authority
            && self.seed_scope() == other.seed_scope()
            && self.configuration_invocations == other.configuration_invocations
    }
}

impl Eq for GraphRandomSeedContext {}

impl fmt::Debug for GraphRandomSeedContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GraphRandomSeedContext")
            .field("authority", &self.authority)
            .field("graph_scope", self.seed_scope())
            .field("configuration_invocations", &self.configuration_invocations)
            .finish()
    }
}

/// A graph domain is allocated once per graph, independently of its display name or seed.
/// Structural equality remains value-based, while only internal views of the same graph share
/// reference identity. Copying a public graph or node rebinds references into its copied tree.
#[derive(Clone)]
pub(crate) struct GraphReferenceDomain(Arc<GraphSeedScope>);

impl GraphReferenceDomain {
    pub(crate) fn same_arena(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl LGraph {
    /// Size returned for a completed provider invocation's root by
    /// `ElkGraphLayoutTransferrer.applyParentNodeLayout` and `ElkUtil.resizeNode`.
    /// This final minimum-size clamp does not realign the graph contents a second time.
    pub fn exported_root_size(&self) -> LSize {
        let mut size = LSize {
            width: self.size.width + self.padding.left + self.padding.right,
            height: self.size.height + self.padding.top + self.padding.bottom,
        };
        if let Some(minimum) = self.options.effective_node_size_minimum() {
            size.width = size.width.max(minimum.width);
            size.height = size.height.max(minimum.height);
        }
        size
    }

    pub fn new(id: impl Into<String>, options: LayeredOptions) -> Self {
        let id = id.into();
        Self::new_with_random_seed_authority(
            id.clone(),
            options,
            RandomSeedAuthority::require_explicit(),
            vec![id],
        )
    }

    pub(crate) fn new_with_random_seed_authority(
        id: impl Into<String>,
        options: LayeredOptions,
        random_seed_authority: RandomSeedAuthority,
        graph_path: Vec<String>,
    ) -> Self {
        let graph_path = graph_path.iter().map(String::as_str).collect::<Vec<_>>();
        let graph_scope = GraphSeedScope::from_components(&graph_path);
        Self::new_with_random_seed_authority_at_scope(
            id,
            options,
            random_seed_authority,
            graph_scope,
        )
    }

    pub(crate) fn new_with_random_seed_authority_at_scope(
        id: impl Into<String>,
        options: LayeredOptions,
        random_seed_authority: RandomSeedAuthority,
        graph_scope: GraphSeedScope,
    ) -> Self {
        Self::new_at_reference_domain(
            id,
            options,
            random_seed_authority,
            GraphReferenceDomain(Arc::new(graph_scope)),
        )
    }

    fn new_at_reference_domain(
        id: impl Into<String>,
        options: LayeredOptions,
        random_seed_authority: RandomSeedAuthority,
        domain: GraphReferenceDomain,
    ) -> Self {
        let id = id.into();
        Self {
            id,
            options,
            size: LSize::default(),
            padding: LPadding::default(),
            offset: LPoint::default(),
            layerless_nodes: Vec::new(),
            layers: Vec::new(),
            edges: Vec::new(),
            graph_properties: GraphProperties::default(),
            cyclic: false,
            max_model_order_nodes: 0,
            // The seed is resolved immediately before processor assembly. The placeholder is
            // never an execution source because every public pipeline entry configures first.
            random: JavaRandom::default(),
            parent_node_id: None,
            hidden_nodes: Vec::new(),
            replaced_external_port_dummies: Vec::new(),
            hierarchy_edges: Vec::new(),
            cross_hierarchy_edges: Vec::new(),
            self_loop_holders: Vec::new(),
            in_layer_successor_constraints_between_non_dummies: false,
            random_seed_context: GraphRandomSeedContext {
                authority: random_seed_authority,
                domain,
                configuration_invocations: 0,
            },
        }
    }

    pub(crate) fn reference_domain(&self) -> &GraphReferenceDomain {
        &self.random_seed_context.domain
    }

    pub(crate) fn scoped_node(&self, node: usize) -> GraphNodeRef {
        GraphNodeRef {
            graph_id: self.id.clone(),
            domain: self.reference_domain().clone(),
            node,
        }
    }

    pub(crate) fn scoped_port(&self, port: PortRef) -> GraphPortRef {
        GraphPortRef {
            graph_id: self.id.clone(),
            domain: self.reference_domain().clone(),
            port,
        }
    }

    /// Copies configured properties without copying the graph's dense arenas.
    /// Components use the already-resolved random stream through their serial executor.
    pub(crate) fn component_shell(&self) -> Self {
        let mut graph = Self::new_at_reference_domain(
            self.id.clone(),
            self.options.clone(),
            self.random_seed_context.authority,
            self.reference_domain().clone(),
        );
        graph.options.node_size_minimum = None;
        graph.padding = self.padding;
        graph.graph_properties = self.graph_properties.clone();
        graph.max_model_order_nodes = self.max_model_order_nodes;
        graph.parent_node_id = self.parent_node_id.clone();
        graph.random = self.random.clone();
        graph.random_seed_context = self.random_seed_context.clone();
        graph.in_layer_successor_constraints_between_non_dummies =
            self.in_layer_successor_constraints_between_non_dummies;
        graph
    }

    pub(crate) fn resolve_random_seed_for_configuration(&mut self) -> Result<i64, RandomSeedError> {
        let invocation = self.random_seed_context.configuration_invocations;
        let seed = self.random_seed_context.authority.resolve_scope(
            self.options.random_seed,
            self.random_seed_context.seed_scope(),
            RandomSeedPhase::GraphConfigurator,
            invocation,
        )?;
        self.random_seed_context.configuration_invocations = self
            .random_seed_context
            .configuration_invocations
            .saturating_add(1);
        Ok(seed)
    }

    pub fn sync_graph_properties_to_options(&mut self) {
        let result = self.try_for_each_graph_mut(|graph| {
            graph.graph_properties.apply_to_options(&mut graph.options);
            Ok::<(), std::convert::Infallible>(())
        });
        result.expect("infallible graph property synchronization");
    }

    pub(crate) fn try_for_each_graph_mut<E>(
        &mut self,
        mut visit: impl FnMut(&mut LGraph) -> Result<(), E>,
    ) -> Result<(), E> {
        struct DetachedGraph {
            graph: Box<LGraph>,
            parent: Option<usize>,
            parent_node: usize,
        }

        fn take_nested_graphs(graph: &mut LGraph) -> Vec<(usize, Box<LGraph>)> {
            graph
                .layerless_nodes
                .iter_mut()
                .enumerate()
                .filter_map(|(node, data)| data.nested_graph.take().map(|graph| (node, graph)))
                .collect()
        }

        let mut detached = take_nested_graphs(self)
            .into_iter()
            .map(|(parent_node, graph)| DetachedGraph {
                graph,
                parent: None,
                parent_node,
            })
            .collect::<Vec<_>>();
        let mut cursor = 0usize;
        while cursor < detached.len() {
            let children = take_nested_graphs(&mut detached[cursor].graph);
            detached.extend(
                children
                    .into_iter()
                    .map(|(parent_node, graph)| DetachedGraph {
                        graph,
                        parent: Some(cursor),
                        parent_node,
                    }),
            );
            cursor += 1;
        }

        let mut root_children = Vec::new();
        let mut children_by_parent = vec![Vec::new(); detached.len()];
        for (index, detached_graph) in detached.iter().enumerate() {
            if let Some(parent) = detached_graph.parent {
                children_by_parent[parent].push(index);
            } else {
                root_children.push(index);
            }
        }
        let mut traversal = Vec::with_capacity(detached.len());
        let mut stack = root_children.into_iter().rev().collect::<Vec<_>>();
        while let Some(index) = stack.pop() {
            traversal.push(index);
            stack.extend(children_by_parent[index].iter().rev().copied());
        }

        let mut result = visit(self);
        if result.is_ok() {
            for index in traversal {
                if let Err(error) = visit(&mut detached[index].graph) {
                    result = Err(error);
                    break;
                }
            }
        }

        while let Some(detached_graph) = detached.pop() {
            let parent_node = detached_graph.parent_node;
            if let Some(parent) = detached_graph.parent {
                detached[parent].graph.layerless_nodes[parent_node].nested_graph =
                    Some(detached_graph.graph);
            } else {
                self.layerless_nodes[parent_node].nested_graph = Some(detached_graph.graph);
            }
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    pub nodes: Vec<usize>,
    pub size: LSize,
}

impl Layer {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            size: LSize::default(),
        }
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq)]
pub struct LNode {
    pub id: String,
    pub kind: LNodeKind,
    pub size: LSize,
    pub position: LPoint,
    pub margin: LMargin,
    pub padding: LPadding,
    pub labels: Vec<LLabel>,
    pub represented_labels: Vec<LLabel>,
    pub node_label_placement: NodeLabelPlacement,
    pub ports: Vec<LPort>,
    pub nested_graph: Option<Box<LGraph>>,
    pub model_order: Option<usize>,
    pub component_priority: i32,
    pub layer_index: Option<usize>,
    pub layer_constraint: super::options::LayerConstraint,
    pub node_alignment: Alignment,
    pub port_constraints: PortConstraints,
    pub port_alignment: Option<PortAlignment>,
    pub node_flexibility: super::options::NodeFlexibility,
    pub ports_surrounding: Option<super::options::SpacingMargin>,
    pub external_port_side: PortSide,
    pub external_port_size: LSize,
    pub port_ratio_or_position: f64,
    pub replaced_external_port_dummy: Option<usize>,
    pub in_layer_successor_constraints: Vec<usize>,
    pub in_layer_layout_unit: Option<usize>,
    pub barycenter_associates: Vec<usize>,
    pub origin_port: Option<GraphPortRef>,
    pub origin_edge: Option<usize>,
    pub parent_port_key: Option<String>,
    pub parent_port_type: Option<PortType>,
    pub long_edge_source: Option<PortRef>,
    pub long_edge_target: Option<PortRef>,
    pub long_edge_has_label_dummies: bool,
    pub label_side: LabelSide,
    pub in_layer_constraint: InLayerConstraint,
    pub layer_constraint_explicit: bool,
    pub hidden: bool,
    pub compound: bool,
}

impl LNode {
    pub fn new(id: impl Into<String>, width: f64, height: f64, model_order: Option<usize>) -> Self {
        Self {
            id: id.into(),
            kind: LNodeKind::Normal,
            size: LSize { width, height },
            position: LPoint::default(),
            margin: LMargin::default(),
            padding: LPadding::default(),
            labels: Vec::new(),
            represented_labels: Vec::new(),
            node_label_placement: NodeLabelPlacement::Fixed,
            ports: Vec::new(),
            nested_graph: None,
            model_order,
            component_priority: 0,
            layer_index: None,
            layer_constraint: super::options::LayerConstraint::None,
            node_alignment: Alignment::Automatic,
            port_constraints: PortConstraints::Undefined,
            port_alignment: None,
            node_flexibility: super::options::NodeFlexibility::None,
            ports_surrounding: None,
            external_port_side: PortSide::Undefined,
            external_port_size: LSize::default(),
            port_ratio_or_position: 0.0,
            replaced_external_port_dummy: None,
            in_layer_successor_constraints: Vec::new(),
            in_layer_layout_unit: None,
            barycenter_associates: Vec::new(),
            origin_port: None,
            origin_edge: None,
            parent_port_key: None,
            parent_port_type: None,
            long_edge_source: None,
            long_edge_target: None,
            long_edge_has_label_dummies: false,
            label_side: LabelSide::Below,
            in_layer_constraint: InLayerConstraint::None,
            layer_constraint_explicit: false,
            hidden: false,
            compound: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LNodeKind {
    #[default]
    Normal,
    LongEdge,
    ExternalPort,
    Label,
    NorthSouthPort,
    BreakingPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InLayerConstraint {
    #[default]
    None,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LPort {
    pub id: String,
    pub crossing_minimization_id: Option<usize>,
    pub node: usize,
    pub port_type: PortType,
    pub side: PortSide,
    pub size: LSize,
    pub position: LPoint,
    pub anchor: LPoint,
    pub margin: LMargin,
    pub labels: Vec<LLabel>,
    pub model_order: Option<usize>,
    pub long_edge_target_node: Option<usize>,
    pub port_index: Option<isize>,
    pub border_offset: Option<f64>,
    pub ratio_or_position: f64,
    pub connected_to_external_nodes: bool,
    pub port_dummy: Option<GraphNodeRef>,
    pub inside_connections: bool,
    pub collector_type: Option<PortType>,
    pub end_label_cell: Option<LabelCellLayout>,
    pub incoming_edges: Vec<usize>,
    pub outgoing_edges: Vec<usize>,
}

impl LPort {
    pub fn new(id: impl Into<String>, node: usize, port_type: PortType) -> Self {
        Self {
            id: id.into(),
            crossing_minimization_id: None,
            node,
            port_type,
            side: PortSide::Undefined,
            size: LSize::default(),
            position: LPoint::default(),
            anchor: LPoint::default(),
            margin: LMargin::default(),
            labels: Vec::new(),
            model_order: None,
            long_edge_target_node: None,
            port_index: None,
            border_offset: None,
            ratio_or_position: 0.0,
            connected_to_external_nodes: true,
            port_dummy: None,
            inside_connections: false,
            collector_type: None,
            end_label_cell: None,
            incoming_edges: Vec::new(),
            outgoing_edges: Vec::new(),
        }
    }

    pub fn set_side(&mut self, side: PortSide) {
        self.side = side;
        self.anchor = match side {
            PortSide::North => LPoint {
                x: self.size.width / 2.0,
                y: 0.0,
            },
            PortSide::East => LPoint {
                x: self.size.width,
                y: self.size.height / 2.0,
            },
            PortSide::South => LPoint {
                x: self.size.width / 2.0,
                y: self.size.height,
            },
            PortSide::West => LPoint {
                x: 0.0,
                y: self.size.height / 2.0,
            },
            PortSide::Undefined => self.anchor,
        };
    }

    pub fn net_flow(&self) -> isize {
        self.incoming_edges.len() as isize - self.outgoing_edges.len() as isize
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredEdge {
    pub id: String,
    pub source: PortRef,
    pub target: PortRef,
    pub source_node_id: String,
    pub target_node_id: String,
    pub labels: Vec<LLabel>,
    pub minlen: usize,
    pub reversed: bool,
    pub bend_points: Vec<LPoint>,
    pub model_order: Option<usize>,
    pub priority_direction: i32,
    pub priority_shortness: i32,
    pub priority_straightness: i32,
    pub thickness: f64,
    pub original_opposite_port: Option<PortRef>,
    pub compound_segment: Option<CompoundEdgeSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyEdge {
    pub id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub source_port_key: String,
    pub target_port_key: String,
    pub source_path: Vec<String>,
    pub target_path: Vec<String>,
    pub labels: Vec<LLabel>,
    pub minlen: usize,
    pub model_order: Option<usize>,
    pub priority_direction: i32,
    pub priority_shortness: i32,
    pub priority_straightness: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortRef {
    pub node: usize,
    pub port: usize,
}

#[derive(Clone)]
pub struct GraphNodeRef {
    pub graph_id: String,
    domain: GraphReferenceDomain,
    pub node: usize,
}

impl PartialEq for GraphNodeRef {
    fn eq(&self, other: &Self) -> bool {
        self.graph_id == other.graph_id && self.node == other.node
    }
}

impl Eq for GraphNodeRef {}

impl fmt::Debug for GraphNodeRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GraphNodeRef")
            .field("graph_id", &self.graph_id)
            .field("node", &self.node)
            .finish()
    }
}

impl GraphNodeRef {
    pub(crate) fn belongs_to(&self, graph: &LGraph) -> bool {
        self.in_domain(graph.reference_domain())
    }

    pub(crate) fn in_domain(&self, domain: &GraphReferenceDomain) -> bool {
        self.domain.same_arena(domain)
    }
}

#[derive(Clone)]
pub struct GraphPortRef {
    pub graph_id: String,
    domain: GraphReferenceDomain,
    pub port: PortRef,
}

impl PartialEq for GraphPortRef {
    fn eq(&self, other: &Self) -> bool {
        self.graph_id == other.graph_id && self.port == other.port
    }
}

impl Eq for GraphPortRef {}

impl fmt::Debug for GraphPortRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GraphPortRef")
            .field("graph_id", &self.graph_id)
            .field("port", &self.port)
            .finish()
    }
}

impl GraphPortRef {
    pub(crate) fn belongs_to(&self, graph: &LGraph) -> bool {
        self.in_domain(graph.reference_domain())
    }

    pub(crate) fn in_domain(&self, domain: &GraphReferenceDomain) -> bool {
        self.domain.same_arena(domain)
    }

    pub(crate) fn same_target(&self, other: &Self) -> bool {
        self.port == other.port && self.in_domain(&other.domain)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompoundEdgeSegment {
    Output { depth: usize },
    Input { depth: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossHierarchyEdge {
    pub original_edge_id: String,
    pub original_model_order: Option<usize>,
    pub graph_id: String,
    pub edge: usize,
    pub segment: CompoundEdgeSegment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelfLoopHolder {
    pub node: usize,
    pub hyper_loops: Vec<SelfHyperLoop>,
    pub ports_hidden: bool,
    pub original_port_constraints: PortConstraints,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelfHyperLoop {
    pub ports: Vec<SelfLoopPort>,
    pub edges: Vec<SelfLoopEdge>,
    pub labels: Option<SelfHyperLoopLabels>,
    pub self_loop_type: Option<SelfLoopType>,
    pub leftmost_port: Option<usize>,
    pub rightmost_port: Option<usize>,
    pub occupied_sides: Vec<PortSide>,
    pub routing_slots: [usize; 5],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelfHyperLoopLabels {
    pub id: Option<usize>,
    pub label_refs: Vec<SelfLoopLabelRef>,
    pub size: LSize,
    pub position: LPoint,
    pub side: Option<PortSide>,
    pub alignment: Option<SelfLoopLabelAlignment>,
    pub alignment_reference_port: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfLoopLabelRef {
    pub edge: usize,
    pub label: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfLoopLabelAlignment {
    Center,
    Left,
    Right,
    Top,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelfLoopPort {
    pub port: usize,
    pub had_only_self_loops: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelfLoopEdge {
    pub edge: usize,
    pub source_port: usize,
    pub target_port: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfLoopType {
    OneSide,
    TwoSidesCorner,
    TwoSidesOpposing,
    ThreeSides,
    FourSides,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LLabel {
    pub text: String,
    pub size: LSize,
    pub position: LPoint,
    pub placement: EdgeLabelPlacement,
    pub inline: bool,
    pub label_side: Option<LabelSide>,
    pub end_label_edge: Option<usize>,
    pub original_label_edge: Option<String>,
    /// Opaque caller-owned label identity, preserved through layout transformations.
    pub source_index: Option<usize>,
}

impl LLabel {
    pub fn new(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            text: text.into(),
            size: LSize { width, height },
            position: LPoint::default(),
            placement: EdgeLabelPlacement::Center,
            inline: false,
            label_side: None,
            end_label_edge: None,
            original_label_edge: None,
            source_index: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LPadding {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LMargin {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PortType {
    #[default]
    Input,
    Output,
}

pub(crate) const fn collector_port_id_suffix(port_type: PortType) -> &'static str {
    match port_type {
        PortType::Input => ":collector:Input",
        PortType::Output => ":collector:Output",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortSide {
    #[default]
    Undefined,
    North,
    East,
    South,
    West,
}

impl PortSide {
    pub fn opposed(self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::Undefined => Self::Undefined,
        }
    }

    pub fn right(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            Self::Undefined => Self::Undefined,
        }
    }

    pub fn left(self) -> Self {
        match self {
            Self::North => Self::West,
            Self::East => Self::North,
            Self::South => Self::East,
            Self::West => Self::South,
            Self::Undefined => Self::Undefined,
        }
    }

    pub fn ordinal(self) -> usize {
        match self {
            Self::Undefined => 0,
            Self::North => 1,
            Self::East => 2,
            Self::South => 3,
            Self::West => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelCellLayout {
    pub position: LPoint,
    pub size: LSize,
    pub horizontal_layout: bool,
    pub horizontal_alignment: HorizontalLabelAlignment,
    pub vertical_alignment: VerticalLabelAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HorizontalLabelAlignment {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalLabelAlignment {
    Top,
    #[default]
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeLabelPlacement {
    Head,
    Tail,
    #[default]
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelSide {
    Above,
    #[default]
    Below,
    Inline,
}

impl LabelSide {
    pub fn opposite(self) -> Self {
        match self {
            Self::Above => Self::Below,
            Self::Below => Self::Above,
            Self::Inline => Self::Inline,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphProperties {
    pub self_loops: bool,
    pub center_labels: bool,
    pub end_labels: bool,
    pub non_free_ports: bool,
    pub north_south_ports: bool,
    pub hyperedges: bool,
    pub external_ports: bool,
    pub hypernodes: bool,
}

impl GraphProperties {
    pub fn apply_to_options(&self, options: &mut LayeredOptions) {
        options.graph_has_self_loops = self.self_loops;
        options.graph_has_center_labels = self.center_labels;
        options.graph_has_end_labels = self.end_labels;
        options.graph_has_non_free_ports = self.non_free_ports;
        options.graph_has_north_south_ports = self.north_south_ports;
        options.graph_has_hyperedges = self.hyperedges;
        options.graph_has_external_ports = self.external_ports;
        options.graph_has_hypernodes = self.hypernodes;
    }
}

impl LGraph {
    pub fn clear_layers(&mut self) {
        self.layers.clear();
        for node in &mut self.layerless_nodes {
            node.layer_index = None;
        }
    }

    pub fn set_node_layer(&mut self, node_index: usize, layer_index: usize) {
        while self.layers.len() <= layer_index {
            self.layers.push(Layer::new());
        }

        if let Some(old_layer) = self.layerless_nodes[node_index].layer_index
            && let Some(layer) = self.layers.get_mut(old_layer)
        {
            remove_node(&mut layer.nodes, node_index);
        }

        self.layerless_nodes[node_index].layer_index = Some(layer_index);
        self.layers[layer_index].nodes.push(node_index);
    }

    pub fn insert_layer(&mut self, layer_index: usize) {
        self.layers.insert(layer_index, Layer::new());
        for node in &mut self.layerless_nodes {
            if let Some(current) = node.layer_index
                && current >= layer_index
            {
                node.layer_index = Some(current + 1);
            }
        }
    }

    pub fn remove_node_from_layer(&mut self, node_index: usize) {
        let Some(layer_index) = self.layerless_nodes[node_index].layer_index.take() else {
            return;
        };
        if let Some(layer) = self.layers.get_mut(layer_index) {
            remove_node(&mut layer.nodes, node_index);
        }
    }

    pub fn compact_empty_layers(&mut self) {
        let mut layer_map = vec![None; self.layers.len()];
        let mut compacted = Vec::new();

        for (old_index, layer) in self.layers.iter().enumerate() {
            if !layer.nodes.is_empty() {
                layer_map[old_index] = Some(compacted.len());
                compacted.push(layer.clone());
            }
        }

        self.layers = compacted;
        for node in &mut self.layerless_nodes {
            if let Some(old_index) = node.layer_index {
                node.layer_index = layer_map.get(old_index).copied().flatten();
            }
        }
    }

    pub fn add_port(
        &mut self,
        node_index: usize,
        port_type: PortType,
        side: PortSide,
        position: LPoint,
    ) -> Option<PortRef> {
        let node = self.layerless_nodes.get_mut(node_index)?;
        let port_index = node.ports.len();
        let mut port = LPort::new(format!("{}:{port_index}", node.id), node_index, port_type);
        port.set_side(side);
        port.position = position;
        node.ports.push(port);
        Some(PortRef {
            node: node_index,
            port: port_index,
        })
    }

    /// Return a collector port for an input or output edge endpoint, creating it if needed.
    ///
    /// Source:
    /// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraphUtil.java
    pub fn provide_collector_port(
        &mut self,
        node_index: usize,
        port_type: PortType,
        side: PortSide,
    ) -> Option<PortRef> {
        let node = self.layerless_nodes.get_mut(node_index)?;
        if let Some(port) = node
            .ports
            .iter()
            .position(|port| port.collector_type == Some(port_type))
        {
            return Some(PortRef {
                node: node_index,
                port,
            });
        }

        let port_index = node.ports.len();
        let mut port = LPort::new(
            format!("{}{}", node.id, collector_port_id_suffix(port_type)),
            node_index,
            port_type,
        );
        port.collector_type = Some(port_type);
        port.set_side(side);
        port.position = centered_port_position(node.size, side);
        node.ports.push(port);
        Some(PortRef {
            node: node_index,
            port: port_index,
        })
    }

    pub fn set_edge_source(&mut self, edge_index: usize, source: PortRef) -> bool {
        if self.edges.get(edge_index).is_none() || !port_exists(self, source) {
            return false;
        }

        let old_source = self.edges[edge_index].source;
        if port_exists(self, old_source) {
            remove_edge(
                &mut self.layerless_nodes[old_source.node].ports[old_source.port].outgoing_edges,
                edge_index,
            );
        }

        self.layerless_nodes[source.node].ports[source.port]
            .outgoing_edges
            .push(edge_index);
        self.edges[edge_index].source = source;
        true
    }

    pub fn set_edge_target(&mut self, edge_index: usize, target: PortRef) -> bool {
        self.set_edge_target_at(edge_index, target, None)
    }

    pub fn set_edge_target_at(
        &mut self,
        edge_index: usize,
        target: PortRef,
        insert_at: Option<usize>,
    ) -> bool {
        if self.edges.get(edge_index).is_none() || !port_exists(self, target) {
            return false;
        }

        let old_target = self.edges[edge_index].target;
        if port_exists(self, old_target) {
            remove_edge(
                &mut self.layerless_nodes[old_target.node].ports[old_target.port].incoming_edges,
                edge_index,
            );
        }

        let incoming_edges =
            &mut self.layerless_nodes[target.node].ports[target.port].incoming_edges;
        if let Some(index) = insert_at {
            incoming_edges.insert(index.min(incoming_edges.len()), edge_index);
        } else {
            incoming_edges.push(edge_index);
        }
        self.edges[edge_index].target = target;
        true
    }

    pub fn detach_edge_source(&mut self, edge_index: usize) -> Option<PortRef> {
        let source = self.edges.get(edge_index)?.source;
        if port_exists(self, source) {
            remove_edge(
                &mut self.layerless_nodes[source.node].ports[source.port].outgoing_edges,
                edge_index,
            );
        }
        Some(source)
    }

    pub fn detach_edge_target(&mut self, edge_index: usize) -> Option<PortRef> {
        let target = self.edges.get(edge_index)?.target;
        if port_exists(self, target) {
            remove_edge(
                &mut self.layerless_nodes[target.node].ports[target.port].incoming_edges,
                edge_index,
            );
        }
        Some(target)
    }

    pub fn detach_edge(&mut self, edge_index: usize) -> Option<(PortRef, PortRef)> {
        let source = self.detach_edge_source(edge_index)?;
        let target = self.detach_edge_target(edge_index)?;
        Some((source, target))
    }

    pub fn edge_source_attached(&self, edge_index: usize) -> bool {
        let Some(edge) = self.edges.get(edge_index) else {
            return false;
        };
        self.layerless_nodes
            .get(edge.source.node)
            .and_then(|node| node.ports.get(edge.source.port))
            .map(|port| port.outgoing_edges.contains(&edge_index))
            .unwrap_or(false)
    }

    pub fn edge_target_attached(&self, edge_index: usize) -> bool {
        let Some(edge) = self.edges.get(edge_index) else {
            return false;
        };
        self.layerless_nodes
            .get(edge.target.node)
            .and_then(|node| node.ports.get(edge.target.port))
            .map(|port| port.incoming_edges.contains(&edge_index))
            .unwrap_or(false)
    }

    pub fn add_edge(&mut self, edge: LayeredEdge) -> Option<usize> {
        if !port_exists(self, edge.source) || !port_exists(self, edge.target) {
            return None;
        }

        let edge_index = self.edges.len();
        let source = edge.source;
        let target = edge.target;
        self.edges.push(edge);
        self.layerless_nodes[source.node].ports[source.port]
            .outgoing_edges
            .push(edge_index);
        self.layerless_nodes[target.node].ports[target.port]
            .incoming_edges
            .push(edge_index);
        Some(edge_index)
    }

    pub fn node_incoming_edges(&self, node_index: usize) -> Vec<usize> {
        self.layerless_nodes[node_index]
            .ports
            .iter()
            .flat_map(|port| port.incoming_edges.iter().copied())
            .collect()
    }

    pub fn node_outgoing_edges(&self, node_index: usize) -> Vec<usize> {
        self.layerless_nodes[node_index]
            .ports
            .iter()
            .flat_map(|port| port.outgoing_edges.iter().copied())
            .collect()
    }

    pub fn node_connected_edges(&self, node_index: usize) -> Vec<usize> {
        self.layerless_nodes[node_index]
            .ports
            .iter()
            .flat_map(|port| {
                port.incoming_edges
                    .iter()
                    .chain(port.outgoing_edges.iter())
                    .copied()
            })
            .collect()
    }

    /// ELK's `LNode#getPorts(PortType.OUTPUT)` is based on actual outgoing edge incidence.
    pub fn port_has_output_type(&self, port_ref: PortRef) -> bool {
        self.layerless_nodes
            .get(port_ref.node)
            .and_then(|node| node.ports.get(port_ref.port))
            .map(|port| !port.outgoing_edges.is_empty())
            .unwrap_or(false)
    }

    /// ELK's `LNode#getPorts(PortType.INPUT)` is based on actual incoming edge incidence.
    pub fn port_has_input_type(&self, port_ref: PortRef) -> bool {
        self.layerless_nodes
            .get(port_ref.node)
            .and_then(|node| node.ports.get(port_ref.port))
            .map(|port| !port.incoming_edges.is_empty())
            .unwrap_or(false)
    }

    pub fn reorder_node_ports(
        &mut self,
        node_index: usize,
        new_order: impl IntoIterator<Item = usize>,
    ) -> bool {
        self.reorder_node_ports_with_map(node_index, new_order)
            .is_some()
    }

    /// Reorder ports and all references in a graph whose port references are well formed.
    /// Local port-owner fields are normalized even when the permutation is the identity.
    pub fn reorder_node_ports_with_map(
        &mut self,
        node_index: usize,
        new_order: impl IntoIterator<Item = usize>,
    ) -> Option<Vec<usize>> {
        self.reorder_node_ports_with_remap(node_index, new_order, None)
    }

    pub(crate) fn reorder_node_ports_with_remap(
        &mut self,
        node_index: usize,
        new_order: impl IntoIterator<Item = usize>,
        flat_remap: Option<&FlatPortRemap>,
    ) -> Option<Vec<usize>> {
        let (old_to_new, changed) = self.permute_node_ports(node_index, new_order)?;
        if changed {
            if let Some(remap) = flat_remap {
                remap.apply(self, node_index, &old_to_new);
            } else {
                self.remap_port_references(
                    |owner| (owner == node_index).then_some(old_to_new.as_slice()),
                    false,
                    remap_port_slot,
                );
            }
        }
        Some(old_to_new)
    }

    /// Apply the first sorter permutation immediately. The census has checked node and holder
    /// slots. Check other edge slots while repairing this owner's references in legacy order.
    pub(crate) fn reorder_first_port_list(&mut self, node_index: usize, order: Vec<usize>) -> bool {
        let Some((old_to_new, true)) = self.permute_node_ports(node_index, order) else {
            return false;
        };
        self.remap_port_references(
            |owner| (owner == node_index).then_some(old_to_new.as_slice()),
            true,
            remap_port_slot,
        )
    }

    fn permute_node_ports(
        &mut self,
        node_index: usize,
        new_order: impl IntoIterator<Item = usize>,
    ) -> Option<(Vec<usize>, bool)> {
        let node = self.layerless_nodes.get(node_index)?;
        let port_count = node.ports.len();
        let new_order = new_order.into_iter().collect::<Vec<_>>();
        if new_order.len() != port_count {
            return None;
        }

        let mut seen = vec![false; port_count];
        let mut identity = true;
        for (new_index, old_index) in new_order.iter().enumerate() {
            if *old_index >= port_count || seen[*old_index] {
                return None;
            }
            seen[*old_index] = true;
            identity &= new_index == *old_index;
        }

        if identity {
            for port in &mut self.layerless_nodes[node_index].ports {
                port.node = node_index;
            }
            return Some((new_order, false));
        }

        let ports = &mut self.layerless_nodes[node_index].ports;
        let mut old_to_new = vec![0usize; port_count];
        // Validation marked every slot as seen. Clear each slot as its cycle is installed.
        // The first port moves through the cycle, so its final slot needs no swap.
        for start in 0..port_count {
            let mut destination = start;
            while seen[destination] {
                seen[destination] = false;
                let source = new_order[destination];
                old_to_new[source] = destination;
                if source != start {
                    ports.swap(destination, source);
                }
                ports[destination].node = node_index;
                destination = source;
            }
        }
        Some((old_to_new, true))
    }

    fn remap_port_references<'a>(
        &mut self,
        slots: impl Fn(usize) -> Option<&'a [usize]>,
        mut can_defer: bool,
        update: impl Fn(&mut usize, &[usize]),
    ) -> bool {
        let mut visit_edge_port = |port: &mut PortRef| {
            if let Some(old_to_new) = slots(port.node) {
                update(&mut port.port, old_to_new);
            } else if can_defer {
                can_defer = port_exists_in_nodes(&self.layerless_nodes, *port);
            }
        };
        for edge in &mut self.edges {
            visit_edge_port(&mut edge.source);
            visit_edge_port(&mut edge.target);
            if let Some(port) = edge.original_opposite_port.as_mut() {
                visit_edge_port(port);
            }
        }

        for holder in &mut self.self_loop_holders {
            if let Some(old_to_new) = slots(holder.node) {
                visit_self_loop_holder_port_slots(holder, |port| update(port, old_to_new));
            }
        }

        let remap_port = |port: &mut PortRef| {
            if let Some(old_to_new) = slots(port.node) {
                update(&mut port.port, old_to_new);
            }
        };

        let domain = &self.random_seed_context.domain;
        for node in &mut self.layerless_nodes {
            remap_origin_port(&mut node.origin_port, domain, &remap_port);
            if let Some(port) = node.long_edge_source.as_mut() {
                remap_port(port);
            }
            if let Some(port) = node.long_edge_target.as_mut() {
                remap_port(port);
            }
            if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
                remap_descendant_origins(nested_graph, domain, &remap_port);
            }
        }
        can_defer
    }

    fn remap_deferred_port_references<'a>(&mut self, slots: impl Fn(usize) -> Option<&'a [usize]>) {
        self.remap_port_references(slots, false, |port, old_to_new| {
            if let Some(&slot) = old_to_new.get(*port) {
                *port = slot;
            }
        });
    }
}

fn remap_port_slot(port: &mut usize, old_to_new: &[usize]) {
    *port = old_to_new[*port];
}

fn remap_self_loop_holder_ports(holder: &mut SelfLoopHolder, old_to_new: &[usize]) {
    visit_self_loop_holder_port_slots(holder, |port| remap_port_slot(port, old_to_new));
}

fn visit_self_loop_holder_port_slots(
    holder: &mut SelfLoopHolder,
    mut visit: impl FnMut(&mut usize),
) {
    for hyper_loop in &mut holder.hyper_loops {
        for port in &mut hyper_loop.ports {
            visit(&mut port.port);
        }
        for edge in &mut hyper_loop.edges {
            visit(&mut edge.source_port);
            visit(&mut edge.target_port);
        }
    }
}

/// Reverse an edge according to `LEdge.reverse(...)`.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LEdge.java
///
/// `adapt_ports` follows ELK's collector-port handling: a reversed edge that previously targeted
/// an input collector is moved to the same node's output collector, and a reversed edge that
/// previously sourced an output collector is moved to the same node's input collector.
pub fn reverse_edge(graph: &mut LGraph, edge_index: usize, adapt_ports: bool) -> bool {
    let Some(edge) = graph.edges.get(edge_index) else {
        return false;
    };
    let old_source = edge.source;
    let old_target = edge.target;

    if !port_exists(graph, old_source) || !port_exists(graph, old_target) {
        return false;
    }

    remove_edge(
        &mut graph.layerless_nodes[old_source.node].ports[old_source.port].outgoing_edges,
        edge_index,
    );
    remove_edge(
        &mut graph.layerless_nodes[old_target.node].ports[old_target.port].incoming_edges,
        edge_index,
    );

    let new_source = if adapt_ports
        && graph.layerless_nodes[old_target.node].ports[old_target.port].collector_type
            == Some(PortType::Input)
    {
        graph
            .provide_collector_port(old_target.node, PortType::Output, PortSide::East)
            .unwrap_or(old_target)
    } else {
        old_target
    };

    let new_target = if adapt_ports
        && graph.layerless_nodes[old_source.node].ports[old_source.port].collector_type
            == Some(PortType::Output)
    {
        graph
            .provide_collector_port(old_source.node, PortType::Input, PortSide::West)
            .unwrap_or(old_source)
    } else {
        old_source
    };

    graph.layerless_nodes[new_source.node].ports[new_source.port]
        .outgoing_edges
        .push(edge_index);
    graph.layerless_nodes[new_target.node].ports[new_target.port]
        .incoming_edges
        .push(edge_index);

    let edge = &mut graph.edges[edge_index];
    edge.source = new_source;
    edge.target = new_target;

    for label in &mut edge.labels {
        label.placement = match label.placement {
            EdgeLabelPlacement::Head => EdgeLabelPlacement::Tail,
            EdgeLabelPlacement::Tail => EdgeLabelPlacement::Head,
            EdgeLabelPlacement::Center => EdgeLabelPlacement::Center,
        };
    }

    edge.reversed = !edge.reversed;
    edge.bend_points.reverse();
    true
}

fn port_exists(graph: &LGraph, port_ref: PortRef) -> bool {
    port_exists_in_nodes(&graph.layerless_nodes, port_ref)
}

fn port_exists_in_nodes(nodes: &[LNode], port_ref: PortRef) -> bool {
    nodes
        .get(port_ref.node)
        .and_then(|node| node.ports.get(port_ref.port))
        .is_some()
}

fn centered_port_position(node_size: LSize, side: PortSide) -> LPoint {
    match side {
        PortSide::North => LPoint {
            x: node_size.width / 2.0,
            y: 0.0,
        },
        PortSide::East => LPoint {
            x: node_size.width,
            y: node_size.height / 2.0,
        },
        PortSide::South => LPoint {
            x: node_size.width / 2.0,
            y: node_size.height,
        },
        PortSide::West => LPoint {
            x: 0.0,
            y: node_size.height / 2.0,
        },
        PortSide::Undefined => LPoint::default(),
    }
}

fn remap_origin_port(
    graph_ref: &mut Option<GraphPortRef>,
    domain: &GraphReferenceDomain,
    remap: &impl Fn(&mut PortRef),
) {
    if let Some(port_ref) = graph_ref.as_mut()
        && port_ref.in_domain(domain)
    {
        remap(&mut port_ref.port);
    }
}

fn remap_descendant_origins(
    graph: &mut LGraph,
    domain: &GraphReferenceDomain,
    remap: &impl Fn(&mut PortRef),
) {
    let mut pending_siblings = Vec::new();
    let mut nodes = graph.layerless_nodes.iter_mut();
    loop {
        let Some(node) = nodes.next() else {
            let Some(siblings) = pending_siblings.pop() else {
                return;
            };
            nodes = siblings;
            continue;
        };
        remap_origin_port(&mut node.origin_port, domain, remap);
        if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
            // Finish each node's descendants before visiting its next sibling.
            if !nodes.as_slice().is_empty() {
                pending_siblings.push(nodes);
            }
            nodes = nested_graph.layerless_nodes.iter_mut();
        }
    }
}

fn remove_edge(edges: &mut Vec<usize>, edge_index: usize) {
    if let Some(position) = edges.iter().position(|candidate| *candidate == edge_index) {
        edges.remove(position);
    }
}

fn remove_node(nodes: &mut Vec<usize>, node_index: usize) {
    if let Some(position) = nodes.iter().position(|candidate| *candidate == node_index) {
        nodes.remove(position);
    }
}

/// Create an external port dummy node following ELK's `LGraphUtil.createExternalPortDummy(...)`.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/LGraphUtil.java
#[allow(clippy::too_many_arguments)]
pub fn create_external_port_dummy(
    id: impl Into<String>,
    port_id: impl Into<String>,
    port_type: PortType,
    port_constraints: PortConstraints,
    port_side: PortSide,
    net_flow: isize,
    port_position: LPoint,
    port_size: LSize,
    port_node_size: LSize,
    border_offset: f64,
    layout_direction: ElkDirection,
) -> LNode {
    let external_side = if port_constraints.is_side_fixed() {
        port_side
    } else if net_flow >= 0 {
        port_side_from_direction(layout_direction)
    } else {
        port_side_from_direction(layout_direction).opposed()
    };

    let mut dummy = LNode::new(id, 0.0, 0.0, None);
    dummy.kind = LNodeKind::ExternalPort;
    dummy.port_constraints = PortConstraints::FixedPos;
    dummy.external_port_side = external_side;
    dummy.external_port_size = port_size;

    let position = match external_side {
        PortSide::West | PortSide::East => LPoint {
            x: 0.0,
            y: port_size.height / 2.0,
        },
        PortSide::North | PortSide::South => LPoint {
            x: port_size.width / 2.0,
            y: 0.0,
        },
        PortSide::Undefined => LPoint {
            x: port_size.width / 2.0,
            y: port_size.height / 2.0,
        },
    };
    let mut dummy_port_side = external_side.opposed();

    match external_side {
        PortSide::West => {
            dummy.layer_constraint = LayerConstraint::FirstSeparate;
            dummy.layer_constraint_explicit = true;
            dummy.size.height = port_size.height;
            if border_offset < 0.0 {
                dummy.size.width = -border_offset;
            }
        }
        PortSide::East => {
            dummy.layer_constraint = LayerConstraint::LastSeparate;
            dummy.layer_constraint_explicit = true;
            dummy.size.height = port_size.height;
            if border_offset < 0.0 {
                dummy.size.width = -border_offset;
            }
        }
        PortSide::North => {
            dummy.in_layer_constraint = InLayerConstraint::Top;
            dummy.size.width = port_size.width;
            if border_offset < 0.0 {
                dummy.size.height = -border_offset;
            }
        }
        PortSide::South => {
            dummy.in_layer_constraint = InLayerConstraint::Bottom;
            dummy.size.width = port_size.width;
            if border_offset < 0.0 {
                dummy.size.height = -border_offset;
            }
        }
        PortSide::Undefined => {
            dummy_port_side = PortSide::Undefined;
        }
    }

    let mut port = LPort::new(port_id, 0, port_type);
    port.side = dummy_port_side;
    port.position = position;
    port.set_side(dummy_port_side);
    port.border_offset = Some(border_offset);
    if port_constraints.is_order_fixed() {
        dummy.port_ratio_or_position = port_ratio_or_position(
            external_side,
            port_position,
            port_node_size,
            port_constraints.is_ratio_fixed(),
        );
    }
    dummy.ports.push(port);
    dummy
}

fn port_side_from_direction(direction: ElkDirection) -> PortSide {
    match direction {
        ElkDirection::Right | ElkDirection::Undefined => PortSide::East,
        ElkDirection::Left => PortSide::West,
        ElkDirection::Down => PortSide::South,
        ElkDirection::Up => PortSide::North,
    }
}

fn port_ratio_or_position(
    side: PortSide,
    port_position: LPoint,
    port_node_size: LSize,
    ratio_fixed: bool,
) -> f64 {
    match side {
        PortSide::West | PortSide::East => {
            if ratio_fixed && port_node_size.height > 0.0 {
                port_position.y / port_node_size.height
            } else {
                port_position.y
            }
        }
        PortSide::North | PortSide::South => {
            if ratio_fixed && port_node_size.width > 0.0 {
                port_position.x / port_node_size.width
            } else {
                port_position.x
            }
        }
        PortSide::Undefined => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_port_dummy_uses_direction_and_net_flow_for_free_ports() {
        let dummy = create_external_port_dummy(
            "external:A",
            "external:A:0",
            PortType::Input,
            PortConstraints::Free,
            PortSide::Undefined,
            1,
            LPoint::default(),
            LSize {
                width: 4.0,
                height: 6.0,
            },
            LSize::default(),
            0.0,
            ElkDirection::Down,
        );

        assert_eq!(dummy.kind, LNodeKind::ExternalPort);
        assert_eq!(dummy.in_layer_constraint, InLayerConstraint::Bottom);
        assert_eq!(dummy.port_constraints, PortConstraints::FixedPos);
        assert_eq!(dummy.ports[0].side, PortSide::North);
        assert_eq!(dummy.ports[0].position, LPoint { x: 2.0, y: 0.0 });
    }

    #[test]
    fn external_port_dummy_keeps_fixed_side_and_order_metadata() {
        let dummy = create_external_port_dummy(
            "external:A",
            "external:A:0",
            PortType::Output,
            PortConstraints::FixedRatio,
            PortSide::West,
            -1,
            LPoint { x: 0.0, y: 40.0 },
            LSize {
                width: 4.0,
                height: 6.0,
            },
            LSize {
                width: 20.0,
                height: 80.0,
            },
            -3.0,
            ElkDirection::Right,
        );

        assert_eq!(dummy.layer_constraint, LayerConstraint::FirstSeparate);
        assert_eq!(dummy.size.width, 3.0);
        assert_eq!(dummy.size.height, 6.0);
        assert_eq!(dummy.ports[0].side, PortSide::East);
        assert_eq!(dummy.ports[0].position, LPoint { x: 0.0, y: 3.0 });
        assert_eq!(dummy.ports[0].anchor, LPoint { x: 0.0, y: 0.0 });
        assert_eq!(dummy.ports[0].size, LSize::default());
        assert_eq!(
            dummy.external_port_size,
            LSize {
                width: 4.0,
                height: 6.0
            }
        );
        assert_eq!(dummy.ports[0].border_offset, Some(-3.0));
        assert_eq!(dummy.port_ratio_or_position, 0.5);
    }

    #[test]
    fn scoped_references_follow_their_arena_not_the_seed_scope_or_display_id() {
        let seed_scope = GraphSeedScope::root("same");
        let new_graph = |scope| {
            let mut graph = LGraph::new_with_random_seed_authority_at_scope(
                "same",
                LayeredOptions::default(),
                RandomSeedAuthority::require_explicit(),
                scope,
            );
            let mut owner = LNode::new("owner", 0.0, 0.0, None);
            owner.ports.push(LPort::new("out", 0, PortType::Output));
            graph.layerless_nodes.push(owner);
            graph
        };
        let graph = new_graph(seed_scope.clone());
        let independent = new_graph(seed_scope);
        let different_seed = new_graph(GraphSeedScope::root("another-caller"));
        let copied = graph.clone();
        let component = graph.component_shell();
        let node = graph.scoped_node(0);
        let endpoint = PortRef { node: 0, port: 0 };
        let port = graph.scoped_port(endpoint);

        assert_eq!(graph, independent);
        assert_eq!(graph, copied);
        assert_ne!(graph, different_seed);
        assert!(!node.belongs_to(&copied));
        assert!(port.belongs_to(&component));
        assert!(!node.belongs_to(&independent));
        assert!(!port.same_target(&independent.scoped_port(endpoint)));
        assert!(!node.belongs_to(&different_seed));
        assert!(!port.same_target(&different_seed.scoped_port(endpoint)));
        let boundary = |target: &LGraph| {
            let mut dummy = LNode::new("boundary", 0.0, 0.0, None);
            dummy.origin_port = Some(target.scoped_port(endpoint));
            let mut own_port = LPort::new("external", 0, PortType::Input);
            own_port.port_dummy = Some(target.scoped_node(0));
            dummy.ports.push(own_port);
            dummy
        };
        assert_eq!(boundary(&graph), boundary(&different_seed));
        assert_eq!(
            format!("{node:?}"),
            r#"GraphNodeRef { graph_id: "same", node: 0 }"#
        );
        assert_eq!(
            format!("{port:?}"),
            r#"GraphPortRef { graph_id: "same", port: PortRef { node: 0, port: 0 } }"#
        );
    }

    #[test]
    fn attaching_renamed_template_clones_does_not_alias_parent_origins() {
        let mut template = LGraph::new("template", LayeredOptions::default());
        let mut owner = LNode::new("owner", 0.0, 0.0, None);
        owner.ports.push(LPort::new("a", 0, PortType::Input));
        owner.ports.push(LPort::new("b", 0, PortType::Output));
        template.layerless_nodes.push(owner);
        let mut local = LNode::new("local-dummy", 0.0, 0.0, None);
        local.kind = LNodeKind::ExternalPort;
        local.origin_port = Some(template.scoped_port(PortRef { node: 0, port: 1 }));
        template.layerless_nodes.push(local);

        let mut parent = template.clone();
        let mut child = template.clone();
        parent.id = "parent".into();
        child.id = "child".into();
        let mut boundary = LNode::new("parent-dummy", 0.0, 0.0, None);
        boundary.kind = LNodeKind::ExternalPort;
        boundary.origin_port = Some(parent.scoped_port(PortRef { node: 0, port: 1 }));
        child.layerless_nodes.push(boundary);
        parent.layerless_nodes[0].ports[1].port_dummy = Some(child.scoped_node(2));
        parent.layerless_nodes[0].nested_graph = Some(Box::new(child));
        let mut copied_tree = parent.clone();
        assert_eq!(copied_tree, parent);

        for graph in [&mut parent, &mut copied_tree] {
            let child = graph.layerless_nodes[0].nested_graph.as_deref().unwrap();
            let backlink = graph.layerless_nodes[0].ports[1]
                .port_dummy
                .as_ref()
                .unwrap();
            assert!(backlink.belongs_to(child));
            let child = graph.layerless_nodes[0]
                .nested_graph
                .as_deref_mut()
                .unwrap();
            assert!(child.reorder_node_ports(0, [1, 0]));
            let child = graph.layerless_nodes[0].nested_graph.as_deref().unwrap();
            let local = child.layerless_nodes[1].origin_port.as_ref().unwrap();
            assert!(local.belongs_to(child));
            assert_eq!(local.port.port, 0);
            let boundary = child.layerless_nodes[2].origin_port.as_ref().unwrap();
            assert_eq!(boundary.port.port, 1);
            assert!(!boundary.belongs_to(child));
            assert!(boundary.belongs_to(graph));

            assert!(graph.reorder_node_ports(0, [1, 0]));
            let child = graph.layerless_nodes[0].nested_graph.as_deref().unwrap();
            let boundary = child.layerless_nodes[2].origin_port.as_ref().unwrap();
            assert_eq!(boundary.port.port, 0);
        }

        let node_copy = parent.layerless_nodes[0].clone();
        let child_copy = node_copy.nested_graph.as_deref().unwrap();
        let backlink = node_copy.ports[0].port_dummy.as_ref().unwrap();
        assert!(backlink.belongs_to(child_copy));
        assert!(!backlink.belongs_to(parent.layerless_nodes[0].nested_graph.as_deref().unwrap()));
        assert!(
            child_copy.layerless_nodes[2]
                .origin_port
                .as_ref()
                .unwrap()
                .belongs_to(&parent)
        );
    }

    #[test]
    fn deep_graph_clone_rebinds_external_dummies_on_a_small_stack() {
        let result = std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                fn attach(graph: &mut LGraph, child: LGraph) {
                    let mut owner = LNode::new("owner", 0.0, 0.0, None);
                    let mut port = LPort::new("port", 0, PortType::Output);
                    port.port_dummy = Some(child.scoped_node(0));
                    owner.ports.push(port);
                    owner.nested_graph = Some(Box::new(child));
                    graph.layerless_nodes.push(owner);
                }

                fn drop_chain(mut graph: Box<LGraph>) {
                    while let Some(child) = graph.layerless_nodes[0].nested_graph.take() {
                        graph = child;
                    }
                }

                let depth = 512;
                let mut root = LGraph::new("same", LayeredOptions::default());
                let mut child = LGraph::new("same", LayeredOptions::default());
                let mut dummy = LNode::new("deep-dummy", 0.0, 0.0, None);
                dummy.kind = LNodeKind::ExternalPort;
                dummy.origin_port = Some(root.scoped_port(PortRef { node: 0, port: 0 }));
                child.layerless_nodes.push(dummy);
                for _ in 1..depth {
                    let mut graph = LGraph::new("same", LayeredOptions::default());
                    attach(&mut graph, child);
                    child = graph;
                }
                attach(&mut root, child);

                let copied = root.clone();
                let mut cursor = &copied;
                let mut visited = 0;
                let mut backlinks_match = true;
                while let Some(child) = cursor.layerless_nodes[0].nested_graph.as_deref() {
                    backlinks_match &= cursor.layerless_nodes[0].ports[0]
                        .port_dummy
                        .as_ref()
                        .is_some_and(|reference| reference.belongs_to(child));
                    visited += 1;
                    cursor = child;
                }
                let origin_rebound = cursor.layerless_nodes[0]
                    .origin_port
                    .as_ref()
                    .is_some_and(|origin| origin.belongs_to(&copied) && !origin.belongs_to(&root));
                drop_chain(Box::new(copied));
                drop_chain(Box::new(root));
                (visited == depth, backlinks_match, origin_rebound)
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(result, (true, true, true));
    }

    #[test]
    fn reorder_node_ports_rewrites_nested_external_dummy_origins() {
        let mut parent = LGraph::new("root", LayeredOptions::default());
        let mut compound = LNode::new("cluster", 10.0, 10.0, Some(0));
        compound.ports.push(LPort::new("p0", 0, PortType::Output));
        compound.ports.push(LPort::new("p1", 0, PortType::Output));

        let mut nested = LGraph::new("cluster", LayeredOptions::default());
        let mut dummy = LNode::new("external:cluster", 0.0, 0.0, None);
        dummy.kind = LNodeKind::ExternalPort;
        dummy.origin_port = Some(parent.scoped_port(PortRef { node: 0, port: 1 }));
        nested.layerless_nodes.push(dummy);
        compound.nested_graph = Some(Box::new(nested));
        parent.layerless_nodes.push(compound);

        assert!(parent.reorder_node_ports(0, [1, 0]));

        let nested = parent.layerless_nodes[0].nested_graph.as_ref().unwrap();
        assert_eq!(
            nested.layerless_nodes[0].origin_port.as_ref().unwrap().port,
            PortRef { node: 0, port: 0 }
        );
    }

    #[test]
    fn reorder_node_ports_with_map_returns_old_to_new_indices() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let mut node = LNode::new("A", 10.0, 10.0, Some(0));
        node.ports.push(LPort::new("p0", 0, PortType::Output));
        node.ports.push(LPort::new("p1", 0, PortType::Output));
        node.ports.push(LPort::new("p2", 0, PortType::Output));
        graph.layerless_nodes.push(node);

        let old_to_new = graph.reorder_node_ports_with_map(0, [2, 0, 1]).unwrap();

        assert_eq!(old_to_new, vec![1, 2, 0]);
        assert_eq!(
            graph.layerless_nodes[0]
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["p2", "p0", "p1"]
        );
    }

    fn port_reorder_reference_graph() -> LGraph {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let mut owner = LNode::new("A", 10.0, 10.0, Some(0));
        for index in 0..3 {
            owner
                .ports
                .push(LPort::new(format!("p{index}"), 0, PortType::Output));
        }
        let mut nested = LGraph::new("child", LayeredOptions::default());
        let mut external = LNode::new("external", 0.0, 0.0, None);
        external.kind = LNodeKind::ExternalPort;
        external.origin_port = Some(graph.scoped_port(PortRef { node: 0, port: 2 }));
        nested.layerless_nodes.push(external);
        owner.nested_graph = Some(Box::new(nested));
        graph.layerless_nodes.push(owner);

        let mut target = LNode::new("B", 10.0, 10.0, Some(1));
        target.ports.push(LPort::new("in", 1, PortType::Input));
        target.origin_port = Some(graph.scoped_port(PortRef { node: 0, port: 1 }));
        target.long_edge_source = Some(PortRef { node: 0, port: 0 });
        target.long_edge_target = Some(PortRef { node: 0, port: 2 });
        graph.layerless_nodes.push(target);

        for (id, source, target) in [
            (
                "edge",
                PortRef { node: 0, port: 0 },
                PortRef { node: 1, port: 0 },
            ),
            (
                "loop",
                PortRef { node: 0, port: 1 },
                PortRef { node: 0, port: 2 },
            ),
        ] {
            graph
                .add_edge(LayeredEdge {
                    id: id.to_string(),
                    source,
                    target,
                    source_node_id: "A".to_string(),
                    target_node_id: if id == "edge" { "B" } else { "A" }.to_string(),
                    labels: Vec::new(),
                    minlen: 1,
                    reversed: false,
                    bend_points: Vec::new(),
                    model_order: None,
                    priority_direction: 0,
                    priority_shortness: 0,
                    priority_straightness: 0,
                    thickness: 0.0,
                    original_opposite_port: Some(PortRef { node: 0, port: 1 }),
                    compound_segment: None,
                })
                .expect("valid reference fixture edge");
        }
        graph.self_loop_holders.push(SelfLoopHolder {
            node: 0,
            hyper_loops: vec![SelfHyperLoop {
                ports: vec![SelfLoopPort {
                    port: 1,
                    had_only_self_loops: true,
                    hidden: false,
                }],
                edges: vec![SelfLoopEdge {
                    edge: 1,
                    source_port: 1,
                    target_port: 2,
                }],
                labels: None,
                self_loop_type: None,
                leftmost_port: None,
                rightmost_port: None,
                occupied_sides: Vec::new(),
                routing_slots: [0; 5],
            }],
            ports_hidden: false,
            original_port_constraints: PortConstraints::Free,
        });
        graph
    }

    #[test]
    fn identity_port_reorder_preserves_all_reference_payloads() {
        let mut graph = port_reorder_reference_graph();
        let before = graph.clone();

        assert_eq!(
            graph.reorder_node_ports_with_map(0, [0, 1, 2]),
            Some(vec![0, 1, 2])
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn pending_port_maps_match_immediate_remaps_with_duplicate_ids_and_detached_edges() {
        let mut immediate = port_reorder_reference_graph();
        immediate.layerless_nodes[1].id = immediate.layerless_nodes[0].id.clone();
        immediate.layerless_nodes[1]
            .ports
            .push(LPort::new("in", 1, PortType::Output));
        immediate.layerless_nodes[0].ports[1].id = immediate.layerless_nodes[0].ports[0].id.clone();
        immediate.detach_edge(1).unwrap();
        let mut deferred = immediate.clone();
        let mut pending = PendingPortRemap::default();

        for (owner, order) in [
            (0, vec![2, 0, 1]),
            (1, vec![1, 0]),
            (0, vec![0, 0, 1]),
            (0, vec![0, 2, 1]),
            (1, vec![0, 1]),
        ] {
            immediate.reorder_node_ports(owner, order.clone());
            pending.reorder(&mut deferred, owner, order);
        }
        pending.apply(&mut deferred);

        assert_eq!(deferred, immediate);
        assert_eq!(deferred.edges[1].source, PortRef { node: 0, port: 1 });
        assert!(!deferred.edge_source_attached(1));
    }

    #[test]
    fn flat_port_remap_matches_the_full_walk_across_permutations() {
        let mut full = port_reorder_reference_graph();
        let mut child = full.layerless_nodes[0].nested_graph.take().unwrap();
        let external = child.layerless_nodes.pop().unwrap();
        let mut foreign = external.clone();
        let peer = LGraph::new("peer", LayeredOptions::default());
        foreign.origin_port = Some(peer.scoped_port(external.origin_port.as_ref().unwrap().port));
        full.layerless_nodes.extend([external, foreign]);
        full.layerless_nodes[1]
            .ports
            .push(LPort::new("out", 1, PortType::Output));
        let mut parallel = full.edges[0].clone();
        parallel.id = "parallel".to_string();
        parallel.target.port = 1;
        full.add_edge(parallel).unwrap();
        let mut indexed = full.clone();
        let remap = FlatPortRemap::new(&indexed, &mut NoopWorkControl).unwrap();

        for (owner, order) in [
            (0, vec![0, 1, 2]),
            (0, vec![2, 0, 1]),
            (1, vec![1, 0]),
            (0, vec![0, 0, 1]),
            (0, vec![1, 2, 0]),
        ] {
            let expected = full.reorder_node_ports_with_map(owner, order.clone());
            assert_eq!(
                indexed.reorder_node_ports_with_remap(owner, order, Some(&remap)),
                expected
            );
            assert_eq!(indexed, full);
        }
        assert_eq!(
            indexed.layerless_nodes[3]
                .origin_port
                .as_ref()
                .unwrap()
                .port,
            PortRef { node: 0, port: 2 }
        );
    }

    #[test]
    fn identity_port_reorder_normalizes_owners_for_empty_single_and_multiple_ports() {
        for count in [0, 1, 3] {
            let mut graph = LGraph::new("root", LayeredOptions::default());
            graph
                .layerless_nodes
                .push(LNode::new("other", 0.0, 0.0, None));
            let mut node = LNode::new("owner", 0.0, 0.0, None);
            for index in 0..count {
                node.ports
                    .push(LPort::new(format!("p{index}"), 99, PortType::Output));
            }
            graph.layerless_nodes.push(node);
            let mut expected = graph.clone();
            for port in &mut expected.layerless_nodes[1].ports {
                port.node = 1;
            }

            assert_eq!(
                graph.reorder_node_ports_with_map(1, 0..count),
                Some((0..count).collect())
            );
            assert_eq!(graph, expected);
        }
    }

    #[test]
    fn invalid_port_permutations_leave_graph_and_owners_unchanged() {
        let mut graph = port_reorder_reference_graph();
        graph.layerless_nodes[0].ports[0].node = 99;
        let before = graph.clone();
        for order in [
            vec![],
            vec![0, 1],
            vec![0, 1, 2, 3],
            vec![0, 0, 2],
            vec![0, 1, 3],
        ] {
            assert_eq!(graph.reorder_node_ports_with_map(0, order), None);
            assert_eq!(graph, before);
        }
        assert_eq!(graph.reorder_node_ports_with_map(99, []), None);
        assert_eq!(graph, before);
    }

    #[test]
    fn nonidentity_port_reorder_normalizes_ports_in_disjoint_cycles_and_fixed_slots() {
        let mut graph = port_reorder_reference_graph();
        let ports = &mut graph.layerless_nodes[1].ports;
        ports[0].node = 99;
        ports.extend((1..6).map(|index| LPort::new(format!("p{index}"), 99, PortType::Output)));
        let order = [2, 0, 1, 3, 5, 4];
        let expected = order.map(|old_index| {
            let mut port = ports[old_index].clone();
            port.node = 1;
            port
        });

        assert_eq!(
            graph.reorder_node_ports_with_map(1, order),
            Some(vec![1, 2, 0, 3, 5, 4])
        );
        assert_eq!(graph.layerless_nodes[1].ports, expected);
        assert_eq!(graph.edges[0].target, PortRef { node: 1, port: 1 });
    }

    #[test]
    fn nonidentity_port_reorder_updates_all_reference_payloads() {
        let mut graph = port_reorder_reference_graph();

        assert_eq!(
            graph.reorder_node_ports_with_map(0, [2, 0, 1]),
            Some(vec![1, 2, 0])
        );
        assert_eq!(graph.edges[0].source, PortRef { node: 0, port: 1 });
        assert_eq!(graph.edges[0].target, PortRef { node: 1, port: 0 });
        assert_eq!(
            graph.edges[0].original_opposite_port,
            Some(PortRef { node: 0, port: 2 })
        );
        assert_eq!(graph.edges[1].source, PortRef { node: 0, port: 2 });
        assert_eq!(graph.edges[1].target, PortRef { node: 0, port: 0 });
        let reference = &graph.layerless_nodes[1];
        assert_eq!(
            reference.origin_port.as_ref().unwrap().port,
            PortRef { node: 0, port: 2 }
        );
        assert_eq!(
            reference.long_edge_source,
            Some(PortRef { node: 0, port: 1 })
        );
        assert_eq!(
            reference.long_edge_target,
            Some(PortRef { node: 0, port: 0 })
        );
        let nested = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
        assert_eq!(
            nested.layerless_nodes[0].origin_port.as_ref().unwrap().port,
            PortRef { node: 0, port: 0 }
        );
        let saved = &graph.self_loop_holders[0].hyper_loops[0];
        assert_eq!(saved.ports[0].port, 2);
        assert_eq!(saved.edges[0].source_port, 2);
        assert_eq!(saved.edges[0].target_port, 0);
    }
}
