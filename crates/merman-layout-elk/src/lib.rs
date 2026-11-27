#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! Source-port policy:
//! - The historical Mermaid adapter derivation is
//!   https://github.com/mermaid-js/mermaid/blob/21f72f07ea22c0af48a3149c550654e80d8e40cb/packages/mermaid/src/rendering-util/layout-algorithms/elk/render.ts.
//! - The historical `elkjs@0.9.3` derivation is
//!   https://github.com/kieler/elkjs/tree/a8304cf79fde75bc2ab1a89d28320f53f8637436.
//! - The historical Eclipse ELK Java derivation is
//!   https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e,
//! - Mermaid 12.3.0's exact engine oracle is elkjs 0.12.0 at
//!   https://github.com/kieler/elkjs/tree/ff5771d7165445c42c408bb8a090c8035272218c,
//!   compared with Eclipse ELK 0.12.0 at
//!   https://github.com/eclipse-elk/elk/tree/323312916048544e4e594eab3303cdb0d01e2929.
//!   elkjs CI clones ELK master, so this tag is not a proven unique npm build input.
//!   The engine crate's `MERMAID_12_3_DELTA.md` records ported deltas and unsupported options.
//!
//! The crate exposes one Mermaid adapter over the source-backed ELK providers. Additional providers
//! handle flat graphs and recursive containers, including Mermaid's explicit metadata presets
//! and cross-boundary downgrade. Remaining cross-provider boundary routes are rejected explicitly.
//! New layout behavior must carry a pinned Mermaid or Eclipse ELK source reference.

use std::collections::HashMap;
use std::num::NonZeroU64;

#[cfg(test)]
mod adapter_tests;
mod container;
mod diagnostics;
mod flat;
mod hierarchy;
mod input;
mod model;
mod output;
#[cfg(test)]
mod terminal_labels_tests;
#[cfg(test)]
mod tests;

use container::ContainerMode;
pub use diagnostics::SourcePhaseDiagnostics;
use merman_elk_layered as source_port;
pub use model::*;
use source_port::LGraph;
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    NodeSize(#[from] source_port::NodeSizeError),
    #[error("phase diagnostics require the Layered algorithm")]
    LayeredDiagnosticsRequired,
    #[error("phase diagnostics require one scope; use layout for separately scheduled containers")]
    SeparateScopesDiagnosticsUnsupported,
    #[error("unsupported ELK hierarchy/provider combination for edge `{edge_id}`")]
    UnsupportedCrossProviderEdge { edge_id: String },
    #[error(transparent)]
    Box(#[from] source_port::algorithms::box_layout::Error),
    #[error(transparent)]
    Rectpacking(#[from] source_port::algorithms::rectpacking::Error),
    #[error(transparent)]
    Force(#[from] source_port::algorithms::force::Error),
    #[error(transparent)]
    Stress(#[from] source_port::algorithms::stress::Error),
    #[error(transparent)]
    MrTree(#[from] source_port::algorithms::mrtree::Error),
    #[error(transparent)]
    Radial(#[from] source_port::algorithms::radial::Error),
    #[error(transparent)]
    SporeOverlap(#[from] source_port::algorithms::spore_overlap::Error),
    #[error(transparent)]
    RandomSeed(#[from] source_port::RandomSeedError),
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
            Self::Box(source_port::algorithms::box_layout::Error::Work(error))
            | Self::Rectpacking(source_port::algorithms::rectpacking::Error::Work(error))
            | Self::Rectpacking(source_port::algorithms::rectpacking::Error::Box(
                source_port::algorithms::box_layout::Error::Work(error),
            ))
            | Self::Force(source_port::algorithms::force::Error::Work(error))
            | Self::Stress(source_port::algorithms::stress::Error::Work(error))
            | Self::Stress(source_port::algorithms::stress::Error::Force(
                source_port::algorithms::force::Error::Work(error),
            ))
            | Self::MrTree(source_port::algorithms::mrtree::Error::Work(error))
            | Self::Radial(source_port::algorithms::radial::Error::Work(error))
            | Self::SporeOverlap(source_port::algorithms::spore_overlap::Error::Work(error)) => {
                Some(*error)
            }
            Self::Work(error) => Some(*error),
            Self::SourceImport(source_port::ImportError::Work(error))
            | Self::SourcePipeline(source_port::PipelineError::Work(error)) => Some(*error),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Executes the selected source-backed ELK provider. Unsupported cross-provider boundary edges
/// return a typed error instead of flattening the graph or substituting the Layered provider.
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

#[derive(Debug)]
struct HierarchyIndex<'a> {
    graph: &'a Graph,
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    container_modes: Vec<ContainerMode>,
    resolved_handling: Vec<HierarchyHandling>,
    node_scope: Vec<usize>,
    edge_model_order: Vec<usize>,
    child_scope_by_anchor: Vec<Option<usize>>,
    scopes: Vec<ScopePlan>,
    postorder: Vec<usize>,
}

#[derive(Debug)]
struct ScopePlan {
    algorithm: Algorithm,
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
    content_shifts: HashMap<String, Point>,
    size_constraints_active: bool,
    label_margin: source_port::LMargin,
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
        if index.scopes[scope].algorithm != Algorithm::Layered {
            let (input, edge_metadata) =
                index.materialize_flat_scope(scope, &arena, work_control)?;
            let mut output = flat::layout(
                &input,
                &index.scopes[scope].seed_scope,
                index.flat_scope_context(scope),
                &index.flat_node_contexts(scope, &arena),
                operation_seed,
                work_control,
            )?;
            index.complete_flat_hierarchy_edges(scope, &arena, &mut output, work_control)?;
            arena[scope] = Some(ScopeLayout {
                layout: output.layout,
                size: output.size,
                edge_metadata,
                content_shifts: output.content_shifts,
                size_constraints_active: output.size_constraints_active,
                label_margin: container::scope_title_margin(
                    index.scopes[scope]
                        .anchor
                        .and_then(|node| graph.nodes[node].label),
                    output.size,
                ),
            });
            continue;
        }
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
        let export_work = output::source_graph_output_work_units(&lgraph)?;
        work_control.check(export_work)?;
        work_control.charge(export_work)?;
        let size = output::actual_source_graph_size(&lgraph);
        arena[scope] = Some(ScopeLayout {
            layout: output::source_graph_to_layout_result(&lgraph),
            size,
            edge_metadata,
            content_shifts: HashMap::new(),
            size_constraints_active: false,
            label_margin: container::scope_title_margin(
                index.scopes[scope]
                    .anchor
                    .and_then(|node| graph.nodes[node].label),
                size,
            ),
        });
    }

    output::flatten_scope_layouts(&index, arena, work_control)
}

fn source_graph_requires_compound_pipeline(graph: &LGraph) -> bool {
    graph.options.hierarchy_handling == source_port::HierarchyHandling::IncludeChildren
        || graph.graph_properties.external_ports
        || !graph.hierarchy_edges.is_empty()
        || !graph.cross_hierarchy_edges.is_empty()
        || graph
            .layerless_nodes
            .iter()
            .any(|node| node.nested_graph.is_some())
}
