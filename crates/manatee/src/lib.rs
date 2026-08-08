#![forbid(unsafe_code)]

//! Headless compound graph layout algorithms (COSE/FCoSE ports).
//!
//! `manatee` is used by `merman-render` as a drop-in, runtime-agnostic layout engine.
//! Baseline sources are tracked under `repo-ref/` (see `tools/upstreams/REPOS.lock.json`).

pub mod algo;
pub mod error;
pub mod graph;
mod work;

pub use algo::{
    Algorithm, AlignmentConstraint, FcoseOptions, FcoseRandomPolicy, FcoseRandomSource,
    RelativePlacementConstraint,
};
pub use error::{Error, Result, WorkFailure};
pub use graph::{
    Anchor, BoundsExtras, Compound, Edge, Graph, LayoutRect, LayoutResult, Node, Point,
};
pub use work::{NoopWorkControl, WorkControl};
/// Headless layout entry point.
pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    match algorithm {
        Algorithm::CoseBilkent => algo::cose_bilkent::layout(graph),
        Algorithm::Fcose(opts) => algo::fcose::layout(graph, &opts),
    }
}
