#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! Source-port policy:
//! - Mermaid's adapter layer is
//!   https://github.com/mermaid-js/mermaid/blob/7c0cafcf42e76bfaf79d0cbbd12edb986612f014/packages/mermaid-layout-elk/src/render.ts.
//! - Mermaid pins `elkjs@0.9.3`; the corresponding source checkout is
//!   https://github.com/kieler/elkjs/tree/a8304cf79fde75bc2ab1a89d28320f53f8637436.
//! - `elkjs` is generated from Eclipse ELK Java sources. The current source baseline is
//!   https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e,
//!   which is the 0.9.x ELK release tag available for the `elkjs@0.9.3` release window.
//!
//! The crate exposes one Mermaid adapter and one source-backed layered implementation. New layout
//! behavior must carry a pinned Mermaid or Eclipse ELK source reference.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::num::NonZeroU64;

mod model;
use merman_elk_layered as source_port;
pub use model::*;
pub use source_port::{
    GraphExecution, HierarchySweepDebugTrace, HierarchySweepNodeDebug, LayeredPhase,
    NoopWorkControl, ProcessorKind, WorkControl, WorkError,
};

/// The nonzero random seed captured by the owner of one render/layout operation.
///
/// This token is intentionally separate from ELK's signed `randomSeed` option. The latter keeps
/// Eclipse ELK's Java semantics for every nonzero value and treats zero as an unseeded sentinel.
/// Passing this token to [`layout_with_operation_seed`] supplies the deterministic replacement
/// for that sentinel without mutating the source option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElkOperationSeed(source_port::OperationSeed);

impl ElkOperationSeed {
    /// Creates an ELK token from the seed captured by an operation owner.
    pub const fn from_operation_seed(seed: NonZeroU64) -> Self {
        Self(source_port::OperationSeed::from_operation_seed(seed))
    }

    const fn source_port_seed(self) -> source_port::OperationSeed {
        self.0
    }
}

use source_port::{
    ElkDirection, ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, LGraph, LNodeKind,
    LPoint, LayeredOptions as SourceLayeredOptions, NodeLabelPlacement, PortRef,
};

/// A guarded diagnostic session for inspecting the source-backed layered pipeline.
///
/// The underlying ELK graph remains private. Every method that executes processors enters a
/// fallible source-port pipeline boundary, so `randomSeed = 0` is either rejected or resolved
/// from the [`ElkOperationSeed`] supplied at construction.
///
/// The former raw re-export is intentionally unavailable:
///
/// ```compile_fail
/// use merman_layout_elk::source_port;
/// ```
pub struct SourcePhaseDiagnostics {
    graph: LGraph,
}

impl SourcePhaseDiagnostics {
    /// Builds a raw diagnostic session. Executing a graph that retains ELK's zero seed sentinel
    /// returns the same typed error as [`layout`].
    pub fn from_graph(graph: &Graph) -> Result<Self> {
        Self::from_graph_with_optional_operation_seed(graph, None)
    }

    /// Builds a diagnostic session using an operation-owned seed for ELK's zero sentinel.
    pub fn from_graph_with_operation_seed(
        graph: &Graph,
        operation_seed: ElkOperationSeed,
    ) -> Result<Self> {
        Self::from_graph_with_optional_operation_seed(graph, Some(operation_seed))
    }

    fn from_graph_with_optional_operation_seed(
        graph: &Graph,
        operation_seed: Option<ElkOperationSeed>,
    ) -> Result<Self> {
        let input = graph_to_source_input(graph);
        let lgraph = match operation_seed {
            Some(operation_seed) => source_port::import_graph_with_operation_seed(
                &input,
                operation_seed.source_port_seed(),
            ),
            None => source_port::import_graph(&input),
        }
        .map_err(Error::SourceImport)?;
        Ok(Self { graph: lgraph })
    }

