//! ELK graph importer scaffold.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/graph/transform/ElkGraphImporter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/LayeredLayoutProvider.java
//! - https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/packages/mermaid-layout-elk/src/render.ts

use std::collections::{HashMap, HashSet, VecDeque};

use crate::compound::{
    PendingCompoundSegment, PendingSegmentEndpoint, ScopedHierarchySegment,
    compound_label_segment_index, introduce_source_ported_scoped_edge_segments,
    source_ported_cross_hierarchy_segments,
};
use crate::graph::{
    CompoundEdgeSegment, EdgeLabelPlacement, HierarchyEdge, LGraph, LLabel, LNode, LPort, LSize,
    LayeredEdge, PortRef, PortSide, PortType, create_external_port_dummy,
};
use crate::options::{
    CycleBreakingStrategy, ElkDirection, ElkPadding, HierarchyHandling, LayerConstraint,
    LayeredOptions, LayeringStrategy, NodeLabelPlacement, OrderingStrategy, PortConstraints,
    SpacingOptions,
};
use crate::random::RandomSeedAuthority;
use crate::work::{NoopWorkControl, WorkControl, WorkError, ceil_log2, checked_mul, checked_sum};
use crate::{GraphSeedScope, OperationSeed};

// `org.eclipse.elk.edge.thickness` defaults to 1 in the pinned elkjs 0.9.3 CoreOptions.
const ELK_DEFAULT_EDGE_THICKNESS: f64 = 1.0;

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
    pub node_label_placement: NodeLabelPlacement,
    pub nested_spacing_base: Option<f64>,
    pub label: Option<ElkInputLabel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElkInputEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<ElkInputLabel>,
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
    pub text: String,
    pub width: f64,
    pub height: f64,
    pub placement: EdgeLabelPlacement,
    pub inline: bool,
}

impl ElkInputLabel {
    pub fn center(text: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
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
    let materialization = ImportMaterializationWorkPlan::new(input, &index, scoped_work)?;
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
        index: &InputIndex<'_>,
        scoped_work: ScopedSegmentWorkPlan,
    ) -> ImportResult<Self> {
        let hierarchy_path_steps = checked_sum([
            hierarchical_import_output_path_steps(input, index)?,
            scoped_work.materialization_path_steps,
        ])?;
        Ok(Self {
            hierarchy_path_steps,
            materialized_segments: scoped_work.materialized_segments,
            label_selection_steps: scoped_work.label_selection_steps,
        })
    }

