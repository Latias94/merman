//! Standalone layout of measured directed graphs. Coordinates use the caller's drawing units.

#![forbid(unsafe_code)]

mod adaptive;
mod compound;
mod flat;
mod model;
mod options;
mod scratch;
mod work;

pub use model::*;
pub use options::*;
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
    let frame = scratch.frame();
    let mut work = work::Work::new(control);
    work.charge(1)?;
    let (output, width_trial) = match options.width_policy {
        WidthPolicy::Natural => (solve(graph, options, frame.expanded, &mut work)?, None),
        WidthPolicy::FitWidth { .. } => {
            let (drawing, trial) = adaptive::run(graph, options, frame.expanded, &mut work)?;
            (drawing, Some(trial))
        }
    };
    let output = if work.editor_choice
        && (output.stats.topological_crossings != 0 || output.stats.routed_crossings != 0)
    {
        let comparison = work.compare(|work| {
            let mut reference = adaptive::copy_graph(graph, work)?;
            for node in &mut reference.nodes {
                work.visit(1)?;
                node.order_hint = None;
            }
            match width_trial {
                Some(trial) => trial.replay(&mut reference, frame.expanded, work),
                None => solve(&reference, options, frame.expanded, work).map(Some),
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
                reference
            }
            Ok(_) | Err(LayoutError::InvalidInput(_)) => output,
            Err(error) => return Err(error),
        }
    } else {
        output
    };
    work.finish()?;
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
    work: &mut work::Work<'_>,
) -> Result<LayoutOutput, LayoutError> {
    if graph.groups.is_empty() {
        flat::run(graph, options, expanded, work)
    } else {
        compound::run(graph, options, expanded, work)
    }
}
