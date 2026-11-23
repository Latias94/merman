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
    let output = solve_with_width(graph, options, frame.expanded, &mut work)?;
    let output = if (output.stats.topological_crossings != 0 || output.stats.routed_crossings != 0)
        && graph
            .nodes
            .iter()
            .any(|node| node.editor_order_hint().is_some())
    {
        let mut reference = adaptive::copy_graph(graph, &mut work)?;
        for node in &mut reference.nodes {
            work.visit(1)?;
            if node.editor_order_hint().is_some() {
                node.order_hint = None;
            }
        }
        match solve_with_width(&reference, options, frame.expanded, &mut work) {
            Ok(reference)
                if reference.stats.topological_crossings < output.stats.topological_crossings
                    || reference.stats.routed_crossings < output.stats.routed_crossings =>
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

fn solve_with_width(
    graph: &Graph,
    options: &Options,
    expanded: &mut flat::Expanded,
    work: &mut work::Work<'_>,
) -> Result<LayoutOutput, LayoutError> {
    match options.width_policy {
        WidthPolicy::Natural => solve(graph, options, expanded, work),
        WidthPolicy::FitWidth { .. } => adaptive::run(graph, options, expanded, work),
    }
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