    fn total(self) -> Result<usize, WorkError> {
        checked_sum([
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScopedSegmentWorkPlan {
    materialization_path_steps: usize,
    materialized_segments: usize,
    label_selection_steps: usize,
    #[cfg(test)]
    hierarchy_query_steps: usize,
}

fn plan_scoped_edge_segments(
    index: &InputIndex<'_>,
    segments: &[ElkInputEdgeSegment],
) -> ImportResult<ScopedSegmentWorkPlan> {
    let mut materialization_path_steps = 0usize;
    let mut materialized_segments = 0usize;
    let mut label_selection_steps = 0usize;
    #[cfg(test)]
    let mut hierarchy_query_steps = 0usize;

    for scoped in segments {
        validate_scoped_segment_endpoint(index, &scoped.edge, &scoped.source)?;
        validate_scoped_segment_endpoint(index, &scoped.edge, &scoped.target)?;
        let source_placeholder = scoped_segment_endpoint_id(&scoped.source);
        let target_placeholder = scoped_segment_endpoint_id(&scoped.target);
        let source_depth = index.scoped_endpoint_depth(&scoped.edge, &scoped.source)?;
        let target_depth = index.scoped_endpoint_depth(&scoped.edge, &scoped.target)?;
        let shape = index.scoped_segment_shape(
            source_placeholder,
            target_placeholder,
            &scoped.source,
            &scoped.target,
            source_depth,
            target_depth,
        )?;
        #[cfg(test)]
        {
            hierarchy_query_steps = checked_sum([hierarchy_query_steps, shape.probe_steps])?;
        }
        let segment_count = checked_sum([
            source_depth
                .checked_sub(shape.common_depth)
                .ok_or(WorkError::ArithmeticOverflow)?,
            target_depth
                .checked_sub(shape.common_depth)
                .ok_or(WorkError::ArithmeticOverflow)?,
            usize::from(!shape.source_is_target_ancestor && !shape.target_is_source_ancestor),
        ])?
        .max(1);
        checked_sum([
            scoped_segment_depth(scoped.segment),
            source_depth.max(target_depth),
        ])?;
        let common_path_probe = usize::from(shape.common_depth < source_depth.min(target_depth));
        let materialization_steps = checked_sum([
            source_depth,
            target_depth,
            shape.common_depth,
            common_path_probe,
        ])?;
        materialization_path_steps =
            checked_sum([materialization_path_steps, materialization_steps])?;
        materialized_segments = checked_sum([materialized_segments, segment_count])?;
        if scoped.edge.label.is_some() {
            label_selection_steps = checked_sum([label_selection_steps, segment_count])?;
        }
    }

    Ok(ScopedSegmentWorkPlan {
        materialization_path_steps,
        materialized_segments,
        label_selection_steps,
        #[cfg(test)]
        hierarchy_query_steps,
    })
}

fn materialize_scoped_edge_segments(
    index: &InputIndex<'_>,
    segments: &[ElkInputEdgeSegment],
    merge_edges: bool,
    work: ScopedSegmentWorkPlan,
) -> PreparedScopedSegments {
    let mut pending_segments = Vec::with_capacity(work.materialized_segments);

    for scoped in segments {
        let edge_order = scoped.edge_order;
        let source_port_key = hierarchy_port_key(
            scoped.edge.source.as_str(),
            edge_order,
            "source",
            merge_edges,
            PortType::Output,
        );
        let target_port_key = hierarchy_port_key(
            scoped.edge.target.as_str(),
            edge_order,
            "target",
            merge_edges,
            PortType::Input,
        );
        let source_placeholder = scoped_segment_endpoint_id(&scoped.source);
        let target_placeholder = scoped_segment_endpoint_id(&scoped.target);
        let source_path = match &scoped.source {
            ElkInputEdgeSegmentEndpoint::Node { id } => index.materialized_graph_path(id).expect(
                "scoped endpoint reachability was validated before materialization was charged",
            ),
            ElkInputEdgeSegmentEndpoint::ParentBoundary { .. } => Vec::new(),
        };
        let target_path = match &scoped.target {
            ElkInputEdgeSegmentEndpoint::Node { id } => index.materialized_graph_path(id).expect(
                "scoped endpoint reachability was validated before materialization was charged",
            ),
            ElkInputEdgeSegmentEndpoint::ParentBoundary { .. } => Vec::new(),
        };
        let mut local = source_ported_cross_hierarchy_segments(
            source_placeholder,
            target_placeholder,
            source_port_key.as_str(),
            target_port_key.as_str(),
            &source_path,
            &target_path,
        );
        if local.is_empty() {
            local.push(PendingCompoundSegment {
                graph_parent: None,
                source: PendingSegmentEndpoint::LocalNode {
                    node_id: source_placeholder.to_string(),
                    port_key: source_port_key.clone(),
                },
                target: PendingSegmentEndpoint::LocalNode {
                    node_id: target_placeholder.to_string(),
                    port_key: target_port_key.clone(),
                },
                segment: scoped.segment,
            });
        }

        for pending in &mut local {
            replace_scoped_boundary_endpoint(
                &mut pending.source,
                &scoped.source,
                true,
                source_port_key.as_str(),
                target_port_key.as_str(),
            );
            replace_scoped_boundary_endpoint(
                &mut pending.target,
                &scoped.target,
                false,
                source_port_key.as_str(),
                target_port_key.as_str(),
            );
            pending.segment = rebase_scoped_segment(pending.segment, scoped.segment);
        }

        let label_segment = scoped
            .edge
            .label
            .as_ref()
            .map(|label| compound_label_segment_index(&local, label.placement));
        for (segment_index, pending) in local.into_iter().enumerate() {
            let labels = scoped
                .edge
                .label
                .as_ref()
                .filter(|_| label_segment == Some(segment_index))
                .map(|label| {
                    let mut label = label_to_lgraph(label);
                    label.original_label_edge = Some(scoped.edge.id.clone());
                    vec![label]
                })
                .unwrap_or_default();
            pending_segments.push(ScopedHierarchySegment {
                edge: HierarchyEdge {
                    id: scoped.edge.id.clone(),
                    source_node_id: scoped.edge.source.clone(),
                    target_node_id: scoped.edge.target.clone(),
                    source_port_key: source_port_key.clone(),
                    target_port_key: target_port_key.clone(),
                    source_path: Vec::new(),
                    target_path: Vec::new(),
                    labels: Vec::new(),
                    minlen: scoped.edge.minlen.max(1),
                    model_order: scoped.edge.model_order,
                    priority_direction: scoped.edge.priority_direction,
                    priority_shortness: scoped.edge.priority_shortness,
                    priority_straightness: scoped.edge.priority_straightness,
                },
                pending,
                labels,
            });
        }
    }

    debug_assert_eq!(pending_segments.len(), work.materialized_segments);
    PreparedScopedSegments {
        segments: pending_segments,
    }
}

fn scoped_segment_depth(segment: CompoundEdgeSegment) -> usize {
    match segment {
        CompoundEdgeSegment::Output { depth } | CompoundEdgeSegment::Input { depth } => depth,
    }
}

fn rebase_scoped_segment(
    local: CompoundEdgeSegment,
    scoped: CompoundEdgeSegment,
) -> CompoundEdgeSegment {
    // `local` is relative to this import scope, but `scoped` already carries the caller's absolute
    // compound side and depth. Preserve that side while rebasing every generated local piece.
    let depth = scoped_segment_depth(scoped)
        .checked_add(scoped_segment_depth(local))
        .expect("scoped segment depth was checked before materialization was charged");
    match scoped {
        CompoundEdgeSegment::Output { .. } => CompoundEdgeSegment::Output { depth },
        CompoundEdgeSegment::Input { .. } => CompoundEdgeSegment::Input { depth },
    }
}

struct PreparedScopedSegments {
    segments: Vec<ScopedHierarchySegment>,
}

fn validate_scoped_segment_endpoint(
    index: &InputIndex<'_>,
    edge: &ElkInputEdge,
    endpoint: &ElkInputEdgeSegmentEndpoint,
) -> ImportResult<()> {
    let ElkInputEdgeSegmentEndpoint::Node { id } = endpoint else {
        return Ok(());
    };
    if index.nodes.contains_key(id.as_str()) {
        return Ok(());
    }
    Err(ImportError::MissingEndpoint {
        edge_id: edge.id.clone(),
        node_id: id.clone(),
    })
}

fn scoped_segment_endpoint_id(endpoint: &ElkInputEdgeSegmentEndpoint) -> &str {
    match endpoint {
        ElkInputEdgeSegmentEndpoint::Node { id }
        | ElkInputEdgeSegmentEndpoint::ParentBoundary { id, .. } => id.as_str(),
    }
}

fn replace_scoped_boundary_endpoint(
    pending: &mut PendingSegmentEndpoint,
    endpoint: &ElkInputEdgeSegmentEndpoint,
    source: bool,
    source_port_key: &str,
    target_port_key: &str,
) {
    let ElkInputEdgeSegmentEndpoint::ParentBoundary { id, connects_node } = endpoint else {
        return;
    };
    let PendingSegmentEndpoint::LocalNode { node_id, .. } = pending else {
        return;
    };
    if node_id != id {
        return;
    }

    let (port_type, parent_port_type, port_key) = if source {
        (
            PortType::Input,
            if *connects_node {
                PortType::Output
            } else {
                PortType::Input
            },
            if *connects_node {
                source_port_key
            } else {
                target_port_key
            },
        )
    } else {
        (
            PortType::Output,
            if *connects_node {
                PortType::Input
            } else {
                PortType::Output
            },
            if *connects_node {
                target_port_key
            } else {
                source_port_key
            },
        )
    };
    *pending = PendingSegmentEndpoint::ParentBoundary {
        node_id: id.clone(),
        port_key: port_key.to_string(),
        port_type,
        parent_port_type,
        connects_parent_node: *connects_node,
    };
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
                apply_inside_node_label_padding(
                    &mut nested_graph,
                    &parent_graph.layerless_nodes[node_index],
                );
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
    if let Some(label) = edge.label.as_ref() {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
        labels.push(label_to_lgraph(label));
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
        || matches!(
            options.layering_strategy,
            LayeringStrategy::BreadthFirstModelOrder | LayeringStrategy::DepthFirstModelOrder
        )
}

fn needs_model_order_based_on_input_parent(index: &InputIndex<'_>, child: &ElkInputNode) -> bool {
    let Some(parent_id) = child.parent.as_deref() else {
        return needs_model_order_based_on_parent(&index.root_options);
    };
    let Some(parent_options) = index.effective_options.get(parent_id) else {
        return false;
    };
    needs_model_order_based_on_parent(parent_options)
}

fn input_node_uses_include_children_layout(index: &InputIndex<'_>, node: &ElkInputNode) -> bool {
    index
        .effective_options
        .get(node.id.as_str())
        .is_some_and(|options| options.hierarchy_handling == HierarchyHandling::IncludeChildren)
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
    // Mirror Mermaid's selective subgraph option boundary rather than cloning the complete root
    // configuration. buildSubgraphLayoutOptions forwards mergeEdges and nodePlacementStrategy;
    // the former also keeps collector-port identity consistent across hierarchy boundaries.
    let mut options = LayeredOptions {
        random_seed: parent_options.random_seed,
        direction: parent_options.direction,
        hierarchy_handling: resolve_child_hierarchy_handling(
            node.hierarchy_handling,
            parent_options,
        ),
        port_constraints: node.port_constraints.unwrap_or(PortConstraints::Free),
        inside_self_loops_activate: parent_options.inside_self_loops_activate,
        merge_edges: parent_options.merge_edges,
        node_placement_strategy: parent_options.node_placement_strategy,
        ..LayeredOptions::default()
    };
    if let Some(spacing_base) = node.nested_spacing_base {
        options.spacing = SpacingOptions::layered_base_value(spacing_base);
    }
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

fn apply_inside_node_label_padding(graph: &mut LGraph, parent_node: &LNode) {
    let padding = compute_inside_node_label_padding(&graph.options, parent_node);
    graph.padding.top += padding.top;
    graph.padding.right += padding.right;
    graph.padding.bottom += padding.bottom;
    graph.padding.left += padding.left;
}

fn compute_inside_node_label_padding(options: &LayeredOptions, node: &LNode) -> ElkPadding {
    let mut cells = [LabelCellSize::default(); 9];
    for label in &node.labels {
        let Some((row, col)) = inside_node_label_cell(node.node_label_placement) else {
            continue;
        };
        cells[row * 3 + col].add_label(label, options.spacing.label_label);
    }

    let container_gap = 2.0 * options.spacing.label_label;
    let mut padding = ElkPadding {
        top: max_cell_height(&cells, [0, 1, 2]),
        right: max_cell_width(&cells, [2, 5, 8]),
        bottom: max_cell_height(&cells, [6, 7, 8]),
        left: max_cell_width(&cells, [0, 3, 6]),
    };
    if padding.top > 0.0 {
        padding.top += options.node_labels_padding.top + container_gap;
    }
    if padding.right > 0.0 {
        padding.right += options.node_labels_padding.right + container_gap;
    }
    if padding.bottom > 0.0 {
        padding.bottom += options.node_labels_padding.bottom + container_gap;
    }
    if padding.left > 0.0 {
        padding.left += options.node_labels_padding.left + container_gap;
    }
    padding
}

#[derive(Debug, Clone, Copy, Default)]
struct LabelCellSize {
    min_width: f64,
    min_height: f64,
    label_count: usize,
}

impl LabelCellSize {
    fn add_label(&mut self, label: &LLabel, label_gap: f64) {
        self.min_width = self.min_width.max(label.size.width);
        if self.label_count > 0 {
            self.min_height += label_gap;
        }
        self.min_height += label.size.height;
        self.label_count += 1;
    }
}

fn max_cell_height(cells: &[LabelCellSize; 9], indices: [usize; 3]) -> f64 {
    indices
        .into_iter()
        .map(|index| cells[index].min_height)
        .fold(0.0, f64::max)
}

fn max_cell_width(cells: &[LabelCellSize; 9], indices: [usize; 3]) -> f64 {
    indices
        .into_iter()
        .map(|index| cells[index].min_width)
        .fold(0.0, f64::max)
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
    if let Some(layer_constraint) = node.layer_constraint {
        lnode.layer_constraint = layer_constraint;
        lnode.layer_constraint_explicit = true;
    }
    if let Some(label) = node.label.as_ref() {
        lnode.labels.push(label_to_lgraph(label));
    }
    lnode.node_label_placement = node.node_label_placement;
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
    transform_edge_between(
        edge,
        graph,
        graph_parent,
        model_order,
        edge.source.as_str(),
        edge.target.as_str(),
        index.child_position(edge.source.as_str()),
        index.child_position(edge.target.as_str()),
        edge.source.as_str(),
        edge.target.as_str(),
        None,
        edge.label.as_ref(),
        port_index,
    )
}

#[allow(clippy::too_many_arguments)]
fn transform_edge_between(
    edge: &ElkInputEdge,
    graph: &mut LGraph,
    graph_parent: Option<&str>,
    model_order: Option<usize>,
    local_source: &str,
    local_target: &str,
    local_source_index: usize,
    local_target_index: usize,
    source_node_id: &str,
    target_node_id: &str,
    compound_segment: Option<CompoundEdgeSegment>,
    label: Option<&ElkInputLabel>,
    port_index: &mut ImportPortIndex<'_>,
) -> ImportResult<usize> {
    let source = ensure_port_at_node(
        graph,
        graph_parent,
        local_source_index,
        local_source,
        PortType::Output,
        port_index,
    )?
    .ok_or_else(|| ImportError::MissingEndpoint {
        edge_id: edge.id.clone(),
        node_id: local_source.to_string(),
    })?;
    let target = ensure_port_at_node(
        graph,
        graph_parent,
        local_target_index,
        local_target,
        PortType::Input,
        port_index,
    )?
    .ok_or_else(|| ImportError::MissingEndpoint {
        edge_id: edge.id.clone(),
        node_id: local_target.to_string(),
    })?;

    if source.node == target.node {
        graph.graph_properties.self_loops = true;
    }

    let mut labels = Vec::new();
    if let Some(label) = label {
        match label.placement {
            EdgeLabelPlacement::Center => graph.graph_properties.center_labels = true,
            EdgeLabelPlacement::Head | EdgeLabelPlacement::Tail => {
                graph.graph_properties.end_labels = true;
            }
        }
        labels.push(label_to_lgraph(label));
    }

    let edge_index = graph
        .add_edge(LayeredEdge {
            id: edge.id.clone(),
            source,
            target,
            source_node_id: source_node_id.to_string(),
            target_node_id: target_node_id.to_string(),
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
            compound_segment,
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
        labels: edge.label.iter().map(label_to_lgraph).collect(),
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
    graph.layerless_nodes[node].ports.push(LPort::new(
        format!("{node_id}:{port:?}"),
        node,
        port_type,
    ));
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

fn label_to_lgraph(label: &ElkInputLabel) -> LLabel {
    let mut llabel = LLabel::new(label.text.clone(), label.width, label.height);
    llabel.placement = label.placement;
    llabel.inline = label.inline;
    llabel.label_side = None;
    llabel
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

struct HierarchyQueryIndex {
    parents: Vec<Option<usize>>,
    depths: Vec<usize>,
    roots: Vec<usize>,
    chain_heads: Vec<usize>,
    chain_positions: Vec<usize>,
    nodes_by_chain_position: Vec<usize>,
    preorder_positions: Vec<usize>,
    subtree_ends: Vec<usize>,
}

impl HierarchyQueryIndex {
    fn new(
        node_order: &HashMap<&str, usize>,
        root_children: &[&ElkInputNode],
        children_by_parent: &HashMap<&str, Vec<&ElkInputNode>>,
    ) -> ImportResult<Self> {
        let mut parents = vec![None; node_order.len()];
        let mut children = vec![Vec::new(); node_order.len()];
        for (parent, input_children) in children_by_parent {
            let parent = node_order[*parent];
            for child in input_children {
                let child_index = node_order[child.id.as_str()];
                parents[child_index] = Some(parent);
                children[parent].push(child_index);
            }
        }

        let mut depths = vec![0usize; node_order.len()];
        let mut roots = vec![0usize; node_order.len()];
        let mut preorder_positions = vec![0usize; node_order.len()];
        let mut preorder = Vec::with_capacity(node_order.len());
        let mut stack = root_children
            .iter()
            .rev()
            .map(|node| {
                let node = node_order[node.id.as_str()];
                (node, node)
            })
            .collect::<Vec<_>>();
        while let Some((node, root)) = stack.pop() {
            roots[node] = root;
            preorder_positions[node] = preorder.len();
            preorder.push(node);
            let child_depth = checked_sum([depths[node], 1])?;
            for &child in children[node].iter().rev() {
                depths[child] = child_depth;
                stack.push((child, root));
            }
        }

        let mut subtree_sizes = vec![1usize; node_order.len()];
        let mut heavy_children = vec![None; node_order.len()];
        for &node in preorder.iter().rev() {
            let mut largest = 0usize;
            for &child in &children[node] {
                subtree_sizes[node] = checked_sum([subtree_sizes[node], subtree_sizes[child]])?;
                if subtree_sizes[child] > largest {
                    largest = subtree_sizes[child];
                    heavy_children[node] = Some(child);
                }
            }
        }
        let mut subtree_ends = vec![0usize; node_order.len()];
        for node in 0..node_order.len() {
            subtree_ends[node] = checked_sum([preorder_positions[node], subtree_sizes[node]])?;
        }

        let mut chain_heads = vec![0usize; node_order.len()];
        let mut chain_positions = vec![0usize; node_order.len()];
        let mut nodes_by_chain_position = Vec::with_capacity(node_order.len());
        let mut chains = root_children
            .iter()
            .rev()
            .map(|node| {
                let node = node_order[node.id.as_str()];
                (node, node)
            })
            .collect::<Vec<_>>();
        while let Some((start, head)) = chains.pop() {
            let mut current = Some(start);
            while let Some(node) = current {
                chain_heads[node] = head;
                chain_positions[node] = nodes_by_chain_position.len();
                nodes_by_chain_position.push(node);
                for &child in children[node].iter().rev() {
                    if Some(child) != heavy_children[node] {
                        chains.push((child, child));
                    }
                }
                current = heavy_children[node];
            }
        }

        Ok(Self {
            parents,
            depths,
            roots,
            chain_heads,
            chain_positions,
            nodes_by_chain_position,
            preorder_positions,
            subtree_ends,
        })
    }

    fn depth(&self, node: usize) -> usize {
        self.depths[node]
    }

    fn lift(&self, mut node: usize, mut steps: usize) -> ImportResult<(Option<usize>, usize)> {
        let mut probes = 0usize;
        while steps > 0 {
            probes = checked_sum([probes, 1])?;
            let head = self.chain_heads[node];
            let distance = self
                .depth(node)
                .checked_sub(self.depth(head))
                .ok_or(WorkError::ArithmeticOverflow)?;
            if steps <= distance {
                let position = self.chain_positions[node]
                    .checked_sub(steps)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                return Ok((Some(self.nodes_by_chain_position[position]), probes));
            }
            steps = steps
                .checked_sub(checked_sum([distance, 1])?)
                .ok_or(WorkError::ArithmeticOverflow)?;
            let Some(parent) = self.parents[head] else {
                return Ok((None, probes));
            };
            node = parent;
        }
        Ok((Some(node), probes))
    }

    fn is_ancestor(&self, ancestor: usize, node: usize) -> ImportResult<(bool, usize)> {
        let position = self.preorder_positions[node];
        Ok((
            self.preorder_positions[ancestor] <= position && position < self.subtree_ends[ancestor],
            1,
        ))
    }

    fn lca(&self, left: usize, right: usize) -> ImportResult<(Option<usize>, usize)> {
        if self.roots[left] != self.roots[right] {
            return Ok((None, 1));
        }
        let mut left = left;
        let mut right = right;
        let mut probes = 0usize;
        while self.chain_heads[left] != self.chain_heads[right] {
            probes = checked_sum([probes, 1])?;
            let left_head = self.chain_heads[left];
            let right_head = self.chain_heads[right];
            if self.depth(left_head) > self.depth(right_head) {
                left = self.parents[left_head]
                    .expect("same-root heavy-light query has a parent above the deeper chain");
            } else {
                right = self.parents[right_head]
                    .expect("same-root heavy-light query has a parent above the deeper chain");
            }
        }
        probes = checked_sum([probes, 1])?;
        let common = if self.depth(left) <= self.depth(right) {
            left
        } else {
            right
        };
        Ok((Some(common), probes))
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
    effective_options: HashMap<&'a str, LayeredOptions>,
    hierarchy_queries: HierarchyQueryIndex,
    container_depths: Vec<Option<usize>>,
    materialized_graph_depths: Vec<Option<usize>>,
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
        let mut effective_options = HashMap::with_capacity(input.nodes.len());
        let mut option_search = root_children
            .iter()
            .rev()
            .copied()
            .map(|node| (node, root_options.clone()))
            .collect::<Vec<_>>();
        while let Some((node, parent_options)) = option_search.pop() {
            let options = nested_graph_options(&parent_options, node);
            effective_options.insert(node.id.as_str(), options.clone());
            option_search.extend(
                children_by_parent
                    .get(node.id.as_str())
                    .into_iter()
                    .flat_map(|children| children.iter().rev().copied())
                    .map(|child| (child, options.clone())),
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
            effective_options,
            hierarchy_queries,
            container_depths: vec![None; input.nodes.len()],
            materialized_graph_depths: vec![None; input.nodes.len()],
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
            .and_then(|parent| self.effective_options.get(parent))
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

    fn scoped_endpoint_depth(
        &self,
        edge: &ElkInputEdge,
        endpoint: &ElkInputEdgeSegmentEndpoint,
    ) -> ImportResult<usize> {
        let ElkInputEdgeSegmentEndpoint::Node { id } = endpoint else {
            return Ok(0);
        };
        self.container_depth(id).ok_or_else(|| {
            let graph_parent = self
                .first_unmaterialized_graph_parent(id)
                .unwrap_or(id.as_str());
            ImportError::UnavailableSegmentScope {
                edge_id: edge.id.clone(),
                graph_parent: graph_parent.to_string(),
            }
        })
    }

    fn first_unmaterialized_graph_parent(&self, node: &str) -> Option<&'a str> {
        self.node_index(node)
            .and_then(|node| self.first_unmaterialized_graph_parents[node])
    }

    fn scoped_segment_shape(
        &self,
        source: &str,
        target: &str,
        source_endpoint: &ElkInputEdgeSegmentEndpoint,
        target_endpoint: &ElkInputEdgeSegmentEndpoint,
        source_depth: usize,
        target_depth: usize,
    ) -> ImportResult<ScopedSegmentShape> {
        let source_owner = self.scoped_endpoint_owner_index(source_endpoint);
        let target_owner = self.scoped_endpoint_owner_index(target_endpoint);
        let (common_owner, common_probe_steps) = match (source_owner, target_owner) {
            (Some(source_owner), Some(target_owner)) => {
                self.hierarchy_queries.lca(source_owner, target_owner)?
            }
            _ => (None, 0),
        };
        let common_depth = match common_owner {
            Some(owner) => checked_sum([self.hierarchy_queries.depth(owner), 1])?,
            None => 0,
        };
        let (source_branch, source_probe_steps) =
            self.scoped_branch_owner(source_endpoint, source_depth, common_depth)?;
        let (target_branch, target_probe_steps) =
            self.scoped_branch_owner(target_endpoint, target_depth, common_depth)?;
        #[cfg(test)]
        let probe_steps =
            checked_sum([common_probe_steps, source_probe_steps, target_probe_steps])?;
        #[cfg(not(test))]
        let _ = (common_probe_steps, source_probe_steps, target_probe_steps);

        Ok(ScopedSegmentShape {
            common_depth,
            source_is_target_ancestor: target_depth > common_depth && target_branch == Some(source),
            target_is_source_ancestor: source_depth > common_depth && source_branch == Some(target),
            #[cfg(test)]
            probe_steps,
        })
    }

    fn scoped_endpoint_owner_index(&self, endpoint: &ElkInputEdgeSegmentEndpoint) -> Option<usize> {
        match endpoint {
            ElkInputEdgeSegmentEndpoint::Node { id } => self
                .node_parent(id)
                .and_then(|parent| self.node_index(parent)),
            ElkInputEdgeSegmentEndpoint::ParentBoundary { .. } => None,
        }
    }

    fn scoped_branch_owner(
        &self,
        endpoint: &ElkInputEdgeSegmentEndpoint,
        depth: usize,
        common_depth: usize,
    ) -> ImportResult<(Option<&'a str>, usize)> {
        if depth <= common_depth {
            return Ok((None, 0));
        }
        let owner = self
            .scoped_endpoint_owner_index(endpoint)
            .expect("positive scoped depth has an indexed graph owner");
        let steps = depth
            .checked_sub(common_depth)
            .and_then(|steps| steps.checked_sub(1))
            .ok_or(WorkError::ArithmeticOverflow)?;
        let (owner, probes) = self.hierarchy_queries.lift(owner, steps)?;
        Ok((owner.map(|owner| self.node_id(owner)), probes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScopedSegmentShape {
    common_depth: usize,
    source_is_target_ancestor: bool,
    target_is_source_ancestor: bool,
    #[cfg(test)]
    probe_steps: usize,
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
mod tests {
    use super::*;
    use crate::compound::preprocess_source_ported_compound_graph;
    use crate::graph::LNodeKind;
    use crate::options::OrderingStrategy;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: NodeLabelPlacement::Fixed,
            nested_spacing_base: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
        ElkInputEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
            inside_self_loops_yo: false,
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> ElkInputGraph {
        ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        }
    }

    #[derive(Debug)]
    struct RecordingWorkControl {
        remaining: usize,
        checked: usize,
        charged: usize,
        check_calls: usize,
        first_check: Option<usize>,
    }

    impl RecordingWorkControl {
        fn unlimited() -> Self {
            Self {
                remaining: usize::MAX,
                checked: 0,
                charged: 0,
                check_calls: 0,
                first_check: None,
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn check(&mut self, units: usize) -> Result<(), WorkError> {
            self.check_calls = checked_sum([self.check_calls, 1])?;
            self.first_check.get_or_insert(units);
            self.checked = checked_sum([self.checked, units])?;
            if units > self.remaining {
                return Err(WorkError::Interrupted);
            }
            Ok(())
        }

        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            if units > self.remaining {
                return Err(WorkError::Interrupted);
            }
            self.remaining -= units;
            self.charged = checked_sum([self.charged, units])?;
            Ok(())
        }
    }

    fn measured_import_work(input: &ElkInputGraph) -> usize {
        let mut work_control = RecordingWorkControl::unlimited();
        import_graph_with_work_control(input, &mut work_control).unwrap();
        assert_eq!(work_control.check_calls, 2);
        assert_eq!(work_control.checked, work_control.charged);
        work_control.charged
    }

    fn measured_scoped_import_work(
        input: &ElkInputGraph,
        segments: &[ElkInputEdgeSegment],
    ) -> usize {
        let mut work_control = RecordingWorkControl::unlimited();
        import_graph_at_scope_and_segments_with_work_control(
            input,
            &["root"],
            segments,
            &mut work_control,
        )
        .unwrap();
        assert_eq!(work_control.check_calls, 2);
        assert_eq!(work_control.checked, work_control.charged);
        work_control.charged
    }

    #[test]
    fn parent_cycle_reports_the_revisited_cycle_member_after_a_stem() {
        let mut stem = node("stem");
        stem.parent = Some("A".to_string());
        let mut a = node("A");
        a.parent = Some("B".to_string());
        let mut b = node("B");
        b.parent = Some("A".to_string());
        let input = graph(vec![stem, a, b], vec![]);
        let node_order = input
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id.as_str(), index))
            .collect();

        assert!(matches!(
            detect_parent_cycles(&input, &node_order),
            Err(ImportError::ParentCycle { node_id }) if node_id == "A"
        ));
    }

    fn measured_hierarchical_import(
        input: &ElkInputGraph,
    ) -> (LGraph, usize, ImportHierarchyProbe) {
        let mut work_control = RecordingWorkControl::unlimited();
        let mut port_probe = ImportPortProbe::default();
        let mut hierarchy_probe = ImportHierarchyProbe::default();
        let graph = import_graph_with_random_seed_authority_and_probe(
            input,
            RandomSeedAuthority::require_explicit(),
            GraphSeedScope::root("root"),
            &[],
            &mut work_control,
            &mut port_probe,
            &mut hierarchy_probe,
        )
        .unwrap();
        assert_eq!(work_control.check_calls, 2);
        assert_eq!(work_control.checked, work_control.charged);
        (graph, work_control.charged, hierarchy_probe)
    }

    #[test]
    fn import_preflight_rejects_before_input_index_validation_or_allocation() {
        let input = graph(vec![node("duplicate"), node("duplicate")], Vec::new());
        let mut work_control = RecordingWorkControl {
            remaining: 1,
            checked: 0,
            charged: 0,
            check_calls: 0,
            first_check: None,
        };

        let error = import_graph_with_work_control(&input, &mut work_control).unwrap_err();

        assert_eq!(error, ImportError::Work(WorkError::Interrupted));
        assert_eq!(work_control.first_check, Some(2));
        assert_eq!(work_control.check_calls, 1);
        assert_eq!(work_control.charged, 0);
    }

    #[test]
    fn import_rejects_duplicate_input_edge_ids_before_endpoint_validation() {
        let input = graph(
            vec![node("A"), node("B")],
            vec![
                edge("duplicate", "A", "B"),
                edge("duplicate", "missing", "B"),
            ],
        );
        let mut work_control = NoopWorkControl;

        let error = import_graph_with_work_control(&input, &mut work_control).unwrap_err();

        assert_eq!(
            error,
            ImportError::DuplicateEdge {
                id: "duplicate".to_string(),
            }
        );
    }

    #[test]
    fn flat_import_planning_bound_is_linear_in_independent_node_and_edge_growth() {
        for node_count in [1usize, 8, 32, 128] {
            let nodes = (0..node_count)
                .map(|index| node(format!("node-{index}").as_str()))
                .collect::<Vec<_>>();
            assert_eq!(measured_import_work(&graph(nodes, Vec::new())), node_count);
        }

        for edge_count in [0usize, 8, 32, 128] {
            let edges = (0..edge_count)
                .map(|index| edge(format!("edge-{index}").as_str(), "A", "B"))
                .collect::<Vec<_>>();
            assert_eq!(
                measured_import_work(&graph(vec![node("A"), node("B")], edges)),
                2 + 3 * edge_count
            );
        }
    }

    #[test]
    fn hierarchical_import_uses_linear_graph_slots_for_deep_chains() {
        for node_count in [1usize, 4, 16, 64, 256] {
            let mut nodes = Vec::with_capacity(node_count);
            for index in 0..node_count {
                let mut current = node(format!("node-{index}").as_str());
                current.parent = index.checked_sub(1).map(|parent| format!("node-{parent}"));
                current.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
                nodes.push(current);
            }
            let mut input = graph(nodes, Vec::new());
            input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
            let (imported, work, probe) = measured_hierarchical_import(&input);

            assert_eq!(work, node_count);
            assert_eq!(probe.node_imports, node_count);
            assert_eq!(probe.edge_imports, 0);
            assert_eq!(probe.nested_graphs, node_count.saturating_sub(1));
            assert_eq!(probe.output_path_components, 0);

            let mut graph = &imported;
            for index in 0..node_count {
                assert_eq!(graph.layerless_nodes.len(), 1);
                assert_eq!(graph.layerless_nodes[0].id, format!("node-{index}"));
                if index + 1 < node_count {
                    graph = graph.layerless_nodes[0]
                        .nested_graph
                        .as_deref()
                        .expect("every non-leaf chain node owns the next graph slot");
                } else {
                    assert!(graph.layerless_nodes[0].nested_graph.is_none());
                }
            }
        }
    }

    #[test]
    fn hierarchical_import_charges_only_retained_cross_edge_paths_by_depth() {
        for node_count in [2usize, 8, 32] {
            for edge_count in [1usize, 4, 16] {
                let mut nodes = Vec::with_capacity(node_count);
                for index in 0..node_count {
                    let mut current = node(format!("node-{index}").as_str());
                    current.parent = index.checked_sub(1).map(|parent| format!("node-{parent}"));
                    current.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
                    nodes.push(current);
                }
                let edges = (0..edge_count)
                    .map(|index| {
                        edge(
                            format!("edge-{index}").as_str(),
                            "node-0",
                            format!("node-{}", node_count - 1).as_str(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut input = graph(nodes, edges);
                input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
                let retained_path_components = edge_count * (node_count - 1);
                // One descriptor plus a two-probe owner-query ceiling per edge, then the path
                // components that ELK's compound preprocessor must consume from the output.
                let expected_work = node_count + 3 * edge_count + retained_path_components;

                let (imported, work, probe) = measured_hierarchical_import(&input);

                assert_eq!(work, expected_work);
                assert_eq!(probe.node_imports, node_count);
                assert_eq!(probe.edge_imports, edge_count);
                assert_eq!(probe.nested_graphs, node_count - 1);
                assert_eq!(probe.output_path_components, retained_path_components);
                assert_eq!(imported.hierarchy_edges.len(), edge_count);
                assert!(imported.hierarchy_edges.iter().all(|edge| {
                    edge.source_path.is_empty() && edge.target_path.len() == node_count - 1
                }));
                assert_eq!(
                    imported
                        .hierarchy_edges
                        .iter()
                        .map(|edge| edge.id.clone())
                        .collect::<Vec<_>>(),
                    (0..edge_count)
                        .map(|index| format!("edge-{index}"))
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn edge_owner_queries_do_not_scale_with_hierarchy_depth() {
        for node_count in [8usize, 64, 512] {
            let mut nodes = Vec::with_capacity(node_count);
            for index in 0..node_count {
                let mut current = node(format!("node-{index}").as_str());
                current.parent = index.checked_sub(1).map(|parent| format!("node-{parent}"));
                nodes.push(current);
            }
            let input = graph(
                nodes,
                vec![edge(
                    "root-leaf",
                    "node-0",
                    format!("node-{}", node_count - 1).as_str(),
                )],
            );

            let index = InputIndex::new(&input).unwrap();

            assert_eq!(index.edge_owner_query_steps, 1);
        }
    }

    #[test]
    fn hierarchy_query_index_resolves_light_branches_and_separate_forests() {
        let mut left = node("left");
        left.parent = Some("root-a".to_string());
        let mut left_leaf = node("left-leaf");
        left_leaf.parent = Some("left".to_string());
        let mut right = node("right");
        right.parent = Some("root-a".to_string());
        let mut right_leaf = node("right-leaf");
        right_leaf.parent = Some("right".to_string());
        let input = graph(
            vec![
                node("root-a"),
                left,
                left_leaf,
                right,
                right_leaf,
                node("root-b"),
            ],
            Vec::new(),
        );
        let index = InputIndex::new(&input).unwrap();
        let root_a = index.node_index("root-a").unwrap();
        let left_leaf = index.node_index("left-leaf").unwrap();
        let right_leaf = index.node_index("right-leaf").unwrap();
        let root_b = index.node_index("root-b").unwrap();

        assert_eq!(
            index.hierarchy_queries.lca(left_leaf, right_leaf).unwrap(),
            (Some(root_a), 2)
        );
        assert_eq!(
            index.hierarchy_queries.lca(left_leaf, root_b).unwrap(),
            (None, 1)
        );
    }

    #[test]
    fn scoped_hierarchy_query_observations_fit_the_precharged_hld_bound() {
        let make_child = |id: &str, parent: &str, has_children: bool| {
            let mut child = node(id);
            child.parent = Some(parent.to_string());
            child.hierarchy_handling = has_children.then_some(HierarchyHandling::IncludeChildren);
            child
        };
        let mut root_a = node("root-a");
        root_a.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut root_b = node("root-b");
        root_b.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut input = graph(
            vec![
                root_a,
                make_child("left", "root-a", true),
                make_child("left-mid", "left", true),
                make_child("left-leaf", "left-mid", false),
                make_child("right", "root-a", true),
                make_child("right-mid", "right", true),
                make_child("right-leaf", "right-mid", false),
                root_b,
                make_child("other-mid", "root-b", true),
                make_child("other-leaf", "other-mid", false),
            ],
            Vec::new(),
        );
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let scoped =
            |id: &str, source: &str, target: &str, edge_order: usize| ElkInputEdgeSegment {
                edge: edge(id, source, target),
                source: ElkInputEdgeSegmentEndpoint::Node {
                    id: source.to_string(),
                },
                target: ElkInputEdgeSegmentEndpoint::Node {
                    id: target.to_string(),
                },
                segment: CompoundEdgeSegment::Output { depth: 0 },
                edge_order,
            };
        let segments = vec![
            scoped("across-light-branches", "left-leaf", "right-leaf", 0),
            scoped("across-forest", "left-leaf", "other-leaf", 1),
        ];

        let index = InputIndex::new(&input).unwrap();
        let observed = plan_scoped_edge_segments(&index, &segments)
            .unwrap()
            .hierarchy_query_steps;
        let bound = ImportPlanningWorkPlan::new(&input, &segments)
            .unwrap()
            .scoped_hierarchy_query_bound;

        assert!(observed > segments.len());
        assert!(observed <= bound, "observed {observed}, bound {bound}");
    }

    #[test]
    fn mixed_separate_subtrees_do_not_add_parent_locator_work() {
        for unreachable_depth in [1usize, 8, 32, 128] {
            let mut materialized = node("materialized");
            materialized.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
            let mut separate = node("separate");
            separate.parent = Some("materialized".to_string());
            separate.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
            let mut nodes = vec![materialized, separate];
            for depth in 0..unreachable_depth {
                let mut descendant = node(format!("unreachable-{depth}").as_str());
                descendant.parent = Some(if depth == 0 {
                    "separate".to_string()
                } else {
                    format!("unreachable-{}", depth - 1)
                });
                nodes.push(descendant);
            }
            let node_count = nodes.len();
            let mut input = graph(nodes, Vec::new());
            input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;

            let (_, work, probe) = measured_hierarchical_import(&input);
            assert_eq!(work, node_count);
            assert_eq!(probe.node_imports, 2);
            assert_eq!(probe.nested_graphs, 1);
            assert_eq!(probe.output_path_components, 0);
        }
    }

    #[test]
    fn separate_children_self_loop_graph_does_not_materialize_regular_children() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let loop_edge = ElkInputEdge {
            inside_self_loops_yo: true,
            ..edge("group-loop", "group", "group")
        };
        let mut input = graph(vec![group, child], vec![loop_edge]);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        input.options.inside_self_loops_activate = true;

        let (imported, work, probe) = measured_hierarchical_import(&input);
        assert_eq!(work, 5);
        assert_eq!(probe.node_imports, 1);
        assert_eq!(probe.edge_imports, 1);
        assert_eq!(probe.nested_graphs, 1);
        assert_eq!(probe.output_path_components, 0);
        let nested = imported.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("inside self-loop creates a nested graph");
        assert!(nested.layerless_nodes.iter().all(|node| node.id != "child"));
    }

    #[test]
    fn scoped_segment_planning_bound_is_linear_in_segments_at_fixed_node_count() {
        for node_count in [8usize, 32] {
            let mut nodes = vec![node("A"), node("B")];
            nodes.extend((2..node_count).map(|index| node(format!("node-{index}").as_str())));
            let input = graph(nodes, Vec::new());
            for segment_count in [8usize, 32] {
                let segments = (0..segment_count)
                    .map(|index| ElkInputEdgeSegment {
                        edge: edge(format!("edge-{index}").as_str(), "A", "B"),
                        source: ElkInputEdgeSegmentEndpoint::Node {
                            id: "A".to_string(),
                        },
                        target: ElkInputEdgeSegmentEndpoint::Node {
                            id: "B".to_string(),
                        },
                        segment: CompoundEdgeSegment::Output { depth: 0 },
                        edge_order: index,
                    })
                    .collect::<Vec<_>>();

                assert_eq!(
                    measured_scoped_import_work(&input, &segments),
                    node_count + segment_count * (2 + 4 * (ceil_log2(node_count.max(1)) + 1))
                );
            }
        }
    }

    #[test]
    fn scoped_pieces_may_reference_the_same_logical_edge_id() {
        let input = graph(vec![node("A"), node("B")], Vec::new());
        let segment = |edge_order| ElkInputEdgeSegment {
            edge: edge("logical-edge", "A", "B"),
            source: ElkInputEdgeSegmentEndpoint::Node {
                id: "A".to_string(),
            },
            target: ElkInputEdgeSegmentEndpoint::Node {
                id: "B".to_string(),
            },
            segment: CompoundEdgeSegment::Output { depth: 0 },
            edge_order,
        };
        let mut work_control = NoopWorkControl;

        let imported = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[segment(0), segment(1)],
            &mut work_control,
        )
        .unwrap();

        assert_eq!(imported.edges.len(), 2);
        assert_eq!(imported.cross_hierarchy_edges.len(), 2);
        assert!(
            imported
                .cross_hierarchy_edges
                .iter()
                .all(|segment| segment.original_edge_id == "logical-edge")
        );
    }

    #[test]
    fn merged_cross_edge_ports_use_constant_operation_local_probes() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let edge_count = 32usize;
        let edges = (0..edge_count)
            .map(|index| edge(format!("edge-{index}").as_str(), "child", "outer"))
            .collect::<Vec<_>>();
        let mut input = graph(vec![group, child, node("outer")], edges);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        input.options.merge_edges = true;
        let mut work_control = NoopWorkControl;
        let mut port_probe = ImportPortProbe::default();
        let mut hierarchy_probe = ImportHierarchyProbe::default();

        import_graph_with_random_seed_authority_and_probe(
            &input,
            RandomSeedAuthority::require_explicit(),
            GraphSeedScope::root("root"),
            &[],
            &mut work_control,
            &mut port_probe,
            &mut hierarchy_probe,
        )
        .unwrap();

        assert_eq!(port_probe.lookups, edge_count * 2);
        assert_eq!(port_probe.creations, 2);
        assert_eq!(port_probe.hits, edge_count * 2 - 2);
    }

    #[test]
    fn dedicated_cross_edge_ports_do_not_populate_the_reuse_index() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let edge_count = 32usize;
        let edges = (0..edge_count)
            .map(|index| edge(format!("edge-{index}").as_str(), "child", "outer"))
            .collect::<Vec<_>>();
        let mut input = graph(vec![group, child, node("outer")], edges);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        input.options.merge_edges = false;
        let mut work_control = NoopWorkControl;
        let mut port_probe = ImportPortProbe::default();
        let mut hierarchy_probe = ImportHierarchyProbe::default();

        let imported = import_graph_with_random_seed_authority_and_probe(
            &input,
            RandomSeedAuthority::require_explicit(),
            GraphSeedScope::root("root"),
            &[],
            &mut work_control,
            &mut port_probe,
            &mut hierarchy_probe,
        )
        .unwrap();

        assert_eq!(port_probe.lookups, 0);
        assert_eq!(port_probe.creations, 0);
        assert_eq!(port_probe.hits, 0);
        assert_eq!(imported.hierarchy_edges.len(), edge_count);
    }

    #[test]
    fn inside_self_loop_dummies_use_constant_operation_local_probes() {
        let edge_count = 32usize;
        let edges = (0..edge_count)
            .map(|index| ElkInputEdge {
                inside_self_loops_yo: true,
                ..edge(format!("loop-{index}").as_str(), "A", "A")
            })
            .collect::<Vec<_>>();
        let mut input = graph(vec![node("A")], edges);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        input.options.inside_self_loops_activate = true;
        let mut work_control = NoopWorkControl;
        let mut port_probe = ImportPortProbe::default();
        let mut hierarchy_probe = ImportHierarchyProbe::default();

        import_graph_with_random_seed_authority_and_probe(
            &input,
            RandomSeedAuthority::require_explicit(),
            GraphSeedScope::root("root"),
            &[],
            &mut work_control,
            &mut port_probe,
            &mut hierarchy_probe,
        )
        .unwrap();

        assert_eq!(port_probe.lookups, edge_count * 2);
        assert_eq!(port_probe.creations, 2);
        assert_eq!(port_probe.hits, edge_count * 2 - 2);
    }

    #[test]
    fn scoped_segment_work_charges_planning_before_output_materialization() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let mut input = graph(vec![group, child, node("outer")], Vec::new());
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let segment = ElkInputEdgeSegment {
            edge: edge("child-outer", "child", "outer"),
            source: ElkInputEdgeSegmentEndpoint::Node {
                id: "child".to_string(),
            },
            target: ElkInputEdgeSegmentEndpoint::Node {
                id: "outer".to_string(),
            },
            segment: CompoundEdgeSegment::Output { depth: 0 },
            edge_order: 0,
        };

        let mut unlimited = RecordingWorkControl::unlimited();
        let imported = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            std::slice::from_ref(&segment),
            &mut unlimited,
        )
        .unwrap();
        let required = unlimited.charged;
        let planning = ImportPlanningWorkPlan::new(&input, std::slice::from_ref(&segment))
            .unwrap()
            .total()
            .unwrap();
        let materialization = required - planning;
        assert_eq!(unlimited.first_check, Some(planning));
        assert_eq!(unlimited.checked, required);
        // Planning covers four descriptors plus the heavy-light query ceiling. Output work then
        // owns one retained path hop and two concrete hierarchy-local segments.
        assert_eq!(planning, 16);
        assert_eq!(materialization, 3);
        assert_eq!(imported.cross_hierarchy_edges.len(), 1);
        let nested = imported.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("group graph is materialized");
        assert_eq!(nested.cross_hierarchy_edges.len(), 1);

        let mut below_planning = RecordingWorkControl {
            remaining: planning - 1,
            checked: 0,
            charged: 0,
            check_calls: 0,
            first_check: None,
        };
        let error = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            std::slice::from_ref(&segment),
            &mut below_planning,
        )
        .unwrap_err();
        assert_eq!(error, ImportError::Work(WorkError::Interrupted));
        assert_eq!(below_planning.remaining, planning - 1);
        assert_eq!(below_planning.charged, 0);

        let mut below_materialization = RecordingWorkControl {
            remaining: required - 1,
            checked: 0,
            charged: 0,
            check_calls: 0,
            first_check: None,
        };
        let error = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            std::slice::from_ref(&segment),
            &mut below_materialization,
        )
        .unwrap_err();
        assert_eq!(error, ImportError::Work(WorkError::Interrupted));
        assert_eq!(below_materialization.charged, planning);
        assert_eq!(below_materialization.remaining, materialization - 1);

        for extra in [0usize, 1] {
            let mut accepted = RecordingWorkControl {
                remaining: required + extra,
                checked: 0,
                charged: 0,
                check_calls: 0,
                first_check: None,
            };
            import_graph_at_scope_and_segments_with_work_control(
                &input,
                &["root"],
                std::slice::from_ref(&segment),
                &mut accepted,
            )
            .unwrap();
            assert_eq!(accepted.checked, required);
            assert_eq!(accepted.charged, required);
            assert_eq!(accepted.remaining, extra);
        }
    }

    #[test]
    fn imports_mermaid_flowchart_nodes_edges_labels_and_model_order() {
        let mut a = node("A");
        a.label = Some(ElkInputLabel::center("Alpha", 42.0, 18.0));
        let mut ab = edge("A-B", "A", "B");
        ab.label = Some(ElkInputLabel::center("go", 20.0, 12.0));

        let lgraph = import_graph(&graph(vec![a, node("B")], vec![ab])).unwrap();

        assert_eq!(lgraph.layerless_nodes.len(), 2);
        assert_eq!(lgraph.layerless_nodes[0].id, "A");
        assert_eq!(lgraph.layerless_nodes[0].model_order, Some(0));
        assert_eq!(lgraph.layerless_nodes[0].labels[0].text, "Alpha");
        assert_eq!(lgraph.edges.len(), 1);
        assert_eq!(lgraph.edges[0].model_order, Some(0));
        assert_eq!(lgraph.edges[0].thickness, 1.0);
        assert_eq!(
            lgraph.edges[0].labels[0].placement,
            EdgeLabelPlacement::Center
        );
        assert!(lgraph.graph_properties.center_labels);
        assert!(lgraph.options.graph_has_center_labels);
    }

    #[test]
    fn importer_creates_nested_graph_for_inside_self_loop() {
        let mut graph = graph(
            vec![node("A")],
            vec![ElkInputEdge {
                inside_self_loops_yo: true,
                ..edge("A-A", "A", "A")
            }],
        );
        graph.options.inside_self_loops_activate = true;

        let lgraph = import_graph(&graph).unwrap();
        let node = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "A")
            .unwrap();
        assert!(node.nested_graph.is_some());
    }

    #[test]
    fn importer_reuses_inside_self_loop_dummies_per_node_role() {
        let mut graph = graph(
            vec![node("A")],
            vec![
                ElkInputEdge {
                    inside_self_loops_yo: true,
                    ..edge("A-A-1", "A", "A")
                },
                ElkInputEdge {
                    inside_self_loops_yo: true,
                    ..edge("A-A-2", "A", "A")
                },
            ],
        );
        graph.options.inside_self_loops_activate = true;

        let lgraph = import_graph(&graph).unwrap();
        let node = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "A")
            .unwrap();
        let nested = node.nested_graph.as_ref().unwrap();
        let external_dummies = nested
            .layerless_nodes
            .iter()
            .filter(|candidate| candidate.kind == LNodeKind::ExternalPort)
            .collect::<Vec<_>>();

        assert_eq!(external_dummies.len(), 2);
        let input_dummy = external_dummies
            .iter()
            .find(|dummy| dummy.parent_port_type == Some(PortType::Input))
            .expect("inside self-loop input dummy should exist");
        let output_dummy = external_dummies
            .iter()
            .find(|dummy| dummy.parent_port_type == Some(PortType::Output))
            .expect("inside self-loop output dummy should exist");
        assert_eq!(input_dummy.ports[0].incoming_edges.len(), 2);
        assert_eq!(output_dummy.ports[0].outgoing_edges.len(), 2);
    }

    #[test]
    fn importer_marks_inside_self_loops_as_graph_self_loops() {
        let mut graph = graph(
            vec![node("A")],
            vec![ElkInputEdge {
                inside_self_loops_yo: true,
                ..edge("A-A", "A", "A")
            }],
        );
        graph.options.inside_self_loops_activate = true;

        let lgraph = import_graph(&graph).unwrap();
        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "A")
            .and_then(|node| node.nested_graph.as_ref())
            .expect("inside self-loop should create a nested graph");

        assert!(nested.graph_properties.self_loops);
        assert!(nested.options.graph_has_self_loops);
    }

    #[test]
    fn importer_applies_layered_padding_option_to_lgraph_padding() {
        let mut input = graph(vec![node("A")], vec![]);
        input.options.padding = ElkPadding {
            top: 7.0,
            right: 8.0,
            bottom: 9.0,
            left: 10.0,
        };

        let lgraph = import_graph(&input).unwrap();

        assert_eq!(lgraph.padding.top, 7.0);
        assert_eq!(lgraph.padding.right, 8.0);
        assert_eq!(lgraph.padding.bottom, 9.0);
        assert_eq!(lgraph.padding.left, 10.0);
    }

    #[test]
    fn importer_applies_layered_padding_option_to_nested_graphs() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(lgraph.padding.top, 12.0);
        assert_eq!(lgraph.padding.left, 12.0);
        assert_eq!(nested.padding.top, 12.0);
        assert_eq!(nested.padding.left, 12.0);
    }

    #[test]
    fn importer_adds_inside_top_node_label_padding_to_nested_graphs() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        cluster.node_label_placement = NodeLabelPlacement::InsideTopCenter;
        cluster.label = Some(ElkInputLabel::center("Cluster", 64.0, 22.0));
        cluster.nested_spacing_base = Some(30.0);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(nested.options.spacing.node_node, 30.0);
        assert_eq!(nested.padding.top, 39.0);
        assert_eq!(nested.padding.right, 12.0);
        assert_eq!(nested.padding.bottom, 12.0);
        assert_eq!(nested.padding.left, 12.0);
    }

    #[test]
    fn importer_applies_nested_hierarchy_merge_without_cloning_root_options() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        cluster.nested_spacing_base = Some(30.0);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut input = graph(vec![cluster, child], vec![]);
        input.options.merge_hierarchy_edges = false;
        input.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;
        input.options.spacing = SpacingOptions::layered_base_value(40.0);

        let lgraph = import_graph(&input).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(lgraph.layerless_nodes[0].model_order, Some(0));
        assert_eq!(nested.layerless_nodes[0].model_order, None);
        assert!(nested.options.merge_hierarchy_edges);
        assert_eq!(
            nested.options.consider_model_order_strategy,
            OrderingStrategy::None
        );
        assert_eq!(nested.options.spacing.node_node, 30.0);
    }

    #[test]
    fn importer_copies_node_port_constraints_to_nested_graph_options() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        cluster.port_constraints = Some(PortConstraints::FixedSide);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(vec![cluster, child], vec![])).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(nested.options.port_constraints, PortConstraints::FixedSide);
    }

    #[test]
    fn importer_forces_nested_direction_to_parent_direction() {
        let mut cluster = node("cluster");
        cluster.direction = Some(ElkDirection::Left);
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut input = graph(vec![cluster, child], vec![]);
        input.options.direction = ElkDirection::Down;

        let lgraph = import_graph(&input).unwrap();
        let nested = lgraph.layerless_nodes[0].nested_graph.as_ref().unwrap();

        assert_eq!(nested.options.direction, ElkDirection::Down);
    }

    #[test]
    fn importer_resolves_root_hierarchy_inherit_to_separate_children() {
        let mut input = graph(vec![node("cluster"), node("A")], vec![]);
        input.options.hierarchy_handling = HierarchyHandling::Inherit;
        input.nodes[1].parent = Some("cluster".to_string());

        let lgraph = import_graph(&input).unwrap();

        assert_eq!(
            lgraph.options.hierarchy_handling,
            HierarchyHandling::SeparateChildren
        );
        assert!(lgraph.layerless_nodes.iter().all(|node| !node.compound));
    }

    #[test]
    fn imports_include_children_hierarchy_into_nested_graphs() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let lgraph = import_graph(&graph(vec![cluster, child, node("B")], vec![])).unwrap();

        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        assert_eq!(nested.parent_node_id.as_deref(), Some("cluster"));
        assert_eq!(nested.layerless_nodes[0].id, "A");
    }

    #[test]
    fn importer_preserves_descendant_edge_for_compound_preprocessor() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        assert!(nested.edges.is_empty());
        assert_eq!(lgraph.hierarchy_edges.len(), 1);
        assert_eq!(lgraph.hierarchy_edges[0].id, "cluster-A");
        assert_eq!(lgraph.hierarchy_edges[0].source_node_id, "cluster");
        assert_eq!(lgraph.hierarchy_edges[0].target_node_id, "A");
        assert_eq!(lgraph.hierarchy_edges[0].source_path, Vec::<String>::new());
        assert_eq!(lgraph.hierarchy_edges[0].target_path, vec!["cluster"]);
    }

    #[test]
    fn cross_hierarchy_import_creates_endpoint_ports_in_input_order() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut sibling = node("C");
        sibling.parent = Some("cluster".to_string());

        let lgraph = import_graph(&graph(
            vec![cluster, child, sibling, node("B")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        ))
        .unwrap();

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        let child = nested
            .layerless_nodes
            .iter()
            .find(|node| node.id == "A")
            .unwrap();

        assert_eq!(
            child
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["A:0:source", "A:1"]
        );
        assert_eq!(nested.edges[0].source.port, 1);
        assert_eq!(lgraph.hierarchy_edges[0].source_port_key, "A:0:source");
    }

    #[test]
    fn source_ported_compound_metadata_links_parent_port_and_external_dummy() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child, node("B")],
            vec![edge("A-B", "A", "B")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "cluster")
            .unwrap();
        let cluster = &lgraph.layerless_nodes[cluster_index];
        let parent_port = &cluster.ports[0];
        let port_dummy = parent_port
            .port_dummy
            .as_ref()
            .expect("compound port should point to nested external dummy");
        assert!(parent_port.inside_connections);
        assert_eq!(port_dummy.graph_id, "cluster");

        let nested = cluster.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];
        assert_eq!(external.external_port_side, PortSide::South);
        assert_eq!(parent_port.border_offset, external.ports[0].border_offset);
        assert_eq!(
            parent_port.border_offset,
            Some(nested.options.spacing.edge_edge / 2.0)
        );
        let origin = external
            .origin_port
            .as_ref()
            .expect("external dummy should point back to parent port");
        assert_eq!(origin.graph_id, "root");
        assert_eq!(origin.port.node, cluster_index);
        assert_eq!(origin.port.port, 0);
    }

    #[test]
    fn source_ported_compound_does_not_duplicate_existing_parent_dummy() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![node("B"), cluster, child],
            vec![edge("B-A", "B", "A")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        let external_dummies = nested
            .layerless_nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.kind == LNodeKind::ExternalPort && node.id == "external:cluster"
            })
            .collect::<Vec<_>>();

        assert_eq!(external_dummies.len(), 1);
        let (dummy_node, external_dummy) = external_dummies[0];
        let dummy_port = external_dummy.ports.first().unwrap();
        assert_eq!(
            dummy_port.incoming_edges.len() + dummy_port.outgoing_edges.len(),
            1
        );
        assert_eq!(cluster.ports.len(), 1);
        assert_eq!(
            cluster.ports[0].port_dummy.as_ref().unwrap().node,
            dummy_node
        );
    }

