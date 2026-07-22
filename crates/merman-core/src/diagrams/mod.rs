//! Public data models for Mermaid diagram families.
//!
//! Built-in family constructors are intentionally crate-private. External callers must parse
//! through [`crate::Engine`], which owns preprocessing, detection, configuration, and the closed
//! [`crate::DiagramParseSnapshot`] contract. This prevents callers from constructing metadata by
//! hand and invoking a semantic, render-model, or editor-facts parser independently.
//!
//! ```compile_fail,E0603
//! use merman_core::diagrams::flowchart::parse_flowchart;
//! ```
//!
//! Built-in parser pointers are not exposed through the public registry either:
//!
//! ```compile_fail,E0624
//! use merman_core::DiagramRegistry;
//!
//! let registry = DiagramRegistry::for_pinned_mermaid_baseline();
//! let _parser = registry.get("flowchart-v2");
//! ```

macro_rules! include_checked_in_lalrpop_parser {
    ($(#[$attr:meta])* $name:ident, $file:literal) => {
        #[rustfmt::skip]
        #[allow(clippy::extra_unused_lifetimes)]
        #[allow(clippy::needless_lifetimes)]
        #[allow(clippy::let_unit_value)]
        #[allow(clippy::just_underscores_and_digits)]
        $(#[$attr])*
        mod $name {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/generated/lalrpop/",
                $file
            ));
        }
    };
}

pub mod architecture;
pub mod block;
pub mod c4;
pub mod class;
pub mod cynefin;
pub mod er;
pub mod error_diagram;
pub mod eventmodeling;
pub mod flowchart;
pub mod gantt;
pub mod git_graph;
pub mod info;
pub mod ishikawa;
pub mod journey;
pub mod kanban;
pub(crate) mod langium_common;
pub mod mindmap;
pub mod packet;
pub mod pie;
pub mod quadrant_chart;
pub mod radar;
pub mod railroad;
pub mod requirement;
pub mod sankey;
pub(crate) mod scan;
pub mod sequence;
pub mod state;
pub mod timeline;
pub mod tree_view;
pub mod treemap;
pub mod venn;
pub mod wardley;
pub mod xychart;
pub mod zenuml;
