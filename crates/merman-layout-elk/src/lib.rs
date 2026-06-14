#![forbid(unsafe_code)]

//! Optional ELK layout engine integration for `merman`.
//!
//! Source-port policy:
//! - Mermaid's adapter layer is
//!   https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid-layout-elk/src/render.ts.
//! - Mermaid pins `elkjs@0.9.3`; the corresponding source checkout is
//!   https://github.com/kieler/elkjs/tree/a8304cf79fde75bc2ab1a89d28320f53f8637436.
//! - `elkjs` is generated from Eclipse ELK Java sources. The current source baseline is
//!   https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e,
//!   which is the 0.9.x ELK release tag available for the `elkjs@0.9.3` release window.
//!
//! The public API currently delegates to `compat` to keep `flowchart-elk` usable while the
//! source-backed layered implementation is ported. New ELK layout behavior must land in
//! `source_port` with a source file reference; do not tune `compat` from fixture output.

mod compat;
pub use merman_elk_layered as source_port;

pub use compat::{
    Algorithm, CycleBreakingStrategy, Direction, Edge, EdgeLayout, EdgeRouting, Error, Graph,
    HierarchyHandling, Label, LayeredOptions, LayoutOptions, LayoutResult, Node, NodeKind,
    NodeLayout, Point, Result, Spacing,
};

pub fn layout(graph: &Graph, algorithm: Algorithm) -> Result<LayoutResult> {
    compat::layout(graph, algorithm)
}