    #[test]
    fn source_ported_compound_metadata_links_parent_to_child_external_dummy() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let cluster_index = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "cluster")
            .unwrap();
        let cluster = &lgraph.layerless_nodes[cluster_index];
        let parent_port = cluster
            .ports
            .iter()
            .find(|port| port.port_dummy.is_some())
            .expect("parent-to-child edge should create a parent external port");
        assert_eq!(cluster.ports.len(), 1);
        assert_eq!(parent_port.id, "cluster:0:source");
        assert_eq!(parent_port.port_type, PortType::Output);
        let port_dummy = parent_port.port_dummy.as_ref().unwrap();
        assert!(parent_port.inside_connections);
        assert_eq!(port_dummy.graph_id, "cluster");

        let nested = cluster.nested_graph.as_ref().unwrap();
        let external = &nested.layerless_nodes[port_dummy.node];
        let origin = external
            .origin_port
            .as_ref()
            .expect("external dummy should point back to parent port");
        assert_eq!(origin.graph_id, "root");
        assert_eq!(origin.port.node, cluster_index);
        assert_eq!(origin.port.port, 0);
    }

    #[test]
    fn source_ported_compound_parent_boundary_segments_use_external_port_dummies() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());

        let mut lgraph = import_graph(&graph(
            vec![cluster, child],
            vec![edge("cluster-A", "cluster", "A")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);
        let cluster = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let nested = cluster.nested_graph.as_ref().unwrap();
        let segment = nested
            .edges
            .iter()
            .find(|edge| edge.id == "cluster-A")
            .unwrap();

        assert_eq!(
            nested.layerless_nodes[segment.source.node].kind,
            LNodeKind::ExternalPort
        );
        assert_eq!(nested.layerless_nodes[segment.target.node].id, "A");
    }

    #[test]
    fn source_ported_compound_import_records_cross_hierarchy_segments() {
        let mut outer = node("outer");
        outer.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut inner = node("inner");
        inner.parent = Some("outer".to_string());
        inner.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("inner".to_string());

        let mut lgraph = import_graph(&graph(
            vec![outer, inner, child, node("B")],
            vec![edge("A-B", "A", "B")],
        ))
        .unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let outer = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "outer")
            .unwrap();
        let inner_graph = outer
            .nested_graph
            .as_ref()
            .unwrap()
            .layerless_nodes
            .iter()
            .find(|node| node.id == "inner")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        let outer_graph = outer.nested_graph.as_ref().unwrap();
        assert_eq!(
            inner_graph.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 2 }
        );
        assert_eq!(
            outer_graph.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 1 }
        );
        assert_eq!(
            lgraph.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 0 }
        );
    }

    #[test]
    fn source_ported_compound_reuses_exported_external_port_when_hierarchy_edges_merge() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut first = edge("A-B", "A", "B");
        let mut first_label = ElkInputLabel::center("first", 12.0, 6.0);
        first_label.placement = EdgeLabelPlacement::Tail;
        first.label = Some(first_label);
        let mut second = edge("A-C", "A", "C");
        let mut second_label = ElkInputLabel::center("second", 18.0, 6.0);
        second_label.placement = EdgeLabelPlacement::Tail;
        second.label = Some(second_label);
        let mut input = graph(
            vec![cluster, child, node("B"), node("C")],
            vec![first, second],
        );
        input.options.merge_edges = true;
        input.options.merge_hierarchy_edges = true;

        let mut lgraph = import_graph(&input).unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        assert_eq!(nested.edges.len(), 1);
        assert_eq!(nested.cross_hierarchy_edges.len(), 2);
        assert!(
            nested
                .cross_hierarchy_edges
                .iter()
                .all(|segment| segment.edge == 0)
        );
        assert_eq!(nested.edges[0].labels.len(), 2);
        assert_eq!(
            nested.edges[0]
                .labels
                .iter()
                .filter_map(|label| label.original_label_edge.as_deref())
                .collect::<Vec<_>>(),
            vec!["A-B", "A-C"]
        );
        assert!(nested.graph_properties.end_labels);
    }

    #[test]
    fn source_ported_compound_keeps_nested_hierarchy_merge_default_when_root_disables_merge() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut input = graph(
            vec![cluster, child, node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        );
        input.options.merge_edges = true;
        input.options.merge_hierarchy_edges = false;

        let mut lgraph = import_graph(&input).unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        assert_eq!(nested.edges.len(), 1);
        assert_eq!(
            nested
                .cross_hierarchy_edges
                .iter()
                .map(|segment| segment.edge)
                .collect::<Vec<_>>(),
            vec![0, 0]
        );
    }

    #[test]
    fn source_ported_compound_parent_end_segments_are_not_reused() {
        let mut cluster = node("cluster");
        cluster.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("A");
        child.parent = Some("cluster".to_string());
        let mut input = graph(
            vec![cluster, child],
            vec![
                edge("cluster-A-1", "cluster", "A"),
                edge("cluster-A-2", "cluster", "A"),
            ],
        );
        input.options.merge_edges = true;
        input.options.merge_hierarchy_edges = true;

        let mut lgraph = import_graph(&input).unwrap();
        preprocess_source_ported_compound_graph(&mut lgraph);

        let nested = lgraph
            .layerless_nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap()
            .nested_graph
            .as_ref()
            .unwrap();
        assert_eq!(nested.edges.len(), 2);
        assert_eq!(
            nested
                .cross_hierarchy_edges
                .iter()
                .map(|segment| segment.edge)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(
            nested.edges.iter().all(
                |edge| nested.layerless_nodes[edge.source.node].kind == LNodeKind::ExternalPort
            )
        );
    }

    #[test]
    fn importer_reuses_collector_ports_when_edges_are_merged() {
        let mut input = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        );
        input.options.merge_edges = true;

        let lgraph = import_graph(&input).unwrap();

        let a = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(lgraph.layerless_nodes[a].ports.len(), 1);
        assert_eq!(
            lgraph.layerless_nodes[a].ports[0].collector_type,
            Some(PortType::Output)
        );
        assert_eq!(
            lgraph.layerless_nodes[a].ports[0].outgoing_edges,
            vec![0, 1]
        );
        assert!(lgraph.graph_properties.hyperedges);
        assert!(lgraph.options.graph_has_hyperedges);
    }

    #[test]
    fn importer_keeps_dedicated_ports_when_edge_merge_is_disabled() {
        let lgraph = import_graph(&graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        ))
        .unwrap();

        let a = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(lgraph.layerless_nodes[a].ports.len(), 2);
        assert!(
            lgraph.layerless_nodes[a]
                .ports
                .iter()
                .all(|port| port.collector_type.is_none())
        );
        assert!(!lgraph.graph_properties.hyperedges);
    }

    #[test]
    fn importer_keeps_dedicated_ports_when_node_port_constraints_are_side_fixed() {
        let mut a = node("A");
        a.port_constraints = Some(PortConstraints::FixedSide);
        let mut input = graph(
            vec![a, node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("A-C", "A", "C")],
        );
        input.options.merge_edges = true;

        let lgraph = import_graph(&input).unwrap();

        let a = lgraph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(lgraph.layerless_nodes[a].ports.len(), 2);
        assert!(
            lgraph.layerless_nodes[a]
                .ports
                .iter()
                .all(|port| port.collector_type.is_none())
        );
    }

    #[test]
    fn import_rejects_invalid_parent_and_endpoints() {
        let mut child = node("A");
        child.parent = Some("missing".to_string());
        assert!(matches!(
            import_graph(&graph(vec![child], vec![])),
            Err(ImportError::MissingParent { .. })
        ));

        assert!(matches!(
            import_graph(&graph(vec![node("A")], vec![edge("A-B", "A", "B")])),
            Err(ImportError::MissingEndpoint { .. })
        ));
    }

    #[test]
    fn scoped_segment_import_rejects_an_unmaterialized_owner_scope() {
        let group = node("group");
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let mut input = graph(vec![group, child, node("outer")], vec![]);
        input.options.hierarchy_handling = HierarchyHandling::SeparateChildren;
        let segment = ElkInputEdgeSegment {
            edge: edge("child-outer", "child", "outer"),
            source: ElkInputEdgeSegmentEndpoint::Node {
                id: "child".to_string(),
            },
            target: ElkInputEdgeSegmentEndpoint::Node {
                id: "outer".to_string(),
            },
            segment: CompoundEdgeSegment::Output { depth: 0 },
            edge_order: 0,
        };
        let mut work_control = NoopWorkControl;

        let error = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[segment],
            &mut work_control,
        )
        .expect_err("a flat import must reject a segment owned by a nested scope");

        assert_eq!(
            error,
            ImportError::UnavailableSegmentScope {
                edge_id: "child-outer".to_string(),
                graph_parent: "group".to_string(),
            }
        );
    }

    #[test]
    fn scoped_segment_import_rejects_a_scope_below_an_unmaterialized_ancestor() {
        let mut outer = node("outer-group");
        outer.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
        let mut inner = node("inner-group");
        inner.parent = Some("outer-group".to_string());
        inner.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("inner-group".to_string());
        let mut input = graph(vec![outer, inner, child, node("root-node")], vec![]);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let segment = ElkInputEdgeSegment {
            edge: edge("child-root", "child", "root-node"),
            source: ElkInputEdgeSegmentEndpoint::Node {
                id: "child".to_string(),
            },
            target: ElkInputEdgeSegmentEndpoint::Node {
                id: "root-node".to_string(),
            },
            segment: CompoundEdgeSegment::Output { depth: 0 },
            edge_order: 0,
        };
        let mut work_control = NoopWorkControl;

        let error = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[segment],
            &mut work_control,
        )
        .expect_err("a locally includable scope below a SeparateChildren ancestor is unreachable");

        assert_eq!(
            error,
            ImportError::UnavailableSegmentScope {
                edge_id: "child-root".to_string(),
                graph_parent: "outer-group".to_string(),
            }
        );
    }

    #[test]
    fn scoped_segment_import_accepts_a_materialized_owner_scope() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let mut input = graph(vec![group, child, node("outer")], vec![]);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let mut segment_edge = edge("child-outer", "child", "outer");
        segment_edge.model_order = Some(7);
        segment_edge.label = Some(ElkInputLabel::center("identity", 40.0, 12.0));
        let segment = ElkInputEdgeSegment {
            edge: segment_edge,
            source: ElkInputEdgeSegmentEndpoint::Node {
                id: "child".to_string(),
            },
            target: ElkInputEdgeSegmentEndpoint::Node {
                id: "outer".to_string(),
            },
            segment: CompoundEdgeSegment::Output { depth: 2 },
            edge_order: 0,
        };
        let mut work_control = NoopWorkControl;

        let imported = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[segment],
            &mut work_control,
        )
        .expect("the nested owner scope is materialized");

        assert_eq!(imported.cross_hierarchy_edges.len(), 1);
        let nested = imported
            .layerless_nodes
            .iter()
            .find(|node| node.id == "group")
            .and_then(|node| node.nested_graph.as_deref())
            .expect("group scope should be materialized");
        assert_eq!(nested.cross_hierarchy_edges.len(), 1);
        assert_eq!(
            imported.cross_hierarchy_edges[0].original_model_order,
            Some(7)
        );
        assert_eq!(
            nested.cross_hierarchy_edges[0].original_model_order,
            Some(7)
        );
        assert_eq!(
            imported.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 2 }
        );
        assert_eq!(
            nested.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Output { depth: 3 }
        );
        let labels = imported
            .edges
            .iter()
            .chain(nested.edges.iter())
            .flat_map(|edge| edge.labels.iter())
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].text, "identity");
        assert_eq!(
            labels[0].original_label_edge.as_deref(),
            Some("child-outer")
        );
    }

    #[test]
    fn scoped_input_segment_rebases_every_materialized_hierarchy_level() {
        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let mut input = graph(vec![group, child, node("outer")], vec![]);
        input.options.hierarchy_handling = HierarchyHandling::IncludeChildren;
        let segment = ElkInputEdgeSegment {
            edge: edge("outer-child", "outer", "child"),
            source: ElkInputEdgeSegmentEndpoint::Node {
                id: "outer".to_string(),
            },
            target: ElkInputEdgeSegmentEndpoint::Node {
                id: "child".to_string(),
            },
            segment: CompoundEdgeSegment::Input { depth: 4 },
            edge_order: 0,
        };
        let mut work_control = NoopWorkControl;

        let imported = import_graph_at_scope_and_segments_with_work_control(
            &input,
            &["root"],
            &[segment],
            &mut work_control,
        )
        .expect("the nested owner scope is materialized");
        let nested = imported
            .layerless_nodes
            .iter()
            .find(|node| node.id == "group")
            .and_then(|node| node.nested_graph.as_deref())
            .expect("group scope should be materialized");

        assert_eq!(
            imported.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Input { depth: 4 }
        );
        assert_eq!(
            nested.cross_hierarchy_edges[0].segment,
            CompoundEdgeSegment::Input { depth: 5 }
        );
    }

    #[test]
    fn import_preserves_model_order_strategy_without_enabling_wrapping() {
        let mut input = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);
        input.options.consider_model_order_strategy = OrderingStrategy::NodesAndEdges;

        let lgraph = import_graph(&input).unwrap();

        assert_eq!(
            lgraph.options.consider_model_order_strategy,
            OrderingStrategy::NodesAndEdges
        );
        assert!(!lgraph.options.graph_has_hyperedges);
    }

    #[test]
    fn import_preserves_elk_unseeded_zero_until_the_pipeline_resolves_it() {
        let mut input = graph(vec![node("A")], vec![]);
        input.options.random_seed = 0;
        let mut imported = import_graph(&input).expect("zero is a valid upstream input value");

        assert!(matches!(
            crate::pipeline::execute_ported_processors(&mut imported),
            Err(crate::PipelineError::RandomSeed(crate::RandomSeedError::Unresolved {
                graph_path
            })) if graph_path == "root"
        ));
        assert_eq!(imported.options.random_seed, 0);
    }

    #[test]
    fn operation_seed_scopes_nested_graphs_and_configuration_invocations() {
        use std::num::NonZeroU64;

        let mut group = node("group");
        group.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
        let mut child = node("child");
        child.parent = Some("group".to_string());
        let mut input = graph(vec![group, child], vec![]);
        input.options.random_seed = 0;
        let operation_seed = OperationSeed::from_operation_seed(
            NonZeroU64::new(0x0ddc_0ffe_e15e_cafe).expect("nonzero operation seed"),
        );

        let mut first = import_graph_with_operation_seed(&input, operation_seed).expect("import");
        let mut second = import_graph_with_operation_seed(&input, operation_seed).expect("import");
        crate::configurator::configure_graph_properties(&mut first).expect("resolve first");
        let first_root_first = first.random.clone().next_long();
        let first_nested_first = first.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("nested graph")
            .random
            .clone()
            .next_long();
        crate::configurator::configure_graph_properties(&mut first)
            .expect("resolve repeated configuration");
        let first_root_second = first.random.clone().next_long();
        let first_nested_second = first.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("nested graph")
            .random
            .clone()
            .next_long();

        crate::configurator::configure_graph_properties(&mut second).expect("resolve replay");
        let second_root_first = second.random.clone().next_long();
        let second_nested_first = second.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("nested graph")
            .random
            .clone()
            .next_long();
        crate::configurator::configure_graph_properties(&mut second)
            .expect("resolve replayed configuration");
        let second_root_second = second.random.clone().next_long();
        let second_nested_second = second.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .expect("nested graph")
            .random
            .clone()
            .next_long();

        assert_eq!(first.options.random_seed, 0);
        assert_eq!(
            first.layerless_nodes[0]
                .nested_graph
                .as_deref()
                .expect("nested graph")
                .options
                .random_seed,
            0
        );
        assert_ne!(first_root_first, first_nested_first);
        assert_ne!(first_root_second, first_nested_second);
        assert_ne!(first_root_first, first_root_second);
        assert_ne!(first_nested_first, first_nested_second);
        assert_eq!(first.options.random_seed, second.options.random_seed);
        assert_eq!(first_root_first, second_root_first);
        assert_eq!(first_nested_first, second_nested_first);
        assert_eq!(first_root_second, second_root_second);
        assert_eq!(first_nested_second, second_nested_second);
    }
}
