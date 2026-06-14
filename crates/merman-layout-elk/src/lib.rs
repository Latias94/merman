#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! Source-port policy:
//! - Mermaid's adapter layer is `repo-ref/mermaid/packages/mermaid-layout-elk/src/render.ts`.
//! - Mermaid pins `elkjs@0.9.3`; the corresponding source checkout is
//!   `repo-ref/elkjs` tag `0.9.3`.
//! - `elkjs` is generated from Eclipse ELK Java sources. The current source baseline is
//!   `repo-ref/elk` tag `v0.9.1`, which is the 0.9.x ELK release tag available for the
//!   `elkjs@0.9.3` release window.
//!
//! The public API currently delegates to `compat` to keep `flowchart-elk` usable while the
//! source-backed layered implementation is ported. New ELK layout behavior must land in
//! `source_port` with a source file reference; do not tune `compat` from fixture output.

mod compat;
pub mod source_port;

pub use compat::{
    Algorithm, CycleBreakingStrategy, Direction, Edge, EdgeLayout, EdgeRouting, Error, Graph,
    HierarchyHandling, Label, LayeredOptions, LayoutOptions, LayoutResult, Node, NodeKind,
    NodeLayout, Point, Result, Spacing,
};

pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    compat::layout(graph, algorithm)
}
