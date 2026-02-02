#![forbid(unsafe_code)]

pub mod algo;
pub mod error;
pub mod graph;

pub use algo::{Algorithm, CoseBilkentOptions, FcoseOptions};
pub use error::{Error, Result};
pub use graph::{Edge, Graph, LayoutResult, Node, Point};

/// Headless layout entry point.
pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    match algorithm {
        Algorithm::CoseBilkent(opts) => algo::cose_bilkent::layout(graph, &opts),
        Algorithm::Fcose(opts) => algo::fcose::layout(graph, &opts),
    }
}
