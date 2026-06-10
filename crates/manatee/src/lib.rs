#![forbid(unsafe_code)]

//! Headless compound graph layout algorithms (COSE/FCoSE ports).
//!
//! `manatee` is used by `merman-render` as a drop-in, runtime-agnostic layout engine.
//! Baseline sources are tracked under `repo-ref/` (see `tools/upstreams/REPOS.lock.json`).

extern crate self as web_time;

pub mod algo;
pub mod error;
pub mod graph;

mod host_time;

pub use algo::{
    Algorithm, AlignmentConstraint, CoseBilkentOptions, FcoseOptions, RelativePlacementConstraint,
};
pub use error::{Error, Result};
pub use graph::{
    Anchor, BoundsExtras, Compound, Edge, Graph, LayoutRect, LayoutResult, Node, Point,
};
pub(crate) use host_time::{Duration, Instant};

/// Headless layout entry point.
pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    match algorithm {
        Algorithm::CoseBilkent(opts) => algo::cose_bilkent::layout(graph, &opts),
        Algorithm::Fcose(opts) => algo::fcose::layout(graph, &opts),
    }
}
