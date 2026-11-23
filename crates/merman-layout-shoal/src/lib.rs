//! Standalone layout of measured directed graphs. Coordinates use the caller's drawing units.

#![forbid(unsafe_code)]

mod model;
mod options;
mod preview;
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
    let output = preview::run(graph, options, frame.buffers, &mut work)?;
    work.finish()?;
    Ok(output)
}
