//! ELK graph importer scaffold.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/transform/ElkGraphImporter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredLayoutProvider.java
//! - https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/packages/mermaid-layout-elk/src/render.ts

mod hierarchy_index;
mod scoped_segments;

use std::collections::{HashMap, HashSet, VecDeque};

use hierarchy_index::HierarchyQueryIndex;
use scoped_segments::{
    ScopedSegmentWorkPlan, materialize_scoped_edge_segments, plan_scoped_edge_segments,
};

use crate::compound::introduce_source_ported_scoped_edge_segments;
use crate::graph::{
    CompoundEdgeSegment, EdgeLabelPlacement, HierarchyEdge, LGraph, LLabel, LNode, LPort, LSize,
    LayeredEdge, PortRef, PortSide, PortType, create_external_port_dummy,
};
use crate::options::{
    CycleBreakingStrategy, ELK_DEFAULT_EDGE_THICKNESS, ElkDirection, ElkPadding, HierarchyHandling,
    LayerConstraint, LayeredOptions, LayeringStrategy, NodeLabelPlacement, OrderingStrategy,
    PortConstraints,
};
use crate::random::RandomSeedAuthority;
use crate::work::{NoopWorkControl, WorkControl, WorkError, ceil_log2, checked_mul, checked_sum};
use crate::{GraphSeedScope, OperationSeed};

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputGraph {
    pub id: String,
    pub options: LayeredOptions,
    pub nodes: Vec<ElkInputNode>,
    pub edges: Vec<ElkInputEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputNode {
    pub id: String,
    pub width: f64,
    pub height: f64,
    pub parent: Option<String>,
    pub direction: Option<ElkDirection>,
    pub hierarchy_handling: Option<HierarchyHandling>,
    pub layer_constraint: Option<LayerConstraint>,
    pub port_constraints: Option<PortConstraints>,
    pub port_alignment: Option<crate::options::PortAlignment>,
    pub node_label_placement: NodeLabelPlacement,
    pub node_flexibility: crate::options::NodeFlexibility,
    pub ports_surrounding: Option<crate::options::SpacingMargin>,
    /// Resolved options for the graph inside this node. Absent values use ELK defaults.
    pub nested_options: Option<Box<LayeredOptions>>,
    pub label: Option<ElkInputLabel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub labels: Vec<ElkInputLabel>,
    pub minlen: usize,
    pub inside_self_loops_yo: bool,
    /// Explicit input-model position within the edge's owning hierarchy scope.
    ///
    /// When absent, standalone imports retain the historical filtered-input ordinal.
    pub model_order: Option<usize>,
    pub priority_direction: i32,
    pub priority_shortness: i32,
    pub priority_straightness: i32,
}

/// One endpoint of a hierarchy edge segment materialized by an outer recursive-layout owner.
///
/// `ParentBoundary` reuses the compound preprocessor's external-port dummy semantics. The named
/// node is the collapsed compound node in the parent scope; it is intentionally absent from the
/// current scope's node list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElkInputEdgeSegmentEndpoint {
    Node { id: String },
    ParentBoundary { id: String, connects_node: bool },
}

/// A scope-local piece of one original hierarchy-crossing edge.
#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputEdgeSegment {
    pub edge: ElkInputEdge,
    pub source: ElkInputEdgeSegmentEndpoint,
    pub target: ElkInputEdgeSegmentEndpoint,
    pub segment: CompoundEdgeSegment,
    pub edge_order: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputLabel {
    /// Caller-owned stable label index, preserved in exported labels.
    pub source_index: Option<usize>,
    pub text: String,
    pub width: f64,
    pub height: f64,
    pub placement: EdgeLabelPlacement,
    pub inline: bool,
}

