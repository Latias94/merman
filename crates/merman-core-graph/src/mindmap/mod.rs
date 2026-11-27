//! Controlled Mindmap source parsing and model construction.

mod db;
mod parse;
#[cfg(feature = "presentation")]
mod render_model;
mod utils;

#[cfg(feature = "editor-facts")]
pub use parse::MindmapObservation;
pub use parse::{
    MindmapSource, MindmapSourceIssue, MindmapSyntax, parse_mindmap_syntax_controlled,
};
