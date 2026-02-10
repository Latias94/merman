//! Graph container APIs used by `dugong`.
//!
//! Baseline: `@dagrejs/graphlib` (see `docs/adr/0044-dugong-parity-and-testing.md`).

mod graph;

pub use graph::alg;
pub use graph::{EdgeKey, Graph, GraphOptions};