impl ElkInputLabel {
    pub fn center(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            source_index: None,
            text: text.into(),
            width,
            height,
            placement: EdgeLabelPlacement::Center,
            inline: true,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImportError {
    #[error(transparent)]
    NodeSize(#[from] crate::options::NodeSizeError),
    #[error("ELK graph has duplicate node id: {id}")]
    DuplicateNode { id: String },
    #[error("ELK graph has duplicate edge id: {id}")]
    DuplicateEdge { id: String },
    #[error("ELK edge `{edge_id}` references missing node `{node_id}`")]
    MissingEndpoint { edge_id: String, node_id: String },
    #[error("ELK node `{node_id}` references missing parent `{parent_id}`")]
    MissingParent { node_id: String, parent_id: String },
    #[error("ELK parent assignment would create a cycle at node `{node_id}`")]
    ParentCycle { node_id: String },
    #[error("ELK scoped edge `{edge_id}` targets unavailable hierarchy scope `{graph_parent}`")]
    UnavailableSegmentScope {
        edge_id: String,
        graph_parent: String,
    },
    #[error(transparent)]
    Work(#[from] WorkError),
}

pub type ImportResult<T> = Result<T, ImportError>;

/// Imports the adapter graph into an ELK layered `LGraph`.
///
/// This mirrors the front half of `ElkGraphImporter.importGraph(...)`: create the root `LGraph`,
/// transform nodes into `layerless_nodes`, create nested graphs for hierarchy-enabled compound
/// nodes, transform edges with synthetic ports, and mark graph properties discovered during import.
pub fn import_graph(input: &ElkInputGraph) -> ImportResult<LGraph> {
    let mut work_control = NoopWorkControl;
    import_graph_with_work_control(input, &mut work_control)
}

/// Imports an adapter graph while charging its unique input items to the caller-owned control.
pub fn import_graph_with_work_control(
    input: &ElkInputGraph,
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    import_graph_at_scope_and_segments_with_work_control(
        input,
        &[input.id.as_str()],
        &[],
        work_control,
    )
}

pub fn import_graph_at_scope_and_segments_with_work_control(
    input: &ElkInputGraph,
    root_scope: &[&str],
    segments: &[ElkInputEdgeSegment],
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    let graph_scope = graph_seed_scope(input, root_scope);
    import_graph_at_seed_scope_and_segments_with_work_control(
        input,
        &graph_scope,
        segments,
        work_control,
    )
}

pub fn import_graph_at_seed_scope_and_segments_with_work_control(
    input: &ElkInputGraph,
    graph_scope: &GraphSeedScope,
    segments: &[ElkInputEdgeSegment],
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    import_graph_with_random_seed_authority(
        input,
        RandomSeedAuthority::require_explicit(),
        graph_scope.clone(),
        segments,
        work_control,
    )
}

/// Imports an adapter graph using the seed captured by its owning layout operation.
///
/// The operation seed is retained by the root and every nested graph using stable graph-id paths.
/// Execution resolves the upstream `randomSeed = 0` sentinel at GraphConfigurator boundaries; it
/// never reads a clock, random device, environment variable, or process-global state. Raw callers
/// that do not own an operation must use [`import_graph`], which fails closed for the sentinel.
pub fn import_graph_with_operation_seed(
    input: &ElkInputGraph,
    operation_seed: OperationSeed,
) -> ImportResult<LGraph> {
    let mut work_control = NoopWorkControl;
    import_graph_with_operation_seed_and_work_control(input, operation_seed, &mut work_control)
}

pub fn import_graph_with_operation_seed_and_work_control(
    input: &ElkInputGraph,
    operation_seed: OperationSeed,
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    import_graph_with_operation_seed_at_scope_and_segments_with_work_control(
        input,
        operation_seed,
        &[input.id.as_str()],
        &[],
        work_control,
    )
}

/// Like [`import_graph_with_operation_seed`], but anchors the graph at a caller-supplied absolute
/// hierarchy path. Recursive layout wrappers use this to keep separately laid-out child graphs
/// distinct from same-named roots.
pub fn import_graph_with_operation_seed_at_scope(
    input: &ElkInputGraph,
    operation_seed: OperationSeed,
    root_scope: &[&str],
) -> ImportResult<LGraph> {
    let mut work_control = NoopWorkControl;
    import_graph_with_operation_seed_at_scope_and_segments_with_work_control(
        input,
        operation_seed,
        root_scope,
        &[],
        &mut work_control,
    )
}

/// Imports a separately materialized hierarchy scope and its boundary edge pieces.
pub fn import_graph_with_operation_seed_at_scope_and_segments_with_work_control(
    input: &ElkInputGraph,
    operation_seed: OperationSeed,
    root_scope: &[&str],
    segments: &[ElkInputEdgeSegment],
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    let graph_scope = graph_seed_scope(input, root_scope);
    import_graph_with_operation_seed_at_seed_scope_and_segments_with_work_control(
        input,
        operation_seed,
        &graph_scope,
        segments,
        work_control,
    )
}

pub fn import_graph_with_operation_seed_at_seed_scope_and_segments_with_work_control(
    input: &ElkInputGraph,
    operation_seed: OperationSeed,
    graph_scope: &GraphSeedScope,
    segments: &[ElkInputEdgeSegment],
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    import_graph_with_random_seed_authority(
        input,
        RandomSeedAuthority::operation(operation_seed),
        graph_scope.clone(),
        segments,
        work_control,
    )
}

fn import_graph_with_random_seed_authority(
    input: &ElkInputGraph,
    random_seed_authority: RandomSeedAuthority,
    root_scope: GraphSeedScope,
    segments: &[ElkInputEdgeSegment],
    work_control: &mut dyn WorkControl,
) -> ImportResult<LGraph> {
    let mut port_probe = ImportPortProbe::default();
    let mut hierarchy_probe = ImportHierarchyProbe::default();
    import_graph_with_random_seed_authority_and_probe(
        input,
        random_seed_authority,
        root_scope,
        segments,
        work_control,
        &mut port_probe,
        &mut hierarchy_probe,
    )
}

fn import_graph_with_random_seed_authority_and_probe(
    input: &ElkInputGraph,
    random_seed_authority: RandomSeedAuthority,
    root_scope: GraphSeedScope,
    segments: &[ElkInputEdgeSegment],
    work_control: &mut dyn WorkControl,
    port_probe: &mut ImportPortProbe,
    hierarchy_probe: &mut ImportHierarchyProbe,
) -> ImportResult<LGraph> {
    // Mermaid/ELK resolves the complete containment model before it emits layered graph objects.
    // The Rust port additionally allocates hash/HLD indices for that planning step, so charge its
    // conservative bound before either validation traversal or allocation becomes observable.
    let planning = ImportPlanningWorkPlan::new(input, segments)?;
    let planning_work = planning.total()?;
    work_control.check(planning_work)?;
    work_control.charge(planning_work)?;
    let index = InputIndex::new(input)?;
    let scoped_work = plan_scoped_edge_segments(&index, segments)?;
    let materialization = ImportMaterializationWorkPlan::new(input, segments, &index, scoped_work)?;
    let materialization_work = materialization.total()?;
    work_control.check(materialization_work)?;
    work_control.charge(materialization_work)?;
    let prepared_segments =
        materialize_scoped_edge_segments(&index, segments, input.options.merge_edges, scoped_work);
    let mut root = LGraph::new_with_random_seed_authority_at_scope(
        input.id.clone(),
        input.options.clone(),
        random_seed_authority,
        root_scope.clone(),
    );
    root.options.direction = resolve_direction(root.options.direction);
    root.options.hierarchy_handling =
        resolve_root_hierarchy_handling(root.options.hierarchy_handling);
    apply_graph_padding_from_options(&mut root);
    let mut port_index = ImportPortIndex::new(port_probe);

    if root.options.hierarchy_handling == HierarchyHandling::IncludeChildren {
        root = import_hierarchical_graph(
            &index,
            root,
            random_seed_authority,
            &root_scope,
            &mut port_index,
            hierarchy_probe,
        )?;
    } else {
        import_flat_graph(input, &index, &mut root, None, &mut port_index)?;
    }

    introduce_source_ported_scoped_edge_segments(&mut root, prepared_segments.segments);

    root.sync_graph_properties_to_options();
    Ok(root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportPlanningWorkPlan {
    // One unit per caller-owned node, edge, or already scoped segment descriptor.
    indexed_descriptors: usize,
    // Each original edge performs at most two constant-time indexed ancestor checks.
    edge_owner_query_bound: usize,
    // Safe heavy-light query ceiling for every already scoped segment.
    scoped_hierarchy_query_bound: usize,
}

impl ImportPlanningWorkPlan {
    fn new(input: &ElkInputGraph, segments: &[ElkInputEdgeSegment]) -> ImportResult<Self> {
        let indexed_descriptors =
            checked_sum([input.nodes.len(), input.edges.len(), segments.len()])?;
        let edge_owner_query_bound = checked_mul(input.edges.len(), 2)?;
        // In a heavy-light decomposition, moving across a light edge at least doubles the subtree
        // size. One LCA plus two branch lifts therefore crosses at most four logarithmic chains;
        // the extra chain covers the final within-chain probe. This is deliberately a ceiling,
        // computed before InputIndex exists, rather than work measured after it already ran.
        let chain_query_bound = checked_sum([ceil_log2(input.nodes.len().max(1)), 1])?;
        let scoped_query_bound = checked_mul(chain_query_bound, 4)?;
        let scoped_hierarchy_query_bound = checked_mul(segments.len(), scoped_query_bound)?;
        Ok(Self {
            indexed_descriptors,
            edge_owner_query_bound,
            scoped_hierarchy_query_bound,
        })
    }

    fn total(self) -> Result<usize, WorkError> {
        checked_sum([
            self.indexed_descriptors,
            self.edge_owner_query_bound,
            self.scoped_hierarchy_query_bound,
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportMaterializationWorkPlan {
    graph_reference_domains: usize,
    // Caller-owned edge labels cloned into imported graph objects.
    edge_labels: usize,
    // Paths retained in hierarchy-edge output and paths consumed while producing scoped pieces.
    hierarchy_path_steps: usize,
    // Concrete hierarchy-local edge pieces allocated by compound materialization.
    materialized_segments: usize,
    // Allocation-free linear scans used to select one label-bearing segment.
    label_selection_steps: usize,
}

impl ImportMaterializationWorkPlan {
    fn new(
        input: &ElkInputGraph,
        segments: &[ElkInputEdgeSegment],
        index: &InputIndex<'_>,
        scoped_work: ScopedSegmentWorkPlan,
    ) -> ImportResult<Self> {
        // Count descriptors only after the initial work check has admitted their traversal.
        let edge_labels = input
            .edges
            .iter()
            .chain(segments.iter().map(|segment| &segment.edge))
            .try_fold(0usize, |count, edge| {
                checked_sum([count, edge.labels.len()])
            })?;
        let hierarchy_path_steps = checked_sum([
            hierarchical_import_output_path_steps(input, index)?,
            scoped_work.materialization_path_steps,
        ])?;
        Ok(Self {
            graph_reference_domains: index.graph_reference_domains,
            edge_labels,
            hierarchy_path_steps,
            materialized_segments: scoped_work.materialized_segments,
            label_selection_steps: scoped_work.label_selection_steps,
        })
    }

    fn total(self) -> Result<usize, WorkError> {
        checked_sum([
            self.graph_reference_domains,
            self.edge_labels,
            self.hierarchy_path_steps,
            self.materialized_segments,
            self.label_selection_steps,
        ])
    }
}

fn hierarchical_import_output_path_steps(
    input: &ElkInputGraph,
    index: &InputIndex<'_>,
) -> Result<usize, WorkError> {
    if index.root_options.hierarchy_handling != HierarchyHandling::IncludeChildren {
        return Ok(0);
    }

    let mut total = 0usize;
    for (edge, owner) in input.edges.iter().zip(index.edge_owners.iter().copied()) {
        if owner.is_some_and(|owner| !index.is_materialized_graph_parent(owner)) {
            continue;
        }
        let source_depth = index.container_depth(edge.source.as_str()).unwrap_or(0);
        let target_depth = index.container_depth(edge.target.as_str()).unwrap_or(0);
        let source_parent = index.node_parent(edge.source.as_str());
        let target_parent = index.node_parent(edge.target.as_str());
        if source_parent != target_parent {
            // ELK's compound preprocessor consumes these complete paths later. Unlike graph
            // location, retaining them on the hierarchy edge is output-sensitive work.
            total = checked_sum([total, source_depth, target_depth])?;
        }
    }
    Ok(total)
}

fn graph_seed_scope(input: &ElkInputGraph, root_scope: &[&str]) -> GraphSeedScope {
    if root_scope.is_empty() {
        GraphSeedScope::root(input.id.as_str())
    } else {
        GraphSeedScope::from_components(root_scope)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ImportPortProbe {
    #[cfg(test)]
    lookups: usize,
    #[cfg(test)]
    hits: usize,
    #[cfg(test)]
    creations: usize,
}

impl ImportPortProbe {
    fn record_lookup(&mut self) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.lookups = checked_sum([self.lookups, 1])?;
        }
        Ok(())
    }

    fn record_hit(&mut self) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.hits = checked_sum([self.hits, 1])?;
        }
        Ok(())
    }

    fn record_creation(&mut self) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.creations = checked_sum([self.creations, 1])?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct ImportNodePortIndex {
    by_type: HashMap<PortType, HashMap<String, PortRef>>,
}

#[derive(Default)]
struct ImportGraphPortIndex {
    nodes: HashMap<usize, ImportNodePortIndex>,
}

struct ImportPortIndex<'a> {
    root: ImportGraphPortIndex,
    graphs: HashMap<String, ImportGraphPortIndex>,
    probe: &'a mut ImportPortProbe,
}

impl<'a> ImportPortIndex<'a> {
    fn new(probe: &'a mut ImportPortProbe) -> Self {
        Self {
            root: ImportGraphPortIndex::default(),
            graphs: HashMap::new(),
            probe,
        }
    }

    fn get(
        &mut self,
        graph_parent: Option<&str>,
        node: usize,
        port_key: &str,
        port_type: PortType,
    ) -> ImportResult<Option<PortRef>> {
        self.probe.record_lookup()?;
        let graph = match graph_parent {
            Some(parent) => self.graphs.get(parent),
            None => Some(&self.root),
        };
        let port = graph
            .and_then(|graph| graph.nodes.get(&node))
            .and_then(|node| node.by_type.get(&port_type))
            .and_then(|ports| ports.get(port_key))
            .copied();
        if port.is_some() {
            self.probe.record_hit()?;
        }
        Ok(port)
    }

    fn insert(
        &mut self,
        graph_parent: Option<&str>,
        node: usize,
        port_key: &str,
        port_type: PortType,
        port: PortRef,
    ) -> ImportResult<()> {
        let graph = match graph_parent {
            Some(parent) => self.graphs.entry(parent.to_string()).or_default(),
            None => &mut self.root,
        };
        graph
            .nodes
            .entry(node)
            .or_default()
            .by_type
            .entry(port_type)
            .or_default()
            .insert(port_key.to_string(), port);
        self.probe.record_creation()?;
        Ok(())
    }
}

fn import_flat_graph(
    input: &ElkInputGraph,
    index: &InputIndex<'_>,
    graph: &mut LGraph,
    parent: Option<&str>,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<()> {
    let needs_model_order = needs_model_order_based_on_parent(&graph.options);
    for (model_order, node) in index.children(parent).iter().copied().enumerate() {
        transform_node(node, graph, needs_model_order.then_some(model_order));
    }
    graph.max_model_order_nodes = if needs_model_order {
        graph.layerless_nodes.len()
    } else {
        0
    };

    for (model_order, edge) in input.edges.iter().enumerate() {
        let source_parent = index.node_parent(edge.source.as_str());
        let target_parent = index.node_parent(edge.target.as_str());
        if source_parent == parent && target_parent == parent {
            transform_edge(
                edge,
                index,
                graph,
                parent,
                needs_model_order.then(|| edge.model_order.unwrap_or(model_order)),
                port_index,
            )?;
        }
    }

    Ok(())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ImportHierarchyProbe {
    #[cfg(test)]
    node_imports: usize,
    #[cfg(test)]
    edge_imports: usize,
    #[cfg(test)]
    nested_graphs: usize,
    #[cfg(test)]
    output_path_components: usize,
}

impl ImportHierarchyProbe {
    fn record_node_import(&mut self) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.node_imports = checked_sum([self.node_imports, 1])?;
        }
        Ok(())
    }

    fn record_edge_import(&mut self) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.edge_imports = checked_sum([self.edge_imports, 1])?;
        }
        Ok(())
    }

    fn record_nested_graph(&mut self) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.nested_graphs = checked_sum([self.nested_graphs, 1])?;
        }
        Ok(())
    }

    fn record_output_path_components(&mut self, components: usize) -> ImportResult<()> {
        #[cfg(test)]
        {
            self.output_path_components = checked_sum([self.output_path_components, components])?;
        }
        #[cfg(not(test))]
        let _ = components;
        Ok(())
    }
}

const ROOT_IMPORT_GRAPH_SLOT: usize = 0;

#[derive(Debug, Clone, Copy)]
struct ImportGraphAttachment {
    parent_slot: usize,
    parent_node: usize,
}

struct ImportGraphSlot {
    graph: Option<LGraph>,
    attachment: Option<ImportGraphAttachment>,
}

struct HierarchicalImportArena {
    slots: Vec<ImportGraphSlot>,
    // Use unique input-node positions, not graph ids: Mermaid may name a compound node exactly
    // like the adapter root, while the root and nested port namespaces must remain distinct.
    nested_graph_slots: Vec<Option<usize>>,
}

impl HierarchicalImportArena {
    fn new(root: LGraph, input_node_count: usize) -> Self {
        let mut slots = Vec::with_capacity(input_node_count.saturating_add(1));
        slots.push(ImportGraphSlot {
            graph: Some(root),
            attachment: None,
        });
        Self {
            slots,
            nested_graph_slots: vec![None; input_node_count],
        }
    }

    fn root(&self) -> &LGraph {
        self.graph(ROOT_IMPORT_GRAPH_SLOT)
    }

    fn root_mut(&mut self) -> &mut LGraph {
        self.graph_mut(ROOT_IMPORT_GRAPH_SLOT)
    }

    fn graph(&self, slot: usize) -> &LGraph {
        self.slots[slot]
            .graph
            .as_ref()
            .expect("import graph slots remain live until arena assembly")
    }

    fn graph_mut(&mut self, slot: usize) -> &mut LGraph {
        self.slots[slot]
            .graph
            .as_mut()
            .expect("import graph slots remain live until arena assembly")
    }

    fn graph_slot_for_parent(&self, index: &InputIndex<'_>, parent: Option<&str>) -> usize {
        // Only IncludeChildren (plus the explicit inside-self-loop dummy case) creates a slot.
        // Unmaterialized SeparateChildren scopes retain the importer's historical root fallback.
        parent
            .and_then(|parent| index.node_index(parent))
            .and_then(|parent| self.nested_graph_slots[parent])
            .unwrap_or(ROOT_IMPORT_GRAPH_SLOT)
    }

    fn add_nested_graph(
        &mut self,
        input_node: usize,
        parent_slot: usize,
        parent_node: usize,
        graph: LGraph,
    ) -> usize {
        let slot = self.slots.len();
        self.slots.push(ImportGraphSlot {
            graph: Some(graph),
            attachment: Some(ImportGraphAttachment {
                parent_slot,
                parent_node,
            }),
        });
        let previous = self.nested_graph_slots[input_node].replace(slot);
        debug_assert!(
            previous.is_none(),
            "an input node owns at most one nested graph"
        );
        slot
    }

    fn into_root(mut self) -> LGraph {
        // Parents are always allocated before descendants. Reverse assembly therefore attaches
        // complete child graphs without recursion or root-to-child path reconstruction.
        for slot in (1..self.slots.len()).rev() {
            let attachment = self.slots[slot]
                .attachment
                .expect("every non-root import graph has an attachment");
            let graph = self.slots[slot]
                .graph
                .take()
                .expect("nested import graph is assembled exactly once");
            let parent = self.slots[attachment.parent_slot]
                .graph
                .as_mut()
                .expect("parent import graph is assembled after its descendants");
            let previous = parent.layerless_nodes[attachment.parent_node]
                .nested_graph
                .replace(Box::new(graph));
            debug_assert!(previous.is_none(), "nested graph attachment is unique");
        }

        self.slots[ROOT_IMPORT_GRAPH_SLOT]
            .graph
            .take()
            .expect("root import graph is assembled exactly once")
    }
}

fn import_hierarchical_graph(
    index: &InputIndex<'_>,
    root: LGraph,
    random_seed_authority: RandomSeedAuthority,
    root_scope: &GraphSeedScope,
    port_index: &mut ImportPortIndex<'_>,
    hierarchy_probe: &mut ImportHierarchyProbe,
) -> ImportResult<LGraph> {
    let mut arena = HierarchicalImportArena::new(root, index.node_ids.len());
    let mut queue = VecDeque::new();
    queue.extend(
        index
            .children(None)
            .iter()
            .copied()
            .map(|node| (node, root_scope.clone(), ROOT_IMPORT_GRAPH_SLOT)),
    );

    let mut model_order = 0usize;
    while let Some((node, parent_scope, parent_slot)) = queue.pop_front() {
        hierarchy_probe.record_node_import()?;
        let node_model_order = needs_model_order_for_child(index, node).then(|| {
            let order = model_order;
            model_order += 1;
            order
        });
        let nested = {
            let parent_graph = arena.graph_mut(parent_slot);
            let node_index = transform_node(node, parent_graph, node_model_order);
            if node_needs_nested_graph(index, &parent_graph.options, node) {
                let materializes_children =
                    node_materializes_children(index, &parent_graph.options, node);
                let nested_options = nested_graph_options(&parent_graph.options, node);
                let nested_scope = parent_scope.child(node.id.as_str());
                let mut nested_graph = LGraph::new_with_random_seed_authority_at_scope(
                    node.id.clone(),
                    nested_options,
                    random_seed_authority,
                    nested_scope.clone(),
                );
                nested_graph.parent_node_id = Some(node.id.clone());
                apply_graph_padding_from_options(&mut nested_graph);
                apply_inside_node_label_padding(&mut nested_graph, node);
                // The input exposes a single owner title. This is the Mermaid-reachable
                // NodeLabelAndSizeCalculator minimum used by ElkGraphImporter.
                if node.node_label_placement == NodeLabelPlacement::InsideTopCenter
                    && let Some(label) = node.label.as_ref()
                {
                    nested_graph
                        .options
                        .include_inside_top_center_label_minimum(LSize {
                            width: label.width,
                            height: label.height,
                        })?;
                }
                parent_graph.layerless_nodes[node_index].compound = true;
                Some((
                    node_index,
                    nested_graph,
                    nested_scope,
                    materializes_children,
                ))
            } else {
                None
            }
        };

        if let Some((parent_node, nested_graph, nested_scope, materializes_children)) = nested {
            let input_node = index
                .node_index(node.id.as_str())
                .expect("every imported node has a stable input index");
            let nested_slot =
                arena.add_nested_graph(input_node, parent_slot, parent_node, nested_graph);
            hierarchy_probe.record_nested_graph()?;
            if materializes_children {
                // Keep the existing ELK importer breadth-first model-order traversal. The graph
                // slot is only an ownership locator and does not reorder Mermaid siblings.
                queue.extend(
                    index
                        .children(Some(node.id.as_str()))
                        .iter()
                        .copied()
                        .map(|child| (child, nested_scope.clone(), nested_slot)),
                );
            }
        }
    }

    let mut edge_order = 0usize;
    let mut edge_graph_queue = VecDeque::new();
    edge_graph_queue.push_back((None, ROOT_IMPORT_GRAPH_SLOT));
    while let Some((parent, owner_slot)) = edge_graph_queue.pop_front() {
        for edge in index.edges(parent).iter().copied() {
            hierarchy_probe.record_edge_import()?;
            let edge_model_order = needs_model_order_based_on_parent(&arena.root().options)
                .then(|| edge.model_order.unwrap_or(edge_order));
            let source_parent = index.node_parent(edge.source.as_str());
            let target_parent = index.node_parent(edge.target.as_str());
            let source_container_slot = arena.graph_slot_for_parent(index, source_parent);
            let inside_self_loops_activate = arena
                .graph(source_container_slot)
                .options
                .inside_self_loops_activate;

            if edge.inside_self_loops_yo && edge.source == edge.target && inside_self_loops_activate
            {
                let graph_slot = arena.graph_slot_for_parent(index, Some(edge.source.as_str()));
                let graph = arena.graph_mut(graph_slot);
                transform_inside_self_loop(edge, graph, edge_model_order, port_index)?;
            } else if source_parent == target_parent {
                let graph = arena.graph_mut(owner_slot);
                transform_edge(
                    edge,
                    index,
                    graph,
                    source_parent,
                    edge_model_order,
                    port_index,
                )?;
            } else {
                transform_cross_hierarchy_edge(
                    edge,
                    &mut arena,
                    index,
                    edge_order,
                    edge_model_order,
                    port_index,
                    hierarchy_probe,
                )?;
            }
            edge_order += 1;
        }

        for child in index
            .children(parent)
            .iter()
            .copied()
            .filter(|child| input_node_uses_include_children_layout(index, child))
        {
            let child_slot = arena.graph_slot_for_parent(index, Some(child.id.as_str()));
            edge_graph_queue.push_back((Some(child.id.as_str()), child_slot));
        }
    }

    arena
        .graph_mut(ROOT_IMPORT_GRAPH_SLOT)
        .max_model_order_nodes = model_order;
    Ok(arena.into_root())
}

fn transform_inside_self_loop(
    edge: &ElkInputEdge,
    graph: &mut LGraph,
    model_order: Option<usize>,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<usize> {
    let source_port_key = inside_self_loop_port_key(edge.source.as_str(), "source");
    let target_port_key = inside_self_loop_port_key(edge.target.as_str(), "target");
    let source = ensure_inside_self_loop_dummy(
        graph,
        edge.source.as_str(),
        PortType::Output,
        source_port_key.as_str(),
        port_index,
    )?;
    let target = ensure_inside_self_loop_dummy(
        graph,
        edge.target.as_str(),
        PortType::Input,
        target_port_key.as_str(),
        port_index,
    )?;

    let mut labels = Vec::new();
    for label in edge.labels.iter().filter_map(label_to_lgraph) {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
        labels.push(label);
    }

    let edge_index = graph
        .add_edge(LayeredEdge {
            id: edge.id.clone(),
            source,
            target,
            source_node_id: edge.source.clone(),
            target_node_id: edge.target.clone(),
            labels,
            minlen: edge.minlen.max(1),
            reversed: false,
            bend_points: Vec::new(),
            model_order,
            priority_direction: edge.priority_direction,
            priority_shortness: edge.priority_shortness,
            priority_straightness: edge.priority_straightness,
            thickness: ELK_DEFAULT_EDGE_THICKNESS,
            original_opposite_port: None,
            compound_segment: None,
        })
        .expect("inside self-loop dummies were created before adding edge");

    graph.graph_properties.self_loops = true;
    if has_parallel_port_edges(&graph.layerless_nodes[source.node].ports[source.port])
        || has_parallel_port_edges(&graph.layerless_nodes[target.node].ports[target.port])
    {
        graph.graph_properties.hyperedges = true;
    }

    Ok(edge_index)
}

fn ensure_inside_self_loop_dummy(
    graph: &mut LGraph,
    parent_node_id: &str,
    port_type: PortType,
    parent_port_key: &str,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<PortRef> {
    if let Some(port) =
        port_index.get(Some(parent_node_id), usize::MAX, parent_port_key, port_type)?
    {
        return Ok(port);
    }
    graph.graph_properties.external_ports = true;
    graph.graph_properties.non_free_ports = true;

    let dummy_id = format!("external:{parent_node_id}");
    let mut dummy = create_external_port_dummy(
        dummy_id,
        parent_port_key.to_string(),
        port_type,
        PortConstraints::Free,
        PortSide::Undefined,
        match port_type {
            PortType::Input => 1,
            PortType::Output => -1,
        },
        Default::default(),
        LSize::default(),
        LSize::default(),
        0.0,
        graph.options.direction,
    );
    dummy.parent_port_key = Some(parent_port_key.to_string());
    dummy.parent_port_type = Some(port_type);

    let node = graph.layerless_nodes.len();
    dummy.ports[0].node = node;
    graph.layerless_nodes.push(dummy);
    let port = PortRef { node, port: 0 };
    port_index.insert(
        Some(parent_node_id),
        usize::MAX,
        parent_port_key,
        port_type,
        port,
    )?;
    Ok(port)
}

fn inside_self_loop_port_key(node_id: &str, role: &str) -> String {
    format!("{node_id}:{role}")
}

/// Mirrors `ElkGraphImporter#needsModelOrder(...)`.
fn needs_model_order_for_child(index: &InputIndex<'_>, child: &ElkInputNode) -> bool {
    needs_model_order_based_on_input_parent(index, child)
}

/// Mirrors `ElkGraphImporter#needsModelOrderBasedOnParent(...)` for currently ported options.
fn needs_model_order_based_on_parent(options: &LayeredOptions) -> bool {
    options.consider_model_order_strategy != OrderingStrategy::None
        || matches!(
            options.cycle_breaking_strategy,
            CycleBreakingStrategy::ModelOrder | CycleBreakingStrategy::GreedyModelOrder
        )
        || options.force_node_model_order
        || options.consider_model_order_crossing_counter_node_influence != 0.0
        || options.consider_model_order_crossing_counter_port_influence != 0.0
        || matches!(
            options.layering_strategy,
            LayeringStrategy::BreadthFirstModelOrder | LayeringStrategy::DepthFirstModelOrder
        )
}

fn needs_model_order_based_on_input_parent(index: &InputIndex<'_>, child: &ElkInputNode) -> bool {
    let Some(parent_id) = child.parent.as_deref() else {
        return needs_model_order_based_on_parent(&index.root_options);
    };
    let Some(parent_options) = index.container_options.get(parent_id) else {
        return false;
    };
    needs_model_order_based_on_parent(parent_options)
}

fn input_node_uses_include_children_layout(index: &InputIndex<'_>, node: &ElkInputNode) -> bool {
    // Leaves still participate in the edge-scope traversal through their own override or parent.
    resolve_child_hierarchy_handling(
        node.hierarchy_handling,
        index.options_for_parent(node.parent.as_deref()),
    ) == HierarchyHandling::IncludeChildren
}

fn input_edge_containing_parent<'a>(
    index: &InputIndex<'a>,
    edge: &'a ElkInputEdge,
) -> ImportResult<(Option<&'a str>, usize)> {
    let source_parent = index.node_parent(edge.source.as_str());
    let target_parent = index.node_parent(edge.target.as_str());
    if source_parent == target_parent {
        return Ok((source_parent, 0));
    }

    let (target_is_descendant, target_steps) =
        index.is_node_ancestor(edge.source.as_str(), edge.target.as_str())?;
    if target_is_descendant {
        return Ok((Some(edge.source.as_str()), target_steps));
    }
    let (source_is_descendant, source_steps) =
        index.is_node_ancestor(edge.target.as_str(), edge.source.as_str())?;
    let ancestor_steps = checked_sum([target_steps, source_steps])?;
    if source_is_descendant {
        return Ok((Some(edge.target.as_str()), ancestor_steps));
    }

    Ok((None, ancestor_steps))
}

fn nested_graph_options(parent_options: &LayeredOptions, node: &ElkInputNode) -> LayeredOptions {
    // ElkGraphImporter.createLGraph copies the container's own properties. Renderer-specific
    // presets and overrides must arrive on that node rather than leak in from the root graph.
    let mut options = node.nested_options.as_deref().cloned().unwrap_or_default();
    options.direction = parent_options.direction;
    options.hierarchy_handling =
        resolve_child_hierarchy_handling(node.hierarchy_handling, parent_options);
    options.port_constraints = node.port_constraints.unwrap_or(PortConstraints::Free);
    options
}

fn resolve_child_hierarchy_handling(
    node_hierarchy_handling: Option<HierarchyHandling>,
    parent_options: &LayeredOptions,
) -> HierarchyHandling {
    match node_hierarchy_handling.unwrap_or(HierarchyHandling::Inherit) {
        HierarchyHandling::Inherit => parent_options.hierarchy_handling,
        handling => handling,
    }
}

fn resolve_root_hierarchy_handling(handling: HierarchyHandling) -> HierarchyHandling {
    match handling {
        HierarchyHandling::Inherit => HierarchyHandling::SeparateChildren,
        handling => handling,
    }
}

fn apply_graph_padding_from_options(graph: &mut LGraph) {
    let ElkPadding {
        top,
        right,
        bottom,
        left,
    } = graph.options.padding;
    graph.padding.top += top;
    graph.padding.right += right;
    graph.padding.bottom += bottom;
    graph.padding.left += left;
}

fn apply_inside_node_label_padding(graph: &mut LGraph, node: &ElkInputNode) {
    let Some(label) = node.label.as_ref() else {
        return;
    };
    let Some((row, col)) = inside_node_label_cell(node.node_label_placement) else {
        return;
    };
    let container_gap = 2.0 * graph.options.spacing.label_label;
    let padding = graph.options.node_labels_padding;
    if label.height > 0.0 {
        if row == 0 {
            graph.padding.top += label.height + padding.top + container_gap;
        } else if row == 2 {
            graph.padding.bottom += label.height + padding.bottom + container_gap;
        }
    }
    if label.width > 0.0 {
        if col == 0 {
            graph.padding.left += label.width + padding.left + container_gap;
        } else if col == 2 {
            graph.padding.right += label.width + padding.right + container_gap;
        }
    }
}

fn inside_node_label_cell(placement: NodeLabelPlacement) -> Option<(usize, usize)> {
    match placement {
        NodeLabelPlacement::InsideTopLeft => Some((0, 0)),
        NodeLabelPlacement::InsideTopCenter => Some((0, 1)),
        NodeLabelPlacement::InsideTopRight => Some((0, 2)),
        NodeLabelPlacement::InsideCenterLeft => Some((1, 0)),
        NodeLabelPlacement::InsideCenter => Some((1, 1)),
        NodeLabelPlacement::InsideCenterRight => Some((1, 2)),
        NodeLabelPlacement::InsideBottomLeft => Some((2, 0)),
        NodeLabelPlacement::InsideBottomCenter => Some((2, 1)),
        NodeLabelPlacement::InsideBottomRight => Some((2, 2)),
        NodeLabelPlacement::Fixed
        | NodeLabelPlacement::OutsideTopLeft
        | NodeLabelPlacement::OutsideTopCenter
        | NodeLabelPlacement::OutsideTopRight
        | NodeLabelPlacement::OutsideBottomLeft
        | NodeLabelPlacement::OutsideBottomCenter
        | NodeLabelPlacement::OutsideBottomRight
        | NodeLabelPlacement::OutsideLeftTop
        | NodeLabelPlacement::OutsideLeftCenter
        | NodeLabelPlacement::OutsideLeftBottom
        | NodeLabelPlacement::OutsideRightTop
        | NodeLabelPlacement::OutsideRightCenter
        | NodeLabelPlacement::OutsideRightBottom => None,
    }
}

fn transform_node(node: &ElkInputNode, graph: &mut LGraph, model_order: Option<usize>) -> usize {
    let mut lnode = LNode::new(node.id.clone(), node.width, node.height, model_order);
    lnode.port_constraints = node.port_constraints.unwrap_or(PortConstraints::Free);
    lnode.port_alignment = node.port_alignment;
    if let Some(layer_constraint) = node.layer_constraint {
        lnode.layer_constraint = layer_constraint;
        lnode.layer_constraint_explicit = true;
    }
    if let Some(label) = node.label.as_ref().and_then(label_to_lgraph) {
        lnode.labels.push(label);
    }
    lnode.node_label_placement = node.node_label_placement;
    lnode.node_flexibility = node.node_flexibility;
    lnode.ports_surrounding = node.ports_surrounding;
    graph.layerless_nodes.push(lnode);
    graph.layerless_nodes.len() - 1
}

fn transform_edge(
    edge: &ElkInputEdge,
    index: &InputIndex<'_>,
    graph: &mut LGraph,
    graph_parent: Option<&str>,
    model_order: Option<usize>,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<usize> {
    let source = ensure_port_at_node(
        graph,
        graph_parent,
        index.child_position(edge.source.as_str()),
        edge.source.as_str(),
        PortType::Output,
        port_index,
    )?
    .ok_or_else(|| ImportError::MissingEndpoint {
        edge_id: edge.id.clone(),
        node_id: edge.source.clone(),
    })?;
    let target = ensure_port_at_node(
        graph,
        graph_parent,
        index.child_position(edge.target.as_str()),
        edge.target.as_str(),
        PortType::Input,
        port_index,
    )?
    .ok_or_else(|| ImportError::MissingEndpoint {
        edge_id: edge.id.clone(),
        node_id: edge.target.clone(),
    })?;

    if source.node == target.node {
        graph.graph_properties.self_loops = true;
    }

    let mut labels = Vec::new();
    for label in edge.labels.iter().filter_map(label_to_lgraph) {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
        labels.push(label);
    }

    let edge_index = graph
        .add_edge(LayeredEdge {
            id: edge.id.clone(),
            source,
            target,
            source_node_id: edge.source.clone(),
            target_node_id: edge.target.clone(),
            labels,
            minlen: edge.minlen.max(1),
            reversed: false,
            bend_points: Vec::new(),
            model_order,
            priority_direction: edge.priority_direction,
            priority_shortness: edge.priority_shortness,
            priority_straightness: edge.priority_straightness,
            thickness: ELK_DEFAULT_EDGE_THICKNESS,
            original_opposite_port: None,
            compound_segment: None,
        })
        .expect("ports were created before adding edge");

    if has_parallel_port_edges(&graph.layerless_nodes[source.node].ports[source.port])
        || has_parallel_port_edges(&graph.layerless_nodes[target.node].ports[target.port])
    {
        graph.graph_properties.hyperedges = true;
    }

    Ok(edge_index)
}

/// Preserve a hierarchy-crossing edge for ELK's compound preprocessor.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/transform/ElkGraphImporter.java
fn transform_cross_hierarchy_edge(
    edge: &ElkInputEdge,
    arena: &mut HierarchicalImportArena,
    index: &InputIndex<'_>,
    edge_order: usize,
    model_order: Option<usize>,
    port_index: &mut ImportPortIndex<'_>,
    hierarchy_probe: &mut ImportHierarchyProbe,
) -> ImportResult<()> {
    let merge_edges = arena.root().options.merge_edges;
    let source_port_key = hierarchy_port_key(
        edge.source.as_str(),
        edge_order,
        "source",
        merge_edges,
        PortType::Output,
    );
    let target_port_key = hierarchy_port_key(
        edge.target.as_str(),
        edge_order,
        "target",
        merge_edges,
        PortType::Input,
    );
    let source_parent = index.node_parent(edge.source.as_str());
    let target_parent = index.node_parent(edge.target.as_str());

    {
        let source_slot = arena.graph_slot_for_parent(index, source_parent);
        let source_graph = arena.graph_mut(source_slot);
        ensure_hierarchy_endpoint_port(
            source_graph,
            source_parent,
            index.child_position(edge.source.as_str()),
            edge.source.as_str(),
            source_port_key.as_str(),
            PortType::Output,
            port_index,
        )?
        .ok_or_else(|| ImportError::MissingEndpoint {
            edge_id: edge.id.clone(),
            node_id: edge.source.clone(),
        })?;
    }
    {
        let target_slot = arena.graph_slot_for_parent(index, target_parent);
        let target_graph = arena.graph_mut(target_slot);
        ensure_hierarchy_endpoint_port(
            target_graph,
            target_parent,
            index.child_position(edge.target.as_str()),
            edge.target.as_str(),
            target_port_key.as_str(),
            PortType::Input,
            port_index,
        )?
        .ok_or_else(|| ImportError::MissingEndpoint {
            edge_id: edge.id.clone(),
            node_id: edge.target.clone(),
        })?;
    }

    // The owned paths are part of ELK's compound-edge input, not graph locators. Build each once
    // and move it into the retained hierarchy edge so deep-chain cost tracks emitted path data.
    let source_path = index
        .hierarchy_edge_output_path(edge.source.as_str())
        .unwrap_or_default();
    let target_path = index
        .hierarchy_edge_output_path(edge.target.as_str())
        .unwrap_or_default();
    hierarchy_probe
        .record_output_path_components(checked_sum([source_path.len(), target_path.len()])?)?;
    arena.root_mut().hierarchy_edges.push(HierarchyEdge {
        id: edge.id.clone(),
        source_node_id: edge.source.clone(),
        target_node_id: edge.target.clone(),
        source_port_key,
        target_port_key,
        source_path,
        target_path,
        labels: edge.labels.iter().filter_map(label_to_lgraph).collect(),
        minlen: edge.minlen.max(1),
        model_order,
        priority_direction: edge.priority_direction,
        priority_shortness: edge.priority_shortness,
        priority_straightness: edge.priority_straightness,
    });

    Ok(())
}

fn ensure_hierarchy_endpoint_port(
    graph: &mut LGraph,
    graph_parent: Option<&str>,
    node: usize,
    node_id: &str,
    port_key: &str,
    port_type: PortType,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<Option<PortRef>> {
    if graph.options.merge_edges
        && graph.layerless_nodes.get(node).is_some_and(|candidate| {
            candidate.id == node_id && !candidate.port_constraints.is_side_fixed()
        })
    {
        return ensure_port_at_node(graph, graph_parent, node, node_id, port_type, port_index);
    }

    let Some(candidate) = graph.layerless_nodes.get(node) else {
        return Ok(None);
    };
    if candidate.id != node_id {
        return Ok(None);
    }

    let port = graph.layerless_nodes[node].ports.len();
    graph.layerless_nodes[node]
        .ports
        .push(LPort::new(port_key.to_string(), node, port_type));
    // Dedicated hierarchy ports are never looked up again: only collector and inside-self-loop
    // identities participate in ELK's reuse rules. Indexing this edge-unique key would retain one
    // copied String and HashMap entry per endpoint without changing any later port selection.
    Ok(Some(PortRef { node, port }))
}

fn hierarchy_port_key(
    node_id: &str,
    edge_order: usize,
    role: &str,
    merge_edges: bool,
    port_type: PortType,
) -> String {
    if merge_edges {
        // Collector identity is graph-local in ImportPortIndex, then keyed by node and direction.
        // Ignoring edge_order here is what lets every eligible parallel edge reuse that collector.
        format!("{node_id}:collector:{port_type:?}")
    } else {
        format!("{node_id}:{edge_order}:{role}")
    }
}

fn ensure_port_at_node(
    graph: &mut LGraph,
    graph_parent: Option<&str>,
    node: usize,
    node_id: &str,
    port_type: PortType,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<Option<PortRef>> {
    let Some(candidate) = graph.layerless_nodes.get(node) else {
        return Ok(None);
    };
    if candidate.id != node_id {
        return Ok(None);
    }

    if graph.options.merge_edges && !graph.layerless_nodes[node].port_constraints.is_side_fixed() {
        let collector_key = hierarchy_port_key(node_id, 0, "collector", true, port_type);
        if let Some(port) = port_index.get(graph_parent, node, collector_key.as_str(), port_type)? {
            return Ok(Some(port));
        }
        let default_side = port_side_from_direction(graph.options.direction);
        let side = match port_type {
            PortType::Output => default_side,
            PortType::Input => default_side.opposed(),
        };
        let port = graph.provide_collector_port(node, port_type, side);
        if let Some(port) = port {
            port_index.insert(graph_parent, node, collector_key.as_str(), port_type, port)?;
        }
        return Ok(port);
    }

    let port = graph.layerless_nodes[node].ports.len();
    let mut implicit_port = LPort::new(format!("{node_id}:{port:?}"), node, port_type);
    // Mirror Mermaid's ELK `LGraphUtil.createPort`: dedicated implicit ports receive
    // a direction-derived side before direction preprocessing/transposition runs.
    let default_side = port_side_from_direction(graph.options.direction);
    implicit_port.set_side(match port_type {
        PortType::Output => default_side,
        PortType::Input => default_side.opposed(),
    });
    graph.layerless_nodes[node].ports.push(implicit_port);
    Ok(Some(PortRef { node, port }))
}

fn has_parallel_port_edges(port: &LPort) -> bool {
    port.incoming_edges.len() + port.outgoing_edges.len() > 1
}

fn port_side_from_direction(direction: ElkDirection) -> PortSide {
    match direction {
        ElkDirection::Right | ElkDirection::Undefined => PortSide::East,
        ElkDirection::Left => PortSide::West,
        ElkDirection::Down => PortSide::South,
        ElkDirection::Up => PortSide::North,
    }
}

fn label_to_lgraph(label: &ElkInputLabel) -> Option<LLabel> {
    if label.text.is_empty() {
        return None;
    }
    let mut llabel = LLabel::new(label.text.clone(), label.width, label.height);
    llabel.source_index = label.source_index;
    llabel.placement = label.placement;
    llabel.inline = label.inline;
    llabel.label_side = None;
    Some(llabel)
}

fn node_materializes_children(
    index: &InputIndex<'_>,
    parent_options: &LayeredOptions,
    node: &ElkInputNode,
) -> bool {
    index.has_children(node.id.as_str())
        && resolve_child_hierarchy_handling(node.hierarchy_handling, parent_options)
            == HierarchyHandling::IncludeChildren
}

fn node_needs_nested_graph(
    index: &InputIndex<'_>,
    parent_options: &LayeredOptions,
    node: &ElkInputNode,
) -> bool {
    // SeparateChildren may still need a dummy-only nested graph for activated inside self-loops;
    // that must not be confused with permission to materialize the node's ordinary children.
    node_materializes_children(index, parent_options, node)
        || (parent_options.inside_self_loops_activate
            && index.inside_self_loop_nodes.contains(node.id.as_str()))
}

fn resolve_direction(direction: ElkDirection) -> ElkDirection {
    match direction {
        ElkDirection::Undefined => ElkDirection::Right,
        direction => direction,
    }
}

struct InputIndex<'a> {
    nodes: HashMap<&'a str, &'a ElkInputNode>,
    node_order: HashMap<&'a str, usize>,
    node_ids: Vec<&'a str>,
    root_children: Vec<&'a ElkInputNode>,
    children_by_parent: HashMap<&'a str, Vec<&'a ElkInputNode>>,
    child_positions: HashMap<&'a str, usize>,
    edges_by_parent: HashMap<Option<&'a str>, Vec<&'a ElkInputEdge>>,
    inside_self_loop_nodes: HashSet<&'a str>,
    root_options: LayeredOptions,
    // Complete node-owned options are needed only for nodes that actually contain children.
    container_options: HashMap<&'a str, LayeredOptions>,
    hierarchy_queries: HierarchyQueryIndex,
    container_depths: Vec<Option<usize>>,
    materialized_graph_depths: Vec<Option<usize>>,
    graph_reference_domains: usize,
    first_unmaterialized_graph_parents: Vec<Option<&'a str>>,
    edge_owners: Vec<Option<&'a str>>,
    #[cfg(test)]
    edge_owner_query_steps: usize,
}

impl<'a> InputIndex<'a> {
    fn new(input: &'a ElkInputGraph) -> ImportResult<Self> {
        let mut nodes = HashMap::new();
        let mut node_order = HashMap::new();
        for (index, node) in input.nodes.iter().enumerate() {
            if nodes.insert(node.id.as_str(), node).is_some() {
                return Err(ImportError::DuplicateNode {
                    id: node.id.clone(),
                });
            }
            node_order.insert(node.id.as_str(), index);
        }

        let mut root_children = Vec::new();
        let mut children_by_parent: HashMap<&str, Vec<&ElkInputNode>> = HashMap::new();
        let mut child_positions = HashMap::with_capacity(input.nodes.len());
        for node in &input.nodes {
            if let Some(parent) = node.parent.as_deref()
                && !nodes.contains_key(parent)
            {
                return Err(ImportError::MissingParent {
                    node_id: node.id.clone(),
                    parent_id: parent.to_string(),
                });
            }
            let children = match node.parent.as_deref() {
                Some(parent) => children_by_parent.entry(parent).or_default(),
                None => &mut root_children,
            };
            child_positions.insert(node.id.as_str(), children.len());
            children.push(node);
        }

        let mut edge_ids = HashSet::with_capacity(input.edges.len());
        for edge in &input.edges {
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(ImportError::DuplicateEdge {
                    id: edge.id.clone(),
                });
            }
        }

        for edge in &input.edges {
            if !nodes.contains_key(edge.source.as_str()) {
                return Err(ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.clone(),
                });
            }
            if !nodes.contains_key(edge.target.as_str()) {
                return Err(ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.clone(),
                });
            }
        }

        detect_parent_cycles(input, &node_order)?;
        let hierarchy_queries =
            HierarchyQueryIndex::new(&node_order, &root_children, &children_by_parent)?;
        let node_ids = input
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();

        let mut root_options = input.options.clone();
        root_options.direction = resolve_direction(root_options.direction);
        root_options.hierarchy_handling =
            resolve_root_hierarchy_handling(root_options.hierarchy_handling);
        let mut container_options = HashMap::with_capacity(children_by_parent.len());
        let mut container_search = root_children
            .iter()
            .rev()
            .copied()
            .filter(|node| children_by_parent.contains_key(node.id.as_str()))
            .collect::<Vec<_>>();
        while let Some(node) = container_search.pop() {
            // Every non-root container has a container parent already visited by this traversal.
            let parent_options = node
                .parent
                .as_deref()
                .and_then(|parent| container_options.get(parent))
                .unwrap_or(&root_options);
            let options = nested_graph_options(parent_options, node);
            container_options.insert(node.id.as_str(), options);
            // Membership entries are created only when an actual child is appended.
            container_search.extend(
                children_by_parent[node.id.as_str()]
                    .iter()
                    .rev()
                    .copied()
                    .filter(|child| children_by_parent.contains_key(child.id.as_str())),
            );
        }

        let inside_self_loop_nodes = input
            .edges
            .iter()
            .filter(|edge| edge.inside_self_loops_yo && edge.source == edge.target)
            .map(|edge| edge.source.as_str())
            .collect::<HashSet<_>>();
        let mut result = Self {
            nodes,
            node_order,
            node_ids,
            root_children,
            children_by_parent,
            child_positions,
            edges_by_parent: HashMap::new(),
            inside_self_loop_nodes,
            root_options,
            container_options,
            hierarchy_queries,
            container_depths: vec![None; input.nodes.len()],
            materialized_graph_depths: vec![None; input.nodes.len()],
            graph_reference_domains: 1,
            first_unmaterialized_graph_parents: vec![None; input.nodes.len()],
            edge_owners: Vec::with_capacity(input.edges.len()),
            #[cfg(test)]
            edge_owner_query_steps: 0,
        };
        result.index_materialized_reachability()?;
        for edge in &input.edges {
            let (parent, ancestor_steps) = input_edge_containing_parent(&result, edge)?;
            #[cfg(test)]
            {
                result.edge_owner_query_steps =
                    checked_sum([result.edge_owner_query_steps, ancestor_steps])?;
            }
            #[cfg(not(test))]
            let _ = ancestor_steps;
            result.edge_owners.push(parent);
            result.edges_by_parent.entry(parent).or_default().push(edge);
        }
        Ok(result)
    }

    /// Returns the stable input-order child slice without cloning the parent's membership vector.
    fn children(&self, parent: Option<&str>) -> &[&'a ElkInputNode] {
        match parent {
            Some(parent) => self
                .children_by_parent
                .get(parent)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            None => &self.root_children,
        }
    }

    fn node_parent(&self, id: &str) -> Option<&'a str> {
        self.nodes.get(id).and_then(|node| node.parent.as_deref())
    }

    fn edges<'index>(&'index self, parent: Option<&'a str>) -> &'index [&'a ElkInputEdge] {
        self.edges_by_parent
            .get(&parent)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn has_children(&self, node: &str) -> bool {
        self.children_by_parent
            .get(node)
            .is_some_and(|children| !children.is_empty())
    }

    fn child_position(&self, node: &str) -> usize {
        *self
            .child_positions
            .get(node)
            .expect("validated input nodes have a stable sibling position")
    }

    fn options_for_parent(&self, parent: Option<&str>) -> &LayeredOptions {
        parent
            .and_then(|parent| self.container_options.get(parent))
            .unwrap_or(&self.root_options)
    }

    fn is_materialized_graph_parent(&self, node_id: &str) -> bool {
        self.materialized_graph_depth(node_id).is_some()
    }

    fn materialized_graph_path(&self, node: &str) -> Option<Vec<&'a str>> {
        // This path is consumed component-by-component by compound segment materialization. Graph
        // lookup itself uses HierarchicalImportArena's constant-time slot locator.
        let depth = self.container_depth(node)?;
        let mut path = Vec::with_capacity(depth);
        let mut current = self.node_parent(node);
        while let Some(parent) = current {
            path.push(parent);
            current = self.node_parent(parent);
        }
        path.reverse();
        debug_assert_eq!(path.len(), depth);
        Some(path)
    }

    fn hierarchy_edge_output_path(&self, node: &str) -> Option<Vec<String>> {
        let depth = self.container_depth(node)?;
        let mut path = Vec::with_capacity(depth);
        let mut current = self.node_parent(node);
        while let Some(parent) = current {
            path.push(parent.to_string());
            current = self.node_parent(parent);
        }
        path.reverse();
        debug_assert_eq!(path.len(), depth);
        Some(path)
    }

    fn node_index(&self, node: &str) -> Option<usize> {
        self.node_order.get(node).copied()
    }

    fn node_id(&self, node: usize) -> &'a str {
        self.node_ids[node]
    }

    fn is_node_ancestor(&self, ancestor: &str, node: &str) -> ImportResult<(bool, usize)> {
        let ancestor = self
            .node_index(ancestor)
            .expect("validated edge ancestor endpoint is indexed");
        let node = self
            .node_index(node)
            .expect("validated edge descendant endpoint is indexed");
        self.hierarchy_queries.is_ancestor(ancestor, node)
    }

    fn container_depth(&self, node: &str) -> Option<usize> {
        self.node_index(node)
            .and_then(|index| self.container_depths[index])
    }

    fn materialized_graph_depth(&self, node: &str) -> Option<usize> {
        self.node_index(node)
            .and_then(|index| self.materialized_graph_depths[index])
    }

    fn index_materialized_reachability(&mut self) -> ImportResult<()> {
        let include_children =
            self.root_options.hierarchy_handling == HierarchyHandling::IncludeChildren;
        let mut stack = self
            .root_children
            .iter()
            .rev()
            .copied()
            .map(|node| (node, Some(0usize), None))
            .collect::<Vec<_>>();
        while let Some((node, container_depth, first_unmaterialized_parent)) = stack.pop() {
            let node_index = self
                .node_index(node.id.as_str())
                .expect("indexed node order exists for every input node");
            self.container_depths[node_index] = container_depth;
            self.first_unmaterialized_graph_parents[node_index] = first_unmaterialized_parent;
            let materializes_children = include_children
                && container_depth.is_some()
                && node_materializes_children(
                    self,
                    self.options_for_parent(node.parent.as_deref()),
                    node,
                );
            let graph_depth = if include_children
                && container_depth.is_some()
                && node_needs_nested_graph(
                    self,
                    self.options_for_parent(node.parent.as_deref()),
                    node,
                ) {
                Some(checked_sum([container_depth.unwrap_or(0), 1])?)
            } else {
                None
            };
            let child_container_depth = if materializes_children {
                graph_depth
            } else {
                None
            };
            let child_unmaterialized_parent = first_unmaterialized_parent
                .or_else(|| (!materializes_children).then_some(node.id.as_str()));
            if graph_depth.is_some() {
                self.graph_reference_domains = checked_sum([self.graph_reference_domains, 1])?;
            }
            self.materialized_graph_depths[node_index] = graph_depth;
            stack.extend(
                self.children(Some(node.id.as_str()))
                    .iter()
                    .rev()
                    .copied()
                    .map(|child| (child, child_container_depth, child_unmaterialized_parent)),
            );
        }
        Ok(())
    }

    fn first_unmaterialized_graph_parent(&self, node: &str) -> Option<&'a str> {
        self.node_index(node)
            .and_then(|node| self.first_unmaterialized_graph_parents[node])
    }
}

fn detect_parent_cycles<'a>(
    input: &'a ElkInputGraph,
    node_order: &HashMap<&'a str, usize>,
) -> ImportResult<()> {
    let mut state = vec![0u8; input.nodes.len()];
    for start in 0..input.nodes.len() {
        if state[start] == 2 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current {
            match state[index] {
                2 => break,
                1 => {
                    return Err(ImportError::ParentCycle {
                        node_id: input.nodes[index].id.clone(),
                    });
                }
                _ => {
                    state[index] = 1;
                    path.push(index);
                    current = input.nodes[index]
                        .parent
                        .as_deref()
                        .and_then(|parent| node_order.get(parent).copied());
                }
            }
        }
        for index in path {
            state[index] = 2;
        }
    }
    Ok(())
}

#[cfg(test)]
mod container_options_tests;

#[cfg(test)]
mod tests;
