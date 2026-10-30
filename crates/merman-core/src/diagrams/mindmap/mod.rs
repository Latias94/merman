mod db;
mod parse;
#[cfg(feature = "presentation")]
mod render_model;
#[cfg(test)]
mod terminal;
mod utils;

#[cfg(all(test, feature = "compat-json"))]
mod tests;

pub use merman_model::families::mindmap::*;
#[cfg(feature = "compat-json")]
pub(crate) use parse::parse_mindmap;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) use parse::parse_mindmap_json_and_editor_facts;
#[cfg(all(test, feature = "presentation"))]
pub(crate) use parse::parse_mindmap_model_for_render;
#[cfg(feature = "presentation")]
pub(crate) use parse::parse_mindmap_model_for_render_controlled;
pub(crate) use parse::parse_mindmap_semantic_model_controlled;
#[cfg(test)]
pub(crate) use parse::{
    mindmap_syntax_construction_count, reset_mindmap_syntax_construction_count,
};
#[cfg(feature = "compat-json")]
pub(crate) use render_model::{
    render_model_to_compat_json, render_model_to_compat_json_controlled,
};