    /// Runs the source-backed pipeline until a phase completes.
    pub fn execute_until(&mut self, target: LayeredPhase) -> Result<Vec<ProcessorKind>> {
        source_port::execute_processors_until(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs the source-backed pipeline until a processor completes.
    pub fn execute_until_processor(&mut self, target: ProcessorKind) -> Result<Vec<ProcessorKind>> {
        source_port::execute_processors_until_processor(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs every currently ported processor for a flat graph.
    pub fn execute_all(&mut self) -> Result<Vec<ProcessorKind>> {
        source_port::execute_ported_processors(&mut self.graph).map_err(Error::SourcePipeline)
    }

    /// Runs the compound pipeline until a phase completes.
    pub fn execute_compound_until(&mut self, target: LayeredPhase) -> Result<Vec<GraphExecution>> {
        source_port::execute_ported_compound_processors_until(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs the compound pipeline until a processor completes.
    pub fn execute_compound_until_processor(
        &mut self,
        target: ProcessorKind,
    ) -> Result<Vec<GraphExecution>> {
        source_port::execute_ported_compound_processors_until_processor(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Runs every currently ported compound processor.
    pub fn execute_compound_all(&mut self) -> Result<Vec<GraphExecution>> {
        source_port::execute_ported_compound_processors(&mut self.graph)
            .map_err(Error::SourcePipeline)
    }

    /// Runs the guarded compound prefix used by the hierarchical crossing-sweep diagnostic.
    pub fn inspect_compound_crossings_after_processor(
        &mut self,
        target: ProcessorKind,
    ) -> Result<(Vec<GraphExecution>, Option<HierarchySweepDebugTrace>)> {
        source_port::inspect_compound_crossings_after_processor(&mut self.graph, target)
            .map_err(Error::SourcePipeline)
    }

    /// Formats the private source graph for human diagnostics without exposing executable phase
    /// APIs or the mutable `LGraph` itself.
    pub fn graph_dump(&self) -> String {
        let mut output = String::new();
        write_source_graph_dump(&mut output, &self.graph, 0)
            .expect("writing to String cannot fail");
        output
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    SourceImport(#[from] merman_elk_layered::ImportError),
    #[error(transparent)]
    SourcePipeline(#[from] merman_elk_layered::PipelineError),
    #[error(transparent)]
    Work(#[from] WorkError),
}

impl Error {
    pub const fn work_error(&self) -> Option<WorkError> {
        match self {
            Self::Work(error) => Some(*error),
            Self::SourceImport(source_port::ImportError::Work(error))
            | Self::SourcePipeline(source_port::PipelineError::Work(error)) => Some(*error),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Execute Mermaid's ELK adapter over the source-backed Eclipse ELK layered pipeline.
pub fn layout(graph: &Graph) -> Result<LayoutResult> {
    let mut work_control = NoopWorkControl;
    layout_with_work_control(graph, &mut work_control)
}

/// Executes Mermaid's ELK adapter with caller-owned checked work control.
pub fn layout_with_work_control(
    graph: &Graph,
    work_control: &mut dyn WorkControl,
) -> Result<LayoutResult> {
    layout_scopes(graph, None, work_control)
}

/// Executes Mermaid's ELK adapter using the seed captured by its owning operation.
///
/// Normal Mermaid adapter graphs keep their source default seed of `1`, so this policy affects
/// only an explicitly configured unseeded graph. [`layout`] is the raw/diagnostic API and rejects
/// the sentinel; this API is for render or layout operation owners that intentionally provide a
/// single immutable operation seed.
pub fn layout_with_operation_seed(
    graph: &Graph,
    operation_seed: ElkOperationSeed,
) -> Result<LayoutResult> {
    let mut work_control = NoopWorkControl;
    layout_with_operation_seed_and_work_control(graph, operation_seed, &mut work_control)
}

/// Executes Mermaid's ELK adapter with one operation-owned seed and checked work control.
pub fn layout_with_operation_seed_and_work_control(
    graph: &Graph,
    operation_seed: ElkOperationSeed,
    work_control: &mut dyn WorkControl,
) -> Result<LayoutResult> {
    layout_scopes(graph, Some(operation_seed.source_port_seed()), work_control)
}

fn graph_to_source_input(graph: &Graph) -> ElkInputGraph {
    graph_to_source_input_with_root_context(graph, None, None)
}

fn graph_to_source_input_with_root_context(
    graph: &Graph,
    root_spacing_base: Option<f64>,
    root_label: Option<Label>,
) -> ElkInputGraph {
    let mut options = layered_options_to_source(graph);
    if let Some(base) = root_spacing_base {
        options.spacing = source_port::SpacingOptions::layered_base_value(base);
    }
    if let Some(label) = root_label {
        apply_root_inside_top_center_label_padding(&mut options, label);
    }
    let node_ids_with_direct_children = graph
        .nodes
        .iter()
        .filter_map(|node| node.parent.as_deref())
        .collect::<HashSet<_>>();

    ElkInputGraph {
        id: graph.id.clone(),
        options,
        nodes: graph
            .nodes
            .iter()
            .map(|node| ElkInputNode {
                id: node.id.clone(),
                width: node.width,
                height: node.height,
                parent: node.parent.clone(),
                direction: node.direction.map(direction_to_source),
                hierarchy_handling: match (node.kind, node.hierarchy_handling) {
                    (NodeKind::Group, Some(hierarchy_handling)) => {
                        Some(hierarchy_handling_to_source(hierarchy_handling))
                    }
                    (NodeKind::Group, None) => Some(hierarchy_handling_to_source(
                        graph.options.layered.hierarchy_handling,
                    )),
                    (NodeKind::Leaf, _) => None,
                },
                layer_constraint: node.layer_constraint.map(layer_constraint_to_source),
                port_constraints: None,
                node_label_placement: match node.kind {
                    NodeKind::Group => NodeLabelPlacement::InsideTopCenter,
                    NodeKind::Leaf => NodeLabelPlacement::Fixed,
                },
                nested_spacing_base: match node.kind {
                    NodeKind::Group => Some(30.0),
                    NodeKind::Leaf => None,
                },
                // Mermaid only exposes a subgraph label to ELK when `childrenById` contains the
                // subgraph. Empty subgraphs retain their label data for SVG rendering, but their
                // titles must not create ELK label margins.
                label: if node.kind == NodeKind::Leaf
                    || node_ids_with_direct_children.contains(node.id.as_str())
                {
                    node.label
                        .map(|label| ElkInputLabel::center("", label.width, label.height))
                } else {
                    None
                },
            })
            .collect(),
        edges: graph
            .edges
            .iter()
            .map(|edge| ElkInputEdge {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                label: edge
                    .label
                    .map(|label| ElkInputLabel::center("", label.width, label.height)),
                minlen: edge.minlen,
                inside_self_loops_yo: edge.inside_self_loops_yo,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
    }
}

fn apply_root_inside_top_center_label_padding(options: &mut SourceLayeredOptions, label: Label) {
    if label.height > 0.0 {
        options.padding.top += label.height + options.node_labels_padding.top;
    }
}

#[derive(Debug)]
struct HierarchyIndex<'a> {
    graph: &'a Graph,
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    node_scope: Vec<usize>,
    edge_model_order: Vec<usize>,
    child_scope_by_anchor: Vec<Option<usize>>,
    scopes: Vec<ScopePlan>,
    postorder: Vec<usize>,
}

#[derive(Debug)]
struct ScopePlan {
    parent: Option<usize>,
    anchor: Option<usize>,
    depth: usize,
    seed_scope: source_port::GraphSeedScope,
    direction: Direction,
    handling: HierarchyHandling,
    nodes: Vec<usize>,
    children: Vec<usize>,
    edges: Vec<ScopedEdge>,
    owned_edges: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
enum ScopedEndpoint {
    Node(usize),
    ParentBoundary { scope: usize, connects_node: bool },
}

type ScopeEdgePiece = (
    usize,
    ScopedEndpoint,
    ScopedEndpoint,
    source_port::CompoundEdgeSegment,
);

#[derive(Debug)]
struct ScopedEdge {
    original: usize,
    source: ScopedEndpoint,
    target: ScopedEndpoint,
    segment: Option<source_port::CompoundEdgeSegment>,
    segment_order: Option<usize>,
    segment_count: usize,
    carries_label: bool,
}

#[derive(Debug)]
struct ScopeLayout {
    layout: LayoutResult,
    size: source_port::LSize,
    edge_metadata: HashMap<String, ScopeEdgeMetadata>,
}

#[derive(Debug, Clone, Copy)]
struct ScopeEdgeMetadata {
    original: usize,
    segment: Option<SegmentedEdgeMetadata>,
}

#[derive(Debug, Clone, Copy)]
struct SegmentedEdgeMetadata {
    segment: source_port::CompoundEdgeSegment,
    model_order: Option<usize>,
    order: usize,
    count: usize,
}

fn layout_scopes(
    graph: &Graph,
    operation_seed: Option<source_port::OperationSeed>,
    work_control: &mut dyn WorkControl,
) -> Result<LayoutResult> {
    let index = HierarchyIndex::build(graph, work_control)?;
    work_control.check(index.scopes.len())?;
    work_control.charge(index.scopes.len())?;
    let mut arena = std::iter::repeat_with(|| None)
        .take(index.scopes.len())
        .collect::<Vec<Option<ScopeLayout>>>();

    for &scope in &index.postorder {
        let (input, segments, edge_metadata) =
            index.materialize_scope(scope, &arena, work_control)?;
        let mut lgraph = match operation_seed {
            Some(operation_seed) => {
                source_port::import_graph_with_operation_seed_at_seed_scope_and_segments_with_work_control(
                    &input,
                    operation_seed,
                    &index.scopes[scope].seed_scope,
                    &segments,
                    work_control,
                )
            }
            None => source_port::import_graph_at_seed_scope_and_segments_with_work_control(
                &input,
                &index.scopes[scope].seed_scope,
                &segments,
                work_control,
            ),
        }
        .map_err(Error::SourceImport)?;
        if source_graph_requires_compound_pipeline(&lgraph) {
            source_port::execute_ported_compound_processors_with_work_control(
                &mut lgraph,
                work_control,
            )
            .map_err(Error::SourcePipeline)?;
        } else {
            source_port::execute_ported_processors_with_work_control(&mut lgraph, work_control)
                .map_err(Error::SourcePipeline)?;
        }
        let export_work = source_graph_output_work_units(&lgraph)?;
        work_control.check(export_work)?;
        work_control.charge(export_work)?;
        arena[scope] = Some(ScopeLayout {
            layout: source_graph_to_layout_result(&lgraph),
            size: actual_source_graph_size(&lgraph),
            edge_metadata,
        });
    }

    flatten_scope_layouts(&index, arena, work_control)
}

impl<'a> HierarchyIndex<'a> {
    fn build(graph: &'a Graph, work_control: &mut dyn WorkControl) -> Result<Self> {
        let unique_items = graph
            .nodes
            .len()
            .checked_add(graph.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work_control.check(unique_items)?;
        work_control.charge(unique_items)?;

        let mut node_by_id = HashMap::with_capacity(graph.nodes.len());
        for (index, node) in graph.nodes.iter().enumerate() {
            if node_by_id.insert(node.id.as_str(), index).is_some() {
                return Err(source_port::ImportError::DuplicateNode {
                    id: node.id.clone(),
                }
                .into());
            }
        }

        // FlowDB normalizes every edge ID before invoking Mermaid's ELK adapter: duplicate
        // user-provided IDs receive unique generated identities. Scope metadata may key by that
        // identity, but non-canonical duplicate input must not silently alias original edges.
        work_control.check(graph.edges.len())?;
        work_control.charge(graph.edges.len())?;
        let mut edge_ids = HashSet::with_capacity(graph.edges.len());
        for edge in &graph.edges {
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(source_port::ImportError::DuplicateEdge {
                    id: edge.id.clone(),
                }
                .into());
            }
        }

        let mut parent = vec![None; graph.nodes.len()];
        let mut children = vec![Vec::new(); graph.nodes.len()];
        let mut roots = Vec::new();
        for (index, node) in graph.nodes.iter().enumerate() {
            if let Some(parent_id) = node.parent.as_deref() {
                let Some(&parent_index) = node_by_id.get(parent_id) else {
                    return Err(source_port::ImportError::MissingParent {
                        node_id: node.id.clone(),
                        parent_id: parent_id.to_string(),
                    }
                    .into());
                };
                parent[index] = Some(parent_index);
                children[parent_index].push(index);
            } else {
                roots.push(index);
            }
        }
        detect_parent_cycles(graph, &parent)?;

        work_control.check(1)?;
        work_control.charge(1)?;
        let mut scopes = vec![ScopePlan {
            parent: None,
            anchor: None,
            depth: 0,
            seed_scope: source_port::GraphSeedScope::root(graph.id.as_str()),
            direction: graph.direction,
            handling: graph.options.layered.hierarchy_handling,
            nodes: Vec::new(),
            children: Vec::new(),
            edges: Vec::new(),
            owned_edges: Vec::new(),
        }];
        let mut node_scope = vec![0usize; graph.nodes.len()];
        let mut resolved_handling =
            vec![graph.options.layered.hierarchy_handling; graph.nodes.len()];
        let mut child_scope_by_anchor = vec![None; graph.nodes.len()];
        let mut hierarchy_preorder = Vec::with_capacity(graph.nodes.len());
        let mut preorder_position = vec![0usize; graph.nodes.len()];
        let mut search = roots
            .into_iter()
            .rev()
            .map(|node| {
                (
                    node,
                    0usize,
                    graph.options.layered.hierarchy_handling,
                    graph.direction,
                )
            })
            .collect::<Vec<_>>();
        while let Some((node_index, scope, parent_handling, parent_direction)) = search.pop() {
            let node = &graph.nodes[node_index];
            let handling = if node.kind == NodeKind::Group {
                node.hierarchy_handling.unwrap_or(parent_handling)
            } else {
                parent_handling
            };
            let direction = node.direction.unwrap_or(parent_direction);
            resolved_handling[node_index] = handling;
            node_scope[node_index] = scope;
            preorder_position[node_index] = hierarchy_preorder.len();
            hierarchy_preorder.push(node_index);

            let separates = node.kind == NodeKind::Group
                && !children[node_index].is_empty()
                && (parent_handling == HierarchyHandling::SeparateChildren
                    || handling == HierarchyHandling::SeparateChildren);
            let child_scope = if separates {
                work_control.check(1)?;
                work_control.charge(1)?;
                let child_scope = scopes.len();
                let seed_scope = scopes[scope].seed_scope.child(node.id.as_str());
                scopes.push(ScopePlan {
                    parent: Some(scope),
                    anchor: Some(node_index),
                    depth: scopes[scope].depth + 1,
                    seed_scope,
                    direction,
                    handling,
                    nodes: Vec::new(),
                    children: Vec::new(),
                    edges: Vec::new(),
                    owned_edges: Vec::new(),
                });
                scopes[scope].children.push(child_scope);
                child_scope_by_anchor[node_index] = Some(child_scope);
                child_scope
            } else {
                scope
            };
            search.extend(
                children[node_index]
                    .iter()
                    .rev()
                    .map(|child| (*child, child_scope, handling, direction)),
            );
        }
        for node in 0..graph.nodes.len() {
            scopes[node_scope[node]].nodes.push(node);
        }

        let mut edge_owner_scope = vec![0usize; graph.edges.len()];
        let mut edge_model_order = vec![0usize; graph.edges.len()];
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let Some(&source) = node_by_id.get(edge.source.as_str()) else {
                return Err(source_port::ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.source.clone(),
                }
                .into());
            };
            let Some(&target) = node_by_id.get(edge.target.as_str()) else {
                return Err(source_port::ImportError::MissingEndpoint {
                    edge_id: edge.id.clone(),
                    node_id: edge.target.clone(),
                }
                .into());
            };
            let source_scope = node_scope[source];
            let target_scope = node_scope[target];
            if source_scope == target_scope {
                let owner = source_scope;
                edge_owner_scope[edge_index] = owner;
                edge_model_order[edge_index] = scopes[owner].owned_edges.len();
                scopes[owner].owned_edges.push(edge_index);
                scopes[owner].edges.push(ScopedEdge {
                    original: edge_index,
                    source: ScopedEndpoint::Node(source),
                    target: ScopedEndpoint::Node(target),
                    segment: None,
                    segment_order: None,
                    segment_count: 0,
                    carries_label: true,
                });
                continue;
            }

            // Mermaid owns a cross-scope edge at the endpoint-inclusive common ancestor. Build
            // that owner and ELK's required boundary sections in one parent-chain pass.
            let (owner, mut pieces) = scope_edge_segments(
                &scopes,
                source,
                target,
                source_scope,
                target_scope,
                work_control,
            )?;
            edge_owner_scope[edge_index] = owner;
            edge_model_order[edge_index] = scopes[owner].owned_edges.len();
            scopes[owner].owned_edges.push(edge_index);
            let label_piece = edge.label.map(|_| compound_center_segment(&pieces));
            let segment_count = pieces.len();
            for (piece_index, (scope, source, target, segment)) in pieces.drain(..).enumerate() {
                scopes[scope].edges.push(ScopedEdge {
                    original: edge_index,
                    source,
                    target,
                    segment: Some(segment),
                    segment_order: Some(piece_index),
                    segment_count,
                    carries_label: label_piece == Some(piece_index),
                });
            }
        }

        let include_order_work = scopes
            .iter()
            .filter(|scope| {
                scope.handling == HierarchyHandling::IncludeChildren && scope.edges.len() >= 2
            })
            .try_fold(graph.nodes.len(), |work, scope| {
                let edge_work = scope
                    .edges
                    .len()
                    .checked_mul(3)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                work.checked_add(scope.nodes.len())
                    .and_then(|work| work.checked_add(scope.owned_edges.len()))
                    .and_then(|work| work.checked_add(edge_work))
                    .ok_or(WorkError::ArithmeticOverflow)
            })?;
        if scopes.iter().any(|scope| {
            scope.handling == HierarchyHandling::IncludeChildren && scope.edges.len() >= 2
        }) {
            work_control.check(include_order_work)?;
            work_control.charge(include_order_work)?;

            let mut subtree_size = vec![1usize; graph.nodes.len()];
            for node in hierarchy_preorder.iter().rev().copied() {
                if let Some(parent) = parent[node] {
                    subtree_size[parent] = subtree_size[parent]
                        .checked_add(subtree_size[node])
                        .ok_or(WorkError::ArithmeticOverflow)?;
                }
            }
            for (scope_index, scope) in scopes.iter_mut().enumerate() {
                if scope.handling != HierarchyHandling::IncludeChildren || scope.edges.len() < 2 {
                    continue;
                }
                order_scope_edges_like_elk_importer(
                    scope,
                    scope_index,
                    &parent,
                    &children,
                    &node_scope,
                    &resolved_handling,
                    &preorder_position,
                    &subtree_size,
                );
                let previous_owned = std::mem::take(&mut scope.owned_edges);
                for &original in &previous_owned {
                    edge_model_order[original] = usize::MAX;
                }
                for scoped in &scope.edges {
                    if edge_owner_scope[scoped.original] == scope_index
                        && edge_model_order[scoped.original] == usize::MAX
                    {
                        edge_model_order[scoped.original] = scope.owned_edges.len();
                        scope.owned_edges.push(scoped.original);
                    }
                }
                for original in previous_owned {
                    if edge_model_order[original] == usize::MAX {
                        edge_model_order[original] = scope.owned_edges.len();
                        scope.owned_edges.push(original);
                    }
                }
            }
        }

        let mut postorder = Vec::with_capacity(scopes.len());
        let mut frames = vec![(0usize, false)];
        while let Some((scope, expanded)) = frames.pop() {
            if expanded {
                postorder.push(scope);
                continue;
            }
            frames.push((scope, true));
            frames.extend(
                scopes[scope]
                    .children
                    .iter()
                    .rev()
                    .map(|child| (*child, false)),
            );
        }

        Ok(Self {
            graph,
            parent,
            children,
            node_scope,
            edge_model_order,
            child_scope_by_anchor,
            scopes,
            postorder,
        })
    }

    fn materialize_scope(
        &self,
        scope_index: usize,
        arena: &[Option<ScopeLayout>],
        work_control: &mut dyn WorkControl,
    ) -> Result<(
        ElkInputGraph,
        Vec<source_port::ElkInputEdgeSegment>,
        HashMap<String, ScopeEdgeMetadata>,
    )> {
        let scope = &self.scopes[scope_index];
        let materialized = scope
            .nodes
            .len()
            .checked_add(scope.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work_control.check(materialized)?;
        work_control.charge(materialized)?;

        let mut options =
            layered_options_to_source_for(self.graph, scope.direction, scope.handling);
        if scope.anchor.is_some() {
            options.spacing = source_port::SpacingOptions::layered_base_value(30.0);
        }
        if let Some(label) = scope.anchor.and_then(|node| self.graph.nodes[node].label) {
            apply_root_inside_top_center_label_padding(&mut options, label);
        }

        let nodes = scope
            .nodes
            .iter()
            .map(|node_index| {
                let source = &self.graph.nodes[*node_index];
                let child_size = self.child_scope_by_anchor[*node_index]
                    .and_then(|child| arena[child].as_ref())
                    .map(|layout| layout.size);
                // Mermaid removes a non-empty group's explicit size before ELK layout. A parent
                // scope must therefore consume the completed child extent, not the input size.
                let width = child_size
                    .map(|size| source.width.max(size.width))
                    .unwrap_or(source.width);
                let height = child_size
                    .map(|size| source.height.max(size.height))
                    .unwrap_or(source.height);
                ElkInputNode {
                    id: source.id.clone(),
                    width,
                    height,
                    parent: self.parent[*node_index]
                        .filter(|parent| self.node_scope[*parent] == scope_index)
                        .map(|parent| self.graph.nodes[parent].id.clone()),
                    direction: source.direction.map(direction_to_source),
                    hierarchy_handling: match (source.kind, source.hierarchy_handling) {
                        (NodeKind::Group, Some(handling)) => {
                            Some(hierarchy_handling_to_source(handling))
                        }
                        (NodeKind::Group, None) => {
                            Some(hierarchy_handling_to_source(scope.handling))
                        }
                        (NodeKind::Leaf, _) => None,
                    },
                    layer_constraint: source.layer_constraint.map(layer_constraint_to_source),
                    port_constraints: None,
                    node_label_placement: match source.kind {
                        NodeKind::Group => NodeLabelPlacement::InsideTopCenter,
                        NodeKind::Leaf => NodeLabelPlacement::Fixed,
                    },
                    nested_spacing_base: (source.kind == NodeKind::Group).then_some(30.0),
                    // Mermaid attaches an ELK label only when childrenById contains the group.
                    // Empty-group titles remain available to SVG rendering without layout margin.
                    label: if source.kind == NodeKind::Leaf
                        || !self.children[*node_index].is_empty()
                    {
                        source
                            .label
                            .map(|label| ElkInputLabel::center("", label.width, label.height))
                    } else {
                        None
                    },
                }
            })
            .collect::<Vec<_>>();

        let mut edges = Vec::new();
        let mut segments = Vec::new();
        let mut edge_metadata = HashMap::new();
        let uses_edge_model_order = layout_uses_edge_model_order(self.graph);
        for (scope_edge_order, scoped) in scope.edges.iter().enumerate() {
            let source = &self.graph.edges[scoped.original];
            let model_order = uses_edge_model_order.then_some(if scoped.segment.is_some() {
                // A boundary segment inherits its original edge owner's ordinal even when this
                // particular piece is materialized in a descendant scope.
                self.edge_model_order[scoped.original]
            } else {
                // Ordinary edges follow the importer-visible order of this materialized scope.
                scope_edge_order
            });
            let input_edge = ElkInputEdge {
                id: source.id.clone(),
                source: source.source.clone(),
                target: source.target.clone(),
                label: if scoped.carries_label {
                    source
                        .label
                        .map(|label| ElkInputLabel::center("", label.width, label.height))
                } else {
                    None
                },
                minlen: source.minlen,
                inside_self_loops_yo: source.inside_self_loops_yo,
                model_order,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            };
            let segment_metadata = scoped.segment.map(|segment| SegmentedEdgeMetadata {
                segment,
                model_order,
                order: scoped
                    .segment_order
                    .expect("segmented edge must carry its original path order"),
                count: scoped.segment_count,
            });
            edge_metadata.insert(
                source.id.clone(),
                ScopeEdgeMetadata {
                    original: scoped.original,
                    segment: segment_metadata,
                },
            );
            if let Some(segment) = scoped.segment {
                segments.push(source_port::ElkInputEdgeSegment {
                    edge: input_edge,
                    source: self.segment_endpoint(scoped.source),
                    target: self.segment_endpoint(scoped.target),
                    segment,
                    // Model order is local to the owning scope, while external-port identity must
                    // be unique among every segment materialized in this scope. Conflating the two
                    // lets edges with different owners accidentally reuse the same hierarchy port.
                    edge_order: scope_edge_order,
                });
            } else {
                edges.push(input_edge);
            }
        }

        Ok((
            ElkInputGraph {
                id: scope
                    .anchor
                    .map(|node| self.graph.nodes[node].id.clone())
                    .unwrap_or_else(|| self.graph.id.clone()),
                options,
                nodes,
                edges,
            },
            segments,
            edge_metadata,
        ))
    }

    fn segment_endpoint(
        &self,
        endpoint: ScopedEndpoint,
    ) -> source_port::ElkInputEdgeSegmentEndpoint {
        match endpoint {
            ScopedEndpoint::Node(node) => source_port::ElkInputEdgeSegmentEndpoint::Node {
                id: self.graph.nodes[node].id.clone(),
            },
            ScopedEndpoint::ParentBoundary {
                scope,
                connects_node,
            } => {
                let anchor = self.scopes[scope]
                    .anchor
                    .expect("non-root scope boundary must have an anchor");
                source_port::ElkInputEdgeSegmentEndpoint::ParentBoundary {
                    id: self.graph.nodes[anchor].id.clone(),
                    connects_node,
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn order_scope_edges_like_elk_importer(
    scope: &mut ScopePlan,
    scope_index: usize,
    parent: &[Option<usize>],
    children: &[Vec<usize>],
    node_scope: &[usize],
    resolved_handling: &[HierarchyHandling],
    preorder_position: &[usize],
    subtree_size: &[usize],
) {
    if scope.edges.len() < 2 {
        return;
    }

    let local_parent =
        |node: usize| parent[node].filter(|parent| node_scope[*parent] == scope_index);
    let is_ancestor = |ancestor: usize, node: usize| {
        let start = preorder_position[ancestor];
        let end = start + subtree_size[ancestor];
        start <= preorder_position[node] && preorder_position[node] < end
    };
    let containing_parent = |edge: &ScopedEdge| {
        let (ScopedEndpoint::Node(source), ScopedEndpoint::Node(target)) =
            (edge.source, edge.target)
        else {
            return None;
        };
        let source_parent = local_parent(source);
        let target_parent = local_parent(target);
        if source_parent == target_parent {
            source_parent
        } else if is_ancestor(source, target) {
            Some(source)
        } else if is_ancestor(target, source) {
            Some(target)
        } else {
            None
        }
    };

    let mut edges_by_parent: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for (edge, scoped) in scope.edges.iter().enumerate() {
        edges_by_parent
            .entry(containing_parent(scoped))
            .or_default()
            .push(edge);
    }

    // ElkGraphImporter visits the root graph first, then IncludeChildren graphs in stable BFS
    // child order. Preserve that model-order stream before scope-local kernel import.
    let mut order = Vec::with_capacity(scope.edges.len());
    let mut graph_queue = VecDeque::from([None]);
    while let Some(graph_parent) = graph_queue.pop_front() {
        if let Some(edges) = edges_by_parent.remove(&graph_parent) {
            order.extend(edges);
        }
        let graph_children: &[usize] = match graph_parent {
            Some(parent) => &children[parent],
            None => &scope.nodes,
        };
        graph_queue.extend(
            graph_children
                .iter()
                .copied()
                .filter(|child| {
                    node_scope[*child] == scope_index
                        && local_parent(*child) == graph_parent
                        && resolved_handling[*child] == HierarchyHandling::IncludeChildren
                })
                .map(Some),
        );
    }

    let mut slots = std::mem::take(&mut scope.edges)
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    scope.edges.reserve(slots.len());
    for edge in order {
        scope.edges.push(
            slots[edge]
                .take()
                .expect("ELK scope edge order must visit each edge at most once"),
        );
    }
    scope.edges.extend(slots.into_iter().flatten());
}

fn detect_parent_cycles(graph: &Graph, parent: &[Option<usize>]) -> Result<()> {
    let mut state = vec![0u8; graph.nodes.len()];
    for start in 0..graph.nodes.len() {
        if state[start] == 2 {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(node) = current {
            match state[node] {
                2 => break,
                1 => {
                    return Err(source_port::ImportError::ParentCycle {
                        node_id: graph.nodes[start].id.clone(),
                    }
                    .into());
                }
                _ => {
                    state[node] = 1;
                    path.push(node);
                    current = parent[node];
                }
            }
        }
        for node in path {
            state[node] = 2;
        }
    }
    Ok(())
}

fn scope_edge_segments(
    scopes: &[ScopePlan],
    source: usize,
    target: usize,
    source_scope: usize,
    target_scope: usize,
    work_control: &mut dyn WorkControl,
) -> Result<(usize, Vec<ScopeEdgePiece>)> {
    let mut pieces = Vec::new();
    let mut target_pieces = Vec::new();
    let mut current_source_scope = source_scope;
    let mut current_target_scope = target_scope;
    let mut current_source = ScopedEndpoint::Node(source);
    let mut current_target = ScopedEndpoint::Node(target);

    while scopes[current_source_scope].depth > scopes[current_target_scope].depth {
        ascend_source_scope(
            scopes,
            target,
            &mut current_source_scope,
            &mut current_source,
            &mut pieces,
            work_control,
        )?;
    }
    while scopes[current_target_scope].depth > scopes[current_source_scope].depth {
        ascend_target_scope(
            scopes,
            source,
            &mut current_target_scope,
            &mut current_target,
            &mut target_pieces,
            work_control,
        )?;
    }
    while current_source_scope != current_target_scope {
        ascend_source_scope(
            scopes,
            target,
            &mut current_source_scope,
            &mut current_source,
            &mut pieces,
            work_control,
        )?;
        ascend_target_scope(
            scopes,
            source,
            &mut current_target_scope,
            &mut current_target,
            &mut target_pieces,
            work_control,
        )?;
    }
    let owner = current_source_scope;

    if !matches!(
        (current_source, current_target),
        (ScopedEndpoint::Node(left), ScopedEndpoint::Node(right)) if left == right
    ) {
        work_control.check(1)?;
        work_control.charge(1)?;
        pieces.push((
            owner,
            current_source,
            current_target,
            if source_scope != owner {
                source_port::CompoundEdgeSegment::Output {
                    depth: scopes[owner].depth,
                }
            } else {
                source_port::CompoundEdgeSegment::Input {
                    depth: scopes[owner].depth,
                }
            },
        ));
    }
    pieces.extend(target_pieces.into_iter().rev());
    Ok((owner, pieces))
}

fn ascend_source_scope(
    scopes: &[ScopePlan],
    target: usize,
    current_scope: &mut usize,
    current_source: &mut ScopedEndpoint,
    pieces: &mut Vec<ScopeEdgePiece>,
    work_control: &mut dyn WorkControl,
) -> Result<()> {
    let anchor = scopes[*current_scope]
        .anchor
        .expect("non-root scope must have an anchor");
    work_control.check(1)?;
    work_control.charge(1)?;
    pieces.push((
        *current_scope,
        *current_source,
        ScopedEndpoint::ParentBoundary {
            scope: *current_scope,
            connects_node: target == anchor,
        },
        source_port::CompoundEdgeSegment::Output {
            depth: scopes[*current_scope].depth,
        },
    ));
    *current_source = ScopedEndpoint::Node(anchor);
    *current_scope = scopes[*current_scope]
        .parent
        .expect("source scope path must reach owner");
    Ok(())
}

fn ascend_target_scope(
    scopes: &[ScopePlan],
    source: usize,
    current_scope: &mut usize,
    current_target: &mut ScopedEndpoint,
    pieces: &mut Vec<ScopeEdgePiece>,
    work_control: &mut dyn WorkControl,
) -> Result<()> {
    let anchor = scopes[*current_scope]
        .anchor
        .expect("non-root scope must have an anchor");
    work_control.check(1)?;
    work_control.charge(1)?;
    pieces.push((
        *current_scope,
        ScopedEndpoint::ParentBoundary {
            scope: *current_scope,
            connects_node: source == anchor,
        },
        *current_target,
        source_port::CompoundEdgeSegment::Input {
            depth: scopes[*current_scope].depth,
        },
    ));
    *current_target = ScopedEndpoint::Node(anchor);
    *current_scope = scopes[*current_scope]
        .parent
        .expect("target scope path must reach owner");
    Ok(())
}

fn compound_center_segment(pieces: &[ScopeEdgePiece]) -> usize {
    pieces
        .iter()
        .position(|(_, _, _, segment)| {
            matches!(segment, source_port::CompoundEdgeSegment::Input { .. })
        })
        .map(|index| index.saturating_sub(1))
        .or_else(|| pieces.len().checked_sub(1))
        .unwrap_or(0)
}

fn flatten_scope_layouts(
    index: &HierarchyIndex<'_>,
    mut arena: Vec<Option<ScopeLayout>>,
    work_control: &mut dyn WorkControl,
) -> Result<LayoutResult> {
    // Charge the planning scan before reading per-scope output geometry. The detailed tranche is
    // then charged before translation and compound-segment merging touch those points and labels.
    work_control.check(index.scopes.len())?;
    work_control.charge(index.scopes.len())?;
    let scoped_edge_count = index.scopes.iter().try_fold(0usize, |total, scope| {
        total
            .checked_add(scope.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)
    })?;
    let flatten_work = index
        .scopes
        .len()
        .checked_mul(3)
        .and_then(|scope_work| scope_work.checked_add(index.graph.nodes.len()))
        .and_then(|work| work.checked_add(scoped_edge_count))
        .and_then(|work| work.checked_add(index.graph.edges.len().checked_mul(3)?))
        .ok_or(WorkError::ArithmeticOverflow)?;

    work_control.check(scoped_edge_count)?;
    work_control.charge(scoped_edge_count)?;
    let geometry_items = arena.iter().try_fold(0usize, |total, layout| {
        let layout = layout.as_ref().expect("postorder fills every scope result");
        layout.layout.edges.iter().try_fold(total, |total, edge| {
            total
                .checked_add(edge.points.len())
                .and_then(|total| total.checked_add(edge.labels.len()))
                .and_then(|total| total.checked_add(1))
                .ok_or(WorkError::ArithmeticOverflow)
        })
    })?;
    let geometry_work = geometry_items
        .checked_mul(2)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let execution_work = flatten_work
        .checked_add(geometry_work)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work_control.check(execution_work)?;
    work_control.charge(execution_work)?;

    let mut offsets = vec![Point { x: 0.0, y: 0.0 }; index.scopes.len()];
    let mut order = Vec::with_capacity(index.scopes.len());
    let mut stack = vec![0usize];
    while let Some(scope) = stack.pop() {
        order.push(scope);
        let layout = &arena[scope]
            .as_ref()
            .expect("scope layout exists before flatten")
            .layout;
        let child_by_id = index.scopes[scope]
            .children
            .iter()
            .map(|child| {
                let anchor = index.scopes[*child]
                    .anchor
                    .expect("child scope must have an anchor");
                (index.graph.nodes[anchor].id.as_str(), *child)
            })
            .collect::<HashMap<_, _>>();
        let mut children = Vec::new();
        for node in &layout.nodes {
            let Some(&child) = child_by_id.get(node.id.as_str()) else {
                continue;
            };
            offsets[child] = Point {
                // Mermaid accumulates nested coordinates from group top-left positions, while the
                // public Merman node layout is center-based.
                x: offsets[scope].x + node.x - node.width / 2.0,
                y: offsets[scope].y + node.y - node.height / 2.0,
            };
            children.push(child);
        }
        stack.extend(children.into_iter().rev());
    }
    let node_capacity = arena.iter().try_fold(0usize, |total, layout| {
        total
            .checked_add(
                layout
                    .as_ref()
                    .expect("postorder fills every scope result")
                    .layout
                    .nodes
                    .len(),
            )
            .ok_or(WorkError::ArithmeticOverflow)
    })?;
    let mut nodes = Vec::with_capacity(node_capacity);
    let mut edge_slots = std::iter::repeat_with(|| None)
        .take(index.graph.edges.len())
        .collect::<Vec<Option<EdgeLayout>>>();
    let mut compound_edges = std::iter::repeat_with(|| None)
        .take(index.graph.edges.len())
        .collect::<Vec<Option<Vec<Option<CompoundEdgeLayoutSegment>>>>>();
    for &scope in &order {
        let offset = offsets[scope];
        let mut scope_layout = arena[scope].take().expect("each scope is flattened once");
        for node in &mut scope_layout.layout.nodes {
            node.x += offset.x;
            node.y += offset.y;
        }
        nodes.append(&mut scope_layout.layout.nodes);
        for mut edge in scope_layout.layout.edges {
            translate_edge_layout(&mut edge, offset);
            let metadata = scope_layout
                .edge_metadata
                .get(edge.id.as_str())
                .copied()
                .expect("scope output edge must retain its source edge metadata");
            if let Some(segment) = metadata.segment {
                let entry = compound_edges[metadata.original]
                    .get_or_insert_with(|| vec![None; segment.count]);
                debug_assert_eq!(entry.len(), segment.count);
                entry[segment.order] = Some(CompoundEdgeLayoutSegment {
                    original_edge_id: edge.id.clone(),
                    segment: segment.segment,
                    model_order: segment.model_order,
                    edge,
                });
            } else {
                debug_assert!(edge_slots[metadata.original].is_none());
                edge_slots[metadata.original] = Some(edge);
            }
        }
    }

    for (original, segments) in compound_edges.into_iter().enumerate() {
        let Some(segments) = segments else {
            continue;
        };
        let segments = segments.into_iter().flatten().collect::<Vec<_>>();
        if let Some(edge) = merge_compound_edge_segments(
            &segments,
            index.graph.options.layered.unnecessary_bendpoints,
        ) {
            debug_assert!(edge_slots[original].is_none());
            edge_slots[original] = Some(edge);
        }
    }

    // ELK derives importer/model order from an edge's containing graph before moving an
    // ancestor-descendant edge into that endpoint's inner graph. A direct parent-child edge can
    // therefore be owned by the parent scope without materializing a segment there. Emit from
    // `owned_edges`, not first-seen segment order; the nested iteration also avoids an O(E) copy.
    let mut edges = Vec::with_capacity(index.graph.edges.len());
    for &scope in &order {
        for &original in &index.scopes[scope].owned_edges {
            if let Some(edge) = edge_slots[original].take() {
                edges.push(edge);
            }
        }
    }
    // Valid adapter inputs should have announced every rendered edge from its owner scope. Keep a
    // deterministic tail in release builds if a future source processor suppresses that marker.
    edges.extend(edge_slots.into_iter().flatten());
    Ok(LayoutResult { nodes, edges })
}

fn translate_edge_layout(edge: &mut EdgeLayout, offset: Point) {
    for point in &mut edge.points {
        point.x += offset.x;
        point.y += offset.y;
    }
    for label in &mut edge.labels {
        label.x += offset.x;
        label.y += offset.y;
    }
}

fn source_graph_requires_compound_pipeline(graph: &LGraph) -> bool {
    graph.graph_properties.external_ports
        || !graph.hierarchy_edges.is_empty()
        || !graph.cross_hierarchy_edges.is_empty()
        || graph
            .layerless_nodes
            .iter()
            .any(|node| node.nested_graph.is_some())
}

fn actual_source_graph_size(graph: &LGraph) -> source_port::LSize {
    source_port::LSize {
        width: graph.size.width + graph.padding.left + graph.padding.right,
        height: graph.size.height + graph.padding.top + graph.padding.bottom,
    }
}

fn source_graph_output_work_units(graph: &LGraph) -> std::result::Result<usize, WorkError> {
    let mut total = 0usize;
    let mut output_edge_upper_bound = 0usize;
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        // A merged hierarchy edge may expand back into several original rendered edges. Source
        // edges plus explicit cross-hierarchy records conservatively bound the final stable sort.
        output_edge_upper_bound = output_edge_upper_bound
            .checked_add(current.edges.len())
            .and_then(|total| total.checked_add(current.cross_hierarchy_edges.len()))
            .ok_or(WorkError::ArithmeticOverflow)?;
        total = total
            .checked_add(current.layerless_nodes.len())
            .and_then(|total| total.checked_add(current.edges.len()))
            .and_then(|total| total.checked_add(current.cross_hierarchy_edges.len()))
            .ok_or(WorkError::ArithmeticOverflow)?;
        for edge in &current.edges {
            let geometry = edge
                .bend_points
                .len()
                .checked_add(edge.labels.len())
                .and_then(|geometry| geometry.checked_add(3))
                .ok_or(WorkError::ArithmeticOverflow)?;
            total = total
                .checked_add(geometry)
                .ok_or(WorkError::ArithmeticOverflow)?;
            if edge.compound_segment.is_some() {
                total = total
                    .checked_add(
                        geometry
                            .checked_mul(2)
                            .ok_or(WorkError::ArithmeticOverflow)?,
                    )
                    .ok_or(WorkError::ArithmeticOverflow)?;
            }
        }
        for segment in &current.cross_hierarchy_edges {
            let edge = current
                .edges
                .get(segment.edge)
                .expect("compound segment must reference a graph edge");
            let geometry = edge
                .bend_points
                .len()
                .checked_add(edge.labels.len())
                .and_then(|geometry| geometry.checked_add(3))
                .and_then(|geometry| geometry.checked_mul(2))
                .ok_or(WorkError::ArithmeticOverflow)?;
            total = total
                .checked_add(geometry)
                .ok_or(WorkError::ArithmeticOverflow)?;
        }
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }
    total = total
        .checked_add(comparison_sort_work_units(output_edge_upper_bound)?)
        .ok_or(WorkError::ArithmeticOverflow)?;
    Ok(total.max(1))
}

fn comparison_sort_work_units(items: usize) -> std::result::Result<usize, WorkError> {
    if items < 2 {
        return Ok(0);
    }
    let levels = usize::BITS as usize - (items - 1).leading_zeros() as usize;
    items
        .checked_mul(levels)
        .ok_or(WorkError::ArithmeticOverflow)
}

fn write_source_graph_dump(output: &mut String, graph: &LGraph, depth: usize) -> std::fmt::Result {
    let mut stack = vec![(graph, depth)];
    while let Some((current, current_depth)) = stack.pop() {
        write_source_graph_dump_frame(output, current, current_depth)?;
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .rev()
                .filter_map(|node| node.nested_graph.as_deref())
                .map(|nested| (nested, current_depth + 1)),
        );
    }
    Ok(())
}

fn write_source_graph_dump_frame(
    output: &mut String,
    graph: &LGraph,
    depth: usize,
) -> std::fmt::Result {
    let indent = "  ".repeat(depth);
    writeln!(
        output,
        "{indent}graph {} parent={:?} size=({}, {}) offset=({}, {}) padding=({}, {}, {}, {})",
        graph.id,
        graph.parent_node_id,
        graph.size.width,
        graph.size.height,
        graph.offset.x,
        graph.offset.y,
        graph.padding.left,
        graph.padding.right,
        graph.padding.top,
        graph.padding.bottom
    )?;
    writeln!(
        output,
        "{indent}options direction={:?} port_constraints={:?} thoroughness={} hierarchical_sweepiness={} consider_model_order={:?} force_node_model_order={} port_model_order={}",
        graph.options.direction,
        graph.options.port_constraints,
        graph.options.thoroughness,
        graph.options.hierarchical_sweepiness,
        graph.options.consider_model_order_strategy,
        graph.options.force_node_model_order,
        graph.options.consider_model_order_port_model_order
    )?;
    writeln!(output, "{indent}layerless:")?;
    for (index, node) in graph.layerless_nodes.iter().enumerate() {
        writeln!(
            output,
            "{indent}- #{index} {} kind={:?} layer={:?} order={:?} pos=({}, {}) size=({}, {}) margin=({}, {}, {}, {}) port_constraints={:?} parent_graph={}",
            node.id,
            node.kind,
            node.layer_index,
            node.model_order,
            node.position.x,
            node.position.y,
            node.size.width,
            node.size.height,
            node.margin.left,
            node.margin.right,
            node.margin.top,
            node.margin.bottom,
            node.port_constraints,
            node.nested_graph.is_some()
        )?;
        if node.kind == LNodeKind::ExternalPort {
            writeln!(
                output,
                "{indent}  external side={:?} size=({}, {}) ratio_or_position={} replaced={:?}",
                node.external_port_side,
                node.external_port_size.width,
                node.external_port_size.height,
                node.port_ratio_or_position,
                node.replaced_external_port_dummy
            )?;
        }
        for (port_index, port) in node.ports.iter().enumerate() {
            let incoming = port
                .incoming_edges
                .iter()
                .map(|edge| graph.edges[*edge].id.as_str())
                .collect::<Vec<_>>();
            let outgoing = port
                .outgoing_edges
                .iter()
                .map(|edge| graph.edges[*edge].id.as_str())
                .collect::<Vec<_>>();
            writeln!(
                output,
                "{indent}  port #{port_index} {} type={:?} side={:?} order={:?} index={:?} pos=({}, {}) anchor=({}, {}) size=({}, {}) border={:?} inside={} dummy={:?} origin={:?} in=[{}] out=[{}]",
                port.id,
                port.port_type,
                port.side,
                port.model_order,
                port.port_index,
                port.position.x,
                port.position.y,
                port.anchor.x,
                port.anchor.y,
                port.size.width,
                port.size.height,
                port.border_offset,
                port.inside_connections,
                port.port_dummy,
                node.origin_port,
                incoming.join(","),
                outgoing.join(",")
            )?;
        }
    }
    if !graph.edges.is_empty() {
        writeln!(output, "{indent}edges:")?;
        for (edge_index, edge) in graph.edges.iter().enumerate() {
            let source_attached = graph.edge_source_attached(edge_index);
            let target_attached = graph.edge_target_attached(edge_index);
            writeln!(
                output,
                "{indent}- #{edge_index} {} {}:{} -> {}:{} segment={:?} reversed={} attached=({source_attached},{target_attached}) bends={:?}",
                edge.id,
                edge.source.node,
                edge.source.port,
                edge.target.node,
                edge.target.port,
                edge.compound_segment,
                edge.reversed,
                edge.bend_points
            )?;
        }
    }
    writeln!(output, "{indent}layers:")?;
    for (index, layer) in graph.layers.iter().enumerate() {
        let nodes = layer
            .nodes
            .iter()
            .map(|node| {
                let lnode = &graph.layerless_nodes[*node];
                format!(
                    "{}#{node}[{:?},order={:?},pos=({},{}),size=({},{})]",
                    lnode.id,
                    lnode.kind,
                    lnode.model_order,
                    lnode.position.x,
                    lnode.position.y,
                    lnode.size.width,
                    lnode.size.height
                )
            })
            .collect::<Vec<_>>();
        writeln!(
            output,
            "{indent}- layer {index} size=({}, {}) nodes={}",
            layer.size.width,
            layer.size.height,
            nodes.join(" -> ")
        )?;
    }
    writeln!(output)?;

    Ok(())
}

fn layered_options_to_source(graph: &Graph) -> SourceLayeredOptions {
    layered_options_to_source_for(
        graph,
        graph.direction,
        graph.options.layered.hierarchy_handling,
    )
}

fn layered_options_to_source_for(
    graph: &Graph,
    direction: Direction,
    hierarchy_handling: HierarchyHandling,
) -> SourceLayeredOptions {
    let mut options =
        SourceLayeredOptions::mermaid_flowchart_defaults(direction_to_source(direction));
    options.random_seed = graph.options.layered.random_seed;
    options.hierarchy_handling = hierarchy_handling_to_source(hierarchy_handling);
    options.edge_routing = edge_routing_to_source(graph.options.layered.edge_routing);
    options.cycle_breaking_strategy =
        cycle_breaking_to_source(graph.options.layered.cycle_breaking);
    options.node_placement_strategy =
        node_placement_to_source(graph.options.layered.node_placement);
    options.node_placement_bk_fixed_alignment =
        node_placement_alignment_to_source(graph.options.layered.node_placement_alignment);
    options.consider_model_order_strategy = if graph.options.layered.consider_model_order {
        model_order_to_source(graph.options.layered.model_order)
    } else {
        source_port::OrderingStrategy::None
    };
    options.force_node_model_order = graph.options.layered.force_node_model_order;
    options.merge_edges = graph.options.layered.merge_edges;
    options.merge_hierarchy_edges = graph.options.layered.merge_hierarchy_edges;
    options.unnecessary_bendpoints = graph.options.layered.unnecessary_bendpoints;
    options.inside_self_loops_activate = graph.options.layered.inside_self_loops_activate;
    options.self_loop_distribution =
        self_loop_distribution_to_source(graph.options.layered.self_loop_distribution);
    options.self_loop_ordering =
        self_loop_ordering_to_source(graph.options.layered.self_loop_ordering);
    options
}

fn layout_uses_edge_model_order(graph: &Graph) -> bool {
    graph.options.layered.consider_model_order
        || graph.options.layered.force_node_model_order
        || matches!(
            graph.options.layered.cycle_breaking,
            CycleBreakingStrategy::ModelOrder | CycleBreakingStrategy::GreedyModelOrder
        )
}

fn direction_to_source(direction: Direction) -> ElkDirection {
    match direction {
        Direction::Left => ElkDirection::Left,
        Direction::Right => ElkDirection::Right,
        Direction::Up => ElkDirection::Up,
        Direction::Down => ElkDirection::Down,
    }
}

fn hierarchy_handling_to_source(
    hierarchy_handling: HierarchyHandling,
) -> source_port::HierarchyHandling {
    match hierarchy_handling {
        HierarchyHandling::IncludeChildren => source_port::HierarchyHandling::IncludeChildren,
        HierarchyHandling::SeparateChildren => source_port::HierarchyHandling::SeparateChildren,
    }
}

fn edge_routing_to_source(edge_routing: EdgeRouting) -> source_port::EdgeRouting {
    match edge_routing {
        EdgeRouting::Orthogonal => source_port::EdgeRouting::Orthogonal,
        EdgeRouting::Polyline => source_port::EdgeRouting::Polyline,
    }
}

fn cycle_breaking_to_source(
    cycle_breaking: CycleBreakingStrategy,
) -> source_port::CycleBreakingStrategy {
    match cycle_breaking {
        CycleBreakingStrategy::Greedy => source_port::CycleBreakingStrategy::Greedy,
        CycleBreakingStrategy::DepthFirst => source_port::CycleBreakingStrategy::DepthFirst,
        CycleBreakingStrategy::Interactive => source_port::CycleBreakingStrategy::Interactive,
        CycleBreakingStrategy::ModelOrder => source_port::CycleBreakingStrategy::ModelOrder,
        CycleBreakingStrategy::GreedyModelOrder => {
            source_port::CycleBreakingStrategy::GreedyModelOrder
        }
    }
}

fn node_placement_to_source(
    node_placement: NodePlacementStrategy,
) -> source_port::NodePlacementStrategy {
    match node_placement {
        NodePlacementStrategy::Simple => source_port::NodePlacementStrategy::Simple,
        NodePlacementStrategy::NetworkSimplex => source_port::NodePlacementStrategy::NetworkSimplex,
        NodePlacementStrategy::LinearSegments => source_port::NodePlacementStrategy::LinearSegments,
        NodePlacementStrategy::BrandesKoepf => source_port::NodePlacementStrategy::BrandesKoepf,
    }
}

fn node_placement_alignment_to_source(
    alignment: NodePlacementAlignment,
) -> source_port::FixedAlignment {
    match alignment {
        NodePlacementAlignment::None => source_port::FixedAlignment::None,
        NodePlacementAlignment::LeftUp => source_port::FixedAlignment::LeftUp,
        NodePlacementAlignment::RightUp => source_port::FixedAlignment::RightUp,
        NodePlacementAlignment::LeftDown => source_port::FixedAlignment::LeftDown,
        NodePlacementAlignment::RightDown => source_port::FixedAlignment::RightDown,
        NodePlacementAlignment::Balanced => source_port::FixedAlignment::Balanced,
    }
}

fn layer_constraint_to_source(layer_constraint: LayerConstraint) -> source_port::LayerConstraint {
    match layer_constraint {
        LayerConstraint::First => source_port::LayerConstraint::First,
        LayerConstraint::FirstSeparate => source_port::LayerConstraint::FirstSeparate,
        LayerConstraint::Last => source_port::LayerConstraint::Last,
        LayerConstraint::LastSeparate => source_port::LayerConstraint::LastSeparate,
    }
}

fn model_order_to_source(model_order: ModelOrderStrategy) -> source_port::OrderingStrategy {
    match model_order {
        ModelOrderStrategy::None => source_port::OrderingStrategy::None,
        ModelOrderStrategy::NodesAndEdges => source_port::OrderingStrategy::NodesAndEdges,
        ModelOrderStrategy::PreferEdges => source_port::OrderingStrategy::PreferEdges,
        ModelOrderStrategy::PreferNodes => source_port::OrderingStrategy::PreferNodes,
    }
}

fn self_loop_distribution_to_source(
    self_loop_distribution: SelfLoopDistributionStrategy,
) -> source_port::SelfLoopDistributionStrategy {
    match self_loop_distribution {
        SelfLoopDistributionStrategy::North => source_port::SelfLoopDistributionStrategy::North,
        SelfLoopDistributionStrategy::Equally => source_port::SelfLoopDistributionStrategy::Equally,
        SelfLoopDistributionStrategy::NorthSouth => {
            source_port::SelfLoopDistributionStrategy::NorthSouth
        }
    }
}

fn self_loop_ordering_to_source(
    self_loop_ordering: SelfLoopOrderingStrategy,
) -> source_port::SelfLoopOrderingStrategy {
    match self_loop_ordering {
        SelfLoopOrderingStrategy::Stacked => source_port::SelfLoopOrderingStrategy::Stacked,
        SelfLoopOrderingStrategy::ReverseStacked => {
            source_port::SelfLoopOrderingStrategy::ReverseStacked
        }
        SelfLoopOrderingStrategy::Sequenced => source_port::SelfLoopOrderingStrategy::Sequenced,
    }
}

fn source_graph_to_layout_result(graph: &LGraph) -> LayoutResult {
    source_graph_to_layout_result_with_ordering(graph, true)
}

fn source_graph_to_layout_result_with_ordering(graph: &LGraph, order_edges: bool) -> LayoutResult {
    let mut result = SourceLayoutAccumulator {
        add_unnecessary_bendpoints: graph.options.unnecessary_bendpoints,
        ..Default::default()
    };
    append_source_graph_layout(graph, LPoint::default(), &mut result);
    result.into_layout_result(order_edges)
}

#[derive(Debug, Default)]
struct SourceLayoutAccumulator {
    nodes: Vec<NodeLayout>,
    node_ids: HashSet<String>,
    edges: Vec<OrderedEdgeLayout>,
    compound_edges: HashMap<String, Vec<CompoundEdgeLayoutSegment>>,
    add_unnecessary_bendpoints: bool,
}

impl SourceLayoutAccumulator {
    fn into_layout_result(mut self, order_edges: bool) -> LayoutResult {
        for segments in self.compound_edges.values_mut() {
            order_compound_layout_segments(segments);
            if let Some(edge) =
                merge_compound_edge_segments(segments, self.add_unnecessary_bendpoints)
            {
                self.edges.push(OrderedEdgeLayout {
                    model_order: segments
                        .iter()
                        .filter_map(|segment| segment.model_order)
                        .min(),
                    edge,
                });
            }
        }
        if order_edges {
            self.edges.sort_by(|left, right| {
                left.model_order
                    .unwrap_or(usize::MAX)
                    .cmp(&right.model_order.unwrap_or(usize::MAX))
                    .then_with(|| left.edge.id.cmp(&right.edge.id))
            });
        }

        LayoutResult {
            nodes: self.nodes,
            edges: self.edges.into_iter().map(|ordered| ordered.edge).collect(),
        }
    }
}

fn order_compound_layout_segments(segments: &mut Vec<CompoundEdgeLayoutSegment>) {
    if segments.len() < 2 {
        return;
    }

    // ELK's compound postprocessor orders Output segments by descending hierarchy depth, then
    // Input segments by ascending depth before concatenation. Imported paths make each side's
    // depths unique and contiguous, so direct placement preserves that order without O(n log n)
    // comparison sorting.
    let mut output_min = usize::MAX;
    let mut output_max = 0usize;
    let mut output_count = 0usize;
    let mut input_min = usize::MAX;
    let mut input_max = 0usize;
    let mut input_count = 0usize;
    for segment in segments.iter() {
        match segment.segment {
            source_port::CompoundEdgeSegment::Output { depth } => {
                output_min = output_min.min(depth);
                output_max = output_max.max(depth);
                output_count += 1;
            }
            source_port::CompoundEdgeSegment::Input { depth } => {
                input_min = input_min.min(depth);
                input_max = input_max.max(depth);
                input_count += 1;
            }
        }
    }

    let output_span = if output_count == 0 {
        0
    } else {
        output_max - output_min + 1
    };
    let input_span = if input_count == 0 {
        0
    } else {
        input_max - input_min + 1
    };
    assert_eq!(
        output_span, output_count,
        "compound output segments must have unique contiguous depths"
    );
    assert_eq!(
        input_span, input_count,
        "compound input segments must have unique contiguous depths"
    );

    let mut ordered = std::iter::repeat_with(|| None)
        .take(segments.len())
        .collect::<Vec<Option<CompoundEdgeLayoutSegment>>>();
    for segment in std::mem::take(segments) {
        let position = match segment.segment {
            source_port::CompoundEdgeSegment::Output { depth } => output_max - depth,
            source_port::CompoundEdgeSegment::Input { depth } => output_count + depth - input_min,
        };
        assert!(
            ordered[position].replace(segment).is_none(),
            "compound segment path position must be unique"
        );
    }
    segments.extend(
        ordered
            .into_iter()
            .map(|segment| segment.expect("compound segment path must be contiguous")),
    );
}

#[derive(Debug, Clone)]
struct OrderedEdgeLayout {
    model_order: Option<usize>,
    edge: EdgeLayout,
}

fn append_source_graph_layout(
    graph: &LGraph,
    parent_origin: LPoint,
    result: &mut SourceLayoutAccumulator,
) {
    enum Frame<'a> {
        Enter {
            graph: &'a LGraph,
            parent_origin: LPoint,
        },
        Exit {
            graph: &'a LGraph,
            graph_origin: LPoint,
        },
    }

    let mut frames = vec![Frame::Enter {
        graph,
        parent_origin,
    }];
    while let Some(frame) = frames.pop() {
        let (graph, parent_origin) = match frame {
            Frame::Enter {
                graph,
                parent_origin,
            } => (graph, parent_origin),
            Frame::Exit {
                graph,
                graph_origin,
            } => {
                append_source_graph_edges(graph, graph_origin, result);
                continue;
            }
        };
        let graph_origin = LPoint {
            x: parent_origin.x + graph.offset.x + graph.padding.left,
            y: parent_origin.y + graph.offset.y + graph.padding.top,
        };
        for node in graph
            .layerless_nodes
            .iter()
            .filter(|node| node.kind == LNodeKind::Normal)
        {
            result.node_ids.insert(node.id.clone());
            result.nodes.push(NodeLayout {
                id: node.id.clone(),
                x: graph_origin.x + node.position.x + node.size.width / 2.0,
                y: graph_origin.y + node.position.y + node.size.height / 2.0,
                width: node.size.width,
                height: node.size.height,
            });
        }
        frames.push(Frame::Exit {
            graph,
            graph_origin,
        });
        frames.extend(graph.layerless_nodes.iter().rev().filter_map(|node| {
            node.nested_graph
                .as_deref()
                .map(|nested_graph| Frame::Enter {
                    graph: nested_graph,
                    parent_origin: LPoint {
                        x: graph_origin.x + node.position.x,
                        y: graph_origin.y + node.position.y,
                    },
                })
        }));
    }
}

fn append_source_graph_edges(
    graph: &LGraph,
    graph_origin: LPoint,
    result: &mut SourceLayoutAccumulator,
) {
    let mut compound_segments_by_edge: HashMap<usize, Vec<CompoundLayoutSegment>> = HashMap::new();
    for segment in &graph.cross_hierarchy_edges {
        compound_segments_by_edge
            .entry(segment.edge)
            .or_default()
            .push(CompoundLayoutSegment {
                original_edge_id: segment.original_edge_id.clone(),
                model_order: segment.original_model_order,
                segment: segment.segment,
            });
    }

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        let compound_segments = compound_segments_by_edge
            .remove(&edge_index)
            .or_else(|| {
                edge.compound_segment.map(|segment| {
                    vec![CompoundLayoutSegment {
                        original_edge_id: edge.id.clone(),
                        model_order: edge.model_order,
                        segment,
                    }]
                })
            })
            .unwrap_or_default();
        if compound_segments.is_empty() {
            if !edge_has_layout_endpoints(graph, result, edge_index, edge) {
                continue;
            }
        } else if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index)
        {
            continue;
        }

        let edge_layout = EdgeLayout {
            id: edge.id.clone(),
            points: edge_points(graph, edge)
                .into_iter()
                .map(|point| Point {
                    x: graph_origin.x + point.x,
                    y: graph_origin.y + point.y,
                })
                .collect(),
            labels: edge_labels(graph_origin, edge),
        };

        if !compound_segments.is_empty() {
            for segment in compound_segments {
                let original_edge_id = segment.original_edge_id.clone();
                let edge_layout = edge_layout_for_original_edge(
                    &edge_layout,
                    graph_origin,
                    edge,
                    original_edge_id.as_str(),
                );
                result
                    .compound_edges
                    .entry(original_edge_id)
                    .or_default()
                    .push(CompoundEdgeLayoutSegment {
                        original_edge_id: segment.original_edge_id,
                        segment: segment.segment,
                        model_order: segment.model_order.or(edge.model_order),
                        edge: edge_layout,
                    });
            }
        } else {
            result.edges.push(OrderedEdgeLayout {
                model_order: edge.model_order,
                edge: edge_layout,
            });
        }
    }
}

fn edge_layout_for_original_edge(
    edge: &EdgeLayout,
    graph_origin: LPoint,
    source_edge: &source_port::LayeredEdge,
    original_edge_id: &str,
) -> EdgeLayout {
    let mut edge = edge.clone();
    edge.id = original_edge_id.to_string();
    edge.labels = edge_labels_for_original_edge(graph_origin, source_edge, original_edge_id);
    edge
}

fn edge_labels_for_original_edge(
    graph_origin: LPoint,
    edge: &source_port::LayeredEdge,
    original_edge_id: &str,
) -> Vec<EdgeLabelLayout> {
    edge.labels
        .iter()
        .filter(|label| {
            label
                .original_label_edge
                .as_deref()
                .unwrap_or(original_edge_id)
                == original_edge_id
        })
        .map(|label| EdgeLabelLayout {
            x: graph_origin.x + label.position.x,
            y: graph_origin.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        })
        .collect()
}

#[derive(Debug, Clone)]
struct CompoundLayoutSegment {
    original_edge_id: String,
    model_order: Option<usize>,
    segment: source_port::CompoundEdgeSegment,
}

#[derive(Debug, Clone)]
struct CompoundEdgeLayoutSegment {
    original_edge_id: String,
    segment: source_port::CompoundEdgeSegment,
    model_order: Option<usize>,
    edge: EdgeLayout,
}

/// Merge hierarchy-local edge segments following ELK's compound postprocessor.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPostprocessor.java
fn merge_compound_edge_segments(
    segments: &[CompoundEdgeLayoutSegment],
    add_unnecessary_bendpoints: bool,
) -> Option<EdgeLayout> {
    let first = segments.first()?;
    let mut points = Vec::new();
    let mut labels = Vec::new();
    let mut last_point = None;

    if let Some(source) = first.edge.points.first().copied() {
        push_distinct_point(&mut points, source);
    }

    for segment in segments {
        let segment_points = &segment.edge.points;
        if segment_points.is_empty() {
            continue;
        }

        let bend_points = if segment_points.len() > 2 {
            &segment_points[1..segment_points.len() - 1]
        } else {
            &[][..]
        };

        if let (Some(previous), Some(next)) = (
            last_point,
            bend_points.first().or_else(|| segment_points.last()),
        ) && compound_boundary_needs_bendpoint(previous, *next, add_unnecessary_bendpoints)
            && let Some(source) = segment_points.first()
        {
            push_distinct_point(&mut points, *source);
        }

        for point in bend_points {
            push_distinct_point(&mut points, *point);
        }
        labels.extend(segment.edge.labels.iter().cloned());
        last_point = bend_points
            .last()
            .copied()
            .or_else(|| segment_points.first().copied());
    }

    if let Some(target) = segments
        .last()
        .and_then(|segment| segment.edge.points.last())
        .copied()
    {
        push_distinct_point(&mut points, target);
    }

    Some(EdgeLayout {
        id: first.original_edge_id.clone(),
        points,
        labels,
    })
}

fn compound_boundary_needs_bendpoint(
    previous: Point,
    next: Point,
    add_unnecessary_bendpoints: bool,
) -> bool {
    const ORTHOGONAL_TOLERANCE: f64 = 0.001;
    let x_diff_enough = (previous.x - next.x).abs() > ORTHOGONAL_TOLERANCE;
    let y_diff_enough = (previous.y - next.y).abs() > ORTHOGONAL_TOLERANCE;
    if add_unnecessary_bendpoints {
        x_diff_enough || y_diff_enough
    } else {
        x_diff_enough && y_diff_enough
    }
}

fn push_distinct_point(points: &mut Vec<Point>, point: Point) {
    if points.last().is_some_and(|last| *last == point) {
        return;
    }
    points.push(point);
}

fn edge_has_layout_endpoints(
    graph: &LGraph,
    result: &SourceLayoutAccumulator,
    edge_index: usize,
    edge: &source_port::LayeredEdge,
) -> bool {
    if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index) {
        return false;
    }

    endpoint_has_layout(graph, result, edge.source, edge.source_node_id.as_str())
        && endpoint_has_layout(graph, result, edge.target, edge.target_node_id.as_str())
}

fn endpoint_has_layout(
    graph: &LGraph,
    result: &SourceLayoutAccumulator,
    endpoint: PortRef,
    original_node_id: &str,
) -> bool {
    graph
        .layerless_nodes
        .get(endpoint.node)
        .is_some_and(|node| node.kind == LNodeKind::Normal)
        || result.node_ids.contains(original_node_id)
}

fn edge_points(graph: &LGraph, edge: &source_port::LayeredEdge) -> Vec<source_port::LPoint> {
    let mut points = Vec::with_capacity(edge.bend_points.len() + 2);
    points.push(port_anchor(graph, edge.source));
    points.extend(edge.bend_points.iter().copied());
    points.push(port_anchor(graph, edge.target));
    points
}

fn edge_labels(graph_origin: LPoint, edge: &source_port::LayeredEdge) -> Vec<EdgeLabelLayout> {
    edge.labels
        .iter()
        .map(|label| EdgeLabelLayout {
            x: graph_origin.x + label.position.x,
            y: graph_origin.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        })
        .collect()
}

fn port_anchor(graph: &LGraph, port_ref: PortRef) -> source_port::LPoint {
    let node = &graph.layerless_nodes[port_ref.node];
    let port = &node.ports[port_ref.port];
    source_port::LPoint {
        x: node.position.x + port.position.x + port.anchor.x,
        y: node.position.y + port.position.y + port.anchor.y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Leaf,
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> Edge {
        Edge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
            inside_self_loops_yo: false,
        }
    }

    fn flat_graph(nodes: Vec<Node>, edges: Vec<Edge>) -> Graph {
        Graph {
            id: "root".to_string(),
            direction: Direction::Down,
            nodes,
            edges,
            ..Default::default()
        }
    }

    fn group(id: &str, parent: Option<&str>, handling: HierarchyHandling) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Group,
            width: 0.0,
            height: 0.0,
            parent: parent.map(str::to_string),
            direction: None,
            hierarchy_handling: Some(handling),
            layer_constraint: None,
            label: Some(Label {
                width: 24.0,
                height: 18.0,
            }),
        }
    }

    fn deep_separate_graph(depth: usize) -> Graph {
        let mut nodes = Vec::with_capacity(depth + 1);
        for index in 0..depth {
            nodes.push(group(
                format!("group-{index}").as_str(),
                index
                    .checked_sub(1)
                    .map(|parent| format!("group-{parent}"))
                    .as_deref(),
                HierarchyHandling::SeparateChildren,
            ));
        }
        let mut terminal = leaf("terminal");
        terminal.parent = depth.checked_sub(1).map(|parent| format!("group-{parent}"));
        nodes.push(terminal);

        flat_graph(nodes, Vec::new())
    }

    fn deep_include_graph(depth: usize) -> Graph {
        let mut graph = deep_separate_graph(depth);
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;
        for node in &mut graph.nodes {
            if node.kind == NodeKind::Group {
                node.hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
            }
        }
        graph
    }

    fn repeated_cross_scope_graph(edge_count: usize) -> Graph {
        let mut child = leaf("child");
        child.parent = Some("group".to_string());
        let edges = (0..edge_count)
            .map(|index| {
                let mut edge = edge(format!("cross-{index}").as_str(), "child", "outer");
                edge.label = Some(Label {
                    width: 20.0,
                    height: 10.0,
                });
                edge
            })
            .collect();
        flat_graph(
            vec![
                group("group", None, HierarchyHandling::SeparateChildren),
                child,
                leaf("outer"),
            ],
            edges,
        )
    }

    fn deep_cross_scope_graph(depth: usize, edge_count: usize) -> Graph {
        let mut graph = deep_separate_graph(depth);
        graph.options.layered.hierarchy_handling = HierarchyHandling::SeparateChildren;
        graph.nodes.push(leaf("outer"));
        graph.edges = (0..edge_count)
            .map(|index| edge(format!("deep-cross-{index}").as_str(), "terminal", "outer"))
            .collect();
        graph
    }

    fn synthetic_scope_arena(
        index: &HierarchyIndex<'_>,
        points_per_segment: usize,
    ) -> Vec<Option<ScopeLayout>> {
        index
            .scopes
            .iter()
            .map(|scope| {
                let nodes = scope
                    .nodes
                    .iter()
                    .map(|node| {
                        let node = &index.graph.nodes[*node];
                        NodeLayout {
                            id: node.id.clone(),
                            x: 50.0,
                            y: 50.0,
                            width: node.width.max(1.0),
                            height: node.height.max(1.0),
                        }
                    })
                    .collect();
                let mut edge_metadata = HashMap::with_capacity(scope.edges.len());
                let edges = scope
                    .edges
                    .iter()
                    .map(|scoped| {
                        let source = &index.graph.edges[scoped.original];
                        let segment = scoped.segment.map(|segment| SegmentedEdgeMetadata {
                            segment,
                            model_order: Some(index.edge_model_order[scoped.original]),
                            order: scoped
                                .segment_order
                                .expect("synthetic segmented edge keeps path order"),
                            count: scoped.segment_count,
                        });
                        edge_metadata.insert(
                            source.id.clone(),
                            ScopeEdgeMetadata {
                                original: scoped.original,
                                segment,
                            },
                        );
                        EdgeLayout {
                            id: source.id.clone(),
                            points: (0..points_per_segment)
                                .map(|point| Point {
                                    x: point as f64,
                                    y: point as f64,
                                })
                                .collect(),
                            labels: scoped
                                .carries_label
                                .then_some(EdgeLabelLayout {
                                    x: 1.0,
                                    y: 1.0,
                                    width: 20.0,
                                    height: 10.0,
                                })
                                .into_iter()
                                .collect(),
                        }
                    })
                    .collect();
                Some(ScopeLayout {
                    layout: LayoutResult { nodes, edges },
                    size: source_port::LSize {
                        width: 100.0,
                        height: 100.0,
                    },
                    edge_metadata,
                })
            })
            .collect()
    }

    #[derive(Debug)]
    struct RecordingWorkControl {
        remaining: usize,
        checked: usize,
        charged: usize,
    }

    impl RecordingWorkControl {
        fn unlimited() -> Self {
            Self {
                remaining: usize::MAX,
                checked: 0,
                charged: 0,
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn check(&mut self, units: usize) -> std::result::Result<(), WorkError> {
            self.checked = self
                .checked
                .checked_add(units)
                .ok_or(WorkError::ArithmeticOverflow)?;
            if units > self.remaining {
                return Err(WorkError::Interrupted);
            }
            Ok(())
        }

        fn charge(&mut self, units: usize) -> std::result::Result<(), WorkError> {
            if units > self.remaining {
                return Err(WorkError::Interrupted);
            }
            self.remaining -= units;
            self.charged = self
                .charged
                .checked_add(units)
                .ok_or(WorkError::ArithmeticOverflow)?;
            Ok(())
        }
    }

    #[test]
    fn separate_children_hierarchy_index_has_unique_linear_ownership() {
        for depth in [8, 16, 32, 64, 128, 256] {
            let graph = deep_separate_graph(depth);
            let mut work_control = RecordingWorkControl::unlimited();
            let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();

            assert_eq!(index.scopes.len(), depth + 1);
            assert_eq!(index.postorder.len(), index.scopes.len());
            assert_eq!(index.postorder.last(), Some(&0));
            assert_eq!(
                index
                    .scopes
                    .iter()
                    .map(|scope| scope.nodes.len())
                    .sum::<usize>(),
                graph.nodes.len()
            );
            let mut node_owners = vec![0usize; graph.nodes.len()];
            for scope in &index.scopes {
                for node in &scope.nodes {
                    node_owners[*node] += 1;
                }
            }
            assert!(node_owners.into_iter().all(|owners| owners == 1));
            assert_eq!(work_control.checked, work_control.charged);
        }
    }

    #[test]
    fn separate_children_wrapper_work_is_linear_in_unique_items() {
        for depth in [8, 16, 32, 64, 128, 256] {
            let graph = deep_separate_graph(depth);
            let mut work_control = RecordingWorkControl::unlimited();
            let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
            let arena = std::iter::repeat_with(|| None)
                .take(index.scopes.len())
                .collect::<Vec<Option<ScopeLayout>>>();
            for scope in index.postorder.iter().copied() {
                index
                    .materialize_scope(scope, &arena, &mut work_control)
                    .unwrap();
            }

            assert_eq!(work_control.charged, graph.nodes.len() * 3);
            assert_eq!(work_control.checked, work_control.charged);
        }
    }

    #[test]
    fn hierarchy_index_work_is_linear_in_emitted_boundary_segments() {
        for depth in [4usize, 8, 16] {
            let mut baseline_work = RecordingWorkControl::unlimited();
            HierarchyIndex::build(&deep_cross_scope_graph(depth, 0), &mut baseline_work).unwrap();

            for edge_count in [1usize, 4, 16] {
                let graph = deep_cross_scope_graph(depth, edge_count);
                let mut work_control = RecordingWorkControl::unlimited();
                let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
                let segment_count = index
                    .scopes
                    .iter()
                    .map(|scope| scope.edges.len())
                    .sum::<usize>();

                assert_eq!(segment_count, edge_count * (depth + 1));
                // Each edge contributes two indexed descriptor visits. The combined owner/section
                // walk then charges exactly once for every hierarchy-local section it emits.
                assert_eq!(
                    work_control.charged - baseline_work.charged,
                    2 * edge_count + segment_count
                );
            }
        }
    }

    #[test]
    fn elk_wrapper_export_and_flatten_work_is_linear_in_cross_scope_geometry() {
        const POINTS_PER_SEGMENT: usize = 5;

        for edge_count in [1usize, 8, 32, 128] {
            let graph = repeated_cross_scope_graph(edge_count);
            let mut index_work = RecordingWorkControl::unlimited();
            let index = HierarchyIndex::build(&graph, &mut index_work).unwrap();
            assert_eq!(index.scopes.len(), 2);
            assert_eq!(
                index
                    .scopes
                    .iter()
                    .map(|scope| scope.edges.len())
                    .sum::<usize>(),
                edge_count * 2
            );

            let mut flatten_work = RecordingWorkControl::unlimited();
            let result = flatten_scope_layouts(
                &index,
                synthetic_scope_arena(&index, POINTS_PER_SEGMENT),
                &mut flatten_work,
            )
            .unwrap();

            // 2 scope-scan units + 2E geometry-scan units + (9 + 5E) structural units
            // + 2 * (10E points + E labels + 2E edge records) geometry units.
            assert_eq!(flatten_work.charged, 11 + 33 * edge_count);
            assert_eq!(flatten_work.checked, flatten_work.charged);
            assert_eq!(result.nodes.len(), graph.nodes.len());
            assert_eq!(result.edges.len(), edge_count);

            let flat = flat_graph(
                vec![leaf("source"), leaf("target")],
                (0..edge_count)
                    .map(|index| {
                        let mut edge = edge(format!("flat-{index}").as_str(), "source", "target");
                        edge.label = Some(Label {
                            width: 20.0,
                            height: 10.0,
                        });
                        edge
                    })
                    .collect(),
            );
            let imported = source_port::import_graph(&graph_to_source_input(&flat)).unwrap();
            let sort_work = match edge_count {
                1 => 0,
                8 => 24,
                32 => 160,
                128 => 896,
                _ => unreachable!("test matrix is fixed"),
            };
            // Two nodes, five linear export units per labelled straight edge, and the independent
            // E*ceil(log2(E)) comparison-sort bound used by the final Mermaid model-order pass.
            assert_eq!(
                source_graph_output_work_units(&imported),
                Ok(2 + 5 * edge_count + sort_work)
            );
        }
    }

    #[test]
    fn output_sort_work_is_checked_and_superlinear() {
        assert_eq!(comparison_sort_work_units(0), Ok(0));
        assert_eq!(comparison_sort_work_units(1), Ok(0));
        assert_eq!(comparison_sort_work_units(2), Ok(2));
        assert_eq!(comparison_sort_work_units(3), Ok(6));
        assert_eq!(comparison_sort_work_units(8), Ok(24));
        assert_eq!(
            comparison_sort_work_units(usize::MAX),
            Err(WorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn flatten_work_is_linear_in_points_at_fixed_cross_scope_cardinality() {
        const EDGE_COUNT: usize = 8;
        let graph = repeated_cross_scope_graph(EDGE_COUNT);
        let mut index_work = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut index_work).unwrap();
        let segment_count = index
            .scopes
            .iter()
            .map(|scope| scope.edges.len())
            .sum::<usize>();
        assert_eq!(segment_count, EDGE_COUNT * 2);

        let measured = [1usize, 8, 64].map(|points_per_segment| {
            let mut work_control = RecordingWorkControl::unlimited();
            flatten_scope_layouts(
                &index,
                synthetic_scope_arena(&index, points_per_segment),
                &mut work_control,
            )
            .unwrap();
            (points_per_segment, work_control.charged)
        });

        for pair in measured.windows(2) {
            let (before_points, before_work) = pair[0];
            let (after_points, after_work) = pair[1];
            assert_eq!(
                after_work - before_work,
                2 * segment_count * (after_points - before_points)
            );
        }
    }

    #[test]
    fn flatten_geometry_execution_budget_is_atomic() {
        const EDGE_COUNT: usize = 4;
        const POINTS_PER_SEGMENT: usize = 5;
        // 2 scope-scan + 8 geometry-scan planning units, then one 29 structural + 104 geometry
        // execution tranche. A failed execution check must retain only the completed planning work.
        const PLANNING_WORK: usize = 10;
        const EXECUTION_WORK: usize = 133;
        const REQUIRED: usize = PLANNING_WORK + EXECUTION_WORK;

        let graph = repeated_cross_scope_graph(EDGE_COUNT);
        let mut index_work = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut index_work).unwrap();

        let mut below = RecordingWorkControl {
            remaining: REQUIRED - 1,
            checked: 0,
            charged: 0,
        };
        let error = flatten_scope_layouts(
            &index,
            synthetic_scope_arena(&index, POINTS_PER_SEGMENT),
            &mut below,
        )
        .unwrap_err();
        assert_eq!(error.work_error(), Some(WorkError::Interrupted));
        assert_eq!(below.charged, PLANNING_WORK);
        assert_eq!(below.remaining, EXECUTION_WORK - 1);

        for extra in [0usize, 1] {
            let mut accepted = RecordingWorkControl {
                remaining: REQUIRED + extra,
                checked: 0,
                charged: 0,
            };
            let result = flatten_scope_layouts(
                &index,
                synthetic_scope_arena(&index, POINTS_PER_SEGMENT),
                &mut accepted,
            )
            .unwrap();
            assert_eq!(result.edges.len(), EDGE_COUNT);
            assert_eq!(accepted.charged, REQUIRED);
            assert_eq!(accepted.remaining, extra);
        }
    }

    #[test]
    fn separate_children_edges_have_one_owner_and_output_sensitive_segments() {
        let mut a = leaf("a");
        a.parent = Some("left".to_string());
        let mut a2 = leaf("a2");
        a2.parent = Some("left".to_string());
        let mut b = leaf("b");
        b.parent = Some("right".to_string());
        let graph = flat_graph(
            vec![
                group("left", None, HierarchyHandling::SeparateChildren),
                a,
                a2,
                group("right", None, HierarchyHandling::SeparateChildren),
                b,
                leaf("outer"),
            ],
            vec![
                edge("a-a2", "a", "a2"),
                edge("a-outer", "a", "outer"),
                edge("a-b", "a", "b"),
                edge("left-a", "left", "a"),
            ],
        );
        let mut work_control = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();

        let mut owners = vec![0usize; graph.edges.len()];
        let mut segments = vec![0usize; graph.edges.len()];
        for scope in &index.scopes {
            for edge in &scope.owned_edges {
                owners[*edge] += 1;
            }
            for edge in &scope.edges {
                segments[edge.original] += 1;
            }
        }
        assert_eq!(owners, vec![1, 1, 1, 1]);
        assert_eq!(segments, vec![1, 2, 3, 1]);
    }

    #[test]
    fn hierarchy_index_rejects_duplicate_edge_ids_before_scope_metadata_can_alias() {
        let graph = flat_graph(
            vec![leaf("a"), leaf("b"), leaf("c")],
            vec![edge("duplicate", "a", "b"), edge("duplicate", "a", "c")],
        );
        let mut work_control = RecordingWorkControl::unlimited();

        let error = HierarchyIndex::build(&graph, &mut work_control).unwrap_err();

        assert!(matches!(
            error,
            Error::SourceImport(source_port::ImportError::DuplicateEdge { id })
                if id == "duplicate"
        ));
    }

    #[test]
    fn separate_children_segment_edge_order_is_unique_within_materialized_scope() {
        let mut nested_group = group("nested", Some("group"), HierarchyHandling::SeparateChildren);
        nested_group.label = None;
        let mut source = leaf("source");
        source.parent = Some("nested".to_string());
        let mut sibling = leaf("sibling");
        sibling.parent = Some("group".to_string());
        let graph = flat_graph(
            vec![
                group("group", None, HierarchyHandling::SeparateChildren),
                nested_group,
                source,
                sibling,
                leaf("root-target"),
            ],
            vec![
                edge("to-sibling", "source", "sibling"),
                edge("to-root", "source", "root-target"),
            ],
        );
        let mut work_control = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
        let nested_scope = index.child_scope_by_anchor[1].unwrap();
        let arena = std::iter::repeat_with(|| None)
            .take(index.scopes.len())
            .collect::<Vec<Option<ScopeLayout>>>();

        let (_, segments, _) = index
            .materialize_scope(nested_scope, &arena, &mut work_control)
            .unwrap();

        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.edge.model_order)
                .collect::<Vec<_>>(),
            vec![Some(0), Some(0)]
        );
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.edge_order)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn separate_children_layout_is_small_stack_safe_and_deterministic() {
        let graph = deep_separate_graph(256);
        let first = std::thread::Builder::new()
            .name("elk-separate-children-small-stack".to_string())
            .stack_size(128 * 1024)
            .spawn(move || layout(&graph))
            .unwrap()
            .join()
            .expect("separate-children layout must not overflow the worker stack")
            .unwrap();
        assert_eq!(first.nodes.len(), 257);

        let replay_graph = deep_separate_graph(256);
        let replayed = layout(&replay_graph).unwrap();
        assert_eq!(first, replayed);
    }

    #[test]
    fn source_graph_dump_is_small_stack_safe_for_deep_hierarchy() {
        let depth = 256;
        let graph = deep_include_graph(depth);
        let dump = std::thread::Builder::new()
            .name("elk-source-dump-small-stack".to_string())
            .stack_size(128 * 1024)
            .spawn(move || {
                SourcePhaseDiagnostics::from_graph(&graph)
                    .unwrap()
                    .graph_dump()
            })
            .unwrap()
            .join()
            .expect("source graph dump must not overflow the worker stack");

        assert_eq!(
            dump.lines()
                .filter(|line| line.trim_start().starts_with("graph "))
                .count(),
            depth + 1
        );
    }

    #[test]
    fn separate_children_cross_boundary_edges_merge_once_with_one_label() {
        let mut a = leaf("a");
        a.parent = Some("left".to_string());
        let mut b = leaf("b");
        b.parent = Some("right".to_string());
        let mut descendant_to_outer = edge("a-outer", "a", "outer");
        descendant_to_outer.label = Some(Label {
            width: 36.0,
            height: 14.0,
        });
        let graph = flat_graph(
            vec![
                group("left", None, HierarchyHandling::SeparateChildren),
                a,
                group("right", None, HierarchyHandling::SeparateChildren),
                b,
                leaf("outer"),
            ],
            vec![descendant_to_outer, edge("a-b", "a", "b")],
        );

        let result = layout(&graph).unwrap();
        for edge_id in ["a-outer", "a-b"] {
            let matches = result
                .edges
                .iter()
                .filter(|edge| edge.id == edge_id)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "edge {edge_id} must be flattened once");
            assert!(matches[0].points.len() >= 2);
            assert!(
                matches[0]
                    .points
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite())
            );
        }
        assert_eq!(
            result
                .edges
                .iter()
                .find(|edge| edge.id == "a-outer")
                .unwrap()
                .labels
                .len(),
            1
        );
    }

    #[test]
    fn separate_children_cross_boundary_edges_keep_owner_model_order() {
        let mut left = leaf("left");
        left.parent = Some("group".to_string());
        let mut right = leaf("right");
        right.parent = Some("group".to_string());
        let mut graph = flat_graph(
            vec![
                group("group", None, HierarchyHandling::SeparateChildren),
                left,
                right,
                leaf("outer-a"),
                leaf("outer-b"),
            ],
            vec![
                edge("child-local", "left", "right"),
                edge("cross-boundary", "left", "outer-a"),
                edge("root-local", "outer-a", "outer-b"),
            ],
        );
        graph.options.layered.consider_model_order = true;

        let mut work_control = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();
        let arena = std::iter::repeat_with(|| None)
            .take(index.scopes.len())
            .collect::<Vec<Option<ScopeLayout>>>();
        let (input, segments, _) = index
            .materialize_scope(0, &arena, &mut work_control)
            .unwrap();
        let imported = source_port::import_graph_at_seed_scope_and_segments_with_work_control(
            &input,
            &index.scopes[0].seed_scope,
            &segments,
            &mut work_control,
        )
        .unwrap();

        let model_order = |edge_id: &str| {
            imported
                .edges
                .iter()
                .find(|edge| edge.id == edge_id)
                .unwrap_or_else(|| panic!("missing imported edge {edge_id}"))
                .model_order
        };
        assert_eq!(model_order("cross-boundary"), Some(0));
        assert_eq!(model_order("root-local"), Some(1));

        let result = layout(&graph).unwrap();
        assert_eq!(
            result
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            vec!["cross-boundary", "root-local", "child-local"]
        );
    }

    fn assert_direct_parent_child_edge_precedes_later_root_edge(source: &str, target: &str) {
        let mut child = leaf("child");
        child.parent = Some("group".to_string());
        let graph = flat_graph(
            vec![
                group("group", None, HierarchyHandling::SeparateChildren),
                child,
                leaf("root-a"),
                leaf("root-b"),
            ],
            vec![
                edge("parent-child", source, target),
                edge("root-local", "root-a", "root-b"),
            ],
        );
        let mut work_control = RecordingWorkControl::unlimited();
        let index = HierarchyIndex::build(&graph, &mut work_control).unwrap();

        let result =
            flatten_scope_layouts(&index, synthetic_scope_arena(&index, 2), &mut work_control)
                .unwrap();

        assert_eq!(
            result
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            ["parent-child", "root-local"]
        );
    }

    #[test]
    fn parent_to_direct_child_edge_keeps_owner_scope_order_when_flattened() {
        assert_direct_parent_child_edge_precedes_later_root_edge("group", "child");
    }

    #[test]
    fn direct_child_to_parent_edge_keeps_owner_scope_order_when_flattened() {
        assert_direct_parent_child_edge_precedes_later_root_edge("child", "group");
    }

    #[test]
    fn elk_work_control_interrupts_before_hierarchy_index_allocation() {
        let graph = deep_separate_graph(8);
        let initial_budget = graph.nodes.len() - 1;
        let mut work_control = RecordingWorkControl {
            remaining: initial_budget,
            checked: 0,
            charged: 0,
        };

        let error = layout_with_work_control(&graph, &mut work_control).unwrap_err();

        assert_eq!(error.work_error(), Some(WorkError::Interrupted));
        assert_eq!(work_control.remaining, initial_budget);
        assert_eq!(work_control.charged, 0);
    }

    #[test]
    fn source_backed_layout_uses_default_median_layer_for_cross_hierarchy_center_label() {
        let group = |id: &str, parent: Option<&str>, label: Label| Node {
            id: id.to_string(),
            kind: NodeKind::Group,
            width: 0.0,
            height: 0.0,
            parent: parent.map(str::to_string),
            direction: None,
            hierarchy_handling: Some(HierarchyHandling::IncludeChildren),
            layer_constraint: None,
            label: Some(label),
        };
        let child = |id: &str, parent: &str, width: f64| Node {
            parent: Some(parent.to_string()),
            width,
            height: 54.0,
            ..leaf(id)
        };
        let labelled_edge = |id: &str, source: &str, target: &str, width: f64| Edge {
            label: Some(Label {
                width,
                height: 24.0,
            }),
            ..edge(id, source, target)
        };
        let graph = flat_graph(
            vec![
                group(
                    "container_Alpha",
                    None,
                    Label {
                        width: 116.859375,
                        height: 22.0,
                    },
                ),
                group(
                    "container_Beta",
                    Some("container_Alpha"),
                    Label {
                        width: 109.171875,
                        height: 22.0,
                    },
                ),
                child("process_C", "container_Beta", 131.28125),
                child("Process_D", "container_Beta", 130.78125),
                child("process_A", "container_Alpha", 131.15625),
                child("process_B", "container_Alpha", 130.765625),
            ],
            vec![
                edge("C-D", "process_C", "Process_D"),
                edge("A-B", "process_A", "process_B"),
                labelled_edge("B-Beta", "process_B", "container_Beta", 99.03125),
                labelled_edge("A-C", "process_A", "process_C", 66.609375),
            ],
        );

        let result = layout(&graph).unwrap();

        let alpha = result
            .nodes
            .iter()
            .find(|node| node.id == "container_Alpha")
            .expect("outer compound node");
        let cross_hierarchy_label = result
            .edges
            .iter()
            .find(|edge| edge.id == "A-C")
            .and_then(|edge| edge.labels.first())
            .expect("cross-hierarchy center label");
        let sibling_label = result
            .edges
            .iter()
            .find(|edge| edge.id == "B-Beta")
            .and_then(|edge| edge.labels.first())
            .expect("sibling center label");

        assert_eq!(alpha.width, 251.375);
        assert_eq!(cross_hierarchy_label.x, 24.0);
        assert_eq!(cross_hierarchy_label.y, 150.0);
        assert_eq!(cross_hierarchy_label.width, 66.609375);
        assert_eq!(sibling_label.x, 135.9921875);
        assert_eq!(sibling_label.y, 219.0);
    }

    #[test]
    fn source_backed_layout_accounts_for_inline_label_edge_thickness() {
        let mut a1 = leaf("a1");
        a1.parent = Some("one".to_string());
        a1.width = 76.796875;
        a1.height = 54.0;
        let mut a2 = leaf("a2");
        a2.parent = Some("one".to_string());
        a2.width = 76.796875;
        a2.height = 54.0;
        let mut first = edge("e1", "a1", "a2");
        first.label = Some(Label {
            width: 13.109375,
            height: 24.0,
        });
        let mut second = edge("e2", "a1", "a2");
        second.label = first.label;
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "one".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: None,
                    hierarchy_handling: None,
                    layer_constraint: None,
                    label: Some(Label {
                        width: 26.046875,
                        height: 22.0,
                    }),
                },
                a1,
                a2,
            ],
            vec![first, second],
        );
        graph.direction = Direction::Left;

        let result = layout(&graph).unwrap();
        let group = result.nodes.iter().find(|node| node.id == "one").unwrap();
        let first = result.edges.iter().find(|edge| edge.id == "e1").unwrap();
        let second = result.edges.iter().find(|edge| edge.id == "e2").unwrap();

        assert_eq!(group.width, 250.703125);
        assert_eq!(group.height, 121.5);
        assert_eq!(first.labels[0].x, 130.796875);
        assert_eq!(first.labels[0].y, 96.5);
        assert_eq!(first.points[2].y, 109.0);
        assert_eq!(second.labels[0].x, 130.796875);
        assert_eq!(second.labels[0].y, 56.5);
        assert!(second.points.iter().all(|point| point.y == 69.0));
    }

    #[test]
    fn source_backed_layout_places_connected_nodes_in_direction_order() {
        let graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);

        let result = layout(&graph).unwrap();

        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        assert!(b.y > a.y);
        assert!(edge.points.len() >= 2);
        assert_eq!(edge.points.first().unwrap().y, a.y + a.height / 2.0);
        assert_eq!(edge.points.last().unwrap().y, b.y - b.height / 2.0);
    }

    #[test]
    fn source_backed_layout_honors_left_right_direction() {
        let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
        graph.direction = Direction::Right;

        let result = layout(&graph).unwrap();

        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.x > a.x);
    }

    #[test]
    fn source_ported_layout_rejects_unported_routing_before_layout() {
        let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
        graph.options.layered.edge_routing = EdgeRouting::Polyline;

        assert!(matches!(
            layout(&graph),
            Err(Error::SourcePipeline(
                source_port::PipelineError::UnsupportedProcessor {
                    kind: source_port::ProcessorKind::PolylineEdgeRouter,
                }
            ))
        ));
    }

    #[test]
    fn source_backed_layout_excludes_empty_group_labels_from_elk_geometry() {
        let empty_group = Node {
            id: "B".to_string(),
            kind: NodeKind::Group,
            width: 0.0,
            height: 0.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            label: Some(Label {
                width: 9.0625,
                height: 22.0,
            }),
        };
        let mut laid_out_group = leaf("A");
        laid_out_group.width = 191.328125;
        laid_out_group.height = 105.0;
        let mut graph = flat_graph(vec![empty_group, laid_out_group], vec![]);
        graph.direction = Direction::Right;

        let result = layout(&graph).unwrap();
        let empty_group = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let laid_out_group = result.nodes.iter().find(|node| node.id == "A").unwrap();

        assert_eq!(empty_group.width, 0.0);
        assert_eq!(empty_group.height, 0.0);
        assert_eq!(laid_out_group.y - laid_out_group.height / 2.0, 52.0);
    }

    #[test]
    fn source_backed_layout_routes_long_edge_after_joiner() {
        let graph = flat_graph(
            vec![leaf("A"), leaf("B"), leaf("C")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("A-C", "A", "C"),
            ],
        );

        let result = layout(&graph).unwrap();

        let long = result.edges.iter().find(|edge| edge.id == "A-C").unwrap();
        assert_eq!(
            result.edges.iter().filter(|edge| edge.id == "A-C").count(),
            1
        );
        assert!(long.points.len() > 4);
    }

    #[test]
    fn source_backed_layout_exports_edge_label_layouts() {
        let mut labelled = edge("A-C", "A", "C");
        labelled.label = Some(Label {
            width: 48.0,
            height: 12.0,
        });
        let graph = flat_graph(
            vec![leaf("A"), leaf("B"), leaf("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
        );

        let result = layout(&graph).unwrap();

        let edge = result.edges.iter().find(|edge| edge.id == "A-C").unwrap();
        let label = edge
            .labels
            .first()
            .expect("source-backed ELK should export placed edge label bounds");
        assert_eq!(label.width, 48.0);
        assert_eq!(label.height, 12.0);
        assert!(label.x.is_finite());
        assert!(label.y.is_finite());
    }

    #[test]
    fn layered_options_to_source_propagates_inside_self_loops_activate() {
        let mut graph = flat_graph(vec![leaf("A")], vec![]);
        graph.options.layered.inside_self_loops_activate = true;

        let input = graph_to_source_input(&graph);

        assert!(input.options.inside_self_loops_activate);
    }

    #[test]
    fn layered_options_to_source_preserves_upstream_random_seed_sentinel() {
        let mut graph = flat_graph(vec![leaf("A")], vec![]);
        graph.options.layered.random_seed = 0;

        let input = graph_to_source_input(&graph);

        assert_eq!(input.options.random_seed, 0);
    }

    #[test]
    fn raw_layout_rejects_unseeded_elk_zero_but_operation_seed_is_replayable() {
        use std::num::NonZeroU64;

        let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
        graph.options.layered.random_seed = 0;

        assert!(matches!(
            layout(&graph),
            Err(Error::SourcePipeline(source_port::PipelineError::RandomSeed(
                source_port::RandomSeedError::Unresolved { graph_path }
            ))) if graph_path == "root"
        ));

        let operation_seed = ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x6d65_726d_616e).expect("nonzero operation seed"),
        );
        let first =
            layout_with_operation_seed(&graph, operation_seed).expect("deterministic layout");
        let replayed = layout_with_operation_seed(&graph, operation_seed).expect("replayed layout");

        assert_eq!(first, replayed);
    }

    #[test]
    fn source_phase_diagnostics_uses_the_same_zero_seed_boundary() {
        use std::num::NonZeroU64;

        let mut graph = flat_graph(vec![leaf("A"), leaf("B")], vec![edge("A-B", "A", "B")]);
        graph.options.layered.random_seed = 0;

        let mut raw = SourcePhaseDiagnostics::from_graph(&graph).expect("diagnostic session");
        assert!(matches!(
            raw.execute_all(),
            Err(Error::SourcePipeline(source_port::PipelineError::RandomSeed(
                source_port::RandomSeedError::Unresolved { graph_path }
            ))) if graph_path == "root"
        ));

        let operation_seed = ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x6469_6167_6e6f_7374).expect("nonzero operation seed"),
        );
        let mut first =
            SourcePhaseDiagnostics::from_graph_with_operation_seed(&graph, operation_seed)
                .expect("seeded diagnostic session");
        let mut replayed =
            SourcePhaseDiagnostics::from_graph_with_operation_seed(&graph, operation_seed)
                .expect("replayed seeded diagnostic session");

        assert_eq!(
            first.execute_all().unwrap(),
            replayed.execute_all().unwrap()
        );
    }

    #[test]
    fn graph_to_source_input_propagates_inside_self_loop_edge_flag() {
        let graph = flat_graph(
            vec![leaf("A")],
            vec![Edge {
                inside_self_loops_yo: true,
                ..edge("A-A", "A", "A")
            }],
        );

        let input = graph_to_source_input(&graph);

        assert!(input.edges[0].inside_self_loops_yo);
    }

    #[test]
    fn source_graph_export_applies_graph_offset_and_padding_to_layout() {
        let mut graph = LGraph::new("root", SourceLayeredOptions::default());
        graph.offset = LPoint { x: 1.0, y: 2.0 };
        graph.padding = source_port::LPadding {
            top: 7.0,
            right: 0.0,
            bottom: 0.0,
            left: 12.0,
        };

        let mut a = source_port::LNode::new("A", 10.0, 20.0, None);
        a.position = LPoint { x: 3.0, y: 5.0 };
        let mut b = source_port::LNode::new("B", 10.0, 20.0, None);
        b.position = LPoint { x: 50.0, y: 60.0 };
        graph.layerless_nodes.push(a);
        graph.layerless_nodes.push(b);

        let source = graph
            .add_port(
                0,
                source_port::PortType::Output,
                source_port::PortSide::South,
                LPoint { x: 5.0, y: 20.0 },
            )
            .unwrap();
        let target = graph
            .add_port(
                1,
                source_port::PortType::Input,
                source_port::PortSide::North,
                LPoint { x: 5.0, y: 0.0 },
            )
            .unwrap();

        let mut label = source_port::LLabel::new("label", 6.0, 7.0);
        label.position = LPoint { x: 30.0, y: 40.0 };
        graph
            .add_edge(source_port::LayeredEdge {
                id: "A-B".to_string(),
                source,
                target,
                source_node_id: "A".to_string(),
                target_node_id: "B".to_string(),
                labels: vec![label],
                minlen: 1,
                reversed: false,
                bend_points: vec![LPoint { x: 20.0, y: 30.0 }],
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
                compound_segment: None,
            })
            .unwrap();

        let result = source_graph_to_layout_result(&graph);

        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        assert_eq!(a.x, 21.0);
        assert_eq!(a.y, 24.0);
        assert_eq!(b.x, 68.0);
        assert_eq!(b.y, 79.0);
        assert_eq!(edge.points[0], Point { x: 21.0, y: 34.0 });
        assert_eq!(edge.points[1], Point { x: 33.0, y: 39.0 });
        assert_eq!(edge.points[2], Point { x: 68.0, y: 69.0 });
        assert_eq!(edge.labels[0].x, 43.0);
        assert_eq!(edge.labels[0].y, 49.0);
    }

    #[test]
    fn source_graph_export_groups_compound_segments_by_original_edge_id() {
        let mut graph = LGraph::new("root", SourceLayeredOptions::default());
        graph
            .layerless_nodes
            .push(source_port::LNode::new("A", 10.0, 20.0, None));
        graph
            .layerless_nodes
            .push(source_port::LNode::new("B", 10.0, 20.0, None));

        let source = graph
            .add_port(
                0,
                source_port::PortType::Output,
                source_port::PortSide::South,
                LPoint { x: 5.0, y: 20.0 },
            )
            .unwrap();
        let target = graph
            .add_port(
                1,
                source_port::PortType::Input,
                source_port::PortSide::North,
                LPoint { x: 5.0, y: 0.0 },
            )
            .unwrap();

        let segment_edge = graph
            .add_edge(source_port::LayeredEdge {
                id: "merged-segment".to_string(),
                source,
                target,
                source_node_id: "A".to_string(),
                target_node_id: "B".to_string(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
                compound_segment: None,
            })
            .unwrap();
        graph
            .cross_hierarchy_edges
            .push(source_port::CrossHierarchyEdge {
                original_edge_id: "A-B".to_string(),
                original_model_order: None,
                graph_id: "root".to_string(),
                edge: segment_edge,
                segment: source_port::CompoundEdgeSegment::Output { depth: 0 },
            });

        let result = source_graph_to_layout_result(&graph);

        assert!(result.edges.iter().any(|edge| edge.id == "A-B"));
        assert!(!result.edges.iter().any(|edge| edge.id == "merged-segment"));
    }

    #[test]
    fn compound_segment_counting_order_matches_elk_cross_hierarchy_order() {
        let segment = |segment| CompoundEdgeLayoutSegment {
            original_edge_id: "edge".to_string(),
            segment,
            model_order: None,
            edge: EdgeLayout {
                id: "edge".to_string(),
                points: Vec::new(),
                labels: Vec::new(),
            },
        };
        let mut segments = vec![
            segment(source_port::CompoundEdgeSegment::Input { depth: 4 }),
            segment(source_port::CompoundEdgeSegment::Output { depth: 5 }),
            segment(source_port::CompoundEdgeSegment::Input { depth: 2 }),
            segment(source_port::CompoundEdgeSegment::Output { depth: 3 }),
            segment(source_port::CompoundEdgeSegment::Output { depth: 4 }),
            segment(source_port::CompoundEdgeSegment::Input { depth: 3 }),
        ];

        order_compound_layout_segments(&mut segments);

        assert_eq!(
            segments
                .into_iter()
                .map(|segment| segment.segment)
                .collect::<Vec<_>>(),
            vec![
                source_port::CompoundEdgeSegment::Output { depth: 5 },
                source_port::CompoundEdgeSegment::Output { depth: 4 },
                source_port::CompoundEdgeSegment::Output { depth: 3 },
                source_port::CompoundEdgeSegment::Input { depth: 2 },
                source_port::CompoundEdgeSegment::Input { depth: 3 },
                source_port::CompoundEdgeSegment::Input { depth: 4 },
            ]
        );
    }

    #[test]
    fn source_graph_export_all_original_edges_for_shared_compound_segment() {
        let mut graph = LGraph::new("root", SourceLayeredOptions::default());
        graph
            .layerless_nodes
            .push(source_port::LNode::new("A", 10.0, 20.0, None));
        graph
            .layerless_nodes
            .push(source_port::LNode::new("B", 10.0, 20.0, None));

        let source = graph
            .add_port(
                0,
                source_port::PortType::Output,
                source_port::PortSide::South,
                LPoint { x: 5.0, y: 20.0 },
            )
            .unwrap();
        let target = graph
            .add_port(
                1,
                source_port::PortType::Input,
                source_port::PortSide::North,
                LPoint { x: 5.0, y: 0.0 },
            )
            .unwrap();

        let segment_edge = graph
            .add_edge(source_port::LayeredEdge {
                id: "merged-segment".to_string(),
                source,
                target,
                source_node_id: "A".to_string(),
                target_node_id: "B".to_string(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
                compound_segment: None,
            })
            .unwrap();

        for original_edge_id in ["A-B-1", "A-B-2"] {
            graph
                .cross_hierarchy_edges
                .push(source_port::CrossHierarchyEdge {
                    original_edge_id: original_edge_id.to_string(),
                    original_model_order: None,
                    graph_id: "root".to_string(),
                    edge: segment_edge,
                    segment: source_port::CompoundEdgeSegment::Output { depth: 0 },
                });
        }

        let result = source_graph_to_layout_result(&graph);

        assert!(result.edges.iter().any(|edge| edge.id == "A-B-1"));
        assert!(result.edges.iter().any(|edge| edge.id == "A-B-2"));
        assert!(!result.edges.iter().any(|edge| edge.id == "merged-segment"));
    }

    #[test]
    fn source_graph_export_filters_shared_compound_segment_labels_by_original_edge() {
        let mut graph = LGraph::new("root", SourceLayeredOptions::default());
        graph
            .layerless_nodes
            .push(source_port::LNode::new("A", 10.0, 20.0, None));
        graph
            .layerless_nodes
            .push(source_port::LNode::new("B", 10.0, 20.0, None));

        let source = graph
            .add_port(
                0,
                source_port::PortType::Output,
                source_port::PortSide::South,
                LPoint { x: 5.0, y: 20.0 },
            )
            .unwrap();
        let target = graph
            .add_port(
                1,
                source_port::PortType::Input,
                source_port::PortSide::North,
                LPoint { x: 5.0, y: 0.0 },
            )
            .unwrap();
        let mut first_label = source_port::LLabel::new("first", 10.0, 4.0);
        first_label.original_label_edge = Some("A-B-1".to_string());
        let mut second_label = source_port::LLabel::new("second", 20.0, 4.0);
        second_label.original_label_edge = Some("A-B-2".to_string());

        let segment_edge = graph
            .add_edge(source_port::LayeredEdge {
                id: "merged-segment".to_string(),
                source,
                target,
                source_node_id: "A".to_string(),
                target_node_id: "B".to_string(),
                labels: vec![first_label, second_label],
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
                compound_segment: None,
            })
            .unwrap();

        for original_edge_id in ["A-B-1", "A-B-2"] {
            graph
                .cross_hierarchy_edges
                .push(source_port::CrossHierarchyEdge {
                    original_edge_id: original_edge_id.to_string(),
                    original_model_order: None,
                    graph_id: "root".to_string(),
                    edge: segment_edge,
                    segment: source_port::CompoundEdgeSegment::Output { depth: 0 },
                });
        }

        let result = source_graph_to_layout_result(&graph);
        let first = result.edges.iter().find(|edge| edge.id == "A-B-1").unwrap();
        let second = result.edges.iter().find(|edge| edge.id == "A-B-2").unwrap();

        assert_eq!(first.labels.len(), 1);
        assert_eq!(first.labels[0].width, 10.0);
        assert_eq!(second.labels.len(), 1);
        assert_eq!(second.labels[0].width, 20.0);
    }

    #[test]
    fn source_backed_layout_exports_nested_compound_nodes_with_parent_offset() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut second_child = leaf("B");
        second_child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    layer_constraint: None,
                    label: None,
                },
                child,
                second_child,
            ],
            vec![edge("A-B", "A", "B")],
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout(&graph).unwrap();

        let cluster = result
            .nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        assert_eq!(result.nodes.len(), 3);
        assert!(cluster.width >= a.width);
        assert!(cluster.height >= b.y - a.y);
        assert!(a.y > cluster.y - cluster.height / 2.0);
        assert!(b.y < cluster.y + cluster.height / 2.0);
        assert!(b.y > a.y);
        assert_eq!(edge.points.first().unwrap().y, a.y + a.height / 2.0);
        assert_eq!(edge.points.last().unwrap().y, b.y - b.height / 2.0);
    }

    #[test]
    fn source_backed_layout_routes_cross_hierarchy_edge() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    layer_constraint: None,
                    label: None,
                },
                child,
            ],
            vec![edge("cluster-A", "cluster", "A")],
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout(&graph).unwrap();

        let cluster = result
            .nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let child = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let edge = result
            .edges
            .iter()
            .find(|edge| edge.id == "cluster-A")
            .unwrap();
        assert_eq!(result.nodes.len(), 2);
        assert!(edge.points.len() >= 2);
        assert!(
            edge.points.first().unwrap().x >= cluster.x - cluster.width / 2.0
                && edge.points.first().unwrap().x <= cluster.x + cluster.width / 2.0
        );
        assert_eq!(edge.points.last().unwrap().x, child.x);
    }

    #[test]
    fn source_backed_layout_exports_edge_from_nested_child_to_outer_node() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Down),
                    hierarchy_handling: None,
                    layer_constraint: None,
                    label: None,
                },
                child,
                leaf("B"),
            ],
            vec![edge("A-B", "A", "B")],
        );
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout(&graph).unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let edge = result
            .edges
            .iter()
            .find(|edge| edge.id == "A-B")
            .expect("cross-hierarchy child edge should be exported");
        assert!(edge.points.len() >= 2);
        assert!(
            edge.points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        assert_eq!(edge.points.last().unwrap().x, b.x);
    }

    #[test]
    fn source_backed_layout_recursively_lays_out_separate_children() {
        let mut child = leaf("A");
        child.parent = Some("cluster".to_string());
        let mut second_child = leaf("B");
        second_child.parent = Some("cluster".to_string());
        let mut graph = flat_graph(
            vec![
                Node {
                    id: "cluster".to_string(),
                    kind: NodeKind::Group,
                    width: 0.0,
                    height: 0.0,
                    parent: None,
                    direction: Some(Direction::Right),
                    hierarchy_handling: Some(HierarchyHandling::SeparateChildren),
                    layer_constraint: None,
                    label: Some(Label {
                        width: 42.0,
                        height: 18.0,
                    }),
                },
                child,
                second_child,
                leaf("outer"),
            ],
            vec![
                edge("A-B", "A", "B"),
                edge("cluster-outer", "cluster", "outer"),
            ],
        );
        graph.direction = Direction::Down;
        graph.options.layered.hierarchy_handling = HierarchyHandling::IncludeChildren;

        let result = layout(&graph).unwrap();

        let cluster = result
            .nodes
            .iter()
            .find(|node| node.id == "cluster")
            .unwrap();
        let a = result.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = result.nodes.iter().find(|node| node.id == "B").unwrap();
        let outer = result.nodes.iter().find(|node| node.id == "outer").unwrap();
        let inner_edge = result.edges.iter().find(|edge| edge.id == "A-B").unwrap();
        let outer_edge = result
            .edges
            .iter()
            .find(|edge| edge.id == "cluster-outer")
            .unwrap();

        assert!(cluster.width >= b.x + b.width / 2.0 - (a.x - a.width / 2.0));
        assert!(cluster.height >= 18.0);
        assert!(a.x < b.x);
        assert!(outer.y > cluster.y);
        assert_eq!(inner_edge.points.first().unwrap().y, a.y);
        assert_eq!(inner_edge.points.last().unwrap().y, b.y);
        assert_eq!(outer_edge.points.first().unwrap().x, cluster.x);
    }
}
