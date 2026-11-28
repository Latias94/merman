//! Standalone layout of measured directed graphs. Coordinates use the caller's drawing units.
//!
//! Layout stages resolve aesthetic conflicts in priority order.
//!
//! | Priority | Owner | Rule |
//! | --- | --- | --- |
//! | P0 | Placement, routing, and clearance | Keep shapes, labels, and marker footprints clear. |
//! | P1 | Orientation and ordering | Minimize crossings, keep spines straight, and route feedback outside. |
//! | P2 | Routing and paint | Prefer straight shafts, then a single monotone sweep. |
//! | P3 | Ports and label placement | Align sibling turns and balance labels on the visible shaft. |
//! | P4 | Spacing and group fitting | Maintain spatial rhythm and let titles yield to routes. |
//! | P5 | Ordering and anchoring | Preserve authored order and stable positions when crossing-neutral. |
//! | P6 | Compaction and packing | Use available space without reducing clearance. |

#![forbid(unsafe_code)]

mod adaptive;
pub mod card_grid;
mod compound;
mod flat;
#[doc(hidden)]
pub mod geom;
pub mod grid;
mod model;
mod options;
mod ortho;
mod orthogonal;
mod scratch;
mod validate;
mod work;

pub use model::*;
pub use options::*;
pub use ortho::{OrthogonalSpan, route_fixed_spans, route_orthogonal_spans};
pub use scratch::LayoutScratch;
pub use work::{LayoutError, NoopWorkControl, WorkControl, WorkError};

/// Lays out a graph using temporary scratch and unrestricted work control.
pub fn layout(graph: &Graph, options: &Options) -> Result<LayoutOutput, LayoutError> {
    layout_with_control(
        graph,
        options,
        &mut LayoutScratch::default(),
        &mut NoopWorkControl,
    )
}

/// Lays out a graph in input order. Scratch can be reused after success or failure.
pub fn layout_with_control(
    graph: &Graph,
    options: &Options,
    scratch: &mut LayoutScratch,
    control: &mut impl WorkControl,
) -> Result<LayoutOutput, LayoutError> {
    layout_impl(graph, options, scratch, control, LayoutDetail::Full)
}

/// Lays out a graph in input order and returns its geometry without statistics.
///
/// The solver can omit routed crossing counts when no layout decision uses them.
/// Work limits and cancellation apply throughout.
pub fn layout_geometry_with_control(
    graph: &Graph,
    options: &Options,
    scratch: &mut LayoutScratch,
    control: &mut impl WorkControl,
) -> Result<LayoutGeometry, LayoutError> {
    layout_impl(graph, options, scratch, control, LayoutDetail::Geometry).map(LayoutGeometry::from)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutDetail {
    Full,
    Geometry,
}

impl LayoutDetail {
    fn flat(self, options: &Options) -> Self {
        if self == Self::Full {
            return self;
        }
        let natural_ranks = options.width_policy == WidthPolicy::Natural
            && (options.capture_node_ranks
                || matches!(options.packing, PackingPolicy::FitWidth { .. }));
        if natural_ranks
            || options.reuse_unchanged_components
            || options.global_orthogonal
            || !matches!(
                options.routing,
                RoutingStyle::Curved | RoutingStyle::Polyline
            )
        {
            Self::Full
        } else {
            self
        }
    }
}

fn layout_impl(
    graph: &Graph,
    options: &Options,
    scratch: &mut LayoutScratch,
    control: &mut impl WorkControl,
    detail: LayoutDetail,
) -> Result<LayoutOutput, LayoutError> {
    let mut frame = scratch.frame();
    let mut work = work::Work::new(control);
    work.charge(1)?;
    let reuse_components = options.reuse_unchanged_components
        && options.width_policy == WidthPolicy::Natural
        && graph.groups.is_empty();
    if reuse_components {
        frame.begin_components();
    }
    let mut keep_components = reuse_components && options.capture_node_ranks;
    let (output, width_trial) = match options.width_policy {
        WidthPolicy::Natural => (
            solve(graph, options, frame.expanded, detail, &mut work)?,
            None,
        ),
        WidthPolicy::FitWidth { .. } => {
            let (drawing, trial) =
                adaptive::run(graph, options, frame.expanded, detail, &mut work)?;
            (drawing, Some(trial))
        }
    };
    let mut output = if work.editor_choice
        && (output.stats.topological_crossings != 0 || output.stats.routed_crossings != 0)
    {
        if reuse_components {
            frame.pause_components();
        }
        let comparison = work.compare(|work| {
            let mut reference = adaptive::copy_graph(graph, work)?;
            for node in &mut reference.nodes {
                work.visit(1)?;
                node.order_hint = None;
                node.position_hint = None;
                node.current_incident_signature = None;
            }
            match width_trial {
                Some(trial) => trial.replay(&mut reference, frame.expanded, work),
                None => solve(
                    &reference,
                    options,
                    frame.expanded,
                    LayoutDetail::Full,
                    work,
                )
                .map(Some),
            }
        });
        match comparison {
            Ok(Some(Some(reference)))
                if improves_crossings(&reference.stats, &output.stats)
                    && match options.width_policy {
                        WidthPolicy::Natural => true,
                        WidthPolicy::FitWidth { max_width, .. } => {
                            reference.bounds.width <= max_width.max(output.bounds.width)
                        }
                    } =>
            {
                keep_components = false;
                reference
            }
            Ok(_) | Err(LayoutError::InvalidInput(_)) => output,
            Err(error) => return Err(error),
        }
    } else {
        output
    };
    if let Some(trial) = width_trial.filter(|_| !graph.lane_order.is_empty()) {
        trial.refine_lanes(graph, &mut output, &mut work)?;
    } else {
        orthogonal::refine(graph, &mut output, options, &mut work)?;
    }
    work.finish()?;
    if keep_components {
        frame.commit_components(&output);
    }
    Ok(output)
}

fn improves_crossings(candidate: &LayoutStats, current: &LayoutStats) -> bool {
    let candidate = (candidate.topological_crossings, candidate.routed_crossings);
    let current = (current.topological_crossings, current.routed_crossings);
    candidate.0 <= current.0 && candidate.1 <= current.1 && candidate != current
}

fn solve(
    graph: &Graph,
    options: &Options,
    expanded: &mut flat::Expanded,
    detail: LayoutDetail,
    work: &mut work::Work<'_>,
) -> Result<LayoutOutput, LayoutError> {
    if graph.groups.is_empty() && graph.lane_order.is_empty() {
        flat::run_with_detail(graph, options, expanded, detail.flat(options), work)
    } else {
        compound::run(graph, options, expanded, detail, work)
    }
}
